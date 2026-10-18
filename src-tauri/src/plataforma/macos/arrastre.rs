//! Arrastrar y pegar imágenes en el chat (Mac). Mismo contrato que plataforma/windows/arrastre.rs.
//!
//! Los archivos del Finder siguen con el arrastre de Tauri: la interfaz escucha sus eventos además de
//! "arrastre". Tauri (wry) solo da rutas de archivos, y una imagen arrastrada desde Safari, Chrome o
//! Discord no trae ninguna: llega como «promesa» de archivo (el navegador lo escribe donde se le pida)
//! o como la imagen misma (TIFF, PNG…). Con Mensajes abierto (`accept_images`), `install` pone delante
//! del soltar de wry (`performDragOperation:` de su WKWebView) una función que guarda esa imagen en
//! datos/temporal y la manda como "arrastre"; después sigue lo de wry, que con nada que soltar no hace
//! nada en la interfaz. También va delante de su `draggingEntered:`: wry da por hecho que los archivos
//! del arrastre siempre tienen ruta, y con uno que macOS no sabe convertir (una referencia
//! `file:///.file/id=…` a un archivo que ya no está) la app se caía; ese arrastre ahora se rechaza.
//!
//! La imagen del portapapeles (`clipboard_image`) se lee con AppleScript (`osascript`), que trae
//! macOS: un archivo de imagen copiado en el Finder o la imagen misma (una captura con ⌃⇧⌘4, una
//! copiada de Safari o de Vista Previa).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, Sel};
use objc2::{msg_send, sel};
use objc2_foundation::{NSArray, NSString};
use tauri::{AppHandle, Emitter, Manager};

/// Las que se mandan al chat (las demás se ignoran al pegar).
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "avif", "heic", "heif", "tif", "tiff", "ico"];
const MAX_BYTES: usize = 25 * 1024 * 1024;

/// La imagen tal cual en el arrastre, con su extensión. El TIFF va aparte: es lo que da Safari además de
/// la promesa (y pierde la animación de un GIF), así que solo se usa si la promesa no llega.
const IMAGE_TYPES: &[(&str, &str)] = &[("public.png", "png"), ("public.jpeg", "jpg"), ("com.compuserve.gif", "gif"), ("org.webmproject.webp", "webp"), ("public.heic", "heic")];
/// Archivos de verdad (los de Tauri) y promesas de archivo (la de antes, que usa Safari, y la nueva).
const FILE_TYPES: &[&str] = &["NSFilenamesPboardType", "public.file-url"];
const PROMISE_TYPES: &[&str] = &["Apple files promise pasteboard type", "com.apple.NSFilePromiseItemMetaData", "com.apple.pasteboard.promised-file-url"];
/// Lo que se espera a que el navegador escriba el archivo prometido (Chrome puede bajarlo antes).
const PROMISE_WAIT: Duration = Duration::from_secs(10);

/// Mensajes está abierto: se aceptan imágenes sin archivo.
static IMAGES: AtomicBool = AtomicBool::new(false);
/// Dónde se guardan las imágenes arrastradas (datos/temporal) y a quién se avisa.
static TEMP: OnceLock<PathBuf> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();
/// El `draggingEntered:` y el `performDragOperation:` de wry, que se llaman después de los propios.
static WRY_ENTER: OnceLock<Imp> = OnceLock::new();
static WRY_DROP: OnceLock<Imp> = OnceLock::new();

/// Devuelve un NSDragOperation (0: no se acepta).
type EnterFn = unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> usize;
type DropFn = unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> Bool;

pub fn accept_images(on: bool) {
    IMAGES.store(on, Ordering::Relaxed);
}

/// Pone lo propio delante de lo de wry, una sola vez (al recargar la interfaz se vuelve a llamar). Se
/// corre en el hilo de la ventana, el mismo de los arrastres.
pub fn install(window: &tauri::WebviewWindow, temp: PathBuf) {
    let _ = TEMP.set(temp);
    let _ = APP.set(window.app_handle().clone());
    let _ = window.with_webview(|webview| {
        let view = webview.inner() as *const AnyObject;
        // SAFETY: Tauri da su WKWebView (de wry), vivo mientras dura la ventana.
        let Some(view) = (unsafe { view.as_ref() }) else { return };
        let class = view.class();
        let (Some(enter), Some(drop)) = (class.instance_method(sel!(draggingEntered:)), class.instance_method(sel!(performDragOperation:))) else { return };
        if WRY_DROP.get().is_some() {
            return;
        }
        let (ours_enter, ours_drop): (EnterFn, DropFn) = (dragging_entered, perform_drag_operation);
        // SAFETY: las mismas firmas que los de wry, a los que se llama después.
        unsafe {
            let _ = WRY_ENTER.set(enter.set_implementation(std::mem::transmute::<EnterFn, Imp>(ours_enter)));
            let _ = WRY_DROP.set(drop.set_implementation(std::mem::transmute::<DropFn, Imp>(ours_drop)));
        }
    });
}

fn pasteboard(info: &AnyObject) -> Option<Retained<AnyObject>> {
    unsafe { msg_send![info, draggingPasteboard] }
}

/// Al entrar un arrastre: si sus archivos no tienen ruta, se rechaza (wry se caería); si no, lo de wry.
unsafe extern "C-unwind" fn dragging_entered(this: *mut AnyObject, selector: Sel, info: *mut AnyObject) -> usize {
    if unsafe { info.as_ref() }.and_then(pasteboard).is_some_and(|p| unreadable_files(&p)) {
        return 0;
    }
    let wry = WRY_ENTER.get().expect("se guarda al cambiar el método");
    // SAFETY: es la implementación original de este mismo método.
    unsafe { std::mem::transmute::<Imp, EnterFn>(*wry)(this, selector, info) }
}

/// Archivos en el arrastre que macOS no sabe convertir en rutas: `NSFilenamesPboardType` está, pero no
/// da una lista de textos (lo que wry da por hecho).
fn unreadable_files(pasteboard: &AnyObject) -> bool {
    if !has_type(pasteboard, &["NSFilenamesPboardType"]) {
        return false;
    }
    let list: Option<Retained<AnyObject>> = unsafe { msg_send![pasteboard, propertyListForType: &*NSString::from_str("NSFilenamesPboardType")] };
    let (Some(list), Some(array), Some(string)) = (list, AnyClass::get(c"NSArray"), AnyClass::get(c"NSString")) else { return true };
    let is = |object: &AnyObject, class: &AnyClass| -> bool { unsafe { msg_send![object, isKindOfClass: class] } };
    if !is(&list, array) {
        return true;
    }
    let count: usize = unsafe { msg_send![&*list, count] };
    (0..count).any(|i| {
        let item: Retained<AnyObject> = unsafe { msg_send![&*list, objectAtIndex: i] };
        !is(&item, string)
    })
}

/// Lo que se hace al soltar: con Mensajes abierto y una imagen sin archivo, la guarda y la manda (o
/// espera la promesa en otro hilo). Después sigue lo de wry (salvo con archivos sin ruta: se caería).
unsafe extern "C-unwind" fn perform_drag_operation(this: *mut AnyObject, selector: Sel, info: *mut AnyObject) -> Bool {
    if unsafe { info.as_ref() }.and_then(pasteboard).is_some_and(|p| unreadable_files(&p)) {
        return Bool::NO;
    }
    if IMAGES.load(Ordering::Relaxed) {
        if let (Some(info), Some(temp)) = (unsafe { info.as_ref() }, TEMP.get()) {
            match dropped_image(info, temp) {
                Dropped::Ready(file) => send(&file),
                Dropped::Promised { folder, names, fallback } => {
                    std::thread::spawn(move || {
                        if let Some(file) = wait_for_image(&folder, &names, PROMISE_WAIT).or(fallback) {
                            send(&file);
                        }
                    });
                }
                Dropped::Nothing => {}
            }
        }
    }
    let wry = WRY_DROP.get().expect("se guarda al cambiar el método");
    // SAFETY: es la implementación original de este mismo método.
    unsafe { std::mem::transmute::<Imp, DropFn>(*wry)(this, selector, info) }
}

fn send(file: &Path) {
    if let Some(app) = APP.get() {
        let _ = app.emit("arrastre", serde_json::json!({ "type": "drop", "paths": [file] }));
    }
}

enum Dropped {
    Nothing,
    Ready(PathBuf),
    /// Los nombres que el origen escribe (o escribirá) en `folder`; `fallback`, el TIFF ya guardado.
    Promised { folder: PathBuf, names: Vec<String>, fallback: Option<PathBuf> },
}

fn strings(values: &[&str]) -> Retained<NSArray<NSString>> {
    NSArray::from_retained_slice(&values.iter().map(|v| NSString::from_str(v)).collect::<Vec<_>>())
}

fn has_type(pasteboard: &AnyObject, types: &[&str]) -> bool {
    let found: Option<Retained<NSString>> = unsafe { msg_send![pasteboard, availableTypeFromArray: &*strings(types)] };
    found.is_some()
}

/// Los bytes de ese tipo en el arrastre (hasta MAX_BYTES).
fn data(pasteboard: &AnyObject, kind: &str) -> Option<Vec<u8>> {
    let data: Option<Retained<AnyObject>> = unsafe { msg_send![pasteboard, dataForType: &*NSString::from_str(kind)] };
    let data = data?;
    let length: usize = unsafe { msg_send![&*data, length] };
    let bytes: *const std::ffi::c_void = unsafe { msg_send![&*data, bytes] };
    (length > 0 && length <= MAX_BYTES && !bytes.is_null()).then(|| unsafe { std::slice::from_raw_parts(bytes as *const u8, length) }.to_vec())
}

fn save(folder: &Path, extension: &str, bytes: &[u8]) -> Option<PathBuf> {
    std::fs::create_dir_all(folder).ok()?;
    let file = folder.join(format!("arrastre.{extension}"));
    std::fs::write(&file, bytes).ok()?;
    Some(file)
}

/// Qué trae el arrastre (`info`, un NSDraggingInfo). Con archivos de verdad, nada: son de Tauri.
fn dropped_image(info: &AnyObject, temp: &Path) -> Dropped {
    let Some(pasteboard) = pasteboard(info) else { return Dropped::Nothing };
    if has_type(&pasteboard, FILE_TYPES) {
        return Dropped::Nothing;
    }
    for (kind, extension) in IMAGE_TYPES {
        if let Some(file) = data(&pasteboard, kind).and_then(|bytes| save(temp, extension, &bytes)) {
            return Dropped::Ready(file);
        }
    }
    let fallback = data(&pasteboard, "public.tiff").and_then(|bytes| save(temp, "tiff", &bytes));
    if has_type(&pasteboard, PROMISE_TYPES) {
        // Una carpeta vacía en cada arrastre: el origen elige el nombre y no se mezcla con otro anterior.
        let folder = temp.join("arrastre");
        let _ = std::fs::remove_dir_all(&folder);
        if std::fs::create_dir_all(&folder).is_ok() {
            let names = promised_names(info, &folder);
            if !names.is_empty() {
                return Dropped::Promised { folder, names, fallback };
            }
        }
    }
    fallback.map_or(Dropped::Nothing, Dropped::Ready)
}

/// Le pide al origen que escriba en `folder` los archivos prometidos y devuelve sus nombres. Safari los
/// escribe en ese momento; Chrome, después.
fn promised_names(info: &AnyObject, folder: &Path) -> Vec<String> {
    let Some(url_class) = AnyClass::get(c"NSURL") else { return Vec::new() };
    let path = NSString::from_str(&folder.to_string_lossy());
    let url: Option<Retained<AnyObject>> = unsafe { msg_send![url_class, fileURLWithPath: &*path, isDirectory: true] };
    let Some(url) = url else { return Vec::new() };
    let names: Option<Retained<NSArray<NSString>>> = unsafe { msg_send![info, namesOfPromisedFilesDroppedAtDestination: &*url] };
    names.map(|names| (0..names.count()).map(|i| names.objectAtIndex(i).to_string()).filter(|n| is_image_name(Path::new(n))).collect()).unwrap_or_default()
}

/// El primero de `names` que aparece en `folder` como imagen completa (el mismo tamaño en dos vueltas).
fn wait_for_image(folder: &Path, names: &[String], wait: Duration) -> Option<PathBuf> {
    let step = Duration::from_millis(100);
    let mut sizes = vec![0u64; names.len()];
    let mut waited = Duration::ZERO;
    while waited <= wait {
        for (name, last) in names.iter().zip(sizes.iter_mut()) {
            let file = folder.join(name);
            let size = file.symlink_metadata().map(|m| m.len()).unwrap_or(0);
            if size > 0 && size == *last && is_image_file(&file) {
                return Some(file);
            }
            *last = size;
        }
        std::thread::sleep(step);
        waited += step;
    }
    None
}

fn is_image_name(path: &Path) -> bool {
    path.extension().is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

/// Que un archivo copiado o arrastrado sea de verdad una imagen y no un enlace a otro archivo: lo que se
/// manda pasa tal cual a la ventana (cuenta::leer_imagen), así que no basta con el nombre.
fn is_image_file(path: &Path) -> bool {
    use std::io::Read;
    if !is_image_name(path) || !path.symlink_metadata().is_ok_and(|m| m.file_type().is_file()) {
        return false;
    }
    let mut head = [0u8; 12];
    let Ok(read) = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)) else { return false };
    crate::imagen::is_image(&head[..read])
}

/// Corre un AppleScript con un argumento (`item 1 of argv`): así una ruta con comillas o espacios no
/// se mezcla con el script. Devuelve lo que escribe, o None si falló (el portapapeles no tenía eso).
fn applescript(lines: &[&str], argument: &str) -> Option<String> {
    let mut command = Command::new("/usr/bin/osascript");
    command.args(["-e", "on run argv"]);
    for line in lines {
        command.args(["-e", line]);
    }
    let output = command.args(["-e", "end run"]).arg(argument).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// La imagen del portapapeles como archivo: la ruta de un archivo de imagen copiado en el Finder o la
/// imagen (sin archivo) guardada en `temp`. None si no hay ninguna.
pub fn clipboard_image(temp: &Path) -> Result<Option<PathBuf>, String> {
    // Un archivo copiado en el Finder (si se copiaron varios, el primero).
    if let Some(path) = applescript(&["return POSIX path of (the clipboard as «class furl»)"], "").map(PathBuf::from) {
        if is_image_file(&path) {
            return Ok(Some(path));
        }
    }
    // La imagen misma: PNG o, si no, TIFF (lo que deja macOS al copiar de Vista Previa).
    std::fs::create_dir_all(temp).map_err(|e| e.to_string())?;
    for (class, extension) in [("PNGf", "png"), ("TIFF", "tiff")] {
        let file = temp.join(format!("portapapeles.{extension}"));
        let _ = std::fs::remove_file(&file);
        let write = format!("write (the clipboard as «class {class}») to f");
        let saved = applescript(
            &[
                "set f to open for access (POSIX file (item 1 of argv)) with write permission",
                // Vacío antes de escribir: si quedara un archivo de antes, no se mezclan sus bytes.
                "set eof f to 0",
                "try",
                &write,
                "on error",
                "close access f",
                "error number -1",
                "end try",
                "close access f",
            ],
            &file.to_string_lossy(),
        );
        if saved.is_some() && file.metadata().is_ok_and(|m| m.len() > 0) {
            return Ok(Some(file));
        }
        let _ = std::fs::remove_file(&file);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_imagenes_de_verdad() {
        let folder = std::env::temp_dir().join(format!("parabellum-arrastre-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let logo = std::fs::canonicalize("../ui/logo.png").unwrap();
        assert!(is_image_file(&logo));
        // Con nombre de imagen pero otro contenido.
        std::fs::write(folder.join("falsa.png"), b"esto no es una imagen").unwrap();
        assert!(!is_image_file(&folder.join("falsa.png")));
        // Un enlace con nombre de imagen, aunque apunte a una de verdad.
        std::os::unix::fs::symlink(&logo, folder.join("enlace.png")).unwrap();
        assert!(!is_image_file(&folder.join("enlace.png")));
        // Una imagen de verdad con otra extensión.
        std::fs::copy(&logo, folder.join("logo.txt")).unwrap();
        assert!(!is_image_file(&folder.join("logo.txt")));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn espera_la_imagen_prometida() {
        let folder = std::env::temp_dir().join(format!("parabellum-promesa-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let names = vec!["foto.png".to_string()];
        // Como Chrome: el archivo llega un rato después de soltar.
        let writer = {
            let folder = folder.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                std::fs::copy("../ui/logo.png", folder.join("foto.png")).unwrap();
            })
        };
        assert_eq!(wait_for_image(&folder, &names, Duration::from_secs(5)), Some(folder.join("foto.png")));
        writer.join().unwrap();
        // Uno que no es imagen no se acepta.
        std::fs::write(folder.join("falsa.png"), b"no").unwrap();
        assert_eq!(wait_for_image(&folder, &["falsa.png".to_string()], Duration::from_millis(300)), None);
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// Un portapapeles propio con un archivo (su URL); se borra al terminar.
    fn pasteboard_with_file(url: &str, test: impl FnOnce(&AnyObject)) {
        let class = AnyClass::get(c"NSPasteboard").unwrap();
        let board: Retained<AnyObject> = unsafe { msg_send![class, pasteboardWithUniqueName] };
        let _: isize = unsafe { msg_send![&*board, clearContents] };
        let written: bool = unsafe { msg_send![&*board, setString: &*NSString::from_str(url), forType: &*NSString::from_str("public.file-url")] };
        assert!(written);
        test(&board);
        let _: () = unsafe { msg_send![&*board, releaseGlobally] };
    }

    #[test]
    fn archivos_sin_ruta() {
        // Una referencia a un archivo que no está: lo que hacía caer a wry (2026-10-18).
        pasteboard_with_file("file:///.file/id=6571367.99999999", |board| assert!(unreadable_files(board)));
        let logo = std::fs::canonicalize("../ui/logo.png").unwrap();
        pasteboard_with_file(&format!("file://{}", logo.display()), |board| assert!(!unreadable_files(board)));
    }

    #[test]
    fn nombres_de_imagen() {
        assert!(is_image_name(Path::new("/Users/yo/Desktop/Captura de pantalla.PNG")));
        assert!(is_image_name(Path::new("foto.heic")));
        assert!(!is_image_name(Path::new("mod.jar")));
        assert!(!is_image_name(Path::new("sin-extension")));
    }
}
