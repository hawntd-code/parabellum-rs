//! Arrastrar y pegar imágenes en el chat (Mac). Mismo contrato que plataforma/windows/arrastre.rs.
//!
//! En Mac sigue el arrastre de Tauri (archivos del Finder): la interfaz escucha sus eventos además de
//! "arrastre". La imagen del portapapeles (`clipboard_image`) se lee con AppleScript (`osascript`), que
//! trae macOS: un archivo de imagen copiado en el Finder o la imagen misma (una captura con ⌃⇧⌘4, una
//! copiada de Safari o de Vista Previa). TODO(mac): una imagen arrastrada desde Safari o Discord (sin
//! archivo) necesita un receptor de arrastre propio en la ventana, como el de Windows.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Las que se mandan al chat (las demás se ignoran al pegar).
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "jfif", "gif", "webp", "bmp", "avif", "heic", "heif", "tif", "tiff", "ico"];

pub fn accept_images(_on: bool) {}

pub fn install(_window: &tauri::WebviewWindow, _temp: PathBuf) {}

fn is_image_name(path: &Path) -> bool {
    path.extension().is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()))
}

/// Que un archivo copiado sea de verdad una imagen y no un enlace a otro archivo: lo que se pega pasa
/// tal cual a la ventana (cuenta::leer_imagen), así que no basta con el nombre. Se miran sus primeros
/// bytes: PNG, JPEG, GIF, WebP, BMP, TIFF, ICO o HEIC/HEIF/AVIF (los de «ftyp»).
fn is_image_file(path: &Path) -> bool {
    use std::io::Read;
    if !is_image_name(path) || !path.symlink_metadata().is_ok_and(|m| m.file_type().is_file()) {
        return false;
    }
    let mut head = [0u8; 12];
    let Ok(read) = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)) else { return false };
    let head = &head[..read];
    head.starts_with(b"\x89PNG\r\n\x1a\n")
        || head.starts_with(&[0xFF, 0xD8, 0xFF])
        || head.starts_with(b"GIF8")
        || (head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WEBP"))
        || head.starts_with(b"BM")
        || head.starts_with(b"II*\0")
        || head.starts_with(b"MM\0*")
        || head.starts_with(&[0, 0, 1, 0])
        || head.get(4..8) == Some(b"ftyp")
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
    fn nombres_de_imagen() {
        assert!(is_image_name(Path::new("/Users/yo/Desktop/Captura de pantalla.PNG")));
        assert!(is_image_name(Path::new("foto.heic")));
        assert!(!is_image_name(Path::new("mod.jar")));
        assert!(!is_image_name(Path::new("sin-extension")));
    }
}
