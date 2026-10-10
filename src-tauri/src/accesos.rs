//! Accesos directos de Parabellum en el Escritorio y en el menú Inicio. Parabellum sigue siendo
//! portable (sin instalador): el acceso apunta al exe en su carpeta, así que actualizarlo no lo rompe.
//! Si la carpeta se mueve, el acceso queda apuntando a la vieja y la interfaz ofrece rehacerlo.

use std::path::{Path, PathBuf};

use serde::Serialize;
use windows::core::{Interface, HSTRING};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, STGM_READ};
use windows::Win32::UI::Shell::{FOLDERID_Desktop, FOLDERID_Programs, IShellLinkW, SHGetKnownFolderPath, ShellLink, KF_FLAG_DEFAULT};

const NAME: &str = "Parabellum.lnk";

#[derive(Clone, Copy, Debug)]
pub enum Place {
    Desktop,
    StartMenu,
}

impl Place {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "escritorio" => Some(Self::Desktop),
            "inicio" => Some(Self::StartMenu),
            _ => None,
        }
    }
}

/// Carpeta del Escritorio o de Programas del menú Inicio (las de verdad, también si OneDrive las movió).
fn folder(place: Place) -> Option<PathBuf> {
    let id = match place {
        Place::Desktop => &FOLDERID_Desktop,
        Place::StartMenu => &FOLDERID_Programs,
    };
    // SAFETY: Windows reserva la cadena con CoTaskMemAlloc; se copia y se libera con CoTaskMemFree.
    unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0 as *const _));
        path
    }
}

fn link_path(place: Place) -> Option<PathBuf> {
    Some(folder(place)?.join(NAME))
}

fn init_com() {
    // SAFETY: iniciar COM en este hilo; si ya estaba iniciado, Windows lo dice y no pasa nada.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
}

/// A dónde apunta el acceso directo, si existe.
fn target(place: Place) -> Option<PathBuf> {
    let path = link_path(place).filter(|p| p.exists())?;
    init_com();
    // SAFETY: llamadas COM con interfaces válidas; el búfer vive durante GetPath.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
        link.cast::<IPersistFile>().ok()?.Load(&HSTRING::from(path.as_os_str()), STGM_READ).ok()?;
        let mut buffer = [0u16; 1024];
        link.GetPath(&mut buffer, std::ptr::null_mut(), 0).ok()?;
        let length = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        Some(PathBuf::from(String::from_utf16_lossy(&buffer[..length])))
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "no" (no hay), "si" (apunta a este Parabellum) u "otro" (apunta a otra carpeta).
    pub desktop: &'static str,
    pub start_menu: &'static str,
}

fn state(place: Place, exe: &Path) -> &'static str {
    match target(place) {
        None => "no",
        Some(target) if same_file(&target, exe) => "si",
        Some(_) => "otro",
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    let normal = |p: &Path| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().to_lowercase();
    normal(a) == normal(b)
}

pub fn status(exe: &Path) -> Status {
    Status { desktop: state(Place::Desktop, exe), start_menu: state(Place::StartMenu, exe) }
}

/// Crea (o rehace) el acceso directo a `exe`.
pub fn create(place: Place, exe: &Path) -> Result<(), String> {
    let path = link_path(place).ok_or("No se encontró la carpeta de Windows para el acceso directo.")?;
    init_com();
    let failed = |e: windows::core::Error| format!("No se pudo crear el acceso directo: {}", e.message());
    // SAFETY: llamadas COM con interfaces válidas y cadenas que viven durante cada llamada.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(failed)?;
        link.SetPath(&HSTRING::from(exe.as_os_str())).map_err(failed)?;
        if let Some(dir) = exe.parent() {
            link.SetWorkingDirectory(&HSTRING::from(dir.as_os_str())).map_err(failed)?;
        }
        link.SetIconLocation(&HSTRING::from(exe.as_os_str()), 0).map_err(failed)?;
        link.SetDescription(&HSTRING::from("Launcher de Minecraft")).map_err(failed)?;
        link.cast::<IPersistFile>().map_err(failed)?.Save(&HSTRING::from(path.as_os_str()), true).map_err(failed)
    }
}

/// Quita el acceso directo (a la papelera, como todo lo que borra Parabellum).
pub fn remove(place: Place) -> Result<(), String> {
    match link_path(place).filter(|p| p.exists()) {
        Some(path) => trash::delete(path).map_err(|e| format!("No se pudo quitar el acceso directo: {e}")),
        None => Ok(()),
    }
}
