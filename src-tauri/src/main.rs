// Sin ventana de consola en Windows.
#![windows_subsystem = "windows"]

mod compat;
mod content;
mod download;
mod instances;
mod java;
mod launch;
mod loaders;
mod modrinth;
mod mojang;
mod paths;
mod settings;
mod version;

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
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
    /// Fabric y Quilt ya instalados (para crear instancias sin conexión).
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

/// Instala Fabric o Quilt (la versión pedida o la recomendada). Devuelve (id de la versión, versión del loader).
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

/// Cambia la versión de Fabric o Quilt de una instancia (la descarga si hace falta).
#[tauri::command]
async fn cambiar_loader(app: AppHandle, state: State<'_, AppState>, id: String, version_loader: String) -> Result<Instance, String> {
    let instance = instance_or_error(&state, &id)?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err("Cierra el juego antes de cambiar la versión.".into());
    }
    let loader = parse_loader(&instance.loader).map_err(|_| "Esta instancia no usa Fabric ni Quilt.".to_string())?;
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
    match instancia {
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
///   --crear <nombre> <versión> [fabric|quilt] [versión del loader]
///                                       descarga y crea la instancia
///   --cambiar-loader <instancia> <versión del loader>
///   --agregar <instancia> <archivo>...  agrega mods, packs, shaders o mundos
///   --jugar <instancia> [servidor]      abre el juego (y entra al servidor)
///   --probar-juego <instancia> [seg]    abre el juego, espera al menú principal y lo cierra
///   --buscar <texto> [mod|resourcepack|shader] [loader] [versión]   busca en Modrinth
///   --instalar <instancia> <proyecto> [versión]                     instala con dependencias
///   --actualizaciones <instancia> [aplicar]
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
        other => Err(format!("Comando desconocido: {other}")),
    }
}

fn main() {
    let paths = Paths::detect();
    let _ = paths.ensure_created();
    let settings = Settings::load(&paths);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a.starts_with("--")) {
        let report = run_command(&args, &paths, &settings).unwrap_or_else(|e| format!("ERROR: {e}"));
        let _ = std::fs::write(paths.logs.join("comando.txt"), report);
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { paths, settings: Mutex::new(settings), games: Mutex::default(), stopped: Mutex::default() })
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
            registro,
            abrir_registro,
            listar_java,
            abrir_carpeta
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir Parabellum");
}
