//! Arrastrar y pegar imágenes en el chat (Windows).
//!
//! El arrastre de Tauri solo acepta archivos (CF_HDROP): una imagen que se arrastra desde el navegador,
//! Discord o un gestor del portapapeles llega como «archivo virtual» (FileContents) o como mapa de bits
//! (PNG, DIB) y Windows la descartaba sin avisar. Aquí un IDropTarget propio reemplaza al de Tauri en la
//! ventana: los archivos llegan igual que antes (el evento "arrastre", con sus rutas) y, con Mensajes
//! abierto (`accept_images`), una imagen sin archivo se guarda en datos/temporal y llega como un archivo
//! más. `clipboard_image` lee lo mismo del portapapeles (lo que la ventana no sabe pegar).

use std::cell::Cell;
use std::ffi::{c_void, OsString};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use tauri::{AppHandle, Emitter, Manager};
use windows::core::{implement, w, BOOL};
use windows::Win32::Foundation::{DRAGDROP_E_INVALIDHWND, HWND, LPARAM, POINTL};
use windows::Win32::System::Com::{IDataObject, IStream, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED, TYMED_HGLOBAL, TYMED_ISTREAM};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    IDropTarget, IDropTarget_Impl, OleGetClipboard, OleInitialize, OleUninitialize, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_DIB, CF_DIBV5, CF_HDROP,
    DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;

/// Las extensiones que el chat sabe abrir (cuenta::leer_imagen).
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "avif", "heic", "heif", "tif", "tiff", "ico", "jxr", "wdp", "dds"];
const MAX_BYTES: usize = 25 * 1024 * 1024;

/// Mensajes está abierto: se aceptan imágenes sin archivo.
static IMAGES: AtomicBool = AtomicBool::new(false);
/// Dónde se guardan las imágenes arrastradas (datos/temporal).
static TEMP: OnceLock<PathBuf> = OnceLock::new();

pub fn accept_images(on: bool) {
    IMAGES.store(on, Ordering::Relaxed);
}

fn is_image_name(name: &str) -> bool {
    Path::new(name).extension().is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

fn format(name: windows::core::PCWSTR) -> u16 {
    unsafe { RegisterClipboardFormatW(name) as u16 }
}

fn request(format: u16, index: i32, tymed: TYMED) -> FORMATETC {
    FORMATETC { cfFormat: format, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: index, tymed: tymed.0 as u32 }
}

fn has(data: &IDataObject, format: u16, index: i32, tymed: TYMED) -> bool {
    unsafe { data.QueryGetData(&request(format, index, tymed)).is_ok() }
}

/// Los bytes de lo que trae el objeto en ese formato (en memoria o en un stream), hasta MAX_BYTES.
fn bytes(data: &IDataObject, format: u16, index: i32, tymed: TYMED) -> Option<Vec<u8>> {
    unsafe {
        let mut medium: STGMEDIUM = data.GetData(&request(format, index, tymed)).ok()?;
        let result = if medium.tymed == TYMED_HGLOBAL.0 as u32 {
            let handle = medium.u.hGlobal;
            let size = GlobalSize(handle).min(MAX_BYTES);
            let pointer = GlobalLock(handle) as *const u8;
            let copy = (!pointer.is_null()).then(|| std::slice::from_raw_parts(pointer, size).to_vec());
            let _ = GlobalUnlock(handle);
            copy
        } else if medium.tymed == TYMED_ISTREAM.0 as u32 {
            (*medium.u.pstm).as_ref().map(read_stream)
        } else {
            None
        };
        ReleaseStgMedium(&mut medium);
        result.filter(|b| !b.is_empty())
    }
}

fn read_stream(stream: &IStream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let mut read = 0u32;
        let ok = unsafe { stream.Read(chunk.as_mut_ptr() as *mut c_void, chunk.len() as u32, Some(&mut read)) };
        if ok.is_err() || read == 0 || out.len() > MAX_BYTES {
            return out;
        }
        out.extend_from_slice(&chunk[..read as usize]);
    }
}

/// Las rutas de los archivos que trae (arrastrados o copiados en el Explorador).
fn paths(data: &IDataObject) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    unsafe {
        let Ok(mut medium) = data.GetData(&request(CF_HDROP.0, -1, TYMED_HGLOBAL)) else { return paths };
        let drop = HDROP(medium.u.hGlobal.0);
        for i in 0..DragQueryFileW(drop, u32::MAX, None) {
            let length = DragQueryFileW(drop, i, None) as usize;
            let mut buffer = vec![0u16; length + 1];
            DragQueryFileW(drop, i, Some(&mut buffer));
            paths.push(OsString::from_wide(&buffer[..length]).into());
        }
        ReleaseStgMedium(&mut medium);
    }
    paths
}

/// Los nombres de los «archivos virtuales» (FileGroupDescriptorW): lo que da el navegador al arrastrar
/// una imagen. Cada FILEDESCRIPTORW mide 592 bytes y su nombre (260 caracteres) empieza en el 72.
fn virtual_names(data: &IDataObject) -> Vec<String> {
    let Some(raw) = bytes(data, format(w!("FileGroupDescriptorW")), -1, TYMED_HGLOBAL) else { return Vec::new() };
    let count = raw.get(..4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).unwrap_or(0);
    (0..count.min(64))
        .filter_map(|i| {
            let start = 4 + i * 592 + 72;
            let name: Vec<u16> = raw.get(start..start + 520)?.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|&c| c != 0).collect();
            Some(String::from_utf16_lossy(&name))
        })
        .collect()
}

/// ¿Trae una imagen sin archivo? (sin leerla: para el cursor al entrar).
fn has_image(data: &IDataObject) -> bool {
    virtual_names(data).iter().any(|n| is_image_name(n))
        || has(data, format(w!("PNG")), -1, TYMED_HGLOBAL)
        || has(data, CF_DIBV5.0, -1, TYMED_HGLOBAL)
        || has(data, CF_DIB.0, -1, TYMED_HGLOBAL)
}

/// La imagen que trae sin archivo: (extensión, bytes). El archivo virtual conserva el original (un GIF
/// sigue animado); un mapa de bits se pasa a PNG.
fn image(data: &IDataObject) -> Option<(String, Vec<u8>)> {
    if let Some((index, name)) = virtual_names(data).into_iter().enumerate().find(|(_, n)| is_image_name(n)) {
        let contents = format(w!("FileContents"));
        let found = bytes(data, contents, index as i32, TYMED(TYMED_HGLOBAL.0 | TYMED_ISTREAM.0));
        if let Some(found) = found {
            let extension = Path::new(&name).extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "png".into());
            return Some((extension, found));
        }
    }
    if let Some(png) = bytes(data, format(w!("PNG")), -1, TYMED_HGLOBAL) {
        return Some(("png".into(), png));
    }
    let dib = bytes(data, CF_DIBV5.0, -1, TYMED_HGLOBAL).or_else(|| bytes(data, CF_DIB.0, -1, TYMED_HGLOBAL))?;
    crate::imagen::to_png(&bmp_file(&dib)?).ok().map(|png| ("png".into(), png))
}

/// Un DIB del portapapeles como archivo .bmp: le falta la cabecera de archivo (14 bytes) con dónde
/// empiezan los píxeles (después de la cabecera, las máscaras de color y la paleta).
fn bmp_file(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |at: usize| dib.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let header = u32_at(0)? as usize;
    let bits = dib.get(14..16).map(|b| u16::from_le_bytes([b[0], b[1]]))?;
    let compression = u32_at(16)?;
    let used = u32_at(32)? as usize;
    let palette = if used > 0 { used } else if bits <= 8 { 1 << bits } else { 0 };
    // BI_BITFIELDS con la cabecera corta (40 bytes): las tres máscaras van aparte.
    let masks = if compression == 3 && header == 40 { 12 } else { 0 };
    let offset = 14 + header + masks + palette * 4;
    let mut file = Vec::with_capacity(14 + dib.len());
    file.extend_from_slice(b"BM");
    file.extend_from_slice(&((14 + dib.len()) as u32).to_le_bytes());
    file.extend_from_slice(&[0; 4]);
    file.extend_from_slice(&(offset as u32).to_le_bytes());
    file.extend_from_slice(dib);
    Some(file)
}

fn save(folder: &Path, stem: &str, extension: &str, bytes: &[u8]) -> Option<PathBuf> {
    std::fs::create_dir_all(folder).ok()?;
    let file = folder.join(format!("{stem}.{extension}"));
    std::fs::write(&file, bytes).ok()?;
    Some(file)
}

fn emit(app: &AppHandle, kind: &str, paths: &[PathBuf]) {
    let _ = app.emit("arrastre", serde_json::json!({ "type": kind, "paths": paths }));
}

/// 0: nada que hacer; 1: archivos; 2: una imagen sin archivo.
#[implement(IDropTarget)]
struct Target {
    app: AppHandle,
    mode: Cell<u8>,
}

#[allow(non_snake_case)]
impl IDropTarget_Impl for Target_Impl {
    fn DragEnter(&self, data: windows::core::Ref<'_, IDataObject>, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        let found = data.as_ref().map(paths).unwrap_or_default();
        let mode = if !found.is_empty() {
            1
        } else if IMAGES.load(Ordering::Relaxed) && data.as_ref().is_some_and(has_image) {
            2
        } else {
            0
        };
        self.mode.set(mode);
        if mode != 0 {
            emit(&self.app, "enter", &found);
        }
        unsafe { *effect = if mode != 0 { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        unsafe { *effect = if self.mode.get() != 0 { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        if self.mode.replace(0) != 0 {
            emit(&self.app, "leave", &[]);
        }
        Ok(())
    }

    fn Drop(&self, data: windows::core::Ref<'_, IDataObject>, _keys: MODIFIERKEYS_FLAGS, _point: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        let mode = self.mode.replace(0);
        let found = match (mode, data.as_ref()) {
            (1, Some(data)) => paths(data),
            (2, Some(data)) => image(data).and_then(|(extension, bytes)| save(TEMP.get()?, "arrastre", &extension, &bytes)).into_iter().collect(),
            _ => Vec::new(),
        };
        if found.is_empty() {
            if mode != 0 {
                emit(&self.app, "leave", &[]);
            }
            unsafe { *effect = DROPEFFECT_NONE };
        } else {
            emit(&self.app, "drop", &found);
            unsafe { *effect = DROPEFFECT_COPY };
        }
        Ok(())
    }
}

/// Pone el receptor propio en la ventana (en lugar del de Tauri). Se puede llamar otra vez (al recargar
/// la interfaz): reemplaza el anterior.
pub fn install(window: &tauri::WebviewWindow, temp: PathBuf) {
    let _ = TEMP.set(temp);
    let Ok(hwnd) = window.hwnd() else { return };
    let parent = hwnd.0 as isize;
    let app = window.app_handle().clone();
    // RegisterDragDrop se llama desde el hilo de la ventana. Igual que Tauri (wry), en cada ventana hija:
    // ahí está la de WebView2 que recibe el arrastre.
    let _ = window.run_on_main_thread(move || {
        unsafe extern "system" fn each(child: HWND, lparam: LPARAM) -> BOOL {
            let app = &*(lparam.0 as *const AppHandle);
            let target: IDropTarget = Target { app: app.clone(), mode: Cell::new(0) }.into();
            if RevokeDragDrop(child) != Err(DRAGDROP_E_INVALIDHWND.into()) {
                let _ = RegisterDragDrop(child, &target);
            }
            true.into()
        }
        unsafe {
            let _ = EnumChildWindows(Some(HWND(parent as *mut c_void)), Some(each), LPARAM(&app as *const AppHandle as isize));
        }
    });
}

/// La imagen del portapapeles como archivo: la ruta de un archivo de imagen copiado en el Explorador o
/// la imagen (sin archivo) guardada en `temp`. None si no hay ninguna.
pub fn clipboard_image(temp: &Path) -> Result<Option<PathBuf>, String> {
    let temp = temp.to_path_buf();
    // El portapapeles de OLE pide un hilo propio (STA) con OLE iniciado.
    std::thread::spawn(move || unsafe {
        if OleInitialize(None).is_err() {
            return Err(t!("No se pudo leer el portapapeles.", "Could not read the clipboard."));
        }
        let result = match OleGetClipboard() {
            Ok(data) => Ok(match paths(&data).into_iter().find(|p| is_image_name(&p.to_string_lossy())) {
                Some(path) => Some(path),
                None => image(&data).and_then(|(extension, bytes)| save(&temp, "portapapeles", &extension, &bytes)),
            }),
            Err(_) => Err(t!("No se pudo leer el portapapeles.", "Could not read the clipboard.")),
        };
        OleUninitialize();
        result
    })
    .join()
    .map_err(|_| t!("No se pudo leer el portapapeles.", "Could not read the clipboard."))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_a_bmp() {
        // Un DIB de 1×1 a 24 bits: cabecera de 40 bytes y 4 de píxeles (con relleno).
        let mut dib = vec![0u8; 44];
        dib[0] = 40;
        dib[4] = 1;
        dib[8] = 1;
        dib[12] = 1;
        dib[14] = 24;
        let file = bmp_file(&dib).unwrap();
        assert_eq!(&file[..2], b"BM");
        assert_eq!(u32::from_le_bytes(file[10..14].try_into().unwrap()), 54);
        assert_eq!(file.len(), 58);
        assert!(is_image_name("Captura de pantalla.PNG") && !is_image_name("mod.jar"));
    }
}
