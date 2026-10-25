//! Las versiones de Parabellum en el catálogo (ver catalog.rs) en Windows: qué se publica, cómo se pone
//! la nueva en lugar de esta y cómo se reabre. Aquí Parabellum es el .exe suelto.

use std::path::{Path, PathBuf};

/// Id de las actualizaciones de este launcher (la versión C# usa "parabellum-lanzador"). Su carpeta en
/// Drive es la misma con «_» delante: _parabellum.
pub const ID: &str = "parabellum";

/// Lo que va en el zip de una versión: este mismo exe.
pub fn published_files() -> Result<Vec<(PathBuf, String)>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(vec![(exe, super::EXECUTABLE.to_string())])
}

/// Deja la versión nueva (el zip, ya comprobado) en lugar del exe actual. Devuelve su ruta.
pub fn install(zip: &Path, staging: &Path) -> Result<PathBuf, String> {
    let fresh = staging.join("Parabellum.nuevo.exe");
    {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(zip).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let executable = super::EXECUTABLE.to_lowercase();
        let name = archive.file_names().find(|n| n.to_lowercase().ends_with(&executable)).map(String::from).ok_or(t!("La actualización no trae el ejecutable.", "The update does not contain the executable."))?;
        let mut source = archive.by_name(&name).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(&fresh).map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut out).map_err(|e| t!("La actualización está dañada: {e}", "The update is damaged: {e}"))?;
    }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    // El .old de la actualización anterior: si no se deja borrar (un Parabellum viejo que sigue abierto,
    // un antivirus revisándolo), el exe actual se aparta con otro nombre en vez de fallar.
    let mut old = PathBuf::from(format!("{}.old", current.display()));
    if std::fs::remove_file(&old).is_err() && old.exists() {
        old = PathBuf::from(format!("{}.old-{}", current.display(), chrono::Utc::now().timestamp()));
    }
    std::fs::rename(&current, &old).map_err(|e| {
        t!("No hay permiso para cambiar la carpeta del launcher ({e}). Cierra los demás Parabellum (también junto al reloj) o cópialo a una carpeta tuya (Escritorio, USB…) y actualiza desde ahí.", "There is no permission to change the launcher folder ({e}). Close the other Parabellum windows (also next to the clock) or copy it to a folder of yours (Desktop, USB…) and update from there.")
    })?;
    if let Err(error) = std::fs::rename(&fresh, &current).or_else(|_| std::fs::copy(&fresh, &current).map(|_| ())) {
        let _ = std::fs::rename(&old, &current); // deshacer: todo queda como estaba
        return Err(t!("No se pudo instalar la actualización: {error}", "Could not install the update: {error}"));
    }
    Ok(current)
}

/// Abre la versión nueva, que espera a que esta termine de cerrarse (--reiniciar).
pub fn restart(path: &Path) -> Result<(), String> {
    std::process::Command::new(path).arg("--reiniciar").spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Al actualizarse, el exe anterior queda como .old (Windows deja renombrar un exe abierto, no borrarlo).
/// También los .old-<hora> que quedaron cuando el .old de antes no se podía borrar, y el .fallida de una
/// versión que no arrancó. Solo se llama cuando esta versión ya confirmó su arranque (ver arranque.rs).
pub fn cleanup() {
    let Ok(exe) = std::env::current_exe() else { return };
    let (Some(folder), Some(name)) = (exe.parent(), exe.file_name().map(|n| n.to_string_lossy().to_lowercase())) else { return };
    for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
        let file = entry.file_name().to_string_lossy().to_lowercase();
        if file == format!("{name}.old") || file.starts_with(&format!("{name}.old-")) || file == format!("{name}.fallida") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// El exe anterior a `installed` (el .old, o el .old-<hora> más nuevo).
fn previous(installed: &Path) -> Option<PathBuf> {
    let (folder, name) = (installed.parent()?, installed.file_name()?.to_string_lossy().to_lowercase());
    std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter(|e| {
            let file = e.file_name().to_string_lossy().to_lowercase();
            file == format!("{name}.old") || file.starts_with(&format!("{name}.old-"))
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// Lo que se abre como vigía de la actualización (arranque.rs): el exe anterior.
pub fn previous_executable(installed: &Path) -> Option<PathBuf> {
    previous(installed)
}

/// La versión nueva no arrancó: queda aparte (.fallida) y la anterior vuelve a su lugar. Devuelve qué abrir.
pub fn rollback(installed: &Path) -> Result<PathBuf, String> {
    let previous = previous(installed).ok_or(t!("No está la versión anterior.", "The previous version is not there."))?;
    let failed = PathBuf::from(format!("{}.fallida", installed.display()));
    let _ = std::fs::remove_file(&failed);
    if installed.exists() {
        std::fs::rename(installed, &failed).map_err(|e| e.to_string())?;
    }
    if let Err(error) = std::fs::rename(&previous, installed) {
        let _ = std::fs::rename(&failed, installed);
        return Err(error.to_string());
    }
    Ok(installed.to_path_buf())
}

/// Instrucciones de la primera vez: el exe no lleva firma de pago, así que Windows avisa al abrirlo.
pub fn readme(version: &str) -> String {
    format!(
        "PARABELLUM {version} — CÓMO EMPEZAR\r\n\
         \r\n\
         1. Descarga Parabellum-{version}.zip y descomprímelo en una carpeta tuya (Escritorio, Documentos o una USB).\r\n\
         \x20  No se instala nada: todo queda en esa carpeta.\r\n\
         2. Abre Parabellum.exe. Si Windows muestra «Windows protegió tu PC», pulsa «Más información» y luego\r\n\
         \x20  «Ejecutar de todas formas». Sale porque el programa no tiene una firma de pago; solo pasa la primera vez.\r\n\
         3. Desde ahí Parabellum se actualiza solo cuando se publica una versión nueva en esta carpeta.\r\n\
         \x20  Para tenerlo a mano: Ajustes › Comportamiento › Acceso directo en el Escritorio o en el menú Inicio.\r\n\
         \r\n\
         Parabellum se usa desde sus descargas oficiales. Su licencia está en LICENCIA.txt y las de los\r\n\
         terceros en AVISOS-DE-TERCEROS.txt.\r\n\
         \r\n\
         ---\r\n\
         \r\n\
         PARABELLUM {version} — GETTING STARTED\r\n\
         \r\n\
         1. Download Parabellum-{version}.zip and unzip it into a folder of yours (Desktop, Documents or a USB drive).\r\n\
         \x20  Nothing is installed: everything stays in that folder.\r\n\
         2. Open Parabellum.exe. If Windows shows «Windows protected your PC», click «More info» and then\r\n\
         \x20  «Run anyway». It appears because the program has no paid signature; it only happens the first time.\r\n\
         3. From then on Parabellum updates itself when a new version is published in this folder.\r\n\
         \x20  To keep it handy: Settings › Behavior › Desktop or Start menu shortcut.\r\n\
         \r\n\
         Parabellum is used from its official downloads. Its license is in LICENCIA.txt and the\r\n\
         third-party ones in AVISOS-DE-TERCEROS.txt.\r\n"
    )
}
