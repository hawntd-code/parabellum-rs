//! Las versiones de Parabellum en el catálogo (ver catalog.rs) en Mac: se publica y se reemplaza
//! Parabellum.app entera (la que arma src-tauri/mac/empaquetar.sh), no solo el programa.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Id de las actualizaciones para Mac. Su carpeta en Drive es _parabellum-mac, junto a la de Windows
/// (_parabellum): ninguno baja la del otro, y publicar una no borra la otra.
pub const ID: &str = "parabellum-mac";

/// El identificador de Parabellum.app (el de tauri.conf.json y src-tauri/mac/Info.plist).
const BUNDLE_ID: &str = "mx.parabellum.launcher";

/// La Parabellum.app con la que se abrió este Parabellum.
fn current_app() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.ancestors().find(|p| p.extension().is_some_and(|e| e == "app")).map(Path::to_path_buf).ok_or(t!(
        "Este Parabellum no está dentro de Parabellum.app (se compiló con cargo build). Ármala con src-tauri/mac/empaquetar.sh.",
        "This Parabellum is not inside Parabellum.app (it was built with cargo build). Build it with src-tauri/mac/empaquetar.sh."
    ))
}

/// Lo que va en el zip: toda la app, con rutas «Parabellum.app/Contents/…».
pub fn published_files() -> Result<Vec<(PathBuf, String)>, String> {
    fn walk(folder: &Path, base: &Path, files: &mut Vec<(PathBuf, String)>) -> Result<(), String> {
        for entry in std::fs::read_dir(folder).map_err(|e| e.to_string())?.flatten() {
            let path = entry.path();
            let kind = entry.file_type().map_err(|e| e.to_string())?;
            if kind.is_dir() {
                walk(&path, base, files)?;
            } else if kind.is_file() {
                let name = path.strip_prefix(base).map_err(|e| e.to_string())?.to_string_lossy().to_string();
                files.push((path, name));
            }
        }
        Ok(())
    }
    let app = current_app()?;
    let mut files = Vec::new();
    walk(&app, app.parent().unwrap_or(Path::new("/")), &mut files)?;
    files.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(files)
}

/// Deja la versión nueva (el zip, ya comprobado) en lugar de la app actual. Devuelve la ruta de la app.
pub fn install(zip: &Path, staging: &Path) -> Result<PathBuf, String> {
    let app = current_app()?;
    replace(&app, zip, staging)?;
    Ok(app)
}

fn replace(app: &Path, zip: &Path, staging: &Path) -> Result<(), String> {
    // Desde el .dmg (/Volumes/…) o desde la copia de solo lectura que hace macOS de una app recién bajada
    // que nadie ha movido («App Translocation») no se puede cambiar.
    let shown = app.display().to_string();
    if shown.contains("/AppTranslocation/") || shown.starts_with("/Volumes/") {
        return Err(t!(
            "Para actualizarse, Parabellum tiene que estar en tu carpeta Aplicaciones: arrástralo ahí (desde el .dmg o desde Descargas), ábrelo desde ahí y vuelve a intentarlo.",
            "To update itself, Parabellum has to be in your Applications folder: drag it there (from the .dmg or from Downloads), open it from there and try again."
        ));
    }
    let unpacked = staging.join("nueva");
    let _ = std::fs::remove_dir_all(&unpacked);
    extract(zip, &unpacked)?;
    let fresh = unpacked.join("Parabellum.app");
    // El zip no guarda el permiso de ejecución.
    super::make_executable(&fresh.join("Contents").join("MacOS").join(super::EXECUTABLE));
    check(&fresh)?;

    let old = PathBuf::from(format!("{shown}.old"));
    let _ = std::fs::remove_dir_all(&old);
    std::fs::rename(app, &old).map_err(|e| {
        t!("No hay permiso para cambiar {shown} ({e}). Si está en Aplicaciones, tu usuario necesita ser administrador.", "There is no permission to change {shown} ({e}). If it is in Applications, your user needs to be an administrator.")
    })?;
    // rename no sirve entre discos; ditto copia la app completa.
    let moved = std::fs::rename(&fresh, app).map_err(|e| e.to_string()).or_else(|_| {
        let status = Command::new("/usr/bin/ditto").arg(&fresh).arg(app).status().map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err(format!("ditto: {status}")) }
    });
    if let Err(error) = moved {
        // Deshacer: todo queda como estaba.
        let _ = std::fs::remove_dir_all(app);
        let _ = std::fs::rename(&old, app);
        return Err(t!("No se pudo instalar la actualización: {error}", "Could not install the update: {error}"));
    }
    let _ = std::fs::remove_dir_all(&unpacked);
    Ok(())
}

/// Saca la app del zip, solo con rutas dentro de «Parabellum.app/».
fn extract(zip: &Path, target: &Path) -> Result<(), String> {
    let damaged = |e: &dyn std::fmt::Display| t!("La actualización está dañada: {e}", "The update is damaged: {e}");
    let mut archive = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| damaged(&e))?;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| damaged(&e))?;
        let Some(relative) = entry.enclosed_name().filter(|n| n.starts_with("Parabellum.app")) else { continue };
        let destination = target.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&destination).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| damaged(&e))?;
    }
    Ok(())
}

/// Que lo descomprimido sea Parabellum.app y esté completa: su identificador y su firma (la firma ad hoc
/// de empaquetar.sh cubre el programa y cada archivo de la app).
fn check(app: &Path) -> Result<(), String> {
    let plist = app.join("Contents").join("Info.plist");
    let output = Command::new("/usr/bin/plutil").args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"]).arg(&plist).output().map_err(|e| e.to_string())?;
    if String::from_utf8_lossy(&output.stdout).trim() != BUNDLE_ID {
        return Err(t!("La actualización no es Parabellum.app.", "The update is not Parabellum.app."));
    }
    let verified = Command::new("/usr/bin/codesign").args(["--verify", "--strict"]).arg(app).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    if !verified {
        return Err(t!("La actualización está dañada (su firma no coincide). Vuelve a intentarlo.", "The update is damaged (its signature does not match). Try again."));
    }
    Ok(())
}

/// Abre la versión nueva, que espera a que esta termine de cerrarse (--reiniciar). Con `open`, como si se
/// abriera desde el Finder; `open` no pasa el entorno, así que PARABELLUM_ROOT (pruebas) va aparte: si no,
/// la nueva abriría la carpeta de verdad.
pub fn restart(app: &Path) -> Result<(), String> {
    let mut command = Command::new("/usr/bin/open");
    command.arg("-n");
    if let Ok(root) = std::env::var("PARABELLUM_ROOT") {
        command.arg("--env").arg(format!("PARABELLUM_ROOT={root}"));
    }
    let status = command.arg(app).args(["--args", "--reiniciar"]).status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("open: {status}"))
    }
}

/// La app anterior, que quedó como «Parabellum.app.old» junto a esta al actualizarse.
pub fn cleanup() {
    if let Ok(app) = current_app() {
        let _ = std::fs::remove_dir_all(format!("{}.old", app.display()));
    }
}

/// Instrucciones de la primera vez: la app no lleva firma de Apple, así que macOS pide confirmar.
pub fn readme(version: &str) -> String {
    format!(
        "PARABELLUM {version} PARA MAC — CÓMO EMPEZAR\n\
         \n\
         Necesita una Mac con chip Apple (M1 o más nueva) y macOS 13 o más nuevo.\n\
         \n\
         1. Descarga Parabellum-{version}.zip y ábrelo (doble clic): sale Parabellum.app.\n\
         2. Arrástrala a tu carpeta Aplicaciones (desde otra carpeta no se puede actualizar sola).\n\
         3. Ábrela. La primera vez macOS dice que no puede comprobar la app: no tiene una firma de pago de\n\
         \x20  Apple. Ve a Ajustes del Sistema › Privacidad y seguridad y pulsa «Abrir de todas formas».\n\
         \x20  Solo pasa la primera vez.\n\
         4. Desde ahí Parabellum se actualiza sola cuando se publica una versión nueva en esta carpeta.\n\
         \n\
         ---\n\
         \n\
         PARABELLUM {version} FOR MAC — GETTING STARTED\n\
         \n\
         It needs a Mac with Apple silicon (M1 or newer) and macOS 13 or newer.\n\
         \n\
         1. Download Parabellum-{version}.zip and open it (double click): you get Parabellum.app.\n\
         2. Drag it to your Applications folder (from another folder it cannot update itself).\n\
         3. Open it. The first time macOS says it cannot check the app: it has no paid Apple signature.\n\
         \x20  Go to System Settings › Privacy & Security and click «Open Anyway». It only happens the first time.\n\
         4. From then on Parabellum updates itself when a new version is published in this folder.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Una Parabellum.app de juguete, firmada ad hoc como la de empaquetar.sh.
    fn toy_app(folder: &Path, marker: &str) -> PathBuf {
        let app = folder.join("Parabellum.app");
        std::fs::create_dir_all(app.join("Contents").join("MacOS")).unwrap();
        std::fs::copy("/usr/bin/true", app.join("Contents").join("MacOS").join("Parabellum")).unwrap();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleIdentifier</key><string>{BUNDLE_ID}</string><key>CFBundleExecutable</key><string>Parabellum</string><key>CFBundleShortVersionString</key><string>{marker}</string></dict></plist>"#
        );
        std::fs::write(app.join("Contents").join("Info.plist"), plist).unwrap();
        assert!(Command::new("/usr/bin/codesign").args(["--force", "--sign", "-"]).arg(&app).stderr(Stdio::null()).status().unwrap().success());
        app
    }

    fn zip_of(app: &Path, zip: &Path) {
        let mut out = crate::packs::ZipOut::create(zip).unwrap();
        let mut stack = vec![app.to_path_buf()];
        while let Some(folder) = stack.pop() {
            for entry in std::fs::read_dir(&folder).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.add_file(&path, &path.strip_prefix(app.parent().unwrap()).unwrap().to_string_lossy()).unwrap();
                }
            }
        }
        out.finish().unwrap();
    }

    fn version_of(app: &Path) -> String {
        let output = Command::new("/usr/bin/plutil").args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"]).arg(app.join("Contents").join("Info.plist")).output().unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn reemplaza_la_app_entera() {
        let folder = std::env::temp_dir().join(format!("parabellum-lanzador-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        // La nueva, empaquetada como la publica Parabellum.
        let fresh = toy_app(&folder.join("publicada"), "2.0.0");
        let zip = folder.join("Parabellum-2.0.0.zip");
        zip_of(&fresh, &zip);
        // La instalada.
        let app = toy_app(&folder.join("Aplicaciones"), "1.0.0");
        replace(&app, &zip, &folder.join("actualizacion")).unwrap();
        assert_eq!(version_of(&app), "2.0.0");
        assert_eq!(version_of(&PathBuf::from(format!("{}.old", app.display()))), "1.0.0");
        // El programa quedó con permiso de ejecución (el zip no lo guarda) y la firma sigue valiendo. No se
        // ejecuta: es una copia de /usr/bin/true firmada ad hoc, y la Mac de GitHub (macOS 14) no la deja
        // correr, aunque esta Mac sí.
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(app.join("Contents").join("MacOS").join("Parabellum")).unwrap().permissions().mode();
        assert_eq!(mode & 0o111, 0o111);
        check(&app).unwrap();

        // Un zip alterado no se instala y la app queda como estaba.
        let broken = folder.join("alterada");
        let altered = toy_app(&broken, "3.0.0");
        std::fs::write(altered.join("Contents").join("Info.plist"), std::fs::read_to_string(altered.join("Contents").join("Info.plist")).unwrap().replace("3.0.0", "3.0.1")).unwrap();
        let bad_zip = folder.join("Parabellum-3.0.0.zip");
        zip_of(&altered, &bad_zip);
        assert!(replace(&app, &bad_zip, &folder.join("actualizacion")).is_err());
        assert_eq!(version_of(&app), "2.0.0");
        let _ = std::fs::remove_dir_all(&folder);
    }
}
