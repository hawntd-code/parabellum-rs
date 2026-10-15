//! Arrastrar y pegar imágenes en el chat (Mac). Mismo contrato que plataforma/windows/arrastre.rs.
//!
//! En Mac sigue el arrastre de Tauri (archivos del Finder): la interfaz escucha sus eventos además de
//! "arrastre". TODO(mac): una imagen arrastrada desde Safari o Discord (sin archivo) y la imagen del
//! portapapeles (NSPasteboard: public.png, public.tiff o un archivo copiado en el Finder) para
//! `clipboard_image`; mientras, pegar con Cmd+V funciona con lo que la ventana entiende sola.

use std::path::{Path, PathBuf};

pub fn accept_images(_on: bool) {}

pub fn install(_window: &tauri::WebviewWindow, _temp: PathBuf) {}

pub fn clipboard_image(_temp: &Path) -> Result<Option<PathBuf>, String> {
    Ok(None)
}
