//! Descargas que el usuario inicia en el portal real de CurseForge. La página no recibe permisos
//! del launcher. Los archivos llegan a una carpeta temporal y solo entran a la instancia si su huella
//! coincide con la versión que se pidió a la API. Los pendientes sobreviven al cierre de Parabellum.

use crate::{curseforge::{self, Manual}, instances::{self, Instance}, AppState};
use serde::Serialize;
use std::{path::{Path, PathBuf}, sync::{Arc, Mutex, atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering}}, time::{Duration, Instant}};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, webview::{DownloadEvent, WebviewWindowBuilder}};
use tauri_plugin_dialog::DialogExt;
use std::hash::{Hash, Hasher};

const RECORD: &str = "curseforge-pendientes.json";
static RECORD_LOCK: Mutex<()> = Mutex::new(());
static ATTEMPT: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Queue {
    id: String,
    name: String,
    files: Vec<Manual>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Notice {
    id: String,
    file_id: u64,
    stage: &'static str,
    message: String,
}

fn notice(app: &AppHandle, id: &str, file: &Manual, stage: &'static str, message: String) {
    let _ = app.emit_to("main", "curseforge-manual", Notice { id: id.into(), file_id: file.file_id, stage, message });
}

fn record(instance: &Instance) -> PathBuf { instance.state_dir().join(RECORD) }

pub fn pending_count(instance: &Instance) -> Result<usize, String> { load(instance).map(|files| files.len()) }

fn load(instance: &Instance) -> Result<Vec<Manual>, String> {
    match std::fs::read(record(instance)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| t!("No se pudo leer la instalación pendiente: {e}", "Could not read the pending installation: {e}")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.to_string()),
    }
}

fn save(instance: &Instance, files: &[Manual]) -> Result<(), String> {
    std::fs::create_dir_all(instance.state_dir()).map_err(|e| e.to_string())?;
    let tmp = instance.state_dir().join("curseforge-pendientes.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(files).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::rename(tmp, record(instance)).map_err(|e| e.to_string())
}

fn valid(file: &Manual) -> bool {
    let extension = if file.folder == "mods" { ".jar" } else { ".zip" };
    file.project_id > 0 && file.file_id > 0 && ["mods", "resourcepacks", "shaderpacks"].contains(&file.folder.as_str())
        && !file.file_name.is_empty() && !file.file_name.starts_with('.') && !file.file_name.contains(['/', '\\', ':'])
        && file.file_name.to_lowercase().ends_with(extension)
        && file.sha1.len() == 40 && file.sha1.bytes().all(|c| c.is_ascii_hexdigit()) && file.size > 0
        && file.replaces.as_deref().is_none_or(|name| !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\', ':']))
        && reqwest::Url::parse(&file.url).is_ok_and(|url| official_page(&url) && url.path().ends_with(&format!("/files/{}", file.file_id)))
}

fn official_page(url: &reqwest::Url) -> bool {
    url.scheme() == "https" && url.port_or_known_default() == Some(443) && url.username().is_empty() && url.password().is_none()
        && matches!(url.host_str(), Some("www.curseforge.com" | "curseforge.com"))
}

fn download_origin(url: &reqwest::Url) -> bool {
    official_page(url) || (url.scheme() == "https" && matches!(url.host_str(), Some("edge.forgecdn.net" | "media.forgecdn.net" | "mediafilez.forgecdn.net")))
}

/// Se guarda solo después de instalar la parte automática; ningún enlace CDN se inventa.
pub fn register(instance: &Instance, files: &[Manual]) -> Result<(), String> {
    if files.is_empty() { return Ok(()); }
    validate(files)?;
    let _guard = RECORD_LOCK.lock().unwrap();
    let mut all = load(instance)?;
    for file in files {
        // Una elección más nueva del mismo proyecto sustituye el pendiente anterior.
        all.retain(|old| old.project_id != file.project_id);
        all.push(file.clone());
    }
    save(instance, &all)
}

pub fn validate(files: &[Manual]) -> Result<(), String> {
    if files.iter().any(|f| !valid(f)) {
        return Err(t!("CurseForge no dio los datos necesarios para comprobar uno de los archivos. No se instalará sin verificarlo.", "CurseForge did not provide the data needed to verify one of the files. It will not be installed without verification."));
    }
    Ok(())
}

fn matches_file(source: &Path, expected: &Manual) -> bool {
    source.metadata().is_ok_and(|m| m.is_file() && m.len() == expected.size)
        && crate::integridad::sha1_file(source).is_some_and(|hash| hash.eq_ignore_ascii_case(&expected.sha1))
}

/// No confía en el nombre que le haya puesto el navegador. Repetir la recepción es inocuo.
fn receive(instance: &Instance, file_id: u64, source: &Path) -> Result<(), String> {
    let _guard = RECORD_LOCK.lock().unwrap();
    let mut pending = load(instance)?;
    let Some(file) = pending.iter().find(|f| f.file_id == file_id).cloned() else { return Ok(()); };
    if !valid(&file) || !matches_file(source, &file) {
        return Err(t!("El archivo descargado no coincide con la versión esperada. Vuelve a descargarlo desde su página.", "The downloaded file does not match the expected version. Download it again from its page."));
    }
    let folder = instance.game_dir.join(&file.folder);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let disabled = file.replaces.as_deref().is_some_and(|name| name.ends_with(".disabled"))
        || source == folder.join(format!("{}.disabled", file.file_name));
    let name = if disabled { format!("{}.disabled", file.file_name) } else { file.file_name.clone() };
    let target = folder.join(name);
    if !matches_file(&target, &file) {
        // Primero queda una copia verificada; si había otro archivo, se conserva en la papelera.
        let staged = instance.state_dir().join(format!("cf-{}.verificado", file.file_id));
        std::fs::copy(source, &staged).map_err(|e| e.to_string())?;
        if !matches_file(&staged, &file) { return Err(t!("El archivo cambió mientras se copiaba. Reintenta.", "The file changed while being copied. Try again.")); }
        if target.exists() { trash::delete(&target).map_err(|e| e.to_string())?; }
        std::fs::rename(&staged, &target).map_err(|e| e.to_string())?;
    }
    if let Some(old) = &file.replaces {
        let old = folder.join(old);
        if old != target && old.is_file() { trash::delete(old).map_err(|e| e.to_string())?; }
    }
    curseforge::remember_manual(instance, &file);
    pending.retain(|f| f.file_id != file_id);
    save(instance, &pending)
}

fn picked(instance: &Instance, file_id: u64) -> Result<Manual, String> {
    load(instance)?.into_iter().find(|f| f.file_id == file_id && valid(f)).ok_or(t!("Ese archivo ya no está pendiente.", "That file is no longer pending."))
}

fn ensure_closed(state: &AppState, id: &str) -> Result<(), String> {
    if state.games.lock().unwrap().contains_key(id) {
        Err(t!("Cierra el juego antes de completar la instalación.", "Close the game before completing the installation."))
    } else { Ok(()) }
}

/// WebView2 puede guardar todos los bytes sin entregar Finished. Solo se acepta la descarga que el
/// usuario inició y cuya huella completa coincide; un archivo parcial nunca satisface esta prueba.
async fn receive_download(app: AppHandle, instance: Instance, file: Manual, label: String, temp: PathBuf,
    accepted: Arc<AtomicBool>, completed: Arc<AtomicU8>) {
    let started = Instant::now();
    let mut checked = None;
    let result = loop {
        if completed.load(Ordering::SeqCst) == 2 || started.elapsed() > Duration::from_secs(600) {
            break Err(t!("La descarga no terminó. Puedes reintentar desde la página.", "The download did not finish. You can retry from the page."));
        }
        let stamp = temp.metadata().ok().filter(|m| m.len() == file.size)
            .map(|m| m.modified().unwrap_or(std::time::UNIX_EPOCH));
        let finished = completed.load(Ordering::SeqCst) == 1;
        if stamp.is_some() && (stamp != checked || finished) {
            checked = stamp;
            let (source, expected) = (temp.clone(), file.clone());
            let exact = tauri::async_runtime::spawn_blocking(move || matches_file(&source, &expected)).await.unwrap_or(false);
            if exact {
                notice(&app, &instance.id, &file, "verifying", t!("Comprobando el archivo…", "Verifying the file…"));
                if let Err(e) = ensure_closed(&app.state::<AppState>(), &instance.id) { break Err(e); }
                let (target, source, file_id) = (instance.clone(), temp.clone(), file.file_id);
                break tauri::async_runtime::spawn_blocking(move || receive(&target, file_id, &source)).await
                    .map_err(|e| e.to_string()).and_then(|r| r);
            }
            if finished {
                break Err(t!("El archivo descargado no coincide con la versión esperada. Vuelve a descargarlo desde su página.", "The downloaded file does not match the expected version. Download it again from its page."));
            }
        } else if finished && stamp.is_none() {
            break Err(t!("La descarga no terminó. Puedes reintentar desde la página.", "The download did not finish. You can retry from the page."));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    };
    // La página se cierra después de copiar el archivo: el navegador puede tener abierto el temporal.
    match result {
        Ok(()) => {
            notice(&app, &instance.id, &file, "installed", t!("{} verificado e instalado.", "{} verified and installed.", file.name));
            if let Some(window) = app.get_webview_window(&label) { let _ = window.close(); }
        }
        Err(error) => notice(&app, &instance.id, &file, "error", error),
    }
    let _ = std::fs::remove_file(&temp);
    accepted.store(false, Ordering::SeqCst);
    if let Some(main) = app.get_webview_window("main") { let _ = main.set_focus(); }
}

/// Reconoce también el archivo exacto que ya estuviera en la instancia o en Descargas, sin mover
/// archivos personales. La selección de archivo cubre las carpetas de descarga personalizadas.
fn receive_existing(instance: &Instance, pending: &[Manual]) -> Result<(), String> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from);
    let downloaded: Vec<PathBuf> = home.and_then(|h| std::fs::read_dir(h.join("Downloads")).ok())
        .into_iter().flatten().flatten().take(2000).map(|e| e.path()).collect();
    let incoming: Vec<PathBuf> = std::fs::read_dir(instance.state_dir().join("curseforge-entrada")).ok()
        .into_iter().flatten().flatten().take(256).map(|e| e.path()).collect();
    for file in pending {
        let target = instance.game_dir.join(&file.folder).join(&file.file_name);
        let disabled = target.with_file_name(format!("{}.disabled", file.file_name));
        let existing = [target, disabled].into_iter().find(|p| matches_file(p, file)).or_else(|| {
            downloaded.iter().find(|p| related_name(p, &file.file_name) && matches_file(p, file)).cloned()
        }).or_else(|| incoming.iter().find(|p| {
            p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&format!("{}-", file.file_id))
                && !n.starts_with(&format!("{}-{}-", file.file_id, std::process::id())) && n.ends_with(".part"))
                && matches_file(p, file)
        }).cloned());
        if let Some(path) = existing { receive(instance, file.file_id, &path)?; }
    }
    Ok(())
}

fn related_name(path: &Path, expected: &str) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if name.eq_ignore_ascii_case(expected) { return true; }
    let expected = Path::new(expected);
    let stem = expected.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let extension = expected.extension().and_then(|s| s.to_str());
    path.extension().and_then(|s| s.to_str()) == extension && path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| {
        s.strip_prefix(stem).is_some_and(|suffix| suffix.strip_prefix(" (").and_then(|s| s.strip_suffix(')')).is_some_and(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit())))
    })
}

#[tauri::command]
pub async fn curseforge_pendientes(state: State<'_, AppState>) -> Result<Vec<Queue>, String> {
    let instances = instances::list(&state.paths);
    let running: Vec<String> = state.games.lock().unwrap().keys().cloned().collect();
    tauri::async_runtime::spawn_blocking(move || {
        let mut queues = Vec::new();
        for instance in instances {
            let files = load(&instance)?;
            if files.is_empty() { continue; }
            if !running.contains(&instance.id) { receive_existing(&instance, &files)?; }
            let files = load(&instance)?;
            if !files.is_empty() { queues.push(Queue { id: instance.id, name: instance.name, files }); }
        }
        Ok(queues)
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn curseforge_preparar(state: State<'_, AppState>, id: String, proyecto: String, version: Option<String>) -> Result<(), String> {
    ensure_closed(&state, &id)?;
    let instance = super::instance_or_error(&state, &id)?;
    let plan = curseforge::plan(&instance, &proyecto, version.as_deref()).await?;
    if let Some(problem) = plan.problem { return Err(problem); }
    if plan.installed { return Ok(()); }
    validate(&plan.manual)?;
    let job = crate::descargas::start(None, "contenido", plan.title);
    job.instance(Some(id));
    let progress = job.progress("instalar");
    job.run(curseforge::install(&instance, &plan.files, move |_, _, bytes, total| {
        progress(t!("Instalando los archivos disponibles…", "Installing available files…"), bytes as f64 / total.max(1) as f64);
    })).await?;
    register(&instance, &plan.manual)
}

#[tauri::command]
pub async fn curseforge_elegir_archivo(app: AppHandle, state: State<'_, AppState>, id: String, archivo: u64) -> Result<(), String> {
    ensure_closed(&state, &id)?;
    let instance = super::instance_or_error(&state, &id)?;
    let expected = picked(&instance, archivo)?;
    let dialog = app.dialog().file().set_title(t!("Elegir el archivo descargado", "Choose the downloaded file")).add_filter("Minecraft", &["jar", "zip"]);
    let chosen = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await.map_err(|e| e.to_string())?;
    let Some(chosen) = chosen else { return Ok(()); };
    let path = chosen.into_path().map_err(|e| e.to_string())?;
    ensure_closed(&state, &id)?;
    tauri::async_runtime::spawn_blocking(move || receive(&instance, archivo, &path)).await.map_err(|e| e.to_string())??;
    notice(&app, &id, &expected, "installed", t!("Archivo verificado e instalado.", "File verified and installed."));
    Ok(())
}

#[tauri::command]
pub async fn curseforge_abrir_portal(app: AppHandle, state: State<'_, AppState>, id: String, archivo: u64) -> Result<(), String> {
    ensure_closed(&state, &id)?;
    let instance = super::instance_or_error(&state, &id)?;
    let file = picked(&instance, archivo)?;
    let mut identity = std::collections::hash_map::DefaultHasher::new();
    instance.id.hash(&mut identity);
    let label = format!("curseforge-portal-{archivo}-{}", identity.finish());
    if let Some(window) = app.get_webview_window(&label) {
        window.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }
    let nonce = ATTEMPT.fetch_add(1, Ordering::Relaxed);
    let temporary = instance.state_dir().join("curseforge-entrada");
    std::fs::create_dir_all(&temporary).map_err(|e| e.to_string())?;
    let destination = temporary.join(format!("{archivo}-{}-{nonce}.part", std::process::id()));
    let accepted = Arc::new(AtomicBool::new(false));
    let completed = Arc::new(AtomicU8::new(0));
    let (handle, instance_id, expected, close_label) = (app.clone(), id.clone(), file.clone(), label.clone());
    let (closed_handle, closed_id, closed_file) = (app.clone(), id.clone(), file.clone());
    let window = WebviewWindowBuilder::new(&app, &label, WebviewUrl::External(file.url.parse().map_err(|_| "URL inválida")?))
        .title(t!("{} · CurseForge · pulsa Descargar para instalar", "{} · CurseForge · click Download to install", file.name))
        .inner_size(1100.0, 760.0).min_inner_size(800.0, 600.0)
        // Sin scripts inyectados ni permisos IPC: es el sitio oficial, con sus controles visibles.
        .on_navigation(download_origin)
        .on_download(move |_, event| {
            match event {
                DownloadEvent::Requested { url, destination: target } => {
                    if !download_origin(&url) || accepted.swap(true, Ordering::SeqCst) { return false; }
                    *target = destination.clone();
                    completed.store(0, Ordering::SeqCst);
                    notice(&handle, &instance_id, &expected, "downloading", t!("Descargando desde CurseForge…", "Downloading from CurseForge…"));
                    tauri::async_runtime::spawn(receive_download(handle.clone(), instance.clone(), expected.clone(), close_label.clone(), destination.clone(), accepted.clone(), completed.clone()));
                }
                DownloadEvent::Finished { success, .. } => {
                    if !accepted.load(Ordering::SeqCst) { return false; }
                    completed.store(if success { 1 } else { 2 }, Ordering::SeqCst);
                }
                _ => (),
            }
            true
        }).build().map_err(|e| e.to_string())?;
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::Destroyed) {
            notice(&closed_handle, &closed_id, &closed_file, "waiting", t!("Puedes volver a abrir la página para continuar.", "You can reopen the page to continue."));
        }
    });
    notice(&app, &id, &file, "waiting", t!("Pulsa Descargar en la página de CurseForge. Parabellum lo colocará en su instancia.", "Click Download on the CurseForge page. Parabellum will place it in its instance."));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file() -> Manual { Manual { name: "Prueba".into(), file_name: "mod.jar".into(), url: "https://www.curseforge.com/minecraft/mc-mods/prueba/files/2".into(), folder: "mods".into(), project_id: 1, file_id: 2, sha1: "a9993e364706816aba3e25717850c26c9cd0d89d".into(), size: 3, replaces: None } }
    #[test]
    fn valida_origen_ruta_y_huella() {
        let mut f = file(); assert!(valid(&f));
        f.file_name = "../mod.jar".into(); assert!(!valid(&f));
        f = file(); f.folder = "../mods".into(); assert!(!valid(&f));
        f = file(); f.sha1.clear(); assert!(!valid(&f));
        f = file(); f.url = "https://www.curseforge.com.ejemplo.com/minecraft/mc-mods/prueba/files/2".into(); assert!(!valid(&f));
    }
    #[test]
    fn solo_reconoce_archivos_terminados_incluso_con_numero_del_navegador() {
        assert!(related_name(Path::new("mod.jar"), "mod.jar"));
        assert!(related_name(Path::new("mod (12).jar"), "mod.jar"));
        assert!(!related_name(Path::new("mod.jar.crdownload"), "mod.jar"));
        assert!(!related_name(Path::new("mod-extra.jar"), "mod.jar"));
    }
    #[test]
    fn recibe_solo_version_exacta_y_conserva_pendientes_al_reabrir() {
        let root = std::env::temp_dir().join(format!("parabellum-manual-{}-{}", std::process::id(), ATTEMPT.fetch_add(1, Ordering::Relaxed)));
        let instance = Instance { id: "test".into(), name: "Prueba".into(), game_dir: root.join("juego"), folder: root.clone(), ..Default::default() };
        std::fs::create_dir_all(&root).unwrap();
        register(&instance, &[file()]).unwrap();
        let source = root.join("mod (1).jar");
        std::fs::write(&source, b"xyz").unwrap();
        assert!(receive(&instance, 2, &source).is_err());
        assert_eq!(load(&instance).unwrap().len(), 1);
        assert!(!instance.game_dir.join("mods/mod.jar").exists());
        std::fs::write(&source, b"abc").unwrap();
        receive(&instance, 2, &source).unwrap();
        assert!(load(&instance).unwrap().is_empty());
        assert_eq!(std::fs::read(instance.game_dir.join("mods/mod.jar")).unwrap(), b"abc");
        assert!(source.exists());
        let (projects, _) = curseforge::installed(&instance); assert_eq!(projects, vec!["cf:1"]);
        // Si se cerró el launcher antes del aviso del navegador, recupera sus bytes al reabrir.
        let mut next = file(); next.file_id = 3; next.project_id = 4; next.file_name = "otro.jar".into();
        next.url = "https://www.curseforge.com/minecraft/mc-mods/prueba/files/3".into();
        register(&instance, &[next.clone()]).unwrap();
        let incoming = instance.state_dir().join("curseforge-entrada");
        std::fs::create_dir_all(&incoming).unwrap();
        std::fs::write(incoming.join("3-0-0.part"), b"ab").unwrap();
        receive_existing(&instance, &[next.clone()]).unwrap();
        assert_eq!(pending_count(&instance).unwrap(), 1);
        std::fs::write(incoming.join("3-0-0.part"), b"abc").unwrap();
        receive_existing(&instance, &[next]).unwrap();
        assert_eq!(pending_count(&instance).unwrap(), 0);
        assert_eq!(std::fs::read(instance.game_dir.join("mods/otro.jar")).unwrap(), b"abc");
        std::fs::remove_dir_all(root).unwrap();
    }
}
