//! Que una actualización de Parabellum que no arranca no deje a nadie sin launcher (mejora 3 de
//! docs/plan-diciembre.md). Al actualizar:
//!  1. La versión anterior queda al lado (.old, ver plataforma::lanzador) y se anota `pendiente.json`:
//!     de qué versión a cuál y dónde quedó la nueva.
//!  2. No se abre la nueva directamente: se abre un vigía, el exe anterior con --vigilar-actualizacion,
//!     que abre la nueva y espera su confirmación.
//!  3. La nueva confirma en cuanto carga su ventana (`confirmar_arranque`), antes de tocar internet: un
//!     fallo de la red no cuenta como un exe roto. Ahí se puede borrar el .old.
//!  4. Si en 90 s no confirma (no abre, se cierra sola o se traba), el vigía la cierra, vuelve a poner la
//!     anterior, la abre y anota la versión fallida: no se vuelve a ofrecer hasta que salga otra.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::paths::Paths;

const PENDING: &str = "pendiente.json";
const FAILED: &str = "fallida.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Pending {
    from: String,
    to: String,
    /// El exe (o la .app en Mac) de la versión nueva.
    installed: PathBuf,
    at: i64,
    /// El proceso de la versión nueva, para cerrarlo si se traba sin confirmar.
    #[serde(default)]
    pid: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct Failed {
    version: String,
    from: String,
    at: i64,
    /// Ya se le avisó al jugador.
    #[serde(default)]
    shown: bool,
}

static FAILED_VERSION: Mutex<Option<String>> = Mutex::new(None);

fn folder(paths: &Paths) -> PathBuf {
    paths.data.join("actualizacion")
}

fn read<T: for<'de> Deserialize<'de>>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).map_err(|e| e.to_string())
}

fn confirmation(paths: &Paths, version: &str) -> PathBuf {
    folder(paths).join(format!("confirmado-{version}"))
}

fn log(paths: &Paths, line: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(&paths.logs);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(paths.logs.join("actualizacion.log")) {
        let _ = writeln!(file, "[{}] {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Cuánto espera el vigía la confirmación (PARABELLUM_ESPERA_ARRANQUE en segundos, para las pruebas).
fn wait() -> Duration {
    Duration::from_secs(std::env::var("PARABELLUM_ESPERA_ARRANQUE").ok().and_then(|s| s.parse().ok()).unwrap_or(90))
}

/// Al abrir Parabellum (con su candado ya tomado). Devuelve si esta es la versión nueva que todavía tiene
/// que confirmar: entonces el .old se conserva hasta que confirme.
pub fn startup(paths: &Paths) -> bool {
    *FAILED_VERSION.lock().unwrap() = read::<Failed>(&folder(paths).join(FAILED)).map(|f| f.version);
    let path = folder(paths).join(PENDING);
    match read::<Pending>(&path) {
        Some(mut pending) if pending.to == current_version() => {
            pending.pid = Some(std::process::id());
            let _ = write(&path, &pending);
            true
        }
        Some(_) => {
            let _ = std::fs::remove_file(&path);
            false
        }
        None => false,
    }
}

/// La versión que no arrancó en este PC: no se ofrece otra vez.
pub fn failed_version() -> Option<String> {
    FAILED_VERSION.lock().unwrap().clone()
}

/// La ventana cargó: esta versión arranca. Devuelve, una sola vez, el aviso de una versión que no arrancó
/// y se deshizo.
pub fn confirm(paths: &Paths) -> Option<String> {
    let path = folder(paths).join(PENDING);
    if let Some(pending) = read::<Pending>(&path).filter(|p| p.to == current_version()) {
        let _ = std::fs::write(confirmation(paths, &pending.to), b"");
        let _ = std::fs::remove_file(&path);
        log(paths, &format!("{} confirmó su arranque (antes {})", pending.to, pending.from));
        // El vigía corre desde el .old: se borra cuando termine de cerrarse.
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_secs(6));
            crate::plataforma::lanzador::cleanup();
        });
    }
    let failed_path = folder(paths).join(FAILED);
    let mut failed = read::<Failed>(&failed_path)?;
    if failed.shown {
        return None;
    }
    failed.shown = true;
    let _ = write(&failed_path, &failed);
    Some(t!(
        "La versión {} de Parabellum no arrancó en este PC, así que volviste a la {}. Se probará de nuevo cuando salga otra. Si puedes, avísale a quien publica.",
        "Parabellum version {} did not start on this PC, so you are back on {}. It will be tried again when another one comes out. If you can, tell whoever publishes.",
        failed.version,
        current_version()
    ))
}

/// Tras dejar la versión nueva en su lugar: anota la espera y abre el vigía (el exe anterior), que abre la
/// nueva. Si el vigía no abre, se abre la nueva directamente (como antes).
pub fn hand_over(paths: &Paths, installed: &Path, to: &str) -> Result<(), String> {
    // Una versión anterior a la que confirma su arranque (al volver a estable) no avisaría nunca: el vigía
    // la daría por rota. Se abre directo.
    if crate::loaders::compare_versions(to, crate::canal::FIRST_CONFIRMING) == std::cmp::Ordering::Less {
        log(paths, &format!("cambiando de {} a {to}: sin vigía (no confirma su arranque)", current_version()));
        return crate::plataforma::lanzador::restart(installed);
    }
    let pending = Pending { from: current_version().into(), to: to.into(), installed: installed.to_path_buf(), at: chrono::Utc::now().timestamp_millis(), pid: None };
    write(&folder(paths).join(PENDING), &pending)?;
    let _ = std::fs::remove_file(confirmation(paths, to));
    let watcher = crate::plataforma::lanzador::previous_executable(installed)
        .and_then(|exe| std::process::Command::new(exe).arg("--vigilar-actualizacion").spawn().ok());
    if watcher.is_none() {
        log(paths, "no se pudo abrir el vigía: se abre la versión nueva sin él");
        let _ = std::fs::remove_file(folder(paths).join(PENDING));
        return crate::plataforma::lanzador::restart(installed);
    }
    log(paths, &format!("actualizando de {} a {to}: vigía abierto", current_version()));
    Ok(())
}

/// El vigía: corre con el exe anterior, sin ventana ni candado. Abre la nueva y, si no confirma a tiempo,
/// vuelve a la anterior.
pub fn watch(paths: &Paths) {
    let Some(pending) = read::<Pending>(&folder(paths).join(PENDING)) else { return };
    // La versión anterior se está cerrando (la nueva además espera su candado).
    std::thread::sleep(Duration::from_millis(1500));
    if let Err(error) = crate::plataforma::lanzador::restart(&pending.installed) {
        log(paths, &format!("la versión {} no se pudo abrir: {error}", pending.to));
        return roll_back(paths, &pending);
    }
    let started = Instant::now();
    let confirmed = confirmation(paths, &pending.to);
    while started.elapsed() < wait() {
        if confirmed.exists() {
            let _ = std::fs::remove_file(&confirmed);
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    log(paths, &format!("la versión {} no confirmó su arranque en {} s", pending.to, wait().as_secs()));
    // Si se trabó, se cierra (anotó su proceso al abrir).
    if let Some(pid) = read::<Pending>(&folder(paths).join(PENDING)).and_then(|p| p.pid) {
        crate::launch::kill(pid);
        std::thread::sleep(Duration::from_secs(1));
    }
    roll_back(paths, &pending);
}

fn roll_back(paths: &Paths, pending: &Pending) {
    match crate::plataforma::lanzador::rollback(&pending.installed) {
        Ok(app) => {
            let failed = Failed { version: pending.to.clone(), from: pending.from.clone(), at: chrono::Utc::now().timestamp_millis(), shown: false };
            let _ = write(&folder(paths).join(FAILED), &failed);
            let _ = std::fs::remove_file(folder(paths).join(PENDING));
            log(paths, &format!("se volvió a la versión {}", pending.from));
            if let Err(error) = crate::plataforma::lanzador::restart(&app) {
                log(paths, &format!("no se pudo abrir la versión anterior: {error}"));
            }
        }
        Err(error) => log(paths, &format!("no se pudo volver a la versión anterior: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirmar_y_avisar_una_vez() {
        let paths = Paths::new(std::env::temp_dir().join(format!("parabellum-arranque-{}", std::process::id())));
        std::fs::create_dir_all(folder(&paths)).unwrap();
        // La versión nueva (esta) espera confirmar.
        let pending = Pending { from: "0.1.0".into(), to: current_version().into(), installed: PathBuf::from("x"), at: 0, pid: None };
        write(&folder(&paths).join(PENDING), &pending).unwrap();
        assert!(startup(&paths));
        assert_eq!(read::<Pending>(&folder(&paths).join(PENDING)).unwrap().pid, Some(std::process::id()));
        assert!(confirm(&paths).is_none());
        assert!(confirmation(&paths, current_version()).exists());
        assert!(!folder(&paths).join(PENDING).exists());
        // Una versión que falló: se avisa una vez y no se ofrece.
        write(&folder(&paths).join(FAILED), &Failed { version: "9.9.9".into(), from: current_version().into(), at: 0, shown: false }).unwrap();
        assert!(!startup(&paths));
        assert_eq!(failed_version().as_deref(), Some("9.9.9"));
        assert!(confirm(&paths).is_some_and(|m| m.contains("9.9.9")));
        assert!(confirm(&paths).is_none());
        // Un pendiente de otra versión (de un vigía que ya terminó) se quita.
        write(&folder(&paths).join(PENDING), &Pending { to: "0.0.1".into(), ..pending }).unwrap();
        assert!(!startup(&paths));
        assert!(!folder(&paths).join(PENDING).exists());
        *FAILED_VERSION.lock().unwrap() = None;
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
