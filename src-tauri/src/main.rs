// Sin ventana de consola en Windows.
#![windows_subsystem = "windows"]

mod catalog;
mod compat;
mod config;
mod content;
mod download;
mod externos;
mod forge;
mod instances;
mod java;
mod launch;
mod loaders;
mod modrinth;
mod mojang;
mod mrpack;
mod packs;
mod paths;
mod remote_zip;
mod settings;
mod version;

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

use instances::{GameVersion, Instance};
use loaders::Loader;
use paths::Paths;
use settings::Settings;

struct AppState {
    paths: Paths,
    settings: Mutex<Settings>,
    /// Juegos abiertos: id de la instancia -> PID.
    games: Mutex<HashMap<String, u32>>,
    /// Juegos que el jugador cerró desde el launcher (no cuentan como error).
    stopped: Mutex<HashSet<String>>,
    config: config::Config,
    /// Último catálogo leído de Drive y de la carpeta paquetes.
    catalog: Mutex<Option<Arc<catalog::Catalog>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Initial {
    settings: Settings,
    offline_uuid: String,
    instances: Vec<Instance>,
    version: &'static str,
    root: String,
    system_ram_gb: u32,
    /// Carpeta de Drive con los modpacks (si está configurada).
    drive_url: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    task: String,
    stage: String,
    fraction: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Versions {
    /// false: sin acceso a Mojang (por ejemplo, en la red de la escuela).
    mojang: bool,
    releases: Vec<mojang::Release>,
    installed: Vec<String>,
    /// Loaders ya instalados (para crear instancias sin conexión).
    loaders: Vec<loaders::Installed>,
    message: Option<String>,
}

#[tauri::command]
fn estado_inicial(state: State<AppState>) -> Initial {
    let settings = state.settings.lock().unwrap().clone();
    Initial {
        offline_uuid: settings::dashed(&settings::offline_uuid(&settings.username)),
        instances: instances::list(&state.paths),
        version: env!("CARGO_PKG_VERSION"),
        root: state.paths.root.display().to_string(),
        system_ram_gb: system_ram_gb(),
        drive_url: state.config.drive_folder_url(),
        settings,
    }
}

#[tauri::command]
fn guardar_ajustes(state: State<AppState>, mut ajustes: Settings) -> Result<Settings, String> {
    ajustes.username = ajustes.username.trim().to_string();
    if let Some(problem) = settings::validate_username(&ajustes.username) {
        return Err(problem.into());
    }
    if !ajustes.uuid.trim().is_empty() && settings::parse_uuid(&ajustes.uuid).is_none() {
        return Err("El UUID no es válido. Déjalo vacío para usar el de tu nombre.".into());
    }
    ajustes.ram_gb = ajustes.ram_gb.clamp(1, 64);
    ajustes.save(&state.paths).map_err(|e| format!("No se pudieron guardar los ajustes: {e}"))?;
    *state.settings.lock().unwrap() = ajustes.clone();
    Ok(ajustes)
}

#[tauri::command]
fn uuid_offline(nombre: String) -> String {
    settings::dashed(&settings::offline_uuid(nombre.trim()))
}

#[tauri::command]
fn listar_instancias(state: State<AppState>) -> Vec<Instance> {
    instances::list(&state.paths)
}

#[tauri::command]
async fn listar_versiones(state: State<'_, AppState>) -> Result<Versions, String> {
    let installed = mojang::installed_versions(&state.paths);
    let loaders = loaders::installed(&state.paths);
    Ok(match mojang::list().await {
        Ok(releases) => Versions { mojang: true, releases, installed, loaders, message: None },
        Err(message) => Versions { mojang: false, releases: Vec::new(), installed, loaders, message: Some(message) },
    })
}

fn parse_loader(text: &str) -> Result<Loader, String> {
    Loader::parse(text).ok_or_else(|| format!("Loader desconocido: {text}."))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoaderGames {
    online: bool,
    /// Versiones de Minecraft que soporta el loader.
    games: Vec<String>,
    message: Option<String>,
}

#[tauri::command]
async fn versiones_compatibles(loader: String) -> Result<LoaderGames, String> {
    Ok(match loaders::game_versions(parse_loader(&loader)?).await {
        Ok(games) => LoaderGames { online: true, games, message: None },
        Err(message) => LoaderGames { online: false, games: Vec::new(), message: Some(message) },
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoaderVersions {
    online: bool,
    /// De la más nueva a la más vieja; sin conexión, solo las instaladas.
    versions: Vec<loaders::LoaderVersion>,
    recommended: Option<String>,
    installed: Vec<String>,
    message: Option<String>,
}

#[tauri::command]
async fn versiones_loader(state: State<'_, AppState>, loader: String, minecraft: String) -> Result<LoaderVersions, String> {
    let loader = parse_loader(&loader)?;
    let installed: Vec<String> = loaders::installed(&state.paths)
        .into_iter()
        .filter(|i| i.loader == loader.name() && i.minecraft == minecraft)
        .map(|i| i.loader_version)
        .collect();
    Ok(match loaders::loader_versions(loader, &minecraft).await {
        Ok(versions) => LoaderVersions {
            online: true,
            recommended: loaders::recommended(&versions).map(|v| v.version.clone()),
            versions,
            installed,
            message: None,
        },
        Err(message) => LoaderVersions {
            online: false,
            recommended: installed.first().cloned(),
            versions: installed.iter().map(|v| loaders::LoaderVersion { stable: !v.contains('-'), version: v.clone() }).collect(),
            installed,
            message: Some(message),
        },
    })
}

/// Descarga (o completa) una versión vanilla. Sin conexión sirve si ya estaba descargada.
async fn install_vanilla(version: &str, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<(), String> {
    let installed = mojang::installed_versions(paths).iter().any(|v| v == version);
    match mojang::list().await {
        Ok(releases) => {
            let release = releases.into_iter().find(|r| r.id == version).ok_or(format!("Mojang no tiene la versión {version}."))?;
            // Se ejecuta siempre: si ya estaba descargada termina al momento y completa lo que faltara.
            mojang::install(&release, paths, progress).await
        }
        Err(message) if !installed => Err(message),
        Err(_) => Ok(()),
    }
}

/// Instala el loader (la versión pedida o la recomendada). Devuelve (id de la versión, versión del loader).
/// Sin conexión sirve si esa versión ya estaba instalada.
async fn install_loader(
    loader: Loader,
    minecraft: &str,
    requested: Option<&str>,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + 'static,
) -> Result<(String, String), String> {
    let version = match requested.map(str::trim).filter(|v| !v.is_empty()) {
        Some(version) => version.to_string(),
        None => match loaders::loader_versions(loader, minecraft).await {
            Ok(versions) => loaders::recommended(&versions)
                .ok_or(format!("{} todavía no está disponible para Minecraft {minecraft}.", loader.name()))?
                .version
                .clone(),
            Err(message) => loaders::installed(paths)
                .into_iter()
                .find(|i| i.loader == loader.name() && i.minecraft == minecraft)
                .map(|i| i.loader_version)
                .ok_or(message)?,
        },
    };
    match loaders::install(loader, minecraft, &version, paths, progress).await {
        Ok(id) => Ok((id, version)),
        Err(message) => loaders::installed_id(loader, minecraft, &version, paths).map(|id| (id, version)).ok_or(message),
    }
}

/// Instala vanilla y, si se pide, el loader encima (la vanilla ocupa el 90 % de la barra).
/// Devuelve (id de la versión a lanzar, versión del loader).
async fn install_game(
    version: &str,
    loader: Option<Loader>,
    loader_version: Option<&str>,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<(String, Option<String>), String> {
    let share = if loader.is_some() { 0.9 } else { 1.0 };
    let vanilla = progress.clone();
    install_vanilla(version, paths, move |stage, fraction| vanilla(stage, fraction * share)).await?;
    match loader {
        None => Ok((version.to_string(), None)),
        Some(loader) => {
            let (id, loader_version) =
                install_loader(loader, version, loader_version, paths, move |stage, fraction| progress(stage, 0.9 + fraction * 0.1)).await?;
            Ok((id, Some(loader_version)))
        }
    }
}

#[tauri::command]
async fn crear_instancia(
    app: AppHandle,
    state: State<'_, AppState>,
    nombre: String,
    version: String,
    loader: Option<String>,
    version_loader: Option<String>,
) -> Result<Instance, String> {
    let paths = state.paths.clone();
    instances::folder_for(&nombre, &paths).and_then(|folder| {
        if folder.exists() { Err(format!("Ya tienes una instancia llamada \"{}\".", nombre.trim())) } else { Ok(()) }
    })?;
    let loader = loader.as_deref().filter(|l| !l.eq_ignore_ascii_case("vanilla")).map(parse_loader).transpose()?;
    let emitter = app.clone();
    let (version_id, loader_version) = install_game(&version, loader, version_loader.as_deref(), &paths, move |stage, fraction| {
        let _ = emitter.emit("progreso", Progress { task: "crear".into(), stage, fraction });
    })
    .await?;
    let game = GameVersion {
        version_id: &version_id,
        minecraft: &version,
        loader: loader.map(|l| l.name()).zip(loader_version.as_deref()),
    };
    instances::create(&nombre, &game, &paths)
}

/// Cambia la versión del loader de una instancia (la descarga si hace falta).
#[tauri::command]
async fn cambiar_loader(app: AppHandle, state: State<'_, AppState>, id: String, version_loader: String) -> Result<Instance, String> {
    let instance = instance_or_error(&state, &id)?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err("Cierra el juego antes de cambiar la versión.".into());
    }
    let loader = parse_loader(&instance.loader).map_err(|_| "Esta instancia es vanilla: no tiene loader.".to_string())?;
    let paths = state.paths.clone();
    let emitter = app.clone();
    let (version_id, loader_version) =
        install_game(&instance.minecraft_version, Some(loader), Some(&version_loader), &paths, move |stage, fraction| {
            let _ = emitter.emit("progreso", Progress { task: "loader".into(), stage, fraction });
        })
        .await?;
    let game = GameVersion {
        version_id: &version_id,
        minecraft: &instance.minecraft_version,
        loader: Some((loader.name(), loader_version.as_deref().unwrap_or(&version_loader))),
    };
    instances::set_version(&instance, &game)?;
    instance_or_error(&state, &id)
}

/// Aviso a la interfaz cuando un juego se cierra (con el motivo si fue por un error).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameClosed {
    id: String,
    code: Option<i32>,
    reason: Option<String>,
}

#[tauri::command]
async fn jugar(app: AppHandle, state: State<'_, AppState>, id: String, servidor: Option<String>) -> Result<u32, String> {
    let paths = state.paths.clone();
    let settings = state.settings.lock().unwrap().clone();
    if let Some(problem) = settings::validate_username(&settings.username) {
        return Err(format!("{problem} Cámbialo en Ajustes."));
    }
    let instance = instances::find(&paths, &id).ok_or("No se encontró la instancia.")?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err("Este juego ya está abierto.".into());
    }
    let launched = instance.clone();
    let mut child = tauri::async_runtime::spawn_blocking(move || {
        let mut plan = launch::plan(&launched, &settings, &paths)?;
        if let Some(address) = servidor.filter(|s| !s.trim().is_empty()) {
            plan.join_server(address.trim(), &launched.minecraft_version);
        }
        launch::start(&plan, &launched)
    })
    .await
    .map_err(|e| e.to_string())??;

    let pid = child.id();
    let started = chrono::Utc::now().timestamp_millis();
    state.games.lock().unwrap().insert(id.clone(), pid);
    // Un hilo espera a que el juego termine para avisar a la interfaz (y explicar el error si lo hubo).
    std::thread::spawn(move || {
        let code = child.wait().ok().and_then(|s| s.code());
        let state = app.state::<AppState>();
        let stopped = state.stopped.lock().unwrap().remove(&id);
        state.games.lock().unwrap().remove(&id);
        let failed = !stopped && code.is_some_and(|c| c != 0);
        let reason = if failed { launch::crash_reason(&instance, started) } else { None };
        let _ = app.emit("juego-cerrado", GameClosed { id, code: if stopped { None } else { code }, reason });
    });
    Ok(pid)
}

#[tauri::command]
fn detener(state: State<AppState>, id: String) -> Result<(), String> {
    let pid = *state.games.lock().unwrap().get(&id).ok_or("Ese juego no está abierto.")?;
    state.stopped.lock().unwrap().insert(id);
    launch::kill(pid);
    Ok(())
}

#[tauri::command]
fn juegos_abiertos(state: State<AppState>) -> Vec<String> {
    state.games.lock().unwrap().keys().cloned().collect()
}

/// Juegos que siguen abiertos de una sesión anterior del launcher: se vigilan hasta que se cierren.
fn adopt_running_games(app: &AppHandle) {
    let state = app.state::<AppState>();
    for instance in instances::list(&state.paths) {
        let Some(pid) = launch::running_pid(&instance) else { continue };
        state.games.lock().unwrap().insert(instance.id.clone(), pid);
        let app = app.clone();
        std::thread::spawn(move || {
            while launch::running_pid(&instance) == Some(pid) {
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
            app.state::<AppState>().games.lock().unwrap().remove(&instance.id);
            let _ = app.emit("juego-cerrado", GameClosed { id: instance.id, code: None, reason: None });
        });
    }
}

// ================= Gestión de instancias

fn instance_or_error(state: &AppState, id: &str) -> Result<Instance, String> {
    instances::find(&state.paths, id).ok_or_else(|| "No se encontró la instancia.".into())
}

#[tauri::command]
fn editar_instancia(state: State<AppState>, id: String, nombre: String, ram_gb: Option<u32>, jvm_args: Vec<String>) -> Result<Instance, String> {
    let instance = instance_or_error(&state, &id)?;
    instances::update(&instance, &nombre, ram_gb, &jvm_args)?;
    instance_or_error(&state, &id)
}

#[tauri::command]
fn icono_instancia(state: State<AppState>, id: String, png: Option<String>) -> Result<Instance, String> {
    let instance = instance_or_error(&state, &id)?;
    instances::set_icon(&instance, png.as_deref())?;
    instance_or_error(&state, &id)
}

#[tauri::command]
async fn duplicar_instancia(state: State<'_, AppState>, id: String) -> Result<Instance, String> {
    let instance = instance_or_error(&state, &id)?;
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || instances::duplicate(&instance, &paths)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
fn eliminar_instancia(state: State<AppState>, id: String) -> Result<(), String> {
    if state.games.lock().unwrap().contains_key(&id) {
        return Err("Cierra el juego antes de eliminar la instancia.".into());
    }
    instances::delete(&instance_or_error(&state, &id)?)
}

#[tauri::command]
async fn contenido(state: State<'_, AppState>, id: String) -> Result<content::Content, String> {
    let instance = instance_or_error(&state, &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut list = content::list(&instance);
        let known = modrinth::known_projects(&instance);
        for (kind, items) in [("mods", &mut list.mods), ("resourcepacks", &mut list.resourcepacks), ("shaderpacks", &mut list.shaderpacks)] {
            for item in items.iter_mut() {
                let base = item.file.strip_suffix(".disabled").unwrap_or(&item.file);
                item.project_id = known.get(&format!("{kind}/{base}")).cloned();
            }
        }
        list
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn activar_contenido(state: State<AppState>, id: String, tipo: String, archivo: String, activo: bool) -> Result<String, String> {
    content::set_enabled(&instance_or_error(&state, &id)?, &tipo, &archivo, activo)
}

#[tauri::command]
fn eliminar_contenido(state: State<AppState>, id: String, tipo: String, archivo: String) -> Result<(), String> {
    content::delete(&instance_or_error(&state, &id)?, &tipo, &archivo)
}

/// Agrega archivos arrastrados a la ventana (cada uno va a su categoría).
#[tauri::command]
async fn agregar_contenido(state: State<'_, AppState>, id: String, rutas: Vec<String>, tipo: Option<String>) -> Result<content::Added, String> {
    let instance = instance_or_error(&state, &id)?;
    let sources: Vec<PathBuf> = rutas.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || content::add(&instance, &sources, tipo.as_deref())).await.map_err(|e| e.to_string())
}

/// Abre la ventana de Windows para elegir archivos y los agrega. None si el jugador canceló.
#[tauri::command]
async fn elegir_contenido(app: AppHandle, state: State<'_, AppState>, id: String, tipo: String) -> Result<Option<content::Added>, String> {
    let instance = instance_or_error(&state, &id)?;
    let (title, filter, extensions): (&str, &str, &[&str]) = match tipo.as_str() {
        "mods" => ("Agregar mods", "Mods de Minecraft", &["jar"]),
        "resourcepacks" => ("Agregar resource packs", "Resource packs", &["zip"]),
        "shaderpacks" => ("Agregar shaders", "Shaders", &["zip"]),
        _ => ("Agregar mundos", "Mundos comprimidos", &["zip"]),
    };
    let dialog = app.dialog().file().set_title(title).add_filter(filter, extensions);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_files()).await.map_err(|e| e.to_string())?;
    let Some(picked) = picked else { return Ok(None) };
    let sources: Vec<PathBuf> = picked.into_iter().filter_map(|f| f.into_path().ok()).collect();
    tauri::async_runtime::spawn_blocking(move || Some(content::add(&instance, &sources, Some(&tipo)))).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn abrir_carpeta_contenido(state: State<AppState>, id: String, tipo: String) -> Result<(), String> {
    let folder = content::folder(&instance_or_error(&state, &id)?, &tipo).ok_or("Categoría desconocida.")?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(folder).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogText {
    file: String,
    text: String,
}

#[tauri::command]
fn registro(state: State<AppState>, id: String) -> Result<LogText, String> {
    let (file, text) = launch::read_log(&instance_or_error(&state, &id)?, 400_000);
    Ok(LogText { file, text })
}

#[tauri::command]
fn abrir_registro(state: State<AppState>, id: String) -> Result<(), String> {
    let instance = instance_or_error(&state, &id)?;
    let folder = instance.game_dir.join("logs");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(folder).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ================= Modrinth: explorar, instalar y actualizar contenido

#[tauri::command]
async fn buscar_contenido(busqueda: modrinth::SearchQuery) -> Result<modrinth::SearchResults, String> {
    modrinth::search(&busqueda).await
}

#[tauri::command]
async fn categorias(tipo: String) -> Result<Vec<modrinth::Category>, String> {
    modrinth::categories(&tipo).await
}

#[tauri::command]
async fn proyecto(id: String) -> Result<modrinth::Project, String> {
    modrinth::project(&id).await
}

/// Versiones de un proyecto; con instancia, solo las compatibles con ella.
#[tauri::command]
async fn versiones_proyecto(
    state: State<'_, AppState>,
    id: String,
    tipo: String,
    instancia: Option<String>,
) -> Result<Vec<modrinth::Version>, String> {
    // Los modpacks se instalan como instancia nueva: todas sus versiones, con el loader que sea.
    if tipo == "modpack" {
        return modrinth::project_versions(&id, modrinth::MODPACK_LOADERS, &[]).await;
    }
    match instancia {
        Some(instance_id) if tipo == "datapack" => {
            let instance = instance_or_error(&state, &instance_id)?;
            modrinth::project_versions(&id, &["datapack"], &[instance.minecraft_version.as_str()]).await
        }
        Some(instance_id) => {
            let instance = instance_or_error(&state, &instance_id)?;
            let kind = modrinth::ContentType::parse(&tipo).ok_or("Tipo de contenido desconocido.")?;
            let loaders = modrinth::instance_loaders(&instance, kind)?;
            modrinth::project_versions(&id, &loaders, &[instance.minecraft_version.as_str()]).await
        }
        None => modrinth::project_versions(&id, &[], &[]).await,
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstalledProjects {
    projects: Vec<String>,
    versions: Vec<String>,
}

/// Proyectos (y versiones exactas) de Modrinth instalados en la instancia, para marcar "Instalado".
#[tauri::command]
async fn proyectos_instalados(state: State<'_, AppState>, id: String) -> Result<InstalledProjects, String> {
    let instance = instance_or_error(&state, &id)?;
    let files = modrinth::identify(&instance).await;
    Ok(InstalledProjects {
        projects: files.iter().filter_map(|f| f.project_id.clone()).collect(),
        versions: files.into_iter().filter_map(|f| f.version_id).collect(),
    })
}

/// Qué se descargaría al instalar un proyecto (con sus dependencias) o por qué no se puede.
#[tauri::command]
async fn preparar_instalacion(
    state: State<'_, AppState>,
    id: String,
    proyecto: String,
    version: Option<String>,
) -> Result<modrinth::Plan, String> {
    let instance = instance_or_error(&state, &id)?;
    modrinth::plan(&instance, &proyecto, version.as_deref()).await
}

#[tauri::command]
async fn instalar_contenido(app: AppHandle, state: State<'_, AppState>, id: String, archivos: Vec<modrinth::PlannedFile>) -> Result<(), String> {
    let instance = instance_or_error(&state, &id)?;
    // Con el juego abierto Windows no deja quitar los archivos viejos y quedarían duplicados.
    if archivos.iter().any(|f| f.replaces.is_some()) && state.games.lock().unwrap().contains_key(&id) {
        return Err("Cierra el juego antes de actualizar o cambiar versiones.".into());
    }
    let name = match archivos.as_slice() {
        [] => return Ok(()),
        [only] => only.title.clone(),
        many => format!("{} archivos", many.len()),
    };
    let emitter = app.clone();
    modrinth::install(&instance, &archivos, move |count, total, bytes, total_bytes| {
        let _ = emitter.emit(
            "progreso",
            Progress {
                task: "instalar".into(),
                stage: format!("Descargando {name}… {count} de {total}"),
                fraction: bytes as f64 / total_bytes.max(1) as f64,
            },
        );
    })
    .await
}

#[tauri::command]
async fn buscar_actualizaciones(state: State<'_, AppState>, id: String) -> Result<Vec<modrinth::PlannedFile>, String> {
    let instance = instance_or_error(&state, &id)?;
    modrinth::updates(&instance).await
}

/// Aplica actualizaciones; devuelve las que se dejaron fuera por no ser compatibles (con el motivo).
#[tauri::command]
async fn actualizar_contenido(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    archivos: Vec<modrinth::PlannedFile>,
) -> Result<Vec<modrinth::HeldBack>, String> {
    let instance = instance_or_error(&state, &id)?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err("Cierra el juego antes de actualizar.".into());
    }
    let emitter = app.clone();
    modrinth::apply_updates(&instance, archivos, move |count, total, bytes, total_bytes| {
        let _ = emitter.emit(
            "progreso",
            Progress {
                task: "instalar".into(),
                stage: format!("Actualizando… {count} de {total}"),
                fraction: bytes as f64 / total_bytes.max(1) as f64,
            },
        );
    })
    .await
}

/// Muestra un archivo exportado en el Explorador de Windows.
#[tauri::command]
fn mostrar_archivo(ruta: String) -> Result<(), String> {
    let path = PathBuf::from(&ruta);
    if !path.exists() {
        return Err("Ese archivo ya no existe.".into());
    }
    std::process::Command::new("explorer").arg(format!("/select,{}", path.display())).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Abre en el navegador un enlace de la página de un proyecto (solo http y https).
#[tauri::command]
fn abrir_enlace(url: String) -> Result<(), String> {
    let valid = (url.starts_with("https://") || url.starts_with("http://"))
        && url.len() < 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || "\"<>^|`".contains(c));
    if !valid {
        return Err("Enlace no válido.".into());
    }
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", &url]).spawn().map(|_| ()).map_err(|e| e.to_string())
}

// ================= Modpacks: exportar, importar e instalar

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Imported {
    /// Carpeta de la instancia creada o actualizada (None si era un paquete base).
    id: Option<String>,
    name: String,
    /// "modpack" o "base"
    kind: String,
    /// Ya existía y se actualizó conservando mundos y opciones.
    updated: bool,
}

/// Instala un modpack desde un archivo: paquete de Parabellum (Drive o USB) o .mrpack de Modrinth.
async fn import_modpack(
    file: &Path,
    paths: &Paths,
    display_name: Option<String>,
    icon: Option<Vec<u8>>,
    source: serde_json::Value,
    running: &HashSet<String>,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<Imported, String> {
    // Paquete de Parabellum (también los de la versión C#)
    if let Some(manifest) = packs::read_manifest(file)? {
        progress("Comprobando que el archivo esté completo…".into(), 0.0);
        let to_verify = file.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || packs::verify_sidecar(&to_verify)).await.map_err(|e| e.to_string())??;
        if !manifest.is_base() {
            if manifest.folder.as_ref().is_some_and(|f| running.contains(f)) {
                return Err("Cierra el juego antes de actualizar este modpack.".into());
            }
            if !packs::base_available(&manifest.minecraft_version, paths) {
                // Con internet, Minecraft se descarga de Mojang; sin internet hace falta el paquete base.
                let report = progress.clone();
                let _ = install_vanilla(&manifest.minecraft_version, paths, move |stage, f| report(stage, f * 0.5)).await;
            }
        }
        let (file, paths, report) = (file.to_path_buf(), paths.clone(), progress.clone());
        let installed = tauri::async_runtime::spawn_blocking(move || packs::install(&file, &paths, report)).await.map_err(|e| e.to_string())??;
        return Ok(Imported { id: installed.folder, name: installed.name, kind: installed.kind, updated: installed.updated });
    }

    // .mrpack
    let info = mrpack::read(file)?.ok_or("No es un modpack: elige un .mrpack o un paquete de Parabellum (.zip con lanzador-pack.json).")?;
    let loader = info.loader.as_ref().map(|(name, _)| parse_loader(name)).transpose()?;
    let name = instances::unique_name(display_name.as_deref().unwrap_or(&info.name), paths)?;
    let report = progress.clone();
    let (version_id, loader_version) =
        install_game(&info.minecraft, loader, info.loader.as_ref().map(|(_, v)| v.as_str()), paths, move |stage, f| report(stage, f * 0.4))
            .await?;
    let game = GameVersion { version_id: &version_id, minecraft: &info.minecraft, loader: loader.map(|l| l.name()).zip(loader_version.as_deref()) };
    let instance = instances::create(&name, &game, paths)?;
    let report = progress.clone();
    if let Err(error) = mrpack::install_files(&info, file, &instance.game_dir, move |stage, f| report(stage, 0.4 + f * 0.6)).await {
        // Una instancia a medias no sirve; es nueva, así que no tiene nada del jugador.
        let _ = std::fs::remove_dir_all(&instance.folder);
        return Err(error);
    }
    let mut source = source;
    source["name"] = serde_json::json!(info.name);
    source["version"] = serde_json::json!(info.version);
    let _ = instances::set_field(&instance, "modpack", source);
    if let Some(bytes) = icon {
        let _ = instances::set_icon_bytes(&instance, &bytes);
    }
    progress("Listo".into(), 1.0);
    Ok(Imported { id: Some(instance.id), name, kind: "modpack".into(), updated: false })
}

fn progress_emitter(app: &AppHandle, task: &'static str) -> impl Fn(String, f64) + Send + Sync + Clone + 'static {
    let emitter = app.clone();
    move |stage, fraction| {
        let _ = emitter.emit("progreso", Progress { task: task.into(), stage, fraction });
    }
}

/// Lo que hay en la instancia para elegir qué incluir al exportar.
#[tauri::command]
fn contenido_exportable(state: State<AppState>, id: String) -> Result<Vec<packs::Entry>, String> {
    Ok(packs::exportable(&instance_or_error(&state, &id)?))
}

/// Exporta un modpack: "drive" (paquete completo, también para USB) o "mrpack". None si se canceló.
#[tauri::command]
async fn exportar_modpack(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    formato: String,
    opciones: packs::ExportOptions,
    elegir: bool,
) -> Result<Option<serde_json::Value>, String> {
    let instance = instance_or_error(&state, &id)?;
    let paths = state.paths.clone();
    let progress = progress_emitter(&app, "exportar");
    if formato == "mrpack" {
        let name = Some(opciones.name.trim()).filter(|n| !n.is_empty()).unwrap_or(&instance.name).to_string();
        let dialog = app
            .dialog()
            .file()
            .set_title("Guardar el modpack (.mrpack)")
            .set_file_name(format!("{} {}.mrpack", name.replace(['<', '>', ':', '"', '/', '\\', '|', '?', '*'], "_"), opciones.version.trim()))
            .add_filter("Modpack de Modrinth", &["mrpack"]);
        let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file()).await.map_err(|e| e.to_string())?;
        let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
        let result = mrpack::export(&instance, &path, &opciones, progress).await?;
        return Ok(Some(serde_json::to_value(result).unwrap()));
    }
    let title = "Carpeta donde guardar el paquete (la de tus paquetes para Drive, o una USB)";
    let Some(folder) = export_folder(&app, &paths, elegir, title).await? else { return Ok(None) };
    let result = tauri::async_runtime::spawn_blocking(move || packs::export(&instance, &paths, &folder, &opciones, progress))
        .await
        .map_err(|e| e.to_string())??;
    Ok(Some(serde_json::to_value(result).unwrap()))
}

/// Instala un modpack desde un archivo (sin ruta, abre la ventana para elegirlo). None si se canceló.
#[tauri::command]
async fn importar_modpack(app: AppHandle, state: State<'_, AppState>, ruta: Option<String>) -> Result<Option<Imported>, String> {
    let file = match ruta {
        Some(path) => PathBuf::from(path),
        None => {
            let dialog = app
                .dialog()
                .file()
                .set_title("Elige un modpack")
                .add_filter("Modpacks (.mrpack o paquete .zip)", &["mrpack", "zip"]);
            let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await.map_err(|e| e.to_string())?;
            match picked.and_then(|p| p.into_path().ok()) {
                Some(path) => path,
                None => return Ok(None),
            }
        }
    };
    let running: HashSet<String> = state.games.lock().unwrap().keys().cloned().collect();
    let progress = progress_emitter(&app, "importar");
    import_modpack(&file, &state.paths, None, None, serde_json::json!({ "source": "archivo" }), &running, progress).await.map(Some)
}

/// Instancias de CurseForge, Prism y MultiMC encontradas en este PC.
#[tauri::command]
async fn instancias_externas() -> Vec<externos::External> {
    tauri::async_runtime::spawn_blocking(externos::discover).await.unwrap_or_default()
}

/// Elegir a mano la carpeta de una instancia de otro launcher. None si se canceló.
#[tauri::command]
async fn elegir_instancia_externa(app: AppHandle) -> Result<Option<externos::External>, String> {
    let dialog = app.dialog().file().set_title("Elige la carpeta de la instancia (CurseForge, Prism o MultiMC)");
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder()).await.map_err(|e| e.to_string())?;
    let Some(folder) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    externos::read(&folder)
        .map(Some)
        .ok_or("Esa carpeta no es una instancia de CurseForge, Prism ni MultiMC (no tiene minecraftinstance.json ni mmc-pack.json).".into())
}

/// Copia una instancia de otro launcher a Parabellum, con su loader instalado por Parabellum.
async fn import_external(folder: &Path, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<Imported, String> {
    let external = externos::read(folder).ok_or("Esa carpeta no es una instancia de CurseForge, Prism ni MultiMC.")?;
    let loader = Some(external.loader.as_str()).filter(|l| *l != "Vanilla").map(parse_loader).transpose()?;
    let name = instances::unique_name(&external.name, paths)?;
    let report = progress.clone();
    let (version_id, loader_version) =
        install_game(&external.minecraft, loader, external.loader_version.as_deref(), paths, move |stage, f| report(stage, f * 0.5)).await?;
    let game = GameVersion { version_id: &version_id, minecraft: &external.minecraft, loader: loader.map(|l| l.name()).zip(loader_version.as_deref()) };
    let instance = instances::create(&name, &game, paths)?;
    let (from, to, report) = (external.game_dir.clone(), instance.game_dir.clone(), progress.clone());
    let copied = tauri::async_runtime::spawn_blocking(move || {
        externos::copy_game(&from, &to, |done, total| {
            report(format!("Copiando mods, mundos y opciones… {} de {} MB", done / 1048576, total / 1048576), 0.5 + 0.5 * done as f64 / total as f64)
        })
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Err(error) = copied {
        // Una instancia a medias no sirve; es nueva, así que no tiene nada del jugador.
        let _ = std::fs::remove_dir_all(&instance.folder);
        return Err(error);
    }
    if external.ram_gb.is_some() || !external.jvm_args.is_empty() {
        let _ = instances::update(&instance, &name, external.ram_gb, &external.jvm_args);
    }
    if let Some(bytes) = external.icon.as_ref().and_then(|icon| std::fs::read(icon).ok()) {
        let _ = instances::set_icon_bytes(&instance, &bytes);
    }
    let _ = instances::set_field(&instance, "importadaDe", serde_json::json!({ "launcher": external.launcher, "carpeta": external.folder }));
    progress("Listo".into(), 1.0);
    Ok(Imported { id: Some(instance.id), name, kind: "instancia".into(), updated: false })
}

#[tauri::command]
async fn importar_externa(app: AppHandle, state: State<'_, AppState>, carpeta: String) -> Result<Imported, String> {
    import_external(Path::new(&carpeta), &state.paths, progress_emitter(&app, "importar")).await
}

/// Instala un modpack de Modrinth como instancia nueva.
#[tauri::command]
async fn instalar_modpack(app: AppHandle, state: State<'_, AppState>, proyecto: String, version: Option<String>) -> Result<Imported, String> {
    let paths = state.paths.clone();
    let progress = progress_emitter(&app, "importar");
    progress("Descargando el modpack…".into(), 0.0);
    let download = modrinth::download_modpack(&proyecto, version.as_deref(), &paths.data.join("temporal")).await?;
    let icon = match &download.icon_url {
        Some(url) => modrinth::download_icon(url).await,
        None => None,
    };
    let source = serde_json::json!({
        "source": "modrinth",
        "projectId": download.project_id,
        "versionId": download.version_id,
        "versionNumber": download.version_number,
    });
    let running: HashSet<String> = state.games.lock().unwrap().keys().cloned().collect();
    let result = import_modpack(&download.file, &paths, Some(download.title.clone()), icon, source, &running, progress).await;
    let _ = std::fs::remove_file(&download.file);
    result
}

/// Instala un data pack de Modrinth dentro de un mundo de la instancia.
#[tauri::command]
async fn instalar_datapack(state: State<'_, AppState>, id: String, mundo: String, proyecto: String, version: Option<String>) -> Result<String, String> {
    let instance = instance_or_error(&state, &id)?;
    let (title, number) = modrinth::install_datapack(&instance, &mundo, &proyecto, version.as_deref()).await?;
    Ok(format!("Se instaló {title} {number} en el mundo «{mundo}»."))
}

// ================= Catálogo de Drive, publicar y actualizar el launcher

/// El catálogo se lee una vez y se guarda; `reload` lo vuelve a leer de Drive y de la carpeta paquetes.
async fn catalog_snapshot(state: &AppState, reload: bool) -> Arc<catalog::Catalog> {
    if !reload {
        if let Some(cached) = state.catalog.lock().unwrap().clone() {
            return cached;
        }
    }
    let loaded = Arc::new(catalog::load(&state.paths, &state.config).await);
    *state.catalog.lock().unwrap() = Some(loaded.clone());
    loaded
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogView {
    /// Hay carpeta de Drive configurada.
    configured: bool,
    folder_url: Option<String>,
    local_folder: String,
    entries: Vec<catalog::EntryView>,
    warnings: Vec<String>,
}

#[tauri::command]
async fn catalogo(state: State<'_, AppState>, recargar: bool) -> Result<CatalogView, String> {
    let catalog = catalog_snapshot(&state, recargar).await;
    let list = instances::list(&state.paths);
    Ok(CatalogView {
        configured: state.config.has_drive(),
        folder_url: state.config.drive_folder_url(),
        local_folder: state.paths.root.join(catalog::LOCAL_FOLDER).display().to_string(),
        entries: catalog.entries.iter().filter(|e| !e.is_launcher()).map(|e| catalog::view(e, &list, &state.paths)).collect(),
        warnings: catalog.warnings.clone(),
    })
}

/// Instala (o actualiza, o repara bajando solo lo dañado) un paquete del catálogo.
#[tauri::command]
async fn instalar_de_catalogo(app: AppHandle, state: State<'_, AppState>, id: String, reparar: Option<bool>) -> Result<catalog::CatalogInstall, String> {
    let catalog = catalog_snapshot(&state, false).await;
    let entry = catalog
        .entries
        .iter()
        .find(|e| !e.is_launcher() && e.manifest.id.eq_ignore_ascii_case(&id))
        .ok_or("Ese paquete ya no está en el catálogo. Recárgalo.")?;
    let list = instances::list(&state.paths);
    if let Some((instance, _)) = catalog::installed_instance(&entry.manifest, &list) {
        if state.games.lock().unwrap().contains_key(&instance.id) {
            return Err("Cierra el juego antes de actualizar este modpack.".into());
        }
    }
    catalog::install(entry, &catalog.entries, &state.paths, &list, reparar.unwrap_or(false), progress_emitter(&app, "catalogo")).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherUpdate {
    version: String,
    notes: String,
    size: u64,
}

/// Versión nueva de Parabellum publicada en Drive o en la carpeta paquetes, si la hay.
#[tauri::command]
async fn actualizacion_launcher(state: State<'_, AppState>, recargar: bool) -> Result<Option<LauncherUpdate>, String> {
    let catalog = catalog_snapshot(&state, recargar).await;
    Ok(catalog::launcher_update(&catalog.entries).map(|e| LauncherUpdate {
        version: e.manifest.version.clone(),
        notes: e.manifest.description.clone(),
        size: e.size,
    }))
}

/// Descarga la versión nueva, reemplaza el exe y reinicia Parabellum.
#[tauri::command]
async fn aplicar_actualizacion(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let catalog = catalog_snapshot(&state, false).await;
    let entry = catalog::launcher_update(&catalog.entries).ok_or("No hay una versión nueva.")?.clone();
    let exe = catalog::apply_launcher_update(&entry, &state.paths, progress_emitter(&app, "actualizacion")).await?;
    std::process::Command::new(&exe).arg("--reiniciar").spawn().map_err(|e| format!("Se actualizó, pero no se pudo reabrir: {e}"))?;
    app.exit(0);
    Ok(())
}

/// Carpeta donde se guardan los paquetes para subir a Drive (se recuerda la última).
fn export_folder_file(paths: &Paths) -> PathBuf {
    paths.data.join("carpeta-exportar.txt")
}

fn remembered_export_folder(paths: &Paths) -> Option<PathBuf> {
    std::fs::read_to_string(export_folder_file(paths)).ok().map(|t| PathBuf::from(t.trim())).filter(|p| p.is_dir())
}

#[tauri::command]
fn carpeta_exportar(state: State<AppState>) -> Option<String> {
    remembered_export_folder(&state.paths).map(|p| p.display().to_string())
}

/// La carpeta recordada o, si no hay (o se pide otra), la que se elija. None si se canceló.
async fn export_folder(app: &AppHandle, paths: &Paths, choose: bool, title: &str) -> Result<Option<PathBuf>, String> {
    if !choose {
        if let Some(folder) = remembered_export_folder(paths) {
            return Ok(Some(folder));
        }
    }
    let mut dialog = app.dialog().file().set_title(title);
    if let Some(folder) = remembered_export_folder(paths) {
        dialog = dialog.set_directory(folder);
    }
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder()).await.map_err(|e| e.to_string())?;
    let folder = picked.and_then(|p| p.into_path().ok());
    if let Some(folder) = &folder {
        let _ = std::fs::write(export_folder_file(paths), folder.display().to_string());
    }
    Ok(folder)
}

/// Elegir (y recordar) la carpeta donde se guardan los paquetes para Drive.
#[tauri::command]
async fn elegir_carpeta_exportar(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>, String> {
    let title = "Carpeta de tus paquetes para Drive";
    Ok(export_folder(&app, &state.paths, true, title).await?.map(|p| p.display().to_string()))
}

/// La carpeta "paquetes" junto a Parabellum (zips copiados de una USB, por ejemplo).
#[tauri::command]
fn abrir_carpeta_paquetes(state: State<AppState>) -> Result<(), String> {
    let folder = state.paths.root.join(catalog::LOCAL_FOLDER);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    std::process::Command::new("explorer").arg(folder).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Prepara la carpeta _parabellum con esta versión para subirla a Drive. None si se canceló.
#[tauri::command]
async fn publicar_launcher(app: AppHandle, state: State<'_, AppState>, notas: String, elegir: bool) -> Result<Option<String>, String> {
    let title = "Carpeta de tus paquetes para Drive (donde va la carpeta _parabellum)";
    let Some(folder) = export_folder(&app, &state.paths, elegir, title).await? else { return Ok(None) };
    let result = tauri::async_runtime::spawn_blocking(move || catalog::publish_launcher(&folder, &notas)).await.map_err(|e| e.to_string())??;
    Ok(Some(result.display().to_string()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JavaInfo {
    name: String,
    version: String,
    major: u32,
    path: String,
}

#[tauri::command]
fn listar_java(state: State<AppState>) -> Vec<JavaInfo> {
    let mut list: Vec<JavaInfo> = java::discover(&state.paths)
        .into_iter()
        .map(|j| JavaInfo { path: j.home.display().to_string(), name: j.name, version: j.version, major: j.major })
        .collect();
    list.sort_by(|a, b| b.major.cmp(&a.major));
    list
}

#[tauri::command]
fn abrir_carpeta(state: State<AppState>, id: Option<String>) -> Result<(), String> {
    let folder = match id {
        Some(id) => instances::find(&state.paths, &id).ok_or("No se encontró la instancia.")?.game_dir,
        None => state.paths.root.clone(),
    };
    std::process::Command::new("explorer").arg(folder).spawn().map(|_| ()).map_err(|e| e.to_string())
}

fn system_ram_gb() -> u32 {
    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page: u64,
        avail_page: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
    }
    let mut status = MemoryStatus {
        length: std::mem::size_of::<MemoryStatus>() as u32,
        load: 0, total_phys: 0, avail_phys: 0, total_page: 0, avail_page: 0, total_virtual: 0, avail_virtual: 0, avail_extended: 0,
    };
    // SAFETY: estructura con el tamaño indicado, como pide la API.
    if unsafe { GlobalMemoryStatusEx(&mut status) } != 0 {
        (status.total_phys as f64 / 1073741824.0).round() as u32
    } else {
        8
    }
}

/// Modo sin ventana para diagnóstico (el resultado queda en datos\logs\comando.txt):
///   --comprobar <instancia>             qué falta para jugar
///   --crear <nombre> <versión> [fabric|quilt|forge|neoforge] [versión]
///                                       descarga y crea la instancia
///   --cambiar-loader <instancia> <versión del loader>
///   --agregar <instancia> <archivo>...  agrega mods, packs, shaders o mundos
///   --jugar <instancia> [servidor]      abre el juego (y entra al servidor)
///   --probar-juego <instancia> [seg]    abre el juego, espera al menú principal y lo cierra
///   --buscar <texto> [mod|resourcepack|shader] [loader] [versión]   busca en Modrinth
///   --instalar <instancia> <proyecto> [versión]                     instala con dependencias
///   --actualizaciones <instancia> [aplicar]
///   --exportar <instancia> <carpeta> [con-mundos]   paquete para Drive/USB (con la base de Minecraft)
///   --exportar-mrpack <instancia> <archivo.mrpack>
///   --importar <archivo> [nombre]                   paquete de Parabellum o .mrpack
///   --externas                                      instancias de CurseForge, Prism y MultiMC en este PC
///   --importar-externa <carpeta>                    copia una de ellas a Parabellum
///   --instalar-modpack <proyecto> [versión]         modpack de Modrinth como instancia nueva
///   --instalar-datapack <instancia> <mundo> <proyecto>
///   --catalogo                                      modpacks y versiones de Drive y de la carpeta paquetes
///   --instalar-catalogo <id> [reparar]              instala, actualiza o repara (bajando solo lo que cambió)
///   --publicar-launcher <carpeta> [notas]           prepara _parabellum para subirla a Drive
fn run_command(args: &[String], paths: &Paths, settings: &Settings) -> Result<String, String> {
    match args[0].as_str() {
        "--comprobar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let plan = launch::plan(&instance, settings, paths)?;
            Ok(format!(
                "{} ({})\njava: {}\n{}",
                instance.name,
                instance.version_id,
                plan.java.as_ref().map(|j| format!("{} {}", j.name, j.version)).unwrap_or("ninguno".into()),
                if plan.missing.is_empty() { "LISTA".to_string() } else { format!("FALTAN:\n{}", plan.missing.join("\n")) }
            ))
        }
        "--crear" => {
            let name = args.get(1).ok_or("Falta el nombre.")?;
            let version = args.get(2).ok_or("Falta la versión.")?;
            let loader = args.get(3).map(|l| parse_loader(l)).transpose()?;
            let started = std::time::Instant::now();
            let (version_id, loader_version) =
                tauri::async_runtime::block_on(install_game(version, loader, args.get(4).map(String::as_str), paths, |_, _| {}))?;
            let game = GameVersion {
                version_id: &version_id,
                minecraft: version,
                loader: loader.map(|l| l.name()).zip(loader_version.as_deref()),
            };
            let instance = instances::create(name, &game, paths)?;
            Ok(format!(
                "Creada {} ({} {} {}) en {:.0} s",
                instance.id,
                instance.loader,
                instance.loader_version.as_deref().unwrap_or("-"),
                instance.version_id,
                started.elapsed().as_secs_f64()
            ))
        }
        "--cambiar-loader" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let wanted = args.get(2).ok_or("Falta la versión del loader.")?;
            let loader = parse_loader(&instance.loader)?;
            let (version_id, _) =
                tauri::async_runtime::block_on(install_game(&instance.minecraft_version, Some(loader), Some(wanted), paths, |_, _| {}))?;
            let game = GameVersion { version_id: &version_id, minecraft: &instance.minecraft_version, loader: Some((loader.name(), wanted)) };
            instances::set_version(&instance, &game)?;
            let changed = instances::find(paths, &instance.id).ok_or("No se encontró la instancia.")?;
            Ok(format!("{}: {} {} ({})", changed.name, changed.loader, changed.loader_version.unwrap_or_default(), changed.version_id))
        }
        "--agregar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let sources: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            Ok(format!("{:#?}", content::add(&instance, &sources, None)))
        }
        "--probar-juego" => {
            // Abre el juego y espera a que llegue al menú principal (o falle); luego lo cierra.
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let limit = args.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(180);
            let plan = launch::plan(&instance, settings, paths)?;
            let started_ms = chrono::Utc::now().timestamp_millis();
            let started = std::time::Instant::now();
            let mut child = launch::start(&plan, &instance)?;
            let log = instance.game_dir.join("logs").join("latest.log");
            let mut out = format!("{} ({} {})\n", instance.name, instance.loader, instance.version_id);
            let outcome = loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if let Ok(Some(status)) = child.try_wait() {
                    break format!("SE CERRÓ con código {:?}: {:?}", status.code(), launch::crash_reason(&instance, started_ms));
                }
                let fresh = std::fs::metadata(&log)
                    .and_then(|m| m.modified())
                    .is_ok_and(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as i64 >= started_ms);
                let text = if fresh { std::fs::read_to_string(&log).unwrap_or_default() } else { String::new() };
                // El juego terminó de cargar cuando crea los atlas de texturas o arranca el sonido.
                if text.contains("Sound engine started") || text.contains("-atlas") {
                    break format!("MENÚ PRINCIPAL en {:.0} s", started.elapsed().as_secs_f64());
                }
                if started.elapsed().as_secs() > limit {
                    break format!("SIN TERMINAR tras {limit} s");
                }
            };
            launch::kill(child.id());
            let _ = child.wait();
            out += &outcome;
            out += "\n";
            // Lo que dijo el loader sobre los mods
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            let mut lines = text.lines().skip_while(|l| !l.contains("Loading ") || !l.contains(" mods"));
            for line in lines.by_ref().take(1) {
                out += line;
                out += "\n";
            }
            for line in lines.take_while(|l| l.trim_start().starts_with('-') || l.trim_start().starts_with("|--") || l.trim_start().starts_with("\\--")).take(40) {
                out += line;
                out += "\n";
            }
            for line in text.lines().filter(|l| l.contains("/ERROR]") || l.contains("/FATAL]")).take(10) {
                out += line;
                out += "\n";
            }
            Ok(out)
        }
        "--jugar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let mut plan = launch::plan(&instance, settings, paths)?;
            if let Some(server) = args.get(2) {
                plan.join_server(server, &instance.minecraft_version);
            }
            Ok(format!("Abierto, PID {}", launch::start(&plan, &instance)?.id()))
        }
        "--probar-gestion" => {
            // Prueba de la fase 3 sobre una COPIA de la instancia indicada (la original no se toca).
            let original = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let mut out = String::new();
            let copy = instances::duplicate(&original, paths)?;
            out += &format!("duplicada: {} -> carpeta {}\n", copy.name, copy.id);

            instances::update(&copy, "Prueba fase 3", Some(3), &["-XX:+UseG1GC".to_string()])?;
            let edited = instances::find(paths, &copy.id).ok_or("No se encontró la copia.")?;
            out += &format!("editada: nombre={} ram={:?} jvm={:?}\n", edited.name, edited.ram_gb, edited.jvm_args);

            // PNG mínimo de 1×1 píxel
            let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";
            instances::set_icon(&edited, Some(png))?;
            out += &format!("icono puesto: {}\n", instances::find(paths, &copy.id).and_then(|i| i.icon).is_some());
            instances::set_icon(&edited, None)?;
            out += &format!("icono quitado: {}\n", instances::find(paths, &copy.id).and_then(|i| i.icon).is_none());

            // Un "mod" falso para probar activar/desactivar y la papelera
            let mods = edited.game_dir.join("mods");
            std::fs::create_dir_all(&mods).map_err(|e| e.to_string())?;
            std::fs::write(mods.join("prueba.jar"), b"no es un jar").map_err(|e| e.to_string())?;
            let renamed = content::set_enabled(&edited, "mods", "prueba.jar", false)?;
            out += &format!("mod desactivado: {renamed}, en la lista: {}\n", content::list(&edited).mods.iter().any(|m| !m.enabled));
            let restored = content::set_enabled(&edited, "mods", &renamed, true)?;
            out += &format!("mod activado: {restored}\n");
            content::delete(&edited, "mods", &restored)?;
            out += &format!("mod a la papelera: {}\n", !mods.join(&restored).exists());

            // Un argumento de Java inválido hace que Java se cierre con error al momento
            instances::update(&edited, "Prueba fase 3", None, &["-XX:+OpcionQueNoExiste".to_string()])?;
            let broken = instances::find(paths, &copy.id).ok_or("No se encontró la copia.")?;
            let started = chrono::Utc::now().timestamp_millis();
            let plan = launch::plan(&broken, settings, paths)?;
            let code = launch::start(&plan, &broken)?.wait().map_err(|e| e.to_string())?.code();
            out += &format!("juego con error: código {code:?}\nmotivo: {:?}\n", launch::crash_reason(&broken, started));
            out += &format!(
                "  (salida de Java: {:?}; latest.log existe: {})\n",
                std::fs::read_to_string(broken.state_dir().join("salida-java.log")).unwrap_or_default(),
                broken.game_dir.join("logs").join("latest.log").exists()
            );

            instances::delete(&broken)?;
            out += &format!("copia a la papelera: {}\n", !broken.folder.exists());
            Ok(out)
        }
        "--contenido" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let c = content::list(&instance);
            let mut out = format!(
                "mods {} · resource packs {} · shaders {} · mundos {}\n",
                c.mods.len(),
                c.resourcepacks.len(),
                c.shaderpacks.len(),
                c.worlds.len()
            );
            for item in c.mods.iter().take(12) {
                out += &format!(
                    "  {}{} {} [{}] icono={}\n",
                    if item.enabled { "" } else { "(desactivado) " },
                    item.name,
                    item.version.as_deref().unwrap_or("?"),
                    item.file,
                    item.icon.is_some()
                );
            }
            Ok(out)
        }
        "--buscar" => {
            let query = modrinth::SearchQuery {
                query: args.get(1).cloned().unwrap_or_default(),
                kind: args.get(2).cloned().unwrap_or_else(|| "mod".into()),
                loaders: args.get(3).map(|l| vec![l.clone()]).unwrap_or_default(),
                game_version: args.get(4).cloned(),
                categories: Vec::new(),
                sort: "relevance".into(),
                offset: 0,
            };
            let results = tauri::async_runtime::block_on(modrinth::search(&query))?;
            let mut out = format!("{} resultados\n", results.total);
            for hit in results.hits.iter().take(10) {
                out += &format!("  {} ({}) por {} · {} descargas · {:?}\n", hit.title, hit.slug, hit.author, hit.downloads, hit.categories);
            }
            Ok(out)
        }
        "--instalar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let project = args.get(2).ok_or("Falta el proyecto.")?;
            let started = std::time::Instant::now();
            let plan = tauri::async_runtime::block_on(modrinth::plan(&instance, project, args.get(3).map(String::as_str)))?;
            let mut out = format!("{} {}\n", plan.title, plan.version_number);
            for file in &plan.files {
                out += &format!(
                    "  {} {} {} ({}){}\n",
                    if file.dependency { "dependencia" } else { "principal" },
                    file.title,
                    file.version_number,
                    file.filename,
                    file.replaces.as_ref().map(|r| format!(" reemplaza {r}")).unwrap_or_default()
                );
            }
            if !plan.optional.is_empty() {
                out += &format!("  opcionales: {}\n", plan.optional.iter().map(|o| o.title.as_str()).collect::<Vec<_>>().join(", "));
            }
            if plan.installed {
                out += "  YA INSTALADA\n";
            } else if let Some(problem) = &plan.problem {
                out += &format!("  NO SE PUEDE: {problem}\n");
            } else {
                tauri::async_runtime::block_on(modrinth::install(&instance, &plan.files, |_, _, _, _| {}))?;
                out += &format!("  INSTALADO en {:.1} s\n", started.elapsed().as_secs_f64());
            }
            Ok(out)
        }
        "--actualizaciones" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let updates = tauri::async_runtime::block_on(modrinth::updates(&instance))?;
            let mut out = format!("{} actualizaciones\n", updates.len());
            for file in &updates {
                out += &format!("  {}: {} -> {} ({})\n", file.title, file.replaces.as_deref().unwrap_or("?"), file.version_number, file.filename);
            }
            if args.get(2).is_some_and(|a| a == "aplicar") && !updates.is_empty() {
                let held_back = tauri::async_runtime::block_on(modrinth::apply_updates(&instance, updates, |_, _, _, _| {}))?;
                out += "  APLICADAS\n";
                for held in held_back {
                    out += &format!("  SIN ACTUALIZAR {}: {}\n", held.title, held.reason);
                }
            }
            Ok(out)
        }
        "--exportar" => {
            // --exportar <instancia> <carpeta> [con-mundos]
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let folder = PathBuf::from(args.get(2).ok_or("Falta la carpeta.")?);
            let with_worlds = args.get(3).is_some_and(|a| a == "con-mundos");
            let include = packs::exportable(&instance).into_iter().filter(|e| e.checked || (with_worlds && e.name == "saves")).map(|e| e.name).collect();
            let started = std::time::Instant::now();
            let result = packs::export(&instance, paths, &folder, &packs::ExportOptions { include, ..Default::default() }, |_, _| {})?;
            Ok(format!("{result:#?}\nen {:.0} s", started.elapsed().as_secs_f64()))
        }
        "--exportar-mrpack" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let file = PathBuf::from(args.get(2).ok_or("Falta el archivo .mrpack.")?);
            let include = packs::exportable(&instance).into_iter().filter(|e| e.checked).map(|e| e.name).collect();
            let options = packs::ExportOptions { include, version: "1.0.0".into(), ..Default::default() };
            let result = tauri::async_runtime::block_on(mrpack::export(&instance, &file, &options, |_, _| {}))?;
            Ok(format!("{result:#?}"))
        }
        "--externas" => Ok(externos::discover()
            .iter()
            .map(|e| {
                format!(
                    "{} [{}] Minecraft {} · {} {} · {} mods · {} mundos · {}
",
                    e.name,
                    e.launcher,
                    e.minecraft,
                    e.loader,
                    e.loader_version.as_deref().unwrap_or(""),
                    e.mods,
                    e.worlds,
                    e.folder.display()
                )
            })
            .collect()),
        "--importar-externa" => {
            let folder = PathBuf::from(args.get(1).ok_or("Falta la carpeta.")?);
            let started = std::time::Instant::now();
            let result = tauri::async_runtime::block_on(import_external(&folder, paths, |stage, _| eprintln!("{stage}")))?;
            Ok(format!("{} -> carpeta {:?}, en {:.0} s", result.name, result.id, started.elapsed().as_secs_f64()))
        }
        "--importar" => {
            let file = PathBuf::from(args.get(1).ok_or("Falta el archivo.")?);
            let started = std::time::Instant::now();
            let result = tauri::async_runtime::block_on(import_modpack(
                &file,
                paths,
                args.get(2).cloned(),
                None,
                serde_json::json!({ "source": "archivo" }),
                &HashSet::new(),
                |_, _| {},
            ))?;
            Ok(format!("{} ({}) carpeta {:?}, actualizada: {}, en {:.0} s", result.name, result.kind, result.id, result.updated, started.elapsed().as_secs_f64()))
        }
        "--instalar-modpack" => {
            let project = args.get(1).ok_or("Falta el proyecto.")?;
            let started = std::time::Instant::now();
            let temp = paths.data.join("temporal");
            let result = tauri::async_runtime::block_on(async {
                let download = modrinth::download_modpack(project, args.get(2).map(String::as_str), &temp).await?;
                let icon = match &download.icon_url {
                    Some(url) => modrinth::download_icon(url).await,
                    None => None,
                };
                let source = serde_json::json!({ "source": "modrinth", "projectId": download.project_id, "versionId": download.version_id });
                let result = import_modpack(&download.file, paths, Some(download.title.clone()), icon, source, &HashSet::new(), |_, _| {}).await;
                let _ = std::fs::remove_file(&download.file);
                result
            })?;
            Ok(format!("{} -> carpeta {:?} en {:.0} s", result.name, result.id, started.elapsed().as_secs_f64()))
        }
        "--instalar-datapack" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let world = args.get(2).ok_or("Falta el mundo.")?;
            let project = args.get(3).ok_or("Falta el proyecto.")?;
            let (title, number) = tauri::async_runtime::block_on(modrinth::install_datapack(&instance, world, project, None))?;
            Ok(format!("{title} {number} instalado en {world}"))
        }
        "--catalogo" => {
            let config = config::Config::load(paths);
            let catalog = tauri::async_runtime::block_on(catalog::load(paths, &config));
            let list = instances::list(paths);
            let mut out = format!("Drive configurado: {}\n", config.has_drive());
            for entry in &catalog.entries {
                if entry.is_launcher() {
                    out += &format!("  launcher {} {} ({:?})\n", entry.manifest.id, entry.manifest.version, entry.source);
                    continue;
                }
                let view = catalog::view(entry, &list, paths);
                out += &format!(
                    "  {} [{}] {} · {} · {} MB · {:?} · {}\n",
                    view.name,
                    view.kind,
                    view.minecraft_version,
                    view.loader,
                    view.size / 1048576,
                    view.source,
                    view.status,
                );
            }
            for warning in &catalog.warnings {
                out += &format!("  aviso: {warning}\n");
            }
            if let Some(update) = catalog::launcher_update(&catalog.entries) {
                out += &format!("  ACTUALIZACIÓN DEL LAUNCHER: {}\n", update.manifest.version);
            }
            Ok(out)
        }
        "--instalar-catalogo" => {
            // --instalar-catalogo <id> [reparar]
            let id = args.get(1).ok_or("Falta el id del paquete.")?;
            let config = config::Config::load(paths);
            let started = std::time::Instant::now();
            let result = tauri::async_runtime::block_on(async {
                let catalog = catalog::load(paths, &config).await;
                let entry = catalog.entries.iter().find(|e| e.manifest.id.eq_ignore_ascii_case(id)).ok_or("No está en el catálogo.")?;
                let list = instances::list(paths);
                catalog::install(entry, &catalog.entries, paths, &list, args.get(2).is_some_and(|a| a == "reparar"), |_, _| {}).await
            })?;
            Ok(format!("{result:#?}\nen {:.0} s", started.elapsed().as_secs_f64()))
        }
        "--publicar-launcher" => {
            let folder = PathBuf::from(args.get(1).ok_or("Falta la carpeta.")?);
            let notes = args.get(2).cloned().unwrap_or_default();
            Ok(format!("{}", catalog::publish_launcher(&folder, &notes)?.display()))
        }
        other => Err(format!("Comando desconocido: {other}")),
    }
}

fn main() {
    let paths = Paths::detect();
    let _ = paths.ensure_created();
    let settings = Settings::load(&paths);
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Tras actualizarse, la versión nueva espera a que la anterior termine de cerrarse.
    let restarting = args.first().is_some_and(|a| a == "--reiniciar");
    if restarting {
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
    if !restarting && args.first().is_some_and(|a| a.starts_with("--")) {
        let report = run_command(&args, &paths, &settings).unwrap_or_else(|e| format!("ERROR: {e}"));
        let _ = std::fs::write(paths.logs.join("comando.txt"), report);
        return;
    }
    catalog::cleanup_previous_version();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            config: config::Config::load(&paths),
            paths,
            settings: Mutex::new(settings),
            games: Mutex::default(),
            stopped: Mutex::default(),
            catalog: Mutex::default(),
        })
        .setup(|app| {
            adopt_running_games(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            estado_inicial,
            guardar_ajustes,
            uuid_offline,
            listar_instancias,
            listar_versiones,
            versiones_compatibles,
            versiones_loader,
            crear_instancia,
            cambiar_loader,
            jugar,
            juegos_abiertos,
            detener,
            editar_instancia,
            icono_instancia,
            duplicar_instancia,
            eliminar_instancia,
            contenido,
            activar_contenido,
            eliminar_contenido,
            agregar_contenido,
            elegir_contenido,
            abrir_carpeta_contenido,
            buscar_contenido,
            categorias,
            proyecto,
            versiones_proyecto,
            proyectos_instalados,
            preparar_instalacion,
            instalar_contenido,
            buscar_actualizaciones,
            actualizar_contenido,
            abrir_enlace,
            contenido_exportable,
            exportar_modpack,
            importar_modpack,
            instancias_externas,
            elegir_instancia_externa,
            importar_externa,
            instalar_modpack,
            instalar_datapack,
            mostrar_archivo,
            catalogo,
            instalar_de_catalogo,
            actualizacion_launcher,
            aplicar_actualizacion,
            carpeta_exportar,
            elegir_carpeta_exportar,
            abrir_carpeta_paquetes,
            publicar_launcher,
            registro,
            abrir_registro,
            listar_java,
            abrir_carpeta
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir Parabellum");
}
