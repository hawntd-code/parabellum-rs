//! Reportes de fallos, si el jugador lo permite (Ajustes › «Enviar reportes de fallos», o la pregunta del
//! doctor la primera vez que se cae el juego). Cuando el juego se cierra por un error se manda a Firebase lo
//! necesario para saber qué falló, sin datos personales (`reporte::redact`):
//!   crashReports/{uid}/{id} = { at, app, os, minecraft, loader, modpack, mods, problem, code, log }
//!   crashReportsLast/{uid} = cuándo fue el último (las reglas dejan uno por minuto)
//! Desde Parabellum nadie los puede leer, ni quien los mandó: solo quien administra el proyecto, en la
//! consola de Firebase. Reglas y pruebas: firebase-reglas.json y docs/probar-reglas.py.

use crate::diagnostico::Diagnosis;
use crate::instances::Instance;
use crate::reporte::{cut, redact};
use serde_json::{json, Map, Value};
use std::path::Path;
use tauri::AppHandle;

const LOG_MAX: usize = 20_000;
const LOG_LINES: usize = 150;

/// Las primeras (informe de fallo) o las últimas (registro) líneas de un archivo de texto.
fn lines(path: &Path, first: bool) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let all: Vec<&str> = text.lines().collect();
    let chosen = if first { &all[..all.len().min(LOG_LINES)] } else { &all[all.len().saturating_sub(LOG_LINES)..] };
    Some(chosen.join("\n"))
}

/// El informe de fallo de Minecraft que dejó esta caída (el más nuevo desde `since_ms`), si dejó uno.
fn crash_report(instance: &Instance, since_ms: i64) -> Option<String> {
    let newest = std::fs::read_dir(instance.game_dir.join("crash-reports"))
        .ok()?
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
            (modified >= since_ms).then(|| (modified, e.path()))
        })
        .max_by_key(|(at, _)| *at)?;
    lines(&newest.1, true)
}

/// El reporte de la caída de `instance` que empezó en `since_ms`.
pub fn build(instance: &Instance, diagnosis: &Diagnosis, code: Option<i32>, since_ms: i64) -> Value {
    let loader = match &instance.loader_version {
        Some(version) if instance.loader != "Vanilla" => format!("{} {version}", instance.loader),
        _ => instance.loader.clone(),
    };
    let problem = diagnosis.findings.iter().find(|f| f.severity != "info").map(|f| f.title.clone()).unwrap_or_else(|| "Sin diagnóstico".into());
    let mods = std::fs::read_dir(instance.game_dir.join("mods"))
        .map(|dir| dir.flatten().filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("jar"))).count())
        .unwrap_or(0);
    let log = crash_report(instance, since_ms).or_else(|| lines(&instance.game_dir.join("logs").join("latest.log"), false)).unwrap_or_default();
    json!({
        "at": crate::firebase::server_timestamp(),
        "app": env!("CARGO_PKG_VERSION"),
        "os": format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
        "minecraft": cut(&instance.minecraft_version, 40),
        "loader": cut(&loader, 60),
        "modpack": cut(instance.modpack.as_deref().unwrap_or(""), 120),
        "mods": mods,
        "problem": cut(&redact(&problem), 200),
        "code": code.unwrap_or(0),
        "log": cut(&redact(&log), LOG_MAX),
    })
}

/// Manda el reporte (si hay sesión de Google). Devuelve si se mandó.
pub async fn send(app: &AppHandle, report: Value) -> Result<bool, String> {
    let Some(db) = crate::cuenta::social_handle(app).map(|s| s.database()) else { return Ok(false) };
    let uid = db.session.uid.clone();
    let mut update = Map::new();
    update.insert(format!("crashReports/{uid}/{}", crate::mundos_archivo::id()), report);
    update.insert(format!("crashReportsLast/{uid}"), crate::firebase::server_timestamp());
    db.patch("", &Value::Object(update)).await.map_err(|e| e.to_string())?;
    Ok(true)
}

/// Manda el reporte de la última caída de esa instancia; con `siempre`, desde ahora se mandan solos.
#[tauri::command]
pub async fn enviar_reporte_fallo(app: AppHandle, id: String, siempre: bool) -> Result<bool, String> {
    use tauri::Manager;
    let state = app.state::<crate::AppState>();
    if siempre {
        let mut settings = state.settings.lock().unwrap();
        settings.crash_reports = Some(true);
        settings.save(&state.paths).map_err(|e| e.to_string())?;
    }
    let instance = crate::instances::find(&state.paths, &id).ok_or_else(|| t!("No se encontró la instancia.", "The instance was not found."))?;
    let since = instance.last_played.unwrap_or(0);
    let ram_gb = instance.ram_gb.unwrap_or_else(|| state.settings.lock().unwrap().ram_gb);
    let diagnosis = crate::diagnostico::after_crash(&instance, &crate::diagnostico::CrashContext { since_ms: since, exit_code: None, ram_gb, system_ram_gb: 0 });
    let report = build(&instance, &diagnosis, None, since);
    send(&app, report).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_reporte_no_lleva_la_carpeta_del_usuario_y_tiene_tope() {
        let root = std::env::temp_dir().join(format!("parabellum-fallos-{}", std::process::id()));
        std::fs::create_dir_all(root.join("logs")).unwrap();
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap();
        let log: String = (0..400).map(|i| format!("[{i}] cargando {home}\\mods\\x.jar\n")).collect();
        std::fs::write(root.join("logs").join("latest.log"), log).unwrap();
        let instance = Instance { name: "Prueba".into(), minecraft_version: "1.20.1".into(), loader: "Fabric".into(), loader_version: Some("0.16.10".into()), game_dir: root.clone(), folder: root.clone(), ..Default::default() };
        let report = build(&instance, &Diagnosis::default(), Some(1), 0);
        let text = report["log"].as_str().unwrap();
        assert!(!text.contains(&home), "sin la carpeta del usuario");
        assert!(text.contains("[399]") && !text.contains("[100]"), "las últimas líneas");
        assert!(text.chars().count() <= LOG_MAX);
        assert_eq!(report["loader"], "Fabric 0.16.10");
        assert_eq!(report["problem"], "Sin diagnóstico");
        let _ = std::fs::remove_dir_all(&root);
    }
}
