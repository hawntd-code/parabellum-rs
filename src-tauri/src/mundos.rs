//! Mundos locales y en Drive. Los trabajos persistidos sobreviven al cierre del launcher.
use crate::{
    drive_mundos::{self, Client, Entry, Pending},
    mundos_archivo::{self as archive, Identity, Recipe, Revision, Source},
    paths::Paths,
    AppState,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use tauri::{AppHandle, Emitter, Manager};
/// Copias «antes de restaurar» que se conservan de cada mundo.
const PREVIOUS_KEEP: usize = 3;
/// Tope de espacio de las copias «antes de restaurar» de cada mundo (la más nueva se queda siempre).
const PREVIOUS_MAX: u64 = 4 * 1024 * 1024 * 1024;
static CLIENT: tokio::sync::Mutex<Option<Arc<Client>>> = tokio::sync::Mutex::const_new(None);
static WORK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static INDEX: Mutex<()> = Mutex::new(());
static BUSY: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
static ACTIVE: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Local {
    source: Source,
    world_id: String,
    owner: String,
    enabled: bool,
    base: Option<String>,
    fingerprint: String,
    last_at: Option<i64>,
    error: Option<String>,
}
fn index_path(paths: &Paths) -> PathBuf {
    paths.data.join("mundos/estado.json")
}
fn read_index(paths: &Paths) -> HashMap<String, Local> {
    std::fs::read(index_path(paths))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn update(paths: &Paths, key: &str, change: impl FnOnce(&mut Local)) -> Result<Local, String> {
    let _guard = INDEX.lock().unwrap();
    let mut index = read_index(paths);
    let local = index.entry(key.into()).or_default();
    change(local);
    let result = local.clone();
    archive::atomic(&index_path(paths), &index)?;
    Ok(result)
}
/// Si hay una operación de mundos en curso para esa instancia o servidor.
pub fn busy(kind: &str, id: &str) -> bool {
    BUSY.get_or_init(Default::default).lock().unwrap().contains(&format!("{kind}:{id}"))
}
pub(crate) struct Guard {
    key: String,
    active: bool,
}
impl Guard {
    fn capture(app: &AppHandle, source: &Source) -> Result<Self, String> {
        let state = app.state::<AppState>();
        let games = state.games.lock().unwrap();
        if (source.kind == "instancia" && games.contains_key(&source.id)) || (source.kind == "servidor" && crate::hosting::is_running(app, &source.id)) {
            return Err(t!(
                "Cierra el juego o el servidor antes de copiar o restaurar el mundo. ⟦accion:cerrar-juego⟧",
                "Close the game or server before copying or restoring the world. ⟦accion:cerrar-juego⟧"
            ));
        }
        let key = format!("{}:{}", source.kind, source.id);
        if !BUSY.get_or_init(Default::default).lock().unwrap().insert(key.clone()) {
            return Err(t!(
                "Esta instancia tiene una operación de mundos en curso.",
                "A world operation is in progress for this instance."
            ));
        }
        drop(games);
        Ok(Self { key, active: false })
    }
    fn active(id: &str) -> Result<Self, String> {
        if !ACTIVE.get_or_init(Default::default).lock().unwrap().insert(id.into()) {
            return Err(t!("Esta copia ya está en la cola.", "This backup is already queued."));
        }
        Ok(Self { key: id.into(), active: true })
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if self.active {
            ACTIVE.get_or_init(Default::default)
        } else {
            BUSY.get_or_init(Default::default)
        }
        .lock()
        .unwrap()
        .remove(&self.key);
    }
}
/// Reserva la instancia o servidor mientras se prepara una operación de mundos.
pub fn starting(app: &AppHandle, kind: &str, id: &str) -> Result<Guard, String> {
    Guard::capture(
        app,
        &Source {
            kind: kind.into(),
            id: id.into(),
            world: String::new(),
        },
    )
}
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let scope = crate::descargas::current();
    tauri::async_runtime::spawn_blocking(move || crate::descargas::enter(scope, work))
        .await
        .map_err(|e| e.to_string())?
}
fn changed(app: &AppHandle) {
    let _ = app.emit("mundos-cambiados", ());
}
async fn client(app: &AppHandle) -> Result<Arc<Client>, String> {
    let mut slot = CLIENT.lock().await;
    if let Some(client) = slot.as_ref() {
        return Ok(client.clone());
    }
    let state = app.state::<AppState>();
    let saved = drive_mundos::load(&state.paths).ok_or_else(|| {
        t!(
            "Conecta Drive para respaldar tus mundos. ⟦accion:conectar-mundos⟧",
            "Connect Drive to back up your worlds. ⟦accion:conectar-mundos⟧"
        )
    })?;
    let client = Client::resume(&state.config, &saved).await?;
    *slot = Some(client.clone());
    Ok(client)
}
fn location(app: &AppHandle, source: &Source, existing: bool) -> Result<(PathBuf, Recipe), String> {
    if !archive::component(&source.world) || !archive::component(&source.id) {
        return Err(t!("La ubicación del mundo no es válida.", "The world location is invalid."));
    }
    let paths = &app.state::<AppState>().paths;
    let (base, recipe) = match source.kind.as_str() {
        "instancia" => {
            let i = crate::instances::find(paths, &source.id).ok_or_else(|| t!("No se encontró la instancia.", "The instance was not found."))?;
            (i.game_dir.join("saves"), Recipe::of(&i))
        }
        "servidor" => {
            let s = crate::servidor::find(paths, &source.id).ok_or_else(|| t!("No se encontró el servidor.", "The server was not found."))?;
            (s.folder.clone(), Recipe::of(&s.as_instance()))
        }
        _ => return Err(t!("El tipo de mundo no es válido.", "The world type is invalid.")),
    };
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let base = base.canonicalize().map_err(|e| e.to_string())?;
    let folder = base.join(&source.world);
    if folder.is_symlink() || (folder.exists() && !folder.canonicalize().map_err(|e| e.to_string())?.starts_with(&base)) {
        return Err(t!("El mundo está fuera de la carpeta permitida.", "The world is outside the allowed folder."));
    }
    if existing && !folder.join("level.dat").is_file() {
        return Err(t!("No se encontró el mundo.", "The world was not found."));
    }
    Ok((folder, recipe))
}
fn device(paths: &Paths) -> String {
    let file = paths.data.join("mundos/equipo.json");
    if let Some(value) = std::fs::read(&file).ok().and_then(|b| serde_json::from_slice::<String>(&b).ok()) {
        return value;
    }
    let label = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| crate::plataforma::NAME.into());
    let value = format!("{} · {}", label.chars().take(48).collect::<String>(), &archive::id()[..8]);
    let _ = archive::atomic(&file, &value);
    value
}
fn player(app: &AppHandle) -> String {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap();
    if settings.uuid.is_empty() {
        crate::settings::offline_uuid(&settings.username)
    } else {
        settings.uuid.replace('-', "")
    }
}
fn local_for(app: &AppHandle, source: &Source, owner: &str) -> Result<Local, String> {
    let (folder, _) = location(app, source, true)?;
    let paths = app.state::<AppState>().paths.clone();
    let _guard = INDEX.lock().unwrap();
    let mut index = read_index(&paths);
    let found = archive::identity(&folder);
    let duplicate = found.as_ref().is_some_and(|identity| {
        index
            .iter()
            .any(|(key, v)| *key != source.key() && v.world_id == identity.world_id && location(app, &v.source, true).is_ok())
    });
    let identity = archive::ensure_identity(&folder, owner, duplicate)?;
    let local = index.entry(source.key()).or_default();
    if local.world_id != identity.world_id || local.owner != owner {
        *local = Local {
            source: source.clone(),
            world_id: identity.world_id,
            owner: owner.into(),
            ..Default::default()
        };
    }
    let result = local.clone();
    archive::atomic(&index_path(&paths), &index)?;
    Ok(result)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalView {
    pub source: Source,
    pub world_id: Option<String>,
    pub enabled: bool,
    pub last_at: Option<i64>,
    pub error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub connected: bool,
    pub email: Option<String>,
    pub pending: Vec<PendingView>,
    pub limit: u64,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingView {
    pub id: String,
    pub name: String,
    pub paused: bool,
    pub bytes: u64,
}
fn pending(paths: &Paths) -> Vec<Pending> {
    std::fs::read_dir(paths.data.join("mundos/pendientes"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| std::fs::read(e.path()).ok().and_then(|b| serde_json::from_slice::<Pending>(&b).ok()))
        .filter(|p| archive::valid_id(&p.revision.id))
        .collect()
}
/// Estado de Drive y de las copias pendientes de subir.
#[tauri::command]
pub fn mundos_estado(app: AppHandle) -> Status {
    let paths = &app.state::<AppState>().paths;
    let saved = drive_mundos::load(paths);
    Status {
        connected: saved.is_some(),
        email: saved.as_ref().map(|s| s.email.clone()),
        limit: archive::MAX_CLOUD,
        pending: pending(paths)
            .into_iter()
            .filter(|p| saved.as_ref().is_some_and(|s| s.owner == p.revision.owner))
            .map(|p| PendingView {
                id: p.revision.id,
                name: p.revision.name,
                paused: p.paused,
                bytes: p.revision.size,
            })
            .collect(),
    }
}
/// Conecta la cuenta de Google y retoma las subidas pendientes.
#[tauri::command]
pub async fn mundos_conectar(app: AppHandle) -> Result<String, String> {
    let state = app.state::<AppState>();
    let hint = crate::cuenta::google_email(&app);
    let (cloud, saved) = Client::connect(&state.config, hint.as_deref()).await?;
    if hint.is_some_and(|hint| !saved.email.eq_ignore_ascii_case(&hint)) {
        return Err(t!(
            "Conecta los mundos con la misma cuenta de Google de Parabellum.",
            "Connect worlds with the same Google account as Parabellum."
        ));
    }
    let email = cloud.email.clone();
    drive_mundos::save(&state.paths, &saved)?;
    *CLIENT.lock().await = Some(cloud);
    changed(&app);
    resume_startup(&app);
    Ok(email)
}
/// Desconecta Drive sin dejar subidas de mundos en curso.
#[tauri::command]
pub async fn mundos_desconectar(app: AppHandle) -> Result<(), String> {
    if !ACTIVE.get_or_init(Default::default).lock().unwrap().is_empty() {
        return Err(t!(
            "Pausa las subidas de mundos antes de desconectar Drive.",
            "Pause world uploads before disconnecting Drive."
        ));
    }
    *CLIENT.lock().await = None;
    drive_mundos::clear(&app.state::<AppState>().paths)?;
    changed(&app);
    Ok(())
}
/// Mundos locales de una instancia o servidor, con su estado en Drive.
#[tauri::command]
pub fn mundos_locales(app: AppHandle, tipo: String, id: String) -> Result<Vec<LocalView>, String> {
    let paths = &app.state::<AppState>().paths;
    let index = read_index(paths);
    let saved = drive_mundos::load(paths);
    let worlds = match tipo.as_str() {
        "instancia" => {
            let instance = crate::instances::find(paths, &id).ok_or_else(|| t!("No se encontró la instancia.", "The instance was not found."))?;
            std::fs::read_dir(instance.game_dir.join("saves"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.path().join("level.dat").is_file())
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect::<Vec<_>>()
        }
        "servidor" => {
            let server = crate::servidor::find(paths, &id).ok_or_else(|| t!("No se encontró el servidor.", "The server was not found."))?;
            let name = crate::servidor::property(&server.folder, "level-name").unwrap_or_else(|| "world".into());
            if server.folder.join(&name).join("level.dat").is_file() {
                vec![name]
            } else {
                vec![]
            }
        }
        _ => return Err(t!("Tipo de mundo no válido.", "Invalid world type.")),
    };
    Ok(worlds
        .into_iter()
        .map(|world| {
            let source = Source {
                kind: tipo.clone(),
                id: id.clone(),
                world,
            };
            let local = index.get(&source.key()).filter(|v| saved.as_ref().is_some_and(|s| s.owner == v.owner));
            LocalView {
                source,
                world_id: local.map(|v| v.world_id.clone()),
                enabled: local.is_some_and(|v| v.enabled),
                last_at: local.and_then(|v| v.last_at),
                error: local.and_then(|v| v.error.clone()),
            }
        })
        .collect())
}
/// Activa o desactiva el respaldo de un mundo.
#[tauri::command]
pub async fn mundos_configurar(app: AppHandle, origen: Source, activo: bool) -> Result<(), String> {
    let paths = app.state::<AppState>().paths.clone();
    if activo {
        let cloud = client(&app).await?;
        local_for(&app, &origen, &cloud.owner)?;
    } else {
        location(&app, &origen, true)?;
    }
    update(&paths, &origen.key(), |v| {
        v.source = origen;
        v.enabled = activo;
    })?;
    changed(&app);
    Ok(())
}
async fn transfer(app: &AppHandle, mut pending: Pending, progress: impl Fn(String, f64)) -> Result<Entry, String> {
    let _work = WORK.lock().await;
    let cloud = client(app).await?;
    let paths = app.state::<AppState>().paths.clone();
    let id = pending.revision.id.clone();
    let state_file = drive_mundos::pending_path(&paths, &id);
    let archive = state_file.with_extension("zip");
    pending.paused = false;
    archive::atomic(&state_file, &pending)?;
    if !cloud.revisions().await?.iter().any(|e| e.revision.id == id) {
        cloud.prune(pending.revision.size).await?;
    }
    let result = cloud
        .upload(&archive, &mut pending, &state_file, |f| progress(t!("Subiendo mundo…", "Uploading world…"), f))
        .await;
    match result {
        Ok(entry) => {
            update(&paths, &pending.revision.source.key(), |v| {
                if v.world_id == pending.revision.world_id && v.owner == pending.revision.owner {
                    v.base = Some(id);
                    v.fingerprint = pending.revision.fingerprint.clone();
                    v.last_at = Some(pending.revision.at);
                    v.error = None;
                }
            })?;
            let _ = std::fs::remove_file(&state_file);
            let _ = std::fs::remove_file(&archive);
            let _ = cloud.prune(0).await;
            Ok(entry)
        }
        Err(error) => {
            pending.paused = crate::descargas::is_cancelled();
            archive::atomic(&state_file, &pending)?;
            update(&paths, &pending.revision.source.key(), |v| {
                if v.world_id == pending.revision.world_id && v.owner == pending.revision.owner {
                    v.error = Some(error.clone());
                }
            })?;
            Err(error)
        }
    }
}
async fn upload_pending(app: &AppHandle, pending: Pending, task: Option<String>) -> Result<Entry, String> {
    let id = pending.revision.id.clone();
    let _active = Guard::active(&format!("source:{}", pending.revision.source.key()))?;
    let job = crate::descargas::start(
        task.or_else(|| Some(format!("m{id}"))),
        "mundos",
        t!("Respaldar {}", "Back up {}", pending.revision.name),
    )
    .queued()
    .retry("mundos_reanudar", json!({"revision":id}));
    job.instance(Some(pending.revision.source.id.clone()));
    let progress = job.progress("mundos");
    let result = job.run(transfer(app, pending, progress)).await;
    changed(app);
    result
}
/// Sube una copia nueva de un mundo, reanudando la pendiente si la hay.
#[tauri::command]
pub async fn mundos_respaldar(app: AppHandle, origen: Source, tarea: Option<String>) -> Result<Entry, String> {
    let cloud = client(&app).await?;
    let paths = app.state::<AppState>().paths.clone();
    if let Some(p) = pending(&paths)
        .into_iter()
        .find(|p| p.revision.source == origen && p.revision.owner == cloud.owner)
    {
        return upload_pending(&app, p, tarea).await;
    }
    let _active = Guard::active(&format!("source:{}", origen.key()))?;
    let id = archive::id();
    let job = crate::descargas::start(
        tarea.or_else(|| Some(format!("m{id}"))),
        "mundos",
        t!("Respaldar {}", "Back up {}", origen.world),
    )
    .queued()
    .retry("mundos_respaldar", json!({"origen":origen}));
    job.instance(Some(origen.id.clone()));
    let progress = job.progress("mundos");
    let result = job
        .run(async {
            let local = local_for(&app, &origen, &cloud.owner)?;
            let (folder, recipe) = location(&app, &origen, true)?;
            let capture = Guard::capture(&app, &origen)?;
            let revision = Revision {
                id,
                world_id: local.world_id,
                owner: cloud.owner.clone(),
                name: origen.world.clone(),
                parents: local.base.into_iter().collect(),
                at: chrono::Utc::now().timestamp_millis(),
                device: device(&paths),
                player_uuid: player(&app),
                recipe,
                source: origen.clone(),
                ..Default::default()
            };
            let state_file = drive_mundos::pending_path(&paths, &revision.id);
            std::fs::create_dir_all(state_file.parent().unwrap()).map_err(|e| e.to_string())?;
            let zip = state_file.with_extension("zip");
            let saved_zip = zip.clone();
            let copy_progress = progress.clone();
            let prepared = blocking(move || {
                archive::snapshot(&folder, &zip, revision, |f| {
                    copy_progress(t!("Preparando copia del mundo…", "Preparing world backup…"), f)
                })
            })
            .await;
            drop(capture);
            let revision = match prepared {
                Ok(r) => r,
                Err(e) => {
                    let _ = std::fs::remove_file(saved_zip);
                    return Err(e);
                }
            };
            if revision.fingerprint == local.fingerprint {
                if let Some(existing) = cloud.revisions().await?.into_iter().find(|e| Some(&e.revision.id) == revision.parents.first()) {
                    let _ = std::fs::remove_file(&saved_zip);
                    progress(
                        t!("El mundo no cambió desde la última copia.", "The world has not changed since the last backup."),
                        1.,
                    );
                    return Ok(existing);
                }
            }
            let pending = Pending {
                revision,
                file: None,
                session: None,
                paused: false,
            };
            archive::atomic(&state_file, &pending)?;
            transfer(&app, pending, progress.clone()).await
        })
        .await;
    changed(&app);
    result
}
/// Retoma una subida pendiente concreta.
#[tauri::command]
pub async fn mundos_reanudar(app: AppHandle, revision: String) -> Result<Entry, String> {
    if !archive::valid_id(&revision) {
        return Err(t!("Copia pendiente no válida.", "Invalid pending backup."));
    }
    let path = drive_mundos::pending_path(&app.state::<AppState>().paths, &revision);
    let pending: Pending = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if pending.revision.id != revision {
        return Err(t!(
            "La identidad de la copia pendiente no coincide.",
            "The pending backup identity does not match."
        ));
    }
    upload_pending(&app, pending, None).await
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudWorld {
    pub world_id: String,
    pub name: String,
    pub conflict: bool,
    pub heads: Vec<String>,
    pub revisions: Vec<Entry>,
    pub used: u64,
}
/// Mundos guardados en Drive, agrupados por identidad y con su historial.
#[tauri::command]
pub async fn mundos_nube(app: AppHandle) -> Result<Vec<CloudWorld>, String> {
    let cloud = client(&app).await?;
    let mut groups: HashMap<String, Vec<Entry>> = HashMap::new();
    for entry in cloud.revisions().await? {
        groups.entry(entry.revision.world_id.clone()).or_default().push(entry);
    }
    let mut worlds: Vec<_> = groups
        .into_iter()
        .map(|(world_id, mut entries)| {
            entries.sort_by_key(|e| std::cmp::Reverse(e.revision.at));
            let revisions = entries.iter().map(|e| e.revision.clone()).collect::<Vec<_>>();
            let heads = archive::heads(&revisions);
            CloudWorld {
                world_id,
                name: entries[0].revision.name.clone(),
                conflict: heads.len() > 1,
                heads,
                used: entries.iter().map(|e| e.bytes).sum(),
                revisions: entries,
            }
        })
        .collect();
    worlds.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(worlds)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Destination {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub world: Option<String>,
}
/// Instancias y servidores compatibles donde se puede restaurar una revisión.
#[tauri::command]
pub async fn mundos_destinos(app: AppHandle, mundo: String, revision: String) -> Result<Vec<Destination>, String> {
    let cloud = client(&app).await?;
    let entry = selected(&cloud, &mundo, &revision).await?;
    let paths = app.state::<AppState>().paths.clone();
    blocking(move || {
        let mut list = crate::instances::list(&paths)
            .into_iter()
            .filter(|i| entry.revision.source.kind == "instancia" && entry.revision.recipe.compatible(i))
            .map(|i| Destination {
                kind: "instancia".into(),
                id: i.id,
                name: i.name,
                world: None,
            })
            .collect::<Vec<_>>();
        list.extend(
            crate::servidor::list(&paths)
                .into_iter()
                .filter(|s| entry.revision.source.kind == "servidor" && entry.revision.recipe.compatible(&s.as_instance()))
                .map(|s| Destination {
                    kind: "servidor".into(),
                    id: s.id,
                    name: s.name,
                    world: Some(crate::servidor::property(&s.folder, "level-name").unwrap_or_else(|| "world".into())),
                }),
        );
        Ok(list)
    })
    .await
}
/// Guarda una revisión de Drive como ZIP en la carpeta que elija el jugador.
#[tauri::command]
pub async fn mundos_exportar(app: AppHandle, mundo: String, revision: String) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let cloud = client(&app).await?;
    let entry = selected(&cloud, &mundo, &revision).await?;
    let dialog =
        app.dialog()
            .file()
            .add_filter("ZIP", &["zip"])
            .set_file_name(format!("{}-{}.zip", crate::packs::slug(&entry.revision.name), &entry.revision.id[..8]));
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file())
        .await
        .map_err(|e| e.to_string())?;
    let Some(picked) = picked else {
        return Ok(None);
    };
    let destination = picked.into_path().map_err(|e| e.to_string())?;
    let paths = app.state::<AppState>().paths.clone();
    let temp = paths.data.join("mundos").join(format!("exportar-{}.zip", archive::id()));
    std::fs::create_dir_all(temp.parent().unwrap()).map_err(|e| e.to_string())?;
    let job = crate::descargas::start(None, "mundos", t!("Guardar copia como ZIP", "Save backup as ZIP")).queued();
    let progress = job.progress("mundos");
    let result = job
        .run(async {
            cloud
                .download(&entry, &temp, |f| progress(t!("Bajando copia…", "Downloading backup…"), f))
                .await?;
            let source = temp.clone();
            let expected = entry.revision.sha256.clone();
            let output = destination.clone();
            blocking(move || {
                if archive::hashes(&source)?.0 != expected {
                    return Err(t!("La huella del ZIP no coincide.", "The ZIP checksum does not match."));
                }
                let staged = output.with_extension(format!("{}.tmp", archive::id()));
                std::fs::copy(source, &staged).map_err(|e| e.to_string())?;
                std::fs::rename(staged, output).map_err(|e| e.to_string())?;
                Ok(())
            })
            .await?;
            Ok(Some(destination.display().to_string()))
        })
        .await;
    let _ = std::fs::remove_file(temp);
    result
}
async fn selected(cloud: &Client, world: &str, revision: &str) -> Result<Entry, String> {
    if !archive::valid_id(world) || !archive::valid_id(revision) {
        return Err(t!("Revisión no válida.", "Invalid revision."));
    }
    cloud
        .revisions()
        .await?
        .into_iter()
        .find(|e| e.revision.world_id == world && e.revision.id == revision)
        .ok_or_else(|| t!("La revisión ya no está disponible.", "The revision is no longer available."))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Restored {
    pub backup: Option<String>,
    pub source: Source,
    pub revision: String,
    pub warning: Option<String>,
}
struct RestoreCleanup {
    download: PathBuf,
    staged: PathBuf,
}
impl Drop for RestoreCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.download);
        if self.staged.is_dir() && !self.staged.is_symlink() {
            let _ = std::fs::remove_dir_all(&self.staged);
        }
    }
}
fn backup_root(app: &AppHandle, source: &Source) -> Result<PathBuf, String> {
    let paths = &app.state::<AppState>().paths;
    let root = if source.kind == "instancia" {
        crate::instances::find(paths, &source.id).map(|i| i.state_dir())
    } else {
        crate::servidor::find(paths, &source.id).map(|s| s.folder.join(".lanzador"))
    }
    .ok_or_else(|| t!("No se encontró la instalación.", "The installation was not found."))?;
    Ok(root.join("respaldos-mundos"))
}
#[derive(Serialize, Deserialize)]
struct Previous {
    source: Source,
    at: i64,
    local: Option<Local>,
}
/// Una copia «antes de restaurar» con lo que ocupa, para decidir la retención sin mirar el disco.
struct PreviousCopy {
    source: Source,
    id: String,
    at: i64,
    bytes: u64,
}
/// Por cada mundo se quedan las 3 copias más nuevas y, salvo la más nueva, no se pasa de 4 GB.
fn expired(copies: Vec<PreviousCopy>) -> Vec<PreviousCopy> {
    let mut groups: HashMap<String, Vec<PreviousCopy>> = HashMap::new();
    for copy in copies {
        groups.entry(copy.source.key()).or_default().push(copy);
    }
    let mut out = Vec::new();
    for mut group in groups.into_values() {
        group.sort_by_key(|c| std::cmp::Reverse(c.at));
        out.extend(group.split_off(PREVIOUS_KEEP.min(group.len())));
        let mut total: u64 = group.iter().map(|c| c.bytes).sum();
        while total > PREVIOUS_MAX && group.len() > 1 {
            let copy = group.pop().unwrap();
            total = total.saturating_sub(copy.bytes);
            out.push(copy);
        }
    }
    out
}
/// Las copias «antes de restaurar» de un mundo que siguen completas (con su `mundo/level.dat`, como
/// las cuenta `mundos_anteriores`): la carpeta que deja «volver a esta copia» no ocupa lugar.
fn previous_copies(root: &Path, source: &Source) -> Vec<PreviousCopy> {
    std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("mundo/level.dat").is_file())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            if !archive::valid_id(&id) {
                return None;
            }
            let previous: Previous = serde_json::from_slice(&std::fs::read(e.path().join("info.json")).ok()?).ok()?;
            if previous.source != *source {
                return None;
            }
            let bytes = crate::packs::walk(&e.path().join("mundo"))
                .iter()
                .filter_map(|f| f.metadata().ok())
                .map(|m| m.len())
                .sum();
            Some(PreviousCopy {
                source: previous.source,
                id,
                at: previous.at,
                bytes,
            })
        })
        .collect()
}
/// Manda a la papelera las copias que sobran de un mundo; si falla, se quedan como están.
fn prune_previous(root: &Path, source: &Source) {
    for copy in expired(previous_copies(root, source)) {
        // Es mantenimiento: si la papelera falla, la restauración ya terminó bien.
        let _ = trash::delete(root.join(&copy.id));
    }
}
#[derive(Serialize)]
pub struct PreviousView {
    id: String,
    at: i64,
}
/// Copias «antes de restaurar» disponibles de un mundo, de la más nueva a la más vieja.
#[tauri::command]
pub fn mundos_anteriores(app: AppHandle, origen: Source) -> Result<Vec<PreviousView>, String> {
    location(&app, &origen, false)?;
    let root = backup_root(&app, &origen)?;
    let mut result = std::fs::read_dir(root)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("mundo/level.dat").is_file())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            if !archive::valid_id(&id) {
                return None;
            }
            let previous: Previous = serde_json::from_slice(&std::fs::read(e.path().join("info.json")).ok()?).ok()?;
            (previous.source == origen).then_some(PreviousView { id, at: previous.at })
        })
        .collect::<Vec<_>>();
    result.sort_by_key(|r| std::cmp::Reverse(r.at));
    Ok(result)
}
/// Devuelve el mundo a una copia «antes de restaurar», guardando la actual.
#[tauri::command]
pub async fn mundos_volver_anterior(app: AppHandle, origen: Source, respaldo: String) -> Result<(), String> {
    if !archive::valid_id(&respaldo) {
        return Err(t!("Respaldo anterior no válido.", "Invalid previous backup."));
    }
    let _work = WORK.lock().await;
    let _capture = Guard::capture(&app, &origen)?;
    let (target, _) = location(&app, &origen, false)?;
    let root = backup_root(&app, &origen)?;
    let previous_dir = root.join(respaldo);
    let previous: Previous = serde_json::from_slice(&std::fs::read(previous_dir.join("info.json")).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if previous.source != origen || previous_dir.is_symlink() {
        return Err(t!("El respaldo corresponde a otro mundo.", "The backup belongs to another world."));
    }
    let paths = app.state::<AppState>().paths.clone();
    let current = read_index(&paths).get(&origen.key()).cloned();
    let record = Previous {
        source: origen.clone(),
        at: chrono::Utc::now().timestamp_millis(),
        local: current,
    };
    let stage = target.parent().unwrap().parent().unwrap().join(format!(".parabellum-recuperar-{}", archive::id()));
    let source = previous_dir.join("mundo");
    let server = origen.kind == "servidor";
    blocking(move || {
        std::fs::rename(&source, &stage).map_err(|e| e.to_string())?;
        match archive::install(&stage, &target, &root, server) {
            Ok(backup) => {
                // Su mundo ya volvió a su lugar: a la carpeta solo le queda info.json.
                let _ = trash::delete(&previous_dir);
                if let Some(backup) = backup {
                    archive::atomic(&backup.parent().unwrap().join("info.json"), &record)?;
                    prune_previous(&root, &record.source);
                }
                Ok(())
            }
            Err(e) => {
                if stage.exists() && !source.exists() {
                    let _ = std::fs::rename(stage, source);
                }
                Err(e)
            }
        }
    })
    .await?;
    update(&paths, &origen.key(), |local| {
        if let Some(previous) = previous.local {
            *local = previous
        } else {
            local.base = None;
            local.fingerprint.clear();
            local.enabled = false;
        }
    })?;
    changed(&app);
    Ok(())
}
/// Restaura una revisión de Drive y conserva el mundo actual como copia anterior.
#[tauri::command]
pub async fn mundos_restaurar(
    app: AppHandle,
    mundo: String,
    revision: String,
    destino: Source,
    nuevo: bool,
    confirmar_jugador: bool,
    resolver: bool,
) -> Result<Restored, String> {
    let _work = WORK.lock().await;
    let cloud = client(&app).await?;
    let entry = selected(&cloud, &mundo, &revision).await?;
    let (target, recipe) = location(&app, &destino, false)?;
    if destino.kind != entry.revision.source.kind
        || recipe.minecraft != entry.revision.recipe.minecraft
        || recipe.loader != entry.revision.recipe.loader
        || recipe.loader_version != entry.revision.recipe.loader_version
        || recipe.content_hash != entry.revision.recipe.content_hash
    {
        return Err(t!(
            "Prepara una instancia con el mismo Minecraft, loader y mods de la copia antes de restaurar. ⟦accion:biblioteca⟧",
            "Prepare an instance with the same Minecraft, loader and mods as the backup before restoring. ⟦accion:biblioteca⟧"
        ));
    }
    if nuevo && (resolver || destino.kind == "servidor") {
        return Err(t!(
            "En servidores se restaura el mundo configurado. Elige restaurar en el original.",
            "Servers restore the configured world. Choose restore in place."
        ));
    }
    if destino.kind == "servidor" {
        let s = crate::servidor::find(&app.state::<AppState>().paths, &destino.id)
            .ok_or_else(|| t!("No se encontró el servidor.", "The server was not found."))?;
        let configured = crate::servidor::property(&s.folder, "level-name").unwrap_or_else(|| "world".into());
        if destino.world != configured {
            return Err(t!("Elige el mundo configurado del servidor.", "Choose the server's configured world."));
        }
    }
    if nuevo && target.exists() {
        return Err(t!(
            "El nombre del mundo ya está ocupado. Elige otro.",
            "The world name is already taken. Choose another."
        ));
    }
    if !confirmar_jugador && entry.revision.player_uuid != player(&app) {
        return Err(t!(
            "Esta copia pertenece a otro UUID de jugador. Confirma el aviso antes de restaurar: su inventario podría no ser el tuyo.",
            "This backup belongs to another player UUID. Confirm the warning before restoring: its inventory may not be yours."
        ));
    }
    let capture = Guard::capture(&app, &destino)?;
    let paths = app.state::<AppState>().paths.clone();
    let task = crate::descargas::start(None, "mundos", t!("Restaurar {}", "Restore {}", entry.revision.name)).queued();
    let progress = task.progress("mundos");
    let result = task
        .run(async {
            let job_id = archive::id();
            let download = paths.data.join("mundos").join(format!("descarga-{job_id}.zip"));
            std::fs::create_dir_all(download.parent().unwrap()).map_err(|e| e.to_string())?;
            // Fuera de saves: un cierre inesperado no presenta una copia incompleta como jugable.
            let staged = target.parent().unwrap().parent().unwrap().join(format!(".parabellum-restaurar-{job_id}"));
            let _cleanup = RestoreCleanup {
                download: download.clone(),
                staged: staged.clone(),
            };
            cloud
                .download(&entry, &download, |f| progress(t!("Bajando mundo…", "Downloading world…"), f))
                .await?;
            progress(t!("Verificando y preparando la restauración…", "Verifying and preparing restoration…"), 0.);
            let world_id = if nuevo { archive::id() } else { mundo.clone() };
            let identity = Identity {
                world_id: world_id.clone(),
                owner: cloud.owner.clone(),
            };
            let backups = backup_root(&app, &destino)?;
            let previous = Previous {
                source: destino.clone(),
                at: chrono::Utc::now().timestamp_millis(),
                local: read_index(&paths).get(&destino.key()).cloned(),
            };
            let (archive, target_copy, staged_copy, rev) = (download.clone(), target.clone(), staged.clone(), entry.revision.clone());
            let server = destino.kind == "servidor";
            let (backup, mut warning) = blocking(move || {
                archive::unpack(&archive, &staged_copy, &rev)?;
                archive::atomic(&staged_copy.join(archive::IDENTITY), &identity)?;
                crate::descargas::check()?;
                let backup = archive::install(&staged_copy, &target_copy, &backups, server)?;
                let warning = backup.as_ref().and_then(|backup| {
                    archive::atomic(&backup.parent().unwrap().join("info.json"), &previous).err().map(|e| {
                        t!(
                            "El mundo ya se restauró, pero no se pudo registrar la copia anterior. Sigue conservada en {}: {}",
                            "The world was restored, but its previous backup could not be registered. It is still kept at {}: {}",
                            backup.display(),
                            e
                        )
                    })
                });
                if backup.is_some() && warning.is_none() {
                    prune_previous(&backups, &previous.source);
                }
                Ok((backup, warning))
            })
            .await?;
            let mut base = entry.revision.id.clone();
            // Registra primero lo que ya se instaló: una caída de red no revierte la restauración local.
            update(&paths, &destino.key(), |v| {
                let enabled = v.enabled && !nuevo;
                *v = Local {
                    source: destino.clone(),
                    world_id: world_id.clone(),
                    owner: cloud.owner.clone(),
                    base: (!nuevo).then_some(base.clone()),
                    fingerprint: if nuevo { String::new() } else { entry.revision.fingerprint.clone() },
                    last_at: Some(entry.revision.at),
                    enabled,
                    error: None,
                };
            })?;
            if resolver {
                let resolution = async {
                    let entries = cloud
                        .revisions()
                        .await?
                        .into_iter()
                        .filter(|e| e.revision.world_id == mundo)
                        .collect::<Vec<_>>();
                    let heads = archive::heads(&entries.iter().map(|e| e.revision.clone()).collect::<Vec<_>>());
                    if !heads.contains(&entry.revision.id) {
                        return Err(t!(
                            "El conflicto cambió mientras restaurabas. Revisa el historial actualizado.",
                            "The conflict changed while restoring. Check the updated history."
                        ));
                    }
                    Ok::<_, String>(cloud.resolve(&entry, heads, device(&paths)).await?.revision.id)
                }
                .await;
                match resolution {
                    Ok(id) => {
                        base = id;
                        update(&paths, &destino.key(), |v| v.base = Some(base.clone()))?;
                    }
                    Err(e) => {
                        let error = t!(
                            "El mundo ya se restauró. Falta resolver el conflicto en Drive: {}",
                            "The world was restored. The Drive conflict still needs resolving: {}",
                            e
                        );
                        warning = Some(warning.map(|w| format!("{w}\n{error}")).unwrap_or(error));
                    }
                }
            }
            let _ = std::fs::remove_file(download);
            Ok(Restored {
                backup: backup.map(|p| p.display().to_string()),
                source: destino.clone(),
                revision: base,
                warning,
            })
        })
        .await;
    drop(capture);
    changed(&app);
    result
}
/// Respalda en segundo plano los mundos habilitados de una instancia o servidor.
pub fn auto_backup(app: &AppHandle, kind: &str, id: &str) {
    let app = app.clone();
    let Some(saved) = drive_mundos::load(&app.state::<AppState>().paths) else {
        return;
    };
    let sources = read_index(&app.state::<AppState>().paths)
        .into_values()
        .filter(|v| v.enabled && v.owner == saved.owner && v.source.kind == kind && v.source.id == id)
        .map(|v| v.source)
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        for source in sources {
            let _ = mundos_respaldar(app.clone(), source, None).await;
        }
        changed(&app);
    });
}
/// Retoma al arrancar las subidas de mundos que quedaron pendientes.
pub fn resume_startup(app: &AppHandle) {
    let app = app.clone();
    let paths = app.state::<AppState>().paths.clone();
    let Some(saved) = drive_mundos::load(&paths) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        for pending in pending(&paths).into_iter().filter(|p| !p.paused && p.revision.owner == saved.owner) {
            let _ = upload_pending(&app, pending, None).await;
        }
        changed(&app);
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    const GIB: u64 = 1024 * 1024 * 1024;

    fn copy(instance: &str, n: u8, at: i64, bytes: u64) -> PreviousCopy {
        PreviousCopy {
            source: Source {
                kind: "instancia".into(),
                id: instance.into(),
                world: "mundo".into(),
            },
            id: format!("{n:032x}"),
            at,
            bytes,
        }
    }

    fn ids(copies: Vec<PreviousCopy>) -> Vec<String> {
        let mut ids: Vec<String> = copies.into_iter().map(|c| c.id).collect();
        ids.sort();
        ids
    }

    #[test]
    fn se_quitan_las_mas_viejas_de_cada_mundo() {
        let mut copies = (0u8..5).map(|i| copy("uno", i, i as i64, 10)).collect::<Vec<_>>();
        copies.push(copy("otro", 9, 99, 10));
        let removed = expired(copies);
        assert!(removed.iter().all(|c| c.source.id == "uno"));
        assert_eq!(ids(removed), vec![format!("{:032x}", 0), format!("{:032x}", 1)]);
    }

    #[test]
    fn el_tope_de_tamano_quita_las_mas_viejas() {
        let copies = vec![
            copy("uno", 1, 1, 2 * GIB),
            copy("uno", 2, 2, 2 * GIB),
            copy("uno", 3, 3, 2 * GIB),
        ];
        assert_eq!(ids(expired(copies)), vec![format!("{:032x}", 1)]);
    }

    #[test]
    fn la_carpeta_que_deja_volver_a_una_copia_no_ocupa_lugar() {
        let root = std::env::temp_dir().join(format!("pbl-anteriores-{}", archive::id()));
        let source = copy("uno", 0, 0, 0).source;
        let other = copy("otro", 0, 0, 0).source;
        let write = |id: u8, source: &Source, complete: bool| {
            let folder = root.join(format!("{id:032x}"));
            std::fs::create_dir_all(folder.join("mundo")).unwrap();
            let previous = Previous { source: source.clone(), at: id as i64, local: None };
            std::fs::write(folder.join("info.json"), serde_json::to_vec(&previous).unwrap()).unwrap();
            if complete {
                std::fs::write(folder.join("mundo/level.dat"), b"nivel").unwrap();
            } else {
                // Como queda después de «volver a esta copia»: el mundo se fue, info.json se quedó.
                std::fs::remove_dir(folder.join("mundo")).unwrap();
            }
        };
        write(1, &source, true);
        write(2, &source, true);
        write(3, &source, true);
        write(4, &source, true);
        write(5, &source, false);
        write(6, &other, true);
        std::fs::create_dir_all(root.join("no-es-un-id")).unwrap();
        // Con 4 completas y una incompleta, la incompleta no ocupa lugar: solo sobra la más vieja.
        let removed = expired(previous_copies(&root, &source));
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(ids(removed), vec![format!("{:032x}", 1)]);
    }

    #[test]
    fn la_mas_nueva_se_queda_aunque_sola_pase_el_tope() {
        let copies = vec![copy("uno", 1, 1, 3 * GIB), copy("uno", 2, 2, 5 * GIB)];
        assert_eq!(ids(expired(copies)), vec![format!("{:032x}", 1)]);
    }
}
