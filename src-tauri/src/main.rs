// Sin ventana de consola en Windows.
#![windows_subsystem = "windows"]

#[macro_use]
mod idioma;
// Notificaciones y accesos directos: cada sistema tiene los suyos (ver plataforma/).
use plataforma::{accesos, avisos};
mod actividad;
mod amigos;
mod catalog;
mod chat;
mod compartir;
mod compat;
mod config;
mod content;
mod diagnostico;
mod cuenta;
mod download;
mod favoritos;
mod drive;
mod externos;
mod firebase;
mod forge;
mod google;
mod hosting;
mod imagen;
mod instances;
mod integridad;
mod java;
mod launch;
mod loaders;
mod modrinth;
mod mojang;
mod mrpack;
mod nbt;
mod opciones;
mod juego;
mod p2p;
mod puente;
mod packs;
mod paths;
mod plataforma;
mod ping;
mod playit;
mod proceso;
mod remote_zip;
mod servidor;
mod sesion_local;
mod settings;
mod sonido;
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
    /// Cuenta de Google, amigos y servidor de sesión local.
    social: cuenta::SocialState,
    /// «Hostear partida»: servidores en marcha y el túnel de playit.gg.
    hosting: hosting::Hosting,
    /// Permiso para publicar en el Drive del catálogo (solo quien publica lo tiene).
    drive_publisher: tokio::sync::Mutex<Option<Arc<drive::Publisher>>>,
    /// Conexión directa con los amigos (P2P); None hasta que arranca.
    p2p: Mutex<Option<Arc<p2p::P2p>>>,
    /// Puente con el mod dentro del juego (interfaz de amigos y chat en Minecraft).
    puente: Mutex<Option<Arc<puente::Puente>>>,
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
    /// Este PC es el de quien publica (ve las herramientas de publicar en Drive).
    creator: bool,
    /// "Windows" o "macOS" (la interfaz oculta lo que no hay en ese sistema).
    platform: &'static str,
    /// El sistema tiene accesos directos (Escritorio y menú Inicio).
    shortcuts: bool,
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
        creator: is_creator(&state.paths),
        platform: plataforma::NAME,
        shortcuts: accesos::SUPPORTED,
        settings,
    }
}

const CREATOR_FILE: &str = "creador.txt";

/// Las herramientas de publicar (carpeta para Drive, conectar Drive, publicar versiones) son solo para
/// quien mantiene el catálogo: se ven si este PC ya tiene Drive conectado para publicar, o si se
/// activaron a mano (siete clics en la versión, en Acerca de). Lo elegido a mano manda.
fn is_creator(paths: &Paths) -> bool {
    match std::fs::read_to_string(paths.data.join(CREATOR_FILE)) {
        Ok(choice) => choice.trim() == "si",
        Err(_) => drive::load(paths).is_some(),
    }
}

#[tauri::command]
fn herramientas_creador(state: State<AppState>, activar: bool) -> Result<bool, String> {
    std::fs::write(state.paths.data.join(CREATOR_FILE), if activar { "si" } else { "no" }).map_err(|e| e.to_string())?;
    Ok(is_creator(&state.paths))
}

#[tauri::command]
fn guardar_ajustes(app: AppHandle, state: State<AppState>, mut ajustes: Settings) -> Result<Settings, String> {
    ajustes.normalize()?;
    {
        let current = state.settings.lock().unwrap();
        ajustes.favorite_chats = current.favorite_chats.clone();
        ajustes.favorite_mods = current.favorite_mods.clone();
        ajustes.favorites_owner = current.favorites_owner.clone();
    }
    ajustes.ram_gb = ajustes.ram_gb.clamp(1, 64);
    ajustes.ui_scale = if ajustes.ui_scale.is_finite() { ajustes.ui_scale.clamp(0.8, 1.5) } else { 1.0 };
    ajustes.language = idioma::normalize(&ajustes.language);
    let language_changed = ajustes.language != state.settings.lock().unwrap().language;
    if (ajustes.ui_scale - state.settings.lock().unwrap().ui_scale).abs() > f64::EPSILON {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_zoom(ajustes.ui_scale);
        }
    }
    ajustes.save(&state.paths).map_err(|e| t!("No se pudieron guardar los ajustes: {e}", "Could not save the settings: {e}"))?;
    *state.settings.lock().unwrap() = ajustes.clone();
    if language_changed {
        idioma::set(&ajustes.language);
        // El menú del icono junto al reloj se vuelve a crear con los textos nuevos.
        cuenta::rebuild_tray(&app);
    }
    cuenta::settings_changed(&app);
    Ok(ajustes)
}

/// Mostrar u ocultar el panel de la derecha (se guarda aparte: no hace falta subir el perfil).
#[tauri::command]
fn panel_lateral(state: State<AppState>, oculto: bool) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.sidebar_hidden = oculto;
    settings.save(&state.paths).map_err(|e| t!("No se pudieron guardar los ajustes: {e}", "Could not save the settings: {e}"))
}

/// Marca o quita una conversación (o grupo) de favoritos. Devuelve la lista nueva.
#[tauri::command]
fn favorito_chat(app: AppHandle, state: State<AppState>, chat: String, activo: bool) -> Result<Vec<String>, String> {
    if chat.is_empty() || chat.len() > 200 || !chat.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(t!("Conversación no válida.", "Invalid conversation."));
    }
    let list = {
        let mut settings = state.settings.lock().unwrap();
        settings.favorite_chats.retain(|c| *c != chat);
        if activo {
            settings.favorite_chats.insert(0, chat.clone());
        }
        settings.save(&state.paths).map_err(|e| t!("No se pudieron guardar los ajustes: {e}", "Could not save the settings: {e}"))?;
        settings.favorite_chats.clone()
    };
    // Con la sesión de Google iniciada, también en la cuenta (te siguen a otro PC).
    favoritos::push_chat(&app, &chat, activo);
    Ok(list)
}

/// Marca o quita un mod (o resource pack, o shader) de Modrinth de favoritos. Devuelve la lista nueva.
#[tauri::command]
fn favorito_mod(app: AppHandle, state: State<AppState>, proyecto: settings::FavoriteMod, activo: bool) -> Result<Vec<settings::FavoriteMod>, String> {
    let id = proyecto.project_id.trim().to_string();
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) || !["mod", "resourcepack", "shader"].contains(&proyecto.kind.as_str()) {
        return Err(t!("Ese contenido no se puede marcar como favorito.", "That content cannot be marked as a favorite."));
    }
    let favorite = settings::FavoriteMod {
        project_id: id.clone(),
        title: proyecto.title.trim().chars().take(100).collect(),
        // Solo enlaces cortos (de Modrinth o del caché de iconos), no imágenes enteras.
        icon_url: proyecto.icon_url.filter(|u| u.len() <= 500 && (u.starts_with("https://") || u.starts_with("http://icono.localhost/"))),
        kind: proyecto.kind,
    };
    let list = {
        let mut settings = state.settings.lock().unwrap();
        settings.favorite_mods.retain(|m| m.project_id != id);
        if activo {
            settings.favorite_mods.insert(0, favorite.clone());
        }
        settings.save(&state.paths).map_err(|e| t!("No se pudieron guardar los ajustes: {e}", "Could not save the settings: {e}"))?;
        settings.favorite_mods.clone()
    };
    favoritos::push_mod(&app, &favorite, activo);
    Ok(list)
}

/// Una imagen que la ventana no sabe abrir (HEIC, TIFF…), convertida por Windows a PNG. `datos`: el
/// archivo en base64. Devuelve un data URL.
#[tauri::command]
async fn convertir_imagen(datos: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD.decode(datos.trim()).map_err(|_| t!("La imagen llegó dañada.", "The image arrived damaged."))?;
    if bytes.len() > imagen::MAX_BYTES {
        return Err(t!("La imagen pesa más de 25 MB.", "The image is larger than 25 MB."));
    }
    let png = tauri::async_runtime::spawn_blocking(move || imagen::to_png(&bytes)).await.map_err(|e| e.to_string())??;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)))
}

/// Prueba todas las rutas a un servidor (este PC, misma red, P2P, IPv6, playit.gg) y las devuelve con
/// su tiempo, la más rápida primero.
#[tauri::command]
async fn mejor_ruta(state: State<'_, AppState>, rutas: Vec<ping::Route>, max_jugadores: Option<u32>) -> Result<Vec<ping::Route>, String> {
    let p2p = state.p2p.lock().unwrap().clone();
    Ok(ping::best_routes(rutas, max_jugadores, p2p).await)
}

/// Si el firewall deja entrar a los servidores (conexión directa por la red o por IPv6).
#[tauri::command]
async fn firewall_servidores() -> bool {
    tauri::async_runtime::spawn_blocking(plataforma::firewall_open).await.unwrap_or(false)
}

/// Abre el firewall para los servidores (pide permiso de administrador una vez).
#[tauri::command]
async fn abrir_firewall_servidores() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(plataforma::open_firewall).await.map_err(|e| e.to_string())?
}

/// Estado de servidores (icono, mensaje, jugadores), como la lista de Multijugador del juego.
#[tauri::command]
async fn estado_servidores(state: State<'_, AppState>, direcciones: Vec<String>) -> Result<HashMap<String, ping::Status>, String> {
    let cache = state.paths.data.join("cache");
    Ok(ping::statuses(direcciones, &cache).await)
}

/// Toca el sonido de mensaje nuevo (al activarlo en Ajustes, para oírlo).
#[tauri::command]
fn sonido_mensaje() {
    sonido::message(false);
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
            let release = releases.into_iter().find(|r| r.id == version).ok_or(t!("Mojang no tiene la versión {version}.", "Mojang does not have version {version}."))?;
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
                .ok_or(t!("{} todavía no está disponible para Minecraft {minecraft}.", "{} is not available for Minecraft {minecraft} yet.", loader.name()))?
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
        if folder.exists() { Err(t!("Ya tienes una instancia llamada \"{}\".", "You already have an instance named \"{}\".", nombre.trim())) } else { Ok(()) }
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
        return Err(t!("Cierra el juego antes de cambiar la versión.", "Close the game before changing the version."));
    }
    let loader = parse_loader(&instance.loader).map_err(|_| t!("Esta instancia es vanilla: no tiene loader.", "This instance is vanilla: it has no loader."))?;
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
    /// Qué falló y cómo arreglarlo (si el juego se cerró con un error).
    diagnosis: Option<diagnostico::Diagnosis>,
}

#[tauri::command]
async fn jugar(app: AppHandle, state: State<'_, AppState>, id: String, servidor: Option<String>) -> Result<u32, String> {
    let paths = state.paths.clone();
    let settings = state.settings.lock().unwrap().clone();
    if let Some(problem) = settings::validate_username(&settings.username) {
        return Err(t!("{problem} Cámbialo en Ajustes.", "{problem} Change it in Settings."));
    }
    let instance = instances::find(&paths, &id).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err(t!("Este juego ya está abierto.", "This game is already open."));
    }
    // Que el modpack esté completo: lo que falte se baja (o se repone) antes de abrir el juego.
    let cached = state.catalog.lock().unwrap().clone();
    let completed = ensure_complete(&instance, &paths, &state.config, cached, false, progress_emitter(&app, "jugar")).await;
    let missing_mods = &completed.missing_mods;
    if !missing_mods.is_empty() {
        let shown: Vec<&str> = missing_mods.iter().take(4).map(String::as_str).collect();
        let shown = shown.join("\n");
        return Err(if missing_mods.len() == 1 {
            t!(
                "Falta 1 mod del modpack y no se pudo reponer:\n{shown}\nConéctate a internet o usa «Verificar y reparar archivos» en el menú ⋯ de la instancia.",
                "1 mod of the modpack is missing and could not be restored:\n{shown}\nConnect to the internet or use «Check and repair files» in the instance's ⋯ menu."
            )
        } else {
            t!(
                "Faltan {} mods del modpack y no se pudieron reponer:\n{shown}\nConéctate a internet o usa «Verificar y reparar archivos» en el menú ⋯ de la instancia.",
                "{} mods of the modpack are missing and could not be restored:\n{shown}\nConnect to the internet or use «Check and repair files» in the instance's ⋯ menu.",
                missing_mods.len()
            )
        });
    }
    // La memoria que pide el propio modpack para este PC (Memory Settings), si el jugador no eligió
    // otra a mano: así no sale su pantalla de aviso al abrir.
    let mut instance = instance;
    if instance.ram_gb.is_none() {
        if let Some(gb) = diagnostico::memory_settings_gb(&instance, settings.ram_gb) {
            if instances::update(&instance, &instance.name, Some(gb), &instance.jvm_args).is_ok() {
                instance.ram_gb = Some(gb);
                progress_emitter(&app, "jugar")(t!("{} usa {gb} GB: lo pide el modpack para este PC.", "{} uses {gb} GB: the modpack asks for it on this PC.", instance.name), 0.0);
            }
        }
    }
    let launched = instance.clone();
    let session = cuenta::session_arguments(&app);
    let ram_gb = instance.ram_gb.unwrap_or(settings.ram_gb);
    // El panel de Parabellum dentro del juego (amigos y chat), si hay mod para esta versión.
    juego::install(&instance, settings.game_panel);
    let mut child = tauri::async_runtime::spawn_blocking(move || {
        let mut plan = launch::plan(&launched, &settings, &paths)?;
        // Mundos LAN y e4mc de amigos sin "Invalid session".
        plan.add_jvm_arguments(session);
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
        launch::forget_pid(&instance);
        juego::remove(&instance);
        let state = app.state::<AppState>();
        let stopped = state.stopped.lock().unwrap().remove(&id);
        state.games.lock().unwrap().remove(&id);
        // Con código distinto de 0, o si dejó un informe de fallo (la pantalla de error de Forge sale con 0).
        let failed = !stopped && (code.is_some_and(|c| c != 0) || diagnostico::crash_report_since(&instance, started));
        // Si fue por el mod de Parabellum, la próxima vez se juega sin él.
        if failed && juego::after_crash(&instance, started) {
            eprintln!("juego: el mod de Parabellum hizo caer {}; se apaga en esa instancia", instance.name);
        }
        let reason = if failed { launch::crash_reason(&instance, started) } else { None };
        let diagnosis = failed.then(|| {
            diagnostico::after_crash(&instance, &diagnostico::CrashContext { since_ms: started, exit_code: code, ram_gb, system_ram_gb: system_ram_gb() })
        });
        let code = if stopped { None } else if failed { Some(code.filter(|c| *c != 0).unwrap_or(1)) } else { code };
        let _ = app.emit("juego-cerrado", GameClosed { id, code, reason, diagnosis });
    });
    Ok(pid)
}

#[tauri::command]
fn detener(state: State<AppState>, id: String) -> Result<(), String> {
    let pid = *state.games.lock().unwrap().get(&id).ok_or(t!("Ese juego no está abierto.", "That game is not open."))?;
    state.stopped.lock().unwrap().insert(id.clone());
    launch::kill(pid);
    if let Some(instance) = instances::find(&state.paths, &id) {
        std::thread::spawn(move || launch::close_helpers(&instance));
    }
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
            let _ = app.emit("juego-cerrado", GameClosed { id: instance.id, code: None, reason: None, diagnosis: None });
        });
    }
}

// ================= Gestión de instancias

fn instance_or_error(state: &AppState, id: &str) -> Result<Instance, String> {
    instances::find(&state.paths, id).ok_or_else(|| t!("No se encontró la instancia.", "The instance was not found."))
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
        return Err(t!("Cierra el juego antes de eliminar la instancia.", "Close the game before deleting the instance."));
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
    let (title, filter, extensions): (String, String, &[&str]) = match tipo.as_str() {
        "mods" => (t!("Agregar mods", "Add mods"), t!("Mods de Minecraft", "Minecraft mods"), &["jar"]),
        "resourcepacks" => (t!("Agregar resource packs", "Add resource packs"), "Resource packs".into(), &["zip"]),
        "shaderpacks" => (t!("Agregar shaders", "Add shaders"), "Shaders".into(), &["zip"]),
        _ => (t!("Agregar mundos", "Add worlds"), t!("Mundos comprimidos", "Compressed worlds"), &["zip"]),
    };
    let dialog = app.dialog().file().set_title(title).add_filter(filter, extensions);
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_files()).await.map_err(|e| e.to_string())?;
    let Some(picked) = picked else { return Ok(None) };
    let sources: Vec<PathBuf> = picked.into_iter().filter_map(|f| f.into_path().ok()).collect();
    tauri::async_runtime::spawn_blocking(move || Some(content::add(&instance, &sources, Some(&tipo)))).await.map_err(|e| e.to_string())
}

/// Accesos directos del Escritorio y del menú Inicio: si están y si abren este Parabellum.
#[tauri::command]
async fn accesos_directos() -> Result<accesos::Status, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || accesos::status(&exe)).await.map_err(|e| e.to_string())
}

/// Crea (o rehace) o quita un acceso directo: `lugar` es "escritorio" o "inicio".
#[tauri::command]
async fn acceso_directo(lugar: String, crear: bool) -> Result<accesos::Status, String> {
    let place = accesos::Place::parse(&lugar).ok_or(t!("Lugar desconocido.", "Unknown place."))?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        if crear { accesos::create(place, &exe) } else { accesos::remove(place) }?;
        Ok(accesos::status(&exe))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn abrir_carpeta_contenido(state: State<AppState>, id: String, tipo: String) -> Result<(), String> {
    let folder = content::folder(&instance_or_error(&state, &id)?, &tipo).ok_or(t!("Categoría desconocida.", "Unknown category."))?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    plataforma::open_path(&folder)
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
    plataforma::open_path(&folder)
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
            let kind = modrinth::ContentType::parse(&tipo).ok_or(t!("Tipo de contenido desconocido.", "Unknown content type."))?;
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
        return Err(t!("Cierra el juego antes de actualizar o cambiar versiones.", "Close the game before updating or changing versions."));
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
                stage: t!("Descargando {name}… {count} de {total}", "Downloading {name}… {count} of {total}"),
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
        return Err(t!("Cierra el juego antes de actualizar.", "Close the game before updating."));
    }
    let emitter = app.clone();
    modrinth::apply_updates(&instance, archivos, move |count, total, bytes, total_bytes| {
        let _ = emitter.emit(
            "progreso",
            Progress {
                task: "instalar".into(),
                stage: t!("Actualizando… {count} de {total}", "Updating… {count} of {total}"),
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
        return Err(t!("Ese archivo ya no existe.", "That file no longer exists."));
    }
    plataforma::reveal_path(&path)
}

/// Abre en el navegador un enlace de la página de un proyecto (solo http y https).
#[tauri::command]
fn abrir_enlace(url: String) -> Result<(), String> {
    let valid = (url.starts_with("https://") || url.starts_with("http://"))
        && url.len() < 2048
        && !url.chars().any(|c| c.is_whitespace() || c.is_control() || "\"<>^|`".contains(c));
    if !valid {
        return Err(t!("Enlace no válido.", "Invalid link."));
    }
    plataforma::open_url(&url)
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
    /// Lo que no se pudo completar (por ejemplo, mods de CurseForge sin internet).
    warning: Option<String>,
    /// Arreglos que se aplicaron solos para que abra en este PC.
    #[serde(default)]
    notes: Vec<String>,
}

/// Aviso de lo que quedó sin reponer, o None si está todo.
fn missing_warning(report: &integridad::Report) -> Option<String> {
    if report.failed.is_empty() {
        return None;
    }
    let shown: Vec<&str> = report.failed.iter().take(4).map(String::as_str).collect();
    let more = if report.failed.len() > shown.len() { t!("\n…y {} más", "\n…and {} more", report.failed.len() - shown.len()) } else { String::new() };
    let shown = shown.join("\n");
    Some(if report.failed.len() == 1 {
        t!("Falta 1 archivo del modpack:\n{shown}{more}", "1 file of the modpack is missing:\n{shown}{more}")
    } else {
        t!("Faltan {} archivos del modpack:\n{shown}{more}", "{} files of the modpack are missing:\n{shown}{more}", report.failed.len())
    })
}

/// Deja la instancia completa: baja lo que falte (con `deep`, también lo dañado) y lo que el modpack
/// pide a CurseForge, y repone lo que venía dentro del modpack (del .mrpack de Modrinth o del paquete de Drive).
async fn ensure_complete(
    instance: &Instance,
    paths: &Paths,
    config: &config::Config,
    catalog: Option<Arc<catalog::Catalog>>,
    deep: bool,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> integridad::Report {
    let package = packs::installed_manifest(instance).filter(|m| !m.is_base());
    // Un modpack de Modrinth exportado como paquete se repone desde el paquete: su lista es la del paquete.
    let modrinth_source = integridad::modrinth_source(instance).filter(|_| package.is_none());
    let temp = paths.data.join("temporal");
    let mut early_failure = None;
    // Desde Drive se compara todo el paquete (por CRC32), así que se hace una sola vez.
    let mut drive: Option<Result<String, String>> = None;
    // Instalada antes de que existiera la lista: se arma con el modpack original, o se repara desde Drive.
    if deep && integridad::load(&instance.game_dir).is_none() {
        if let Some((project, version)) = &modrinth_source {
            progress(t!("Descargando el modpack original para comparar…", "Downloading the original modpack to compare…"), 0.0);
            match modrinth::download_modpack(project, Some(version), &temp).await {
                Ok(download) => {
                    if let Ok(Some(info)) = mrpack::read(&download.file) {
                        integridad::save(&instance.game_dir, integridad::from_mrpack(&info, &download.file));
                    }
                    let _ = std::fs::remove_file(&download.file);
                }
                Err(error) => early_failure = Some(t!("No se pudo bajar el modpack original para comparar: {error}", "Could not download the original modpack to compare: {error}")),
            }
        } else if let Some(manifest) = &package {
            drive = Some(drive_repair(manifest, paths, config, catalog.clone(), progress.clone()).await);
        }
    }
    let mut report = integridad::repair(&instance.game_dir, deep, progress.clone()).await;
    if let Some(error) = early_failure {
        report.fail(None, error);
    }
    let pending = std::mem::take(&mut report.from_pack);
    // Cada archivo que no se pudo reponer se anota con su ruta (así se sabe si falta un mod).
    let fail_each = |report: &mut integridad::Report, paths: &[&str], reason: &str| {
        for path in paths {
            report.fail(Some(path), format!("{}: {reason}", integridad::file_name(path)));
        }
    };
    if !pending.is_empty() {
        let pending_paths: Vec<&str> = pending.iter().map(|t| t.path.as_str()).collect();
        if let Some((project, version)) = &modrinth_source {
            progress(t!("Descargando el modpack para reponer lo que traía dentro…", "Downloading the modpack to restore what it included…"), 0.5);
            match modrinth::download_modpack(project, Some(version), &temp).await {
                Ok(download) => {
                    let (file, game_dir, wanted) = (download.file.clone(), instance.game_dir.clone(), pending.clone());
                    let restored = tauri::async_runtime::spawn_blocking(move || integridad::restore_from_mrpack(&file, &game_dir, &wanted)).await;
                    let _ = std::fs::remove_file(&download.file);
                    match restored.map_err(|e| e.to_string()).and_then(|r| r) {
                        Ok((done, absent)) => {
                            report.repaired.extend(done);
                            fail_each(&mut report, &absent.iter().map(String::as_str).collect::<Vec<_>>(), &t!("el modpack ya no lo trae", "the modpack no longer includes it"));
                        }
                        Err(error) => fail_each(&mut report, &pending_paths, &error),
                    }
                }
                Err(error) => fail_each(&mut report, &pending_paths, &t!("no se pudo bajar el modpack ({error})", "the modpack could not be downloaded ({error})")),
            }
        } else if let Some(manifest) = &package {
            if drive.is_none() {
                drive = Some(drive_repair(manifest, paths, config, catalog, progress.clone()).await);
            }
            let still: Vec<&str> = pending_paths.iter().copied().filter(|p| !integridad::present(&instance.game_dir, p)).collect();
            match drive.take() {
                Some(Ok(text)) => {
                    report.repaired.push(text);
                    fail_each(&mut report, &still, &t!("el paquete ya no lo trae", "the package no longer includes it"));
                }
                Some(Err(error)) => fail_each(&mut report, &still, &error),
                None => {}
            }
        } else {
            fail_each(&mut report, &pending_paths, &t!("venía dentro del modpack; vuelve a instalarlo para reponerlo", "it came inside the modpack; reinstall it to restore it"));
        }
    }
    match drive {
        Some(Ok(text)) => report.repaired.push(text),
        Some(Err(error)) => report.fail(None, error),
        None => {}
    }
    progress(t!("Listo", "Done"), 1.0);
    report
}

/// Repara un modpack instalado desde Drive o la carpeta paquetes (baja solo lo que falte o esté dañado).
async fn drive_repair(
    manifest: &packs::Manifest,
    paths: &Paths,
    config: &config::Config,
    catalog: Option<Arc<catalog::Catalog>>,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<String, String> {
    let catalog = match catalog {
        Some(cached) => cached,
        None => Arc::new(catalog::load(paths, config).await),
    };
    let entry = catalog
        .entries
        .iter()
        .find(|e| !e.is_launcher() && !e.manifest.is_base() && e.manifest.id.eq_ignore_ascii_case(&manifest.id))
        .ok_or(t!("«{}» no está en el catálogo de Drive ni en la carpeta paquetes, y venía dentro de su paquete.", "«{}» is not in the Drive catalog or the packages folder, and it came inside its package.", manifest.name))?;
    let list = instances::list(paths);
    let result = catalog::install(entry, &catalog.entries, paths, &list, true, progress).await?;
    Ok(match result.downloaded_files {
        Some(0) => t!("Se comparó con el paquete «{}» de Drive: estaba todo bien", "Compared with the «{}» Drive package: everything was fine", manifest.name),
        Some(files) => t!("Se repusieron {files} archivos desde el paquete «{}»", "{files} files were restored from the «{}» package", manifest.name),
        None => t!("Se reinstaló el paquete «{}» (se conservaron mundos y opciones)", "The «{}» package was reinstalled (worlds and options were kept)", manifest.name),
    })
}

/// Instala un modpack desde un archivo: paquete de Parabellum (Drive o USB) o .mrpack de Modrinth.
async fn import_modpack(
    file: &Path,
    paths: &Paths,
    display_name: Option<String>,
    icon: Option<Vec<u8>>,
    source: serde_json::Value,
    running: &HashSet<String>,
    default_ram: u32,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<Imported, String> {
    // Paquete de Parabellum (también los de la versión C#)
    if let Some(manifest) = packs::read_manifest(file)? {
        progress(t!("Comprobando que el archivo esté completo…", "Checking that the file is complete…"), 0.0);
        let to_verify = file.to_path_buf();
        tauri::async_runtime::spawn_blocking(move || packs::verify_sidecar(&to_verify)).await.map_err(|e| e.to_string())??;
        if !manifest.is_base() {
            if manifest.folder.as_ref().is_some_and(|f| running.contains(f)) {
                return Err(t!("Cierra el juego antes de actualizar este modpack.", "Close the game before updating this modpack."));
            }
            if !packs::base_available(&manifest.minecraft_version, paths) {
                // Con internet, Minecraft se descarga de Mojang; sin internet hace falta el paquete base.
                let report = progress.clone();
                let _ = install_vanilla(&manifest.minecraft_version, paths, move |stage, f| report(stage, f * 0.5)).await;
            }
        }
        let (file, paths_copy, report) = (file.to_path_buf(), paths.clone(), progress.clone());
        let installed = tauri::async_runtime::spawn_blocking(move || packs::install(&file, &paths_copy, report)).await.map_err(|e| e.to_string())??;
        // Si el paquete se exportó antes de bajar lo que el modpack pide a CurseForge, se baja ahora.
        let mut warning = None;
        let mut notes = Vec::new();
        if let Some(instance) = installed.folder.as_deref().and_then(|f| instances::find(paths, f)) {
            warning = missing_warning(&integridad::repair(&instance.game_dir, false, progress.clone()).await);
            notes = apply_known_fixes(&instance, default_ram, progress.clone()).await;
        }
        return Ok(Imported { id: installed.folder, name: installed.name, kind: installed.kind, updated: installed.updated, warning, notes });
    }

    // .mrpack
    let info = mrpack::read(file)?.ok_or(t!("No es un modpack: elige un .mrpack o un paquete de Parabellum (.zip con lanzador-pack.json).", "It is not a modpack: choose a .mrpack or a Parabellum package (.zip with lanzador-pack.json)."))?;
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
    // Las opciones del modpack suelen venir en inglés: Minecraft abre en el idioma de Parabellum.
    instances::set_game_language(&instance.game_dir);
    let mut source = source;
    source["name"] = serde_json::json!(info.name);
    source["version"] = serde_json::json!(info.version);
    let _ = instances::set_field(&instance, "modpack", source);
    if let Some(bytes) = icon {
        let _ = instances::set_icon_bytes(&instance, &bytes);
    }
    // Lo que el modpack no puede traer de Modrinth y pide a CurseForge (Missing Mods Checker).
    let report = progress.clone();
    let completed = integridad::repair(&instance.game_dir, false, move |stage, f| report(stage, 0.95 + f * 0.05)).await;
    // Lo que se sabe que este modpack necesita en este PC para abrir a la primera.
    progress(t!("Revisando que abra en este PC…", "Checking that it opens on this PC…"), 0.99);
    let notes = match instances::find(paths, &instance.id) {
        Some(current) => apply_known_fixes(&current, default_ram, progress.clone()).await,
        None => Vec::new(),
    };
    progress(t!("Listo", "Done"), 1.0);
    Ok(Imported { id: Some(instance.id), name, kind: "modpack".into(), updated: false, warning: missing_warning(&completed), notes })
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
    let completed = integridad::repair(&instance.game_dir, false, progress.clone()).await;
    let missing_mods = &completed.missing_mods;
    if !missing_mods.is_empty() {
        let shown = missing_mods.iter().take(4).cloned().collect::<Vec<_>>().join("\n");
        return Err(t!(
            "El modpack está incompleto y saldría así. Faltan {} mods:\n{shown}",
            "The modpack is incomplete and would be exported that way. {} mods are missing:\n{shown}",
            missing_mods.len()
        ));
    }
    if formato == "mrpack" {
        let name = Some(opciones.name.trim()).filter(|n| !n.is_empty()).unwrap_or(&instance.name).to_string();
        let dialog = app
            .dialog()
            .file()
            .set_title(t!("Guardar el modpack (.mrpack)", "Save the modpack (.mrpack)"))
            .set_file_name(format!("{} {}.mrpack", name.replace(['<', '>', ':', '"', '/', '\\', '|', '?', '*'], "_"), opciones.version.trim()))
            .add_filter(t!("Modpack de Modrinth", "Modrinth modpack"), &["mrpack"]);
        let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_save_file()).await.map_err(|e| e.to_string())?;
        let Some(path) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
        let result = mrpack::export(&instance, &path, &opciones, progress).await?;
        return Ok(Some(serde_json::to_value(result).unwrap()));
    }
    let title = t!("Carpeta donde guardar el paquete (la de tus paquetes para Drive, o una USB)", "Folder to save the package in (your Drive packages folder, or a USB drive)");
    let Some(folder) = export_folder(&app, &paths, elegir, &title).await? else { return Ok(None) };
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
                .set_title(t!("Elige un modpack", "Choose a modpack"))
                .add_filter(t!("Modpacks (.mrpack o paquete .zip)", "Modpacks (.mrpack or .zip package)"), &["mrpack", "zip"]);
            let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await.map_err(|e| e.to_string())?;
            match picked.and_then(|p| p.into_path().ok()) {
                Some(path) => path,
                None => return Ok(None),
            }
        }
    };
    let running: HashSet<String> = state.games.lock().unwrap().keys().cloned().collect();
    let progress = progress_emitter(&app, "importar");
    let ram = state.settings.lock().unwrap().ram_gb;
    import_modpack(&file, &state.paths, None, None, serde_json::json!({ "source": "archivo" }), &running, ram, progress).await.map(Some)
}

/// Instancias de CurseForge, Prism y MultiMC encontradas en este PC.
#[tauri::command]
async fn instancias_externas() -> Vec<externos::External> {
    tauri::async_runtime::spawn_blocking(externos::discover).await.unwrap_or_default()
}

/// Elegir a mano la carpeta de una instancia de otro launcher. None si se canceló.
#[tauri::command]
async fn elegir_instancia_externa(app: AppHandle) -> Result<Option<externos::External>, String> {
    let dialog = app.dialog().file().set_title(t!("Elige la carpeta de la instancia (CurseForge, Prism o MultiMC)", "Choose the instance folder (CurseForge, Prism or MultiMC)"));
    let picked = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder()).await.map_err(|e| e.to_string())?;
    let Some(folder) = picked.and_then(|p| p.into_path().ok()) else { return Ok(None) };
    externos::read(&folder)
        .map(Some)
        .ok_or(t!("Esa carpeta no es una instancia de CurseForge, Prism ni MultiMC (no tiene minecraftinstance.json ni mmc-pack.json).", "That folder is not a CurseForge, Prism or MultiMC instance (it has no minecraftinstance.json or mmc-pack.json)."))
}

/// Copia una instancia de otro launcher a Parabellum, con su loader instalado por Parabellum.
async fn import_external(folder: &Path, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<Imported, String> {
    let external = externos::read(folder).ok_or(t!("Esa carpeta no es una instancia de CurseForge, Prism ni MultiMC.", "That folder is not a CurseForge, Prism or MultiMC instance."))?;
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
            report(t!("Copiando mods, mundos y opciones… {} de {} MB", "Copying mods, worlds and options… {} of {} MB", done / 1048576, total / 1048576), 0.5 + 0.5 * done as f64 / total as f64)
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
    let completed = integridad::repair(&instance.game_dir, false, progress.clone()).await;
    progress(t!("Listo", "Done"), 1.0);
    Ok(Imported { id: Some(instance.id), name, kind: "instancia".into(), updated: false, warning: missing_warning(&completed), notes: Vec::new() })
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
    progress(t!("Descargando el modpack…", "Downloading the modpack…"), 0.0);
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
    let ram = state.settings.lock().unwrap().ram_gb;
    let result = import_modpack(&download.file, &paths, Some(download.title.clone()), icon, source, &running, ram, progress).await;
    let _ = std::fs::remove_file(&download.file);
    result
}

/// Arma en este PC una instancia que compartió un amigo por el chat. Devuelve su id.
#[tauri::command]
async fn instalar_compartida(app: AppHandle, state: State<'_, AppState>, texto: String) -> Result<String, String> {
    let recipe = compartir::decode(&texto).ok_or(t!("Esa instancia compartida no se puede leer.", "That shared instance cannot be read."))?;
    match recipe.source.as_str() {
        "modrinth" => {
            let project = recipe.project.clone().unwrap_or_default();
            let imported = instalar_modpack(app, state, project, recipe.version.clone()).await?;
            imported.id.ok_or(t!("No se creó la instancia.", "The instance was not created."))
        }
        "catalogo" => {
            let id = recipe.catalog.clone().unwrap_or_default();
            let installed = instalar_de_catalogo(app, state, id, None).await.map_err(|error| {
                t!("{error} Pídele a quien la compartió que la suba al catálogo de Drive, o que te pase el paquete por USB.", "{error} Ask whoever shared it to upload it to the Drive catalog, or to give you the package on a USB drive.")
            })?;
            installed.id.ok_or(t!("No se creó la instancia.", "The instance was not created."))
        }
        _ => {
            let emitter = app.clone();
            let progress = move |stage: String, fraction: f64| {
                let _ = emitter.emit("progreso", Progress { task: "compartida".into(), stage, fraction });
            };
            install_recipe_mods(&recipe, &state.paths, progress).await.map(|instance| instance.id)
        }
    }
}

/// Instancia hecha a mano que compartió un amigo: misma versión y loader, y sus archivos de Modrinth.
async fn install_recipe_mods(recipe: &compartir::Recipe, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<Instance, String> {
    // Un nombre libre: «Mi pack», «Mi pack (2)»…
    let mut name = recipe.name.trim().to_string();
    let mut number = 2;
    while instances::folder_for(&name, paths).map(|f| f.exists()).unwrap_or(false) {
        name = format!("{} ({number})", recipe.name.trim());
        number += 1;
    }
    let loader = Some(recipe.loader.as_str()).filter(|l| !l.eq_ignore_ascii_case("vanilla") && !l.eq_ignore_ascii_case("none")).map(parse_loader).transpose()?;
    progress(t!("Preparando Minecraft {}…", "Preparing Minecraft {}…", recipe.minecraft), 0.0);
    let report = progress.clone();
    let (version_id, loader_version) = install_game(&recipe.minecraft, loader, recipe.loader_version.as_deref(), paths, move |stage, fraction| report(stage, fraction * 0.4)).await?;
    let game = GameVersion {
        version_id: &version_id,
        minecraft: &recipe.minecraft,
        loader: loader.map(|l| l.name()).zip(loader_version.as_deref()),
    };
    let instance = instances::create(&name, &game, paths)?;
    let files = modrinth::files_for_versions(&recipe.versions()).await?;
    if !files.is_empty() {
        let total = files.len();
        progress(t!("Descargando {total} archivos de Modrinth…", "Downloading {total} files from Modrinth…"), 0.4);
        let report = progress.clone();
        modrinth::install(&instance, &files, move |_, _, bytes, all| {
            report(t!("Descargando {total} archivos de Modrinth…", "Downloading {total} files from Modrinth…"), 0.4 + 0.6 * bytes as f64 / all.max(1) as f64)
        })
        .await?;
    }
    progress(t!("Listo", "Done"), 1.0);
    Ok(instance)
}

// ================= Doctor: revisar, diagnosticar y arreglar

/// Revisa los mods sin abrir el juego: dependencias, versiones del loader y del juego, repetidos.
#[tauri::command]
async fn revisar_instancia(state: State<'_, AppState>, id: String) -> Result<diagnostico::Diagnosis, String> {
    let instance = instance_or_error(&state, &id)?;
    tauri::async_runtime::spawn_blocking(move || diagnostico::check(&instance)).await.map_err(|e| e.to_string())
}

/// Explica el último arranque de la instancia (su informe de fallo y registros) y revisa sus mods.
#[tauri::command]
async fn diagnosticar(state: State<'_, AppState>, id: String) -> Result<diagnostico::Diagnosis, String> {
    let instance = instance_or_error(&state, &id)?;
    let ram_gb = instance.ram_gb.unwrap_or(state.settings.lock().unwrap().ram_gb);
    let since_ms = instance.last_played.unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || {
        diagnostico::after_crash(&instance, &diagnostico::CrashContext { since_ms, exit_code: None, ram_gb, system_ram_gb: system_ram_gb() })
    })
    .await
    .map_err(|e| e.to_string())
}

/// «Probar y arreglar»: abre el juego hasta el menú principal (y lo cierra). Si no llega, aplica los
/// arreglos recomendados del diagnóstico y vuelve a probar, hasta 5 veces.
#[tauri::command]
async fn probar_y_arreglar(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<diagnostico::AutoFix, String> {
    if state.games.lock().unwrap().contains_key(&id) {
        return Err(t!("Cierra el juego antes de probarlo.", "Close the game before testing it."));
    }
    let settings = state.settings.lock().unwrap().clone();
    let cached = state.catalog.lock().unwrap().clone();
    let session = cuenta::session_arguments(&app);
    let (running, key) = (app.clone(), id.clone());
    let on_start = move |pid: Option<u32>| {
        let state = running.state::<AppState>();
        match pid {
            Some(pid) => state.games.lock().unwrap().insert(key.clone(), pid),
            None => state.games.lock().unwrap().remove(&key),
        };
    };
    auto_fix(&id, &state.paths, &state.config, cached, &settings, session, on_start, progress_emitter(&app, "doctor")).await
}

#[allow(clippy::too_many_arguments)]
async fn auto_fix(
    id: &str,
    paths: &Paths,
    config: &config::Config,
    catalog: Option<Arc<catalog::Catalog>>,
    settings: &Settings,
    session: Vec<String>,
    on_start: impl Fn(Option<u32>) + Send + Sync + Clone + 'static,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<diagnostico::AutoFix, String> {
    let mut report = diagnostico::AutoFix::default();
    let mut tried: Vec<diagnostico::Fix> = Vec::new();
    const ATTEMPTS: u32 = 5;
    for _ in 1..=ATTEMPTS {
        // Cada arreglo cambia la instancia (loader, argumentos, memoria…): se vuelve a leer.
        let instance = instances::find(paths, id).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?;
        progress(t!("Revisando los mods…", "Checking the mods…"), 0.0);
        let _ = ensure_complete(&instance, paths, config, catalog.clone(), false, progress.clone()).await;
        let checked = {
            let instance = instance.clone();
            tauri::async_runtime::spawn_blocking(move || diagnostico::check(&instance)).await.map_err(|e| e.to_string())?
        };
        let before_launch = diagnostico::recommended(&checked, &tried);
        if !before_launch.is_empty() {
            for (label, fix) in before_launch {
                progress(t!("Arreglando: {label}…", "Fixing: {label}…"), 0.1);
                tried.push(fix.clone());
                match apply_fix(&instance, paths, config, catalog.clone(), fix, progress.clone()).await {
                    Ok(done) => report.applied.push(done),
                    Err(error) => report.applied.push(t!("No se pudo: {label} ({error})", "Could not: {label} ({error})")),
                }
            }
            continue;
        }
        // Abre el juego y espera al menú principal (los intentos cuentan las veces que se abre).
        report.attempts += 1;
        let launch = report.attempts;
        progress(t!("Abriendo el juego para probar (intento {launch})…", "Opening the game to test (attempt {launch})…"), 0.3);
        let ram_gb = instance.ram_gb.unwrap_or(settings.ram_gb);
        let (launched, settings_copy, paths_copy, session_copy, started_hook) = (instance.clone(), settings.clone(), paths.clone(), session.clone(), on_start.clone());
        let probe = tauri::async_runtime::spawn_blocking(move || -> Result<(Result<u64, Option<i32>>, i64), String> {
            let mut plan = launch::plan(&launched, &settings_copy, &paths_copy)?;
            plan.add_jvm_arguments(session_copy);
            let started = chrono::Utc::now().timestamp_millis();
            let mut child = launch::start(&plan, &launched)?;
            started_hook(Some(child.id()));
            let outcome = diagnostico::wait_for_menu(&mut child, &launched, started, 480);
            launch::close_helpers(&launched);
            launch::forget_pid(&launched);
            started_hook(None);
            Ok((outcome, started))
        })
        .await
        .map_err(|e| e.to_string())??;
        match probe {
            (Ok(_), started) => {
                report.ok = true;
                report.diagnosis = None;
                // Llegó, pero puede haber mostrado avisos (memoria, mods que faltan): lo que se puede
                // arreglar solo se arregla; lo demás queda a la vista.
                let warnings = {
                    let instance = instance.clone();
                    tauri::async_runtime::spawn_blocking(move || diagnostico::in_game_warnings(&instance, started, ram_gb)).await.unwrap_or_default()
                };
                let mut left = Vec::new();
                for finding in warnings.findings {
                    let fixable: Vec<(String, diagnostico::Fix)> =
                        finding.fixes.iter().filter(|o| o.recommended && !tried.contains(&o.fix)).map(|o| (o.label.clone(), o.fix.clone())).collect();
                    if fixable.is_empty() {
                        left.push(finding);
                        continue;
                    }
                    for (label, fix) in fixable {
                        progress(t!("Arreglando: {label}…", "Fixing: {label}…"), 0.9);
                        tried.push(fix.clone());
                        match apply_fix(&instance, paths, config, catalog.clone(), fix, progress.clone()).await {
                            Ok(done) => report.applied.push(done),
                            Err(error) => report.applied.push(t!("No se pudo: {label} ({error})", "Could not: {label} ({error})")),
                        }
                    }
                }
                report.warnings = (!left.is_empty()).then(|| diagnostico::Diagnosis { findings: left, checked_mods: 0 });
                progress(t!("Llega al menú principal.", "It reaches the main menu."), 1.0);
                return Ok(report);
            }
            (Err(code), started) => {
                let context = diagnostico::CrashContext { since_ms: started, exit_code: code, ram_gb, system_ram_gb: system_ram_gb() };
                let diagnosis = {
                    let instance = instance.clone();
                    tauri::async_runtime::spawn_blocking(move || diagnostico::after_crash(&instance, &context)).await.map_err(|e| e.to_string())?
                };
                let fixes = diagnostico::recommended(&diagnosis, &tried);
                if fixes.is_empty() {
                    report.diagnosis = Some(diagnosis);
                    return Ok(report);
                }
                for (label, fix) in fixes {
                    progress(t!("Arreglando: {label}…", "Fixing: {label}…"), 0.6);
                    tried.push(fix.clone());
                    match apply_fix(&instance, paths, config, catalog.clone(), fix, progress.clone()).await {
                        Ok(done) => report.applied.push(done),
                        Err(error) => report.applied.push(t!("No se pudo: {label} ({error})", "Could not: {label} ({error})")),
                    }
                }
                report.diagnosis = Some(diagnosis);
            }
        }
    }
    Ok(report)
}

/// Aplica un arreglo del doctor. Devuelve qué se hizo.
#[tauri::command]
async fn aplicar_arreglo(app: AppHandle, state: State<'_, AppState>, id: String, arreglo: diagnostico::Fix) -> Result<String, String> {
    let instance = instance_or_error(&state, &id)?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err(t!("Cierra el juego antes de arreglarlo.", "Close the game before fixing it."));
    }
    let cached = state.catalog.lock().unwrap().clone();
    apply_fix(&instance, &state.paths, &state.config, cached, arreglo, progress_emitter(&app, "doctor")).await
}

async fn apply_fix(
    instance: &Instance,
    paths: &Paths,
    config: &config::Config,
    catalog: Option<Arc<catalog::Catalog>>,
    fix: diagnostico::Fix,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<String, String> {
    use diagnostico::Fix;
    match fix {
        Fix::AddJvmArg { arg } => {
            let mut args = instance.jvm_args.clone();
            if !args.contains(&arg) {
                args.push(arg.clone());
            }
            instances::update(instance, &instance.name, instance.ram_gb, &args)?;
            Ok(t!("Se agregó {arg} a los argumentos de Java.", "{arg} was added to the Java arguments."))
        }
        Fix::SetLoader { requirements } => {
            let loader = parse_loader(&instance.loader)?;
            let versions = loaders::loader_versions(loader, &instance.minecraft_version).await?;
            let fits = |v: &&loaders::LoaderVersion| requirements.iter().all(|r| compat::matches(&v.version, r));
            let chosen = versions
                .iter()
                .filter(fits)
                .find(|v| v.stable)
                .or_else(|| versions.iter().find(fits))
                .ok_or(t!(
                    "No hay una versión de {} para Minecraft {} que cumpla lo que piden los mods.",
                    "There is no {} version for Minecraft {} that meets what the mods ask for.",
                    loader.name(),
                    instance.minecraft_version
                ))?
                .version
                .clone();
            let (version_id, loader_version) = install_game(&instance.minecraft_version, Some(loader), Some(&chosen), paths, progress).await?;
            let game = GameVersion {
                version_id: &version_id,
                minecraft: &instance.minecraft_version,
                loader: Some((loader.name(), loader_version.as_deref().unwrap_or(&chosen))),
            };
            instances::set_version(instance, &game)?;
            Ok(t!("{} cambió a {chosen}.", "{} switched to {chosen}.", loader.name()))
        }
        Fix::InstallMod { id, requirements, replaces } => {
            progress(t!("Buscando {id} en Modrinth…", "Looking for {id} on Modrinth…"), 0.0);
            let (project, version) = modrinth::find_by_mod_id(instance, &id, &requirements).await?.ok_or(t!(
                "{id} no está en Modrinth para esta versión. Búscalo en CurseForge o desactiva el mod que lo pide.",
                "{id} is not on Modrinth for this version. Look for it on CurseForge or disable the mod that asks for it."
            ))?;
            let plan = modrinth::plan(instance, &project, Some(&version)).await?;
            if let Some(problem) = plan.problem {
                return Err(problem);
            }
            if !plan.installed {
                let report = progress.clone();
                modrinth::install(instance, &plan.files, move |count, total, _, _| {
                    report(t!("Instalando… {count} de {total}", "Installing… {count} of {total}"), count as f64 / total.max(1) as f64)
                })
                .await?;
            }
            // La copia que no servía (si no la reemplazó la instalación) se desactiva para que no quede repetida.
            if let Some(old) = replaces.filter(|old| !plan.files.iter().any(|f| f.replaces.as_deref() == Some(old.as_str()) || &f.filename == old)) {
                if instance.game_dir.join("mods").join(&old).is_file() {
                    content::set_enabled(instance, "mods", &old, false)?;
                }
            }
            Ok(t!("Se instaló {} {}.", "{} {} was installed.", plan.title, plan.version_number))
        }
        Fix::EnableMod { file } => {
            let name = content::set_enabled(instance, "mods", &file, true)?;
            Ok(t!("Se activó {name}.", "{name} was enabled."))
        }
        Fix::DisableMod { file } => {
            content::set_enabled(instance, "mods", &file, false)?;
            Ok(t!("Se desactivó {file}. Lo puedes volver a activar en Contenido.", "{file} was disabled. You can turn it back on in Content."))
        }
        Fix::ResetConfig { path } => {
            let relative = mrpack::safe_relative(&path)
                .filter(|p| p.starts_with("config") || p.starts_with("defaultconfigs"))
                .ok_or(t!("Ruta de configuración no válida.", "Invalid settings path."))?;
            let full = instance.game_dir.join(relative);
            if full.is_file() {
                trash::delete(&full).map_err(|e| t!("No se pudo mandar a la papelera: {e}", "Could not move it to the Recycle Bin: {e}"))?;
            }
            Ok(t!("{path} se fue a la papelera: el mod creará uno nuevo.", "{path} went to the Recycle Bin: the mod will create a new one."))
        }
        Fix::SetRam { gb } => {
            let gb = gb.clamp(1, 64);
            instances::update(instance, &instance.name, Some(gb), &instance.jvm_args)?;
            Ok(t!("La instancia ahora usa {gb} GB.", "The instance now uses {gb} GB."))
        }
        Fix::UseJava { major } => {
            mojang::ensure_java(major, paths, progress).await?;
            instances::set_field(instance, "javaMajor", serde_json::json!(major))?;
            Ok(t!("La instancia ahora usa Java {major}.", "The instance now uses Java {major}."))
        }
        Fix::InstallProject { project, title } => install_project(instance, &project, &title, progress).await,
        Fix::Repair => {
            let report = ensure_complete(instance, paths, config, catalog, true, progress).await;
            Ok(match (report.repaired.len(), report.failed.len()) {
                (0, 0) => t!("Los archivos del modpack están completos.", "The modpack files are complete."),
                (repaired, 0) => t!("Se repusieron {repaired} archivos.", "{repaired} files were restored."),
                (_, failed) => t!("Faltan {failed} archivos que no se pudieron reponer.", "{failed} files are missing and could not be restored."),
            })
        }
    }
}

/// Instala un proyecto de Modrinth en la instancia (con sus dependencias).
async fn install_project(instance: &Instance, project: &str, title: &str, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<String, String> {
    progress(t!("Instalando {title}…", "Installing {title}…"), 0.0);
    let plan = modrinth::plan(instance, project, None).await?;
    if let Some(problem) = plan.problem {
        return Err(problem);
    }
    if !plan.installed {
        modrinth::install(instance, &plan.files, move |count, total, _, _| {
            if total > 0 {
                progress(t!("Instalando… {count} de {total}", "Installing… {count} of {total}"), count as f64 / total as f64);
            }
        })
        .await?;
    }
    Ok(t!("Se instaló {} {}.", "{} {} was installed.", plan.title, plan.version_number))
}

/// Los arreglos conocidos que necesita el modpack en este PC (TF-Intel-Fix, la memoria que pide):
/// se aplican al instalarlo. Devuelve qué se hizo.
async fn apply_known_fixes(instance: &Instance, default_ram: u32, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Vec<String> {
    let ram = instance.ram_gb.unwrap_or(default_ram);
    let fixes = {
        let instance = instance.clone();
        tauri::async_runtime::spawn_blocking(move || diagnostico::known_fixes(&instance, ram)).await.unwrap_or_default()
    };
    let mut done = Vec::new();
    for (label, fix) in fixes {
        let result = match fix {
            diagnostico::Fix::InstallProject { project, title } => install_project(instance, &project, &title, progress.clone()).await,
            diagnostico::Fix::SetRam { gb } => {
                instances::update(instance, &instance.name, Some(gb.clamp(1, 64)), &instance.jvm_args).map(|_| t!("La instancia usa {gb} GB, lo que pide el modpack para este PC.", "The instance uses {gb} GB, what the modpack asks for on this PC."))
            }
            _ => continue,
        };
        match result {
            Ok(message) => done.push(message),
            Err(error) => done.push(t!("No se pudo: {label} ({error})", "Could not: {label} ({error})")),
        }
    }
    done
}

/// Comprueba la huella de cada archivo del modpack y repone lo que falte o esté dañado.
#[tauri::command]
async fn verificar_instancia(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<integridad::Report, String> {
    let instance = instance_or_error(&state, &id)?;
    if state.games.lock().unwrap().contains_key(&id) {
        return Err(t!("Cierra el juego antes de verificar sus archivos.", "Close the game before checking its files."));
    }
    let cached = state.catalog.lock().unwrap().clone();
    Ok(ensure_complete(&instance, &state.paths, &state.config, cached, true, progress_emitter(&app, "verificar")).await)
}

/// Instala un data pack de Modrinth dentro de un mundo de la instancia.
#[tauri::command]
async fn instalar_datapack(state: State<'_, AppState>, id: String, mundo: String, proyecto: String, version: Option<String>) -> Result<String, String> {
    let instance = instance_or_error(&state, &id)?;
    let (title, number) = modrinth::install_datapack(&instance, &mundo, &proyecto, version.as_deref()).await?;
    Ok(t!("Se instaló {title} {number} en el mundo «{mundo}».", "{title} {number} was installed in the world «{mundo}»."))
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
        .ok_or(t!("Ese paquete ya no está en el catálogo. Recárgalo.", "That package is no longer in the catalog. Reload it."))?;
    let list = instances::list(&state.paths);
    if let Some((instance, _)) = catalog::installed_instance(&entry.manifest, &list) {
        if state.games.lock().unwrap().contains_key(&instance.id) {
            return Err(t!("Cierra el juego antes de actualizar este modpack.", "Close the game before updating this modpack."));
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

/// Cierra Parabellum del todo (desde la ventana de la actualización obligatoria).
#[tauri::command]
fn cerrar_parabellum(app: AppHandle) {
    app.exit(0);
}

/// Descarga la versión nueva, reemplaza el exe y reinicia Parabellum.
#[tauri::command]
async fn aplicar_actualizacion(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let catalog = catalog_snapshot(&state, false).await;
    let entry = catalog::launcher_update(&catalog.entries).ok_or(t!("No hay una versión nueva.", "There is no new version."))?.clone();
    let exe = catalog::apply_launcher_update(&entry, &state.paths, progress_emitter(&app, "actualizacion")).await?;
    std::process::Command::new(&exe).arg("--reiniciar").spawn().map_err(|e| t!("Se actualizó, pero no se pudo reabrir: {e}", "It was updated, but could not be reopened: {e}"))?;
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
    let title = t!("Carpeta de tus paquetes para Drive", "Your Drive packages folder");
    Ok(export_folder(&app, &state.paths, true, &title).await?.map(|p| p.display().to_string()))
}

/// La carpeta "paquetes" junto a Parabellum (zips copiados de una USB, por ejemplo).
#[tauri::command]
fn abrir_carpeta_paquetes(state: State<AppState>) -> Result<(), String> {
    let folder = state.paths.root.join(catalog::LOCAL_FOLDER);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    plataforma::open_path(&folder)
}

// ================= Hostear partida

#[tauri::command]
fn hosting_estado(app: AppHandle) -> hosting::HostView {
    hosting::view(&app)
}

/// Vincula playit.gg (baja su agente la primera vez y abre la página para aceptar).
#[tauri::command]
async fn conectar_playit(app: AppHandle) -> Result<hosting::HostView, String> {
    hosting::connect_playit(&app, progress_emitter(&app, "playit")).await?;
    Ok(hosting::view(&app))
}

/// Vuelve a dejar listo playit.gg (tras un error, o tras verificar el correo en playit.gg).
#[tauri::command]
async fn reintentar_playit(app: AppHandle) -> Result<(), String> {
    hosting::recheck_playit(&app).await
}

#[tauri::command]
fn cancelar_playit(app: AppHandle) {
    hosting::cancel_claim(&app);
}

#[tauri::command]
async fn desconectar_playit(app: AppHandle) -> Result<hosting::HostView, String> {
    hosting::disconnect_playit(&app).await;
    Ok(hosting::view(&app))
}

/// Compartir también por playit.gg el mundo abierto en LAN (Ajustes).
#[tauri::command]
fn compartir_mundo(app: AppHandle, activo: bool) {
    hosting::set_world_playit(&app, activo);
}

/// Por dónde pueden entrar los amigos a un servidor: P2P, red local y playit.gg.
#[tauri::command]
async fn conexiones_servidor(app: AppHandle, id: String, conexiones: servidor::Connections) -> Result<hosting::HostView, String> {
    hosting::set_connections(&app, &id, conexiones).await?;
    Ok(hosting::view(&app))
}

/// Instancias con e4mc (u otro mod de túnel) activado: con playit.gg sobran.
#[tauri::command]
fn mods_de_tunel(state: State<AppState>) -> Vec<servidor::TunnelModInstance> {
    servidor::instances_with_tunnel_mods(&state.paths)
}

#[tauri::command]
fn preparar_servidor(app: AppHandle, instancia: String) -> Result<hosting::Preparation, String> {
    hosting::prepare(&app, &instancia)
}

#[tauri::command]
async fn crear_servidor(app: AppHandle, state: State<'_, AppState>, opciones: servidor::NewServer) -> Result<String, String> {
    let instance = instance_or_error(&state, &opciones.instance)?;
    let host = state.settings.lock().unwrap().username.clone();
    let config = servidor::create(opciones, instance, state.paths.clone(), host, progress_emitter(&app, "servidor")).await?;
    hosting::emit_view(&app);
    Ok(config.id)
}

#[tauri::command]
fn iniciar_servidor(app: AppHandle, id: String) -> Result<(), String> {
    hosting::start_server(&app, &id)
}

#[tauri::command]
fn detener_servidor(app: AppHandle, id: String) {
    hosting::stop_server(&app, &id);
}

#[tauri::command]
fn reiniciar_servidor(app: AppHandle, id: String) {
    hosting::restart_server(&app, &id);
}

/// Las opciones del servidor (server.properties, reglas del juego y las de Parabellum).
#[tauri::command]
fn opciones_servidor(state: State<AppState>, id: String) -> Result<opciones::OptionsView, String> {
    let config = servidor::find(&state.paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
    Ok(opciones::read(&config, system_ram_gb()))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OptionChanged {
    option: Option<opciones::OptionView>,
    /// Ya se aplicó en el servidor encendido.
    applied: bool,
    /// Se aplica al reiniciar el servidor.
    needs_restart: bool,
}

/// Cambia una opción y, con el servidor encendido, la aplica si se puede.
#[tauri::command]
async fn cambiar_opcion_servidor(app: AppHandle, id: String, opcion: String, valor: serde_json::Value) -> Result<OptionChanged, String> {
    let paths = app.state::<AppState>().paths.clone();
    let mut config = servidor::find(&paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
    let running = hosting::is_running(&app, &id);
    let change = opciones::change(&mut config, &opcion, &valor, running, system_ram_gb())?;
    let applied = !change.commands.is_empty() && hosting::is_ready(&app, &id);
    if applied {
        for command in &change.commands {
            hosting::send_command(&app, &id, command);
        }
    }
    if change.needs_restart {
        hosting::needs_restart(&app, &id);
    } else {
        hosting::emit_view(&app);
    }
    if opcion == "name" {
        hosting::republish(&app, &id).await;
    }
    Ok(OptionChanged { option: change.option, applied, needs_restart: change.needs_restart })
}

/// Pone (PNG de 64×64 como data URL) o quita el icono del servidor.
#[tauri::command]
fn icono_servidor(app: AppHandle, state: State<AppState>, id: String, datos: Option<String>) -> Result<(), String> {
    let config = servidor::find(&state.paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
    opciones::set_icon(&config, datos.as_deref())?;
    if hosting::is_running(&app, &id) {
        hosting::needs_restart(&app, &id);
    }
    Ok(())
}

#[tauri::command]
fn comando_servidor(app: AppHandle, id: String, texto: String) {
    hosting::send_command(&app, &id, &texto);
}

#[tauri::command]
fn consola_servidor(app: AppHandle, id: String) -> Vec<String> {
    hosting::console(&app, &id)
}

#[tauri::command]
fn ajustes_servidor(state: State<AppState>, id: String) -> Result<servidor::Settings, String> {
    let config = servidor::find(&state.paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
    Ok(servidor::read_settings(&config))
}

#[tauri::command]
fn guardar_servidor(app: AppHandle, state: State<AppState>, id: String, ajustes: servidor::Settings) -> Result<(), String> {
    let mut config = servidor::find(&state.paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
    servidor::apply_settings(&mut config, &ajustes)?;
    hosting::emit_view(&app);
    Ok(())
}

/// Guarda los invitados. Devuelve un aviso si no se pudo avisar a los amigos por Firebase.
#[tauri::command]
async fn invitados_servidor(app: AppHandle, id: String, invitados: Vec<servidor::Guest>) -> Result<Option<String>, String> {
    hosting::set_guests(&app, &id, invitados).await
}

#[tauri::command]
async fn eliminar_servidor(app: AppHandle, id: String) -> Result<(), String> {
    hosting::delete_server(&app, &id).await
}

#[tauri::command]
async fn sincronizar_servidor(app: AppHandle, id: String) -> Result<usize, String> {
    hosting::sync_server(&app, &id).await
}

#[tauri::command]
fn carpeta_servidor(state: State<AppState>, id: String) -> Result<(), String> {
    let config = servidor::find(&state.paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
    plataforma::open_path(&config.folder)
}

/// Arreglo del doctor para un servidor. Instalar un mod va a la instancia (los jugadores también
/// lo necesitan) y después se copia al servidor.
#[tauri::command]
async fn arreglar_servidor(app: AppHandle, state: State<'_, AppState>, id: String, arreglo: diagnostico::Fix) -> Result<String, String> {
    if let diagnostico::Fix::InstallMod { .. } = arreglo {
        let config = servidor::find(&state.paths, &id).ok_or(t!("Ese servidor ya no existe.", "That server no longer exists."))?;
        let instance = instance_or_error(&state, &config.instance)?;
        let cached = state.catalog.lock().unwrap().clone();
        let done = apply_fix(&instance, &state.paths, &state.config, cached, arreglo, progress_emitter(&app, "doctor")).await?;
        hosting::sync_server(&app, &id).await?;
        return Ok(done);
    }
    hosting::apply_server_fix(&app, &id, arreglo).await
}

/// Quita de tu lista un servidor al que te invitó un amigo.
#[tauri::command]
async fn salir_de_servidor(app: AppHandle, anfitrion: String, clave: String) -> Result<(), String> {
    let social = cuenta::social_handle(&app).ok_or(t!("Inicia sesión con Google.", "Sign in with Google."))?;
    social.leave_server(&anfitrion, &clave).await
}

/// Prepara la carpeta _parabellum con esta versión para subirla a Drive. None si se canceló.
#[tauri::command]
async fn publicar_launcher(app: AppHandle, state: State<'_, AppState>, notas: String, elegir: bool) -> Result<Option<String>, String> {
    let title = t!("Carpeta de tus paquetes para Drive (donde va la carpeta _parabellum)", "Your Drive packages folder (where the _parabellum folder goes)");
    let Some(folder) = export_folder(&app, &state.paths, elegir, &title).await? else { return Ok(None) };
    let result = tauri::async_runtime::spawn_blocking(move || catalog::publish_launcher(&folder, &notas)).await.map_err(|e| e.to_string())??;
    Ok(Some(result.display().to_string()))
}

// ================= Publicar en el Drive del catálogo

/// El permiso de Drive para publicar, si se dio en este PC (se recupera la primera vez).
async fn drive_publisher(state: &AppState) -> Result<Option<Arc<drive::Publisher>>, String> {
    let mut slot = state.drive_publisher.lock().await;
    if slot.is_none() {
        if let Some(refresh) = drive::load(&state.paths) {
            *slot = Some(Arc::new(drive::Publisher::resume(&state.config, refresh).await?));
        }
    }
    Ok(slot.clone())
}

/// La cuenta con la que se publica en Drive (None si todavía no se conectó).
#[tauri::command]
async fn drive_publicar(state: State<'_, AppState>) -> Result<Option<String>, String> {
    Ok(drive_publisher(&state).await?.map(|p| p.email().to_string()))
}

/// Pide (en el navegador) el permiso de escribir en Drive. Propone la cuenta con la que ya se inició sesión.
#[tauri::command]
async fn conectar_drive(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    if !state.config.has_drive() {
        return Err(t!("Parabellum no tiene configurada la carpeta de Drive del catálogo.", "Parabellum has no catalog Drive folder configured."));
    }
    let hint = cuenta::google_email(&app);
    let publisher = drive::Publisher::connect(&state.config, hint.as_deref()).await?;
    drive::save(&state.paths, publisher.refresh_token())?;
    let email = publisher.email().to_string();
    *state.drive_publisher.lock().await = Some(Arc::new(publisher));
    Ok(email)
}

#[tauri::command]
async fn desconectar_drive(state: State<'_, AppState>) -> Result<(), String> {
    drive::clear(&state.paths);
    *state.drive_publisher.lock().await = None;
    Ok(())
}

/// Los archivos en el orden en que se suben: los .json al final, para que el catálogo no anuncie un
/// paquete cuyo .zip todavía se está subiendo.
fn upload_order(mut files: Vec<PathBuf>) -> Vec<PathBuf> {
    files.sort_by_key(|f| (f.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")), f.clone()));
    files
}

fn drive_result(uploaded: usize, same: usize) -> String {
    match (uploaded, same) {
        (0, _) => t!("Ya estaba igual en Drive: no hubo que subir nada.", "It was already the same in Drive: nothing had to be uploaded."),
        (n, 0) => t!("Listo: se subieron {n} archivos a Drive. Tus compañeros ya lo ven en el catálogo.", "Done: {n} files were uploaded to Drive. Your classmates already see it in the catalog."),
        (n, m) => t!(
            "Listo: se subieron {n} archivos a Drive ({m} ya estaban iguales). Tus compañeros ya lo ven en el catálogo.",
            "Done: {n} files were uploaded to Drive ({m} were already the same). Your classmates already see it in the catalog."
        ),
    }
}

/// Sube a Drive el paquete recién exportado: el modpack (con su icono) y Minecraft de esa versión
/// (si ya estaba igual en Drive, no se vuelve a subir).
#[tauri::command]
async fn subir_paquete_drive(app: AppHandle, state: State<'_, AppState>, modpack: String, base: String) -> Result<String, String> {
    let root = remembered_export_folder(&state.paths).ok_or(t!("No hay carpeta de paquetes para Drive.", "There is no Drive packages folder."))?;
    let (modpack, base) = (PathBuf::from(modpack), PathBuf::from(base));
    // Solo lo que está en la carpeta de paquetes.
    if ![&modpack, &base].iter().all(|f| f.is_file() && f.starts_with(&root)) {
        return Err(t!("El paquete no está en la carpeta de paquetes para Drive.", "The package is not in the Drive packages folder."));
    }
    let publisher = drive_publisher(&state).await?.ok_or(t!("Primero conecta Drive.", "Connect Drive first."))?;
    let mut files = drive::files_in(modpack.parent().unwrap_or(&root));
    files.push(base.clone());
    if base.with_extension("json").is_file() {
        files.push(base.with_extension("json"));
    }
    let (uploaded, same) = publisher.publish(&root, &upload_order(files), progress_emitter(&app, "drive")).await?;
    *state.catalog.lock().unwrap() = None;
    Ok(drive_result(uploaded, same))
}

/// Sube a Drive la carpeta _parabellum que dejó «Publicar esta versión» y quita de ahí las versiones
/// anteriores (a la papelera de Drive). Quien abra Parabellum se actualizará.
#[tauri::command]
async fn subir_launcher_drive(app: AppHandle, state: State<'_, AppState>, carpeta: String) -> Result<String, String> {
    let root = remembered_export_folder(&state.paths).ok_or(t!("No hay carpeta de paquetes para Drive.", "There is no Drive packages folder."))?;
    let folder = PathBuf::from(carpeta);
    if !folder.is_dir() || folder.parent() != Some(root.as_path()) {
        return Err(t!("La carpeta no está en la carpeta de paquetes para Drive.", "The folder is not in the Drive packages folder."));
    }
    let publisher = drive_publisher(&state).await?.ok_or(t!("Primero conecta Drive.", "Connect Drive first."))?;
    let files = upload_order(drive::files_in(&folder));
    let (uploaded, same) = publisher.publish(&root, &files, progress_emitter(&app, "drive")).await?;
    let keep: Vec<String> = files.iter().filter_map(|f| f.file_name()).map(|n| n.to_string_lossy().to_string()).collect();
    let name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let removed = publisher.remove_old_launchers(&name, &keep).await?;
    *state.catalog.lock().unwrap() = None;
    let mut message = drive_result(uploaded, same);
    if removed > 0 {
        message += &t!(" Las versiones anteriores se fueron a la papelera de Drive.", " The previous versions went to the Drive trash.");
    }
    Ok(message)
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
        Some(id) => instances::find(&state.paths, &id).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?.game_dir,
        None => state.paths.root.clone(),
    };
    plataforma::open_path(&folder)
}

fn system_ram_gb() -> u32 {
    match plataforma::system_memory().0 {
        0 => 8,
        total => (total as f64 / 1073741824.0).round() as u32,
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
///   --verificar <instancia> [rapido]                comprueba y repone los archivos del modpack
///   --diagnosticar <instancia>                      qué falla (revisión de mods y último informe de fallo)
///   --arreglar <instancia>                          prueba el arranque y aplica arreglos hasta que abra
///   --crear-servidor <instancia> [nombre] [mundo]   servidor dedicado a partir de la instancia (sin aceptar el EULA)
///   --probar-servidor <servidor> [seg]              lo enciende, espera a que esté listo y lo apaga
///   --playit                                        estado de la cuenta y del túnel de playit.gg
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
            let changed = instances::find(paths, &instance.id).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?;
            Ok(format!("{}: {} {} ({})", changed.name, changed.loader, changed.loader_version.unwrap_or_default(), changed.version_id))
        }
        "--agregar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let sources: Vec<PathBuf> = args[2..].iter().map(PathBuf::from).collect();
            Ok(format!("{:#?}", content::add(&instance, &sources, None)))
        }
        "--abrir-juego" => {
            // Abre el juego (con el mod de Parabellum) y lo deja abierto unos segundos; para probar el
            // panel dentro del juego sin que se cierre al llegar al menú.
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let seconds = args.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(120);
            // «sin-mod»: igual pero sin el mod (para comparar cuánto consume).
            let with_mod = args.get(3).map(String::as_str) != Some("sin-mod");
            let mut plan = launch::plan(&instance, settings, paths)?;
            // Con una dirección, entra directo a ese servidor (para probar el mod dentro de un mundo).
            if let Some(address) = args.get(4) {
                plan.join_server(address, &instance.minecraft_version);
            }
            juego::write_fonts(paths);
            plan.add_jvm_arguments([format!("-Dparabellum.fuentes={}", paths.data.join("fuentes").display())]);
            juego::install(&instance, with_mod);
            let mut child = launch::start(&plan, &instance)?;
            let started = std::time::Instant::now();
            while started.elapsed().as_secs() < seconds {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            let closed = child.try_wait().ok().flatten();
            launch::kill(child.id());
            let _ = child.wait();
            launch::close_helpers(&instance);
            juego::remove(&instance);
            Ok(match closed {
                Some(status) => format!("se cerró solo a los {} s ({status})", started.elapsed().as_secs()),
                None => format!("abierto {} s", started.elapsed().as_secs()),
            })
        }
        "--probar-juego" => {
            // Abre el juego y espera a que llegue al menú principal (o falle); luego lo cierra.
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let limit = args.get(2).and_then(|s| s.parse::<u64>().ok()).unwrap_or(180);
            let mut plan = launch::plan(&instance, settings, paths)?;
            let port = tauri::async_runtime::block_on(sesion_local::start(paths.logs.join("sesion-local.log"))).map_err(|e| e.to_string())?;
            plan.add_jvm_arguments(sesion_local::jvm_arguments(port));
            juego::write_fonts(paths);
            plan.add_jvm_arguments([format!("-Dparabellum.fuentes={}", paths.data.join("fuentes").display())]);
            juego::install(&instance, settings.game_panel);
            let started_ms = chrono::Utc::now().timestamp_millis();
            let started = std::time::Instant::now();
            let mut child = launch::start(&plan, &instance)?;
            let log = instance.game_dir.join("logs").join("latest.log");
            let mut out = format!("{} ({} {})\n", instance.name, instance.loader, instance.version_id);
            // Igual que «Probar y arreglar»: espera a que cargue y a que aguante sin caerse.
            let ram_gb = instance.ram_gb.unwrap_or(settings.ram_gb);
            let outcome = diagnostico::wait_for_menu(&mut child, &instance, started_ms, limit);
            launch::close_helpers(&instance);
            juego::remove(&instance);
            match outcome {
                Ok(_) => {
                    out += &format!("MENÚ PRINCIPAL en {:.0} s\n", started.elapsed().as_secs_f64());
                    for finding in diagnostico::in_game_warnings(&instance, started_ms, ram_gb).findings {
                        out += &format!("  AVISO EN EL JUEGO: {} — {}\n", finding.title, finding.detail);
                    }
                }
                Err(code) => {
                    out += &format!("NO LLEGÓ AL MENÚ (código {code:?}) en {:.0} s\n", started.elapsed().as_secs_f64());
                    let context = diagnostico::CrashContext { since_ms: started_ms, exit_code: code, ram_gb, system_ram_gb: system_ram_gb() };
                    for finding in diagnostico::after_crash(&instance, &context).findings {
                        out += &format!("  {}: {} — {}\n", finding.severity, finding.title, finding.detail);
                    }
                }
            }
            // Lo que dijo el loader sobre los mods
            let text = String::from_utf8_lossy(&std::fs::read(&log).unwrap_or_default()).to_string();
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
        "--probar-sonido" => {
            // Toca el sonido de mensaje nuevo (y lo guarda como .wav si se da una ruta).
            if let Some(path) = args.get(1) {
                sonido::save(std::path::Path::new(path)).map_err(|e| e.to_string())?;
            }
            Ok(if sonido::message(true) { "Sonó.".into() } else { "Windows no pudo tocarlo.".into() })
        }
        "--probar-aviso" => {
            // Muestra un aviso de Windows y dice si llegó al centro de notificaciones.
            avisos::init_thread();
            avisos::register(&paths.data.join("cache"));
            let before = avisos::history_count().unwrap_or(0);
            let title = args.get(1).map(String::as_str).unwrap_or("Parabellum");
            let body = args.get(2).map(String::as_str).unwrap_or("Así se ven los avisos de tus amigos.");
            avisos::show(title, body, false, || {}).map_err(|e| format!("No se pudo mostrar el aviso: {e}"))?;
            std::thread::sleep(std::time::Duration::from_secs(2));
            let after = avisos::history_count().unwrap_or(0);
            Ok(format!("Avisos de Parabellum en el centro de notificaciones: {before} -> {after}"))
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
                settings.ram_gb,
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
                let result = import_modpack(&download.file, paths, Some(download.title.clone()), icon, source, &HashSet::new(), settings.ram_gb, |_, _| {}).await;
                let _ = std::fs::remove_file(&download.file);
                result
            })?;
            let mut out = format!("{} -> carpeta {:?} en {:.0} s
", result.name, result.id, started.elapsed().as_secs_f64());
            if let Some(warning) = &result.warning {
                out += &format!("AVISO: {warning}
");
            }
            for note in &result.notes {
                out += &format!("ARREGLO: {note}
");
            }
            Ok(out)
        }
        "--revisar" => {
            // La revisión de antes de jugar (la que abre el doctor si encuentra algo que impide abrir).
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let diagnosis = diagnostico::check(&instance);
            let mut out = format!("{} mods revisados
", diagnosis.checked_mods);
            for finding in &diagnosis.findings {
                out += &format!("{}: {} — {} [{}]
", finding.severity, finding.title, finding.detail, finding.evidence.as_deref().unwrap_or(""));
            }
            Ok(out)
        }
        "--crear-servidor" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let new = servidor::NewServer {
                instance: instance.id.clone(),
                name: args.get(2).cloned().unwrap_or_default(),
                world: args.get(3).cloned(),
                seed: String::new(),
                ram_gb: 3,
                jvm_args: Vec::new(),
                eula: false,
                gamemode: "survival".into(),
                difficulty: "normal".into(),
                max_players: 10,
                pvp: true,
            };
            let started = std::time::Instant::now();
            let config = tauri::async_runtime::block_on(servidor::create(new, instance, paths.clone(), settings.username.clone(), |stage, fraction| eprintln!("{:>3.0}% {stage}", fraction * 100.0)))?;
            Ok(format!("{}\nmods en el servidor: {}\nen {:.0} s", serde_json::to_string_pretty(&config).unwrap(), servidor::mod_count(&config.folder), started.elapsed().as_secs_f64()))
        }
        "--probar-servidor" => {
            use std::io::BufRead;
            let config = servidor::find(paths, args.get(1).ok_or("Falta el servidor.")?).ok_or("No existe ese servidor.")?;
            let limit: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(600);
            servidor::write_eula(&config.folder, config.eula)?;
            servidor::write_access_lists(&config, &settings.username)?;
            let started = std::time::Instant::now();
            let since = chrono::Utc::now().timestamp_millis();
            let mut child = servidor::command(&config, paths)?.spawn().map_err(|e| e.to_string())?;
            proceso::tie_to_launcher(&child);
            let mut stdin = child.stdin.take();
            let (sender, receiver) = std::sync::mpsc::channel::<String>();
            for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as _)].into_iter().flatten() {
                let sender = sender.clone();
                std::thread::spawn(move || {
                    for line in std::io::BufReader::new(stream).lines().map_while(Result::ok) {
                        let _ = sender.send(line);
                    }
                });
            }
            drop(sender);
            let mut last = std::collections::VecDeque::new();
            let mut ready = None;
            while started.elapsed().as_secs() < limit {
                match receiver.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(line) => {
                        if servidor::parse_line(&line) == Some(servidor::LineEvent::Ready) {
                            ready = Some(started.elapsed().as_secs_f64());
                            break;
                        }
                        last.push_back(line);
                        if last.len() > 12 {
                            last.pop_front();
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => break,
                }
            }
            let report = match ready {
                Some(seconds) => {
                    if let Some(stdin) = stdin.as_mut() {
                        use std::io::Write;
                        let _ = stdin.write_all(b"stop\n");
                        let _ = stdin.flush();
                    }
                    let _ = child.wait();
                    format!("LISTO en {seconds:.0} s con {} mods", servidor::mod_count(&config.folder))
                }
                None => {
                    let _ = child.kill();
                    let code = child.wait().ok().and_then(|s| s.code());
                    let context = diagnostico::CrashContext { since_ms: since, exit_code: code, ram_gb: config.ram_gb, system_ram_gb: system_ram_gb() };
                    let diagnosis = diagnostico::after_crash(&config.as_instance(), &context);
                    format!("NO ARRANCÓ (código {code:?})\n{}\n{}", last.into_iter().collect::<Vec<_>>().join("\n"), serde_json::to_string_pretty(&diagnosis).unwrap())
                }
            };
            Ok(report)
        }
        "--probar-receta" => {
            // Comparte una instancia (su receta, como por el chat) y la vuelve a armar como otra instancia.
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            tauri::async_runtime::block_on(async move {
                let recipe = compartir::recipe(&instance).await?;
                let text = compartir::encode(&recipe)?;
                let decoded = compartir::decode(&text).ok_or("La receta no se pudo leer de vuelta.")?;
                let mut report = format!("receta ({} letras): {}\n", text.chars().count(), text);
                if decoded.source != "mods" {
                    report += &format!("viene de «{}»: se instala desde ahí\n", decoded.source);
                    return Ok::<String, String>(report);
                }
                let copy = install_recipe_mods(&decoded, paths, |_, _| {}).await?;
                let original = modrinth::identify(&instance).await;
                let made = modrinth::identify(&copy).await;
                let ids = |files: &[modrinth::InstalledFile]| {
                    let mut ids: Vec<String> = files.iter().filter(|f| !f.file.ends_with(".disabled")).filter_map(|f| f.version_id.clone()).collect();
                    ids.sort();
                    ids
                };
                report += &format!(
                    "armada: «{}» ({} {} {})\noriginal: {} archivos de Modrinth · copia: {} · iguales: {}\nno viajaron (no son de Modrinth): {}",
                    copy.name,
                    copy.minecraft_version,
                    copy.loader,
                    copy.loader_version.clone().unwrap_or_default(),
                    ids(&original).len(),
                    ids(&made).len(),
                    ids(&original) == ids(&made),
                    decoded.left_out
                );
                Ok(report)
            })
        }
        "--ping" => {
            // Estado de un servidor como lo ve la lista de Multijugador (icono, mensaje, jugadores).
            let address = args.get(1).ok_or("Falta la dirección.")?.clone();
            let status = tauri::async_runtime::block_on(ping::status(&address))?;
            let favicon = status.favicon.as_ref().map(|f| format!("{} letras", f.len())).unwrap_or_else(|| "sin icono".into());
            Ok(format!(
                "{address}: {}/{} jugadores · {} · {} ms · icono: {favicon}\nmensaje: {:?}\n{}",
                status.players,
                status.max_players,
                status.version,
                status.latency_ms.unwrap_or(0),
                status.motd,
                status.sample.join(", ")
            ))
        }
        "--probar-p2p" => {
            // Dos extremos P2P en este PC: un «anfitrión» que deja pasar al puerto dado y un «invitado»
            // que abre el túnel y le hace ping como la lista de Multijugador. Con segundos, el túnel
            // queda abierto ese tiempo (para entrar con el juego a 127.0.0.1:puerto; ver p2p.log).
            let port: u16 = args.get(1).and_then(|p| p.parse().ok()).ok_or("Falta el puerto del servidor.")?;
            let seconds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            p2p::log_to(paths.logs.join("p2p.log"));
            tauri::async_runtime::block_on(async move {
                let host = p2p::P2p::start(Arc::new(move |_: &str, target: &str| (target == "world").then_some(port).ok_or_else(|| "solo «world»".to_string()))).await?;
                let guest = p2p::P2p::start(Arc::new(|_: &str, _: &str| Err("invitado".to_string()))).await?;
                if !host.online(std::time::Duration::from_secs(15)).await {
                    return Err("Sin relevo (¿sin internet?).".to_string());
                }
                let address = host.address().ok_or("Sin dirección P2P.")?;
                let link = guest.open(&address, "world").await?;
                let status = ping::status(&format!("127.0.0.1:{}", link.port)).await?;
                let mut report = format!(
                    "{} · túnel en 127.0.0.1:{} · {} · ida y vuelta {} ms · ping del servidor {} ms · {}/{} jugadores · {}",
                    address,
                    link.port,
                    if link.direct { "directo" } else { "relevo" },
                    link.rtt_ms.unwrap_or(0),
                    status.latency_ms.unwrap_or(0),
                    status.players,
                    status.max_players,
                    status.version
                );
                if seconds > 0 {
                    let _ = std::fs::write(paths.logs.join("p2p-puerto.txt"), format!("{}
{address}", link.port));
                    tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;
                    report.push_str(&format!("
abierto {seconds} s"));
                }
                guest.close().await;
                host.close().await;
                Ok(report)
            })
        }
        "--convertir-imagen" => {
            // Abre una imagen con Windows y la guarda como PNG (lo que hace el chat con HEIC, TIFF…).
            let input = args.get(1).ok_or("Falta la imagen.")?;
            let output = args.get(2).ok_or("Falta dónde guardar el PNG.")?;
            let png = imagen::to_png(&std::fs::read(input).map_err(|e| e.to_string())?)?;
            std::fs::write(output, &png).map_err(|e| e.to_string())?;
            Ok(format!("PNG de {} bytes", png.len()))
        }
        "--probar-favoritos" => {
            // Con la sesión guardada: lee los favoritos de la cuenta, escribe uno de prueba (y lo borra) y
            // comprueba que las reglas rechazan lo que no deben.
            let config = config::Config::load(paths);
            let refresh = google::load_session(paths).ok_or("No hay sesión de Google guardada.")?;
            tauri::async_runtime::block_on(async move {
                let account = google::GoogleAccount::resume(&config, refresh).await?;
                let token = account.id_token().await?;
                let session = firebase::Session::sign_in_with_google(&config.firebase_api_key, &token).await?;
                let uid = session.uid.clone();
                let db = Arc::new(firebase::Database::new(&config.firebase_database_url, session));
                let base = format!("favorites/{uid}");
                let before = db.get(&base).await.map_err(|e| e.to_string())?;
                let mut report = format!("favoritos en la cuenta antes: {}\n", if before.is_null() { "(ninguno todavía)".to_string() } else { before.to_string() });
                let seen: Arc<Mutex<Vec<serde_json::Value>>> = Arc::default();
                let log = seen.clone();
                let _listener = db.listen(base.clone(), move |data| log.lock().unwrap().push(data), |error| eprintln!("escucha: {error}"));
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let test = format!("{base}/mods/parabellumPrueba");
                let ok = |result: Result<serde_json::Value, firebase::DbError>| match result {
                    Ok(_) => "aceptado".to_string(),
                    Err(error) => format!("rechazado ({error})"),
                };
                let entry = serde_json::json!({ "title": "Prueba de Parabellum", "kind": "mod", "at": 1, "icon": "https://cdn.modrinth.com/prueba.png" });
                report += &format!("1. escribir un mod favorito válido: {}\n", ok(db.put(&test, &entry).await));
                report += &format!("2. conversación favorita válida: {}\n", ok(db.put(&format!("{base}/chats/d_prueba_parabellum"), &serde_json::json!(true)).await));
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                let heard = seen.lock().unwrap().last().map(|d| d.pointer("/mods/parabellumPrueba/title").is_some() && d.pointer("/chats/d_prueba_parabellum").is_some()).unwrap_or(false);
                report += &format!("3. la escucha (como otro PC) lo vio llegar: {}\n", if heard { "sí" } else { "NO" });
                report += &format!("4. tipo inventado (debe rechazarse): {}\n", ok(db.put(&test, &serde_json::json!({ "title": "x", "kind": "virus", "at": 1 })).await));
                report += &format!("5. icono que no es https (debe rechazarse): {}\n", ok(db.put(&test, &serde_json::json!({ "title": "x", "kind": "mod", "at": 1, "icon": "http://malo" })).await));
                report += &format!("6. campo que no existe (debe rechazarse): {}\n", ok(db.put(&format!("{base}/otra-cosa"), &serde_json::json!(1)).await));
                report += &format!("7. favoritos de OTRA cuenta (debe rechazarse): {}\n", ok(db.put("favorites/otraCuentaCualquiera/chats/x", &serde_json::json!(true)).await));
                report += &format!("8. leer los de OTRA cuenta (debe rechazarse): {}\n", ok(db.get("favorites/otraCuentaCualquiera").await));
                let _ = db.delete(&test).await;
                let _ = db.delete(&format!("{base}/chats/d_prueba_parabellum")).await;
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let after = db.get(&base).await.map_err(|e| e.to_string())?;
                report += &format!("9. borrada la prueba; la cuenta quedó como estaba: {}", if after == before { "sí" } else { "NO" });
                Ok::<String, String>(report)
            })
        }
        "--probar-tiempo-real" => {
            // Cuánto tarda en llegar por la escucha algo que se acaba de escribir (con la sesión guardada).
            let config = config::Config::load(paths);
            let refresh = google::load_session(paths).ok_or("No hay sesión de Google guardada.")?;
            let rounds: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(5);
            tauri::async_runtime::block_on(async move {
                let account = google::GoogleAccount::resume(&config, refresh).await?;
                let token = account.id_token().await?;
                let session = firebase::Session::sign_in_with_google(&config.firebase_api_key, &token).await?;
                let uid = session.uid.clone();
                let db = Arc::new(firebase::Database::new(&config.firebase_database_url, session));
                let path = format!("reads/{uid}/_latencia");
                let seen: Arc<Mutex<Vec<(i64, std::time::Instant)>>> = Arc::default();
                let log = seen.clone();
                let _listener = db.listen(
                    format!("reads/{uid}"),
                    move |data| {
                        if let Some(value) = data["_latencia"].as_i64() {
                            log.lock().unwrap().push((value, std::time::Instant::now()));
                        }
                    },
                    |error| eprintln!("escucha: {error}"),
                );
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                let mut report = String::new();
                for round in 1..=rounds as i64 {
                    let sent = std::time::Instant::now();
                    db.put(&path, &serde_json::json!(round)).await.map_err(|e| e.to_string())?;
                    let wrote = sent.elapsed();
                    let mut arrived = None;
                    for _ in 0..1000 {
                        if let Some((_, at)) = seen.lock().unwrap().iter().find(|(value, _)| *value == round) {
                            arrived = Some(at.duration_since(sent));
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                    report += &format!(
                        "{round}: escribir {} ms · llegar por la escucha {}\n",
                        wrote.as_millis(),
                        arrived.map(|d| format!("{} ms", d.as_millis())).unwrap_or_else(|| "NO LLEGÓ en 10 s".into())
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(700)).await;
                }
                let _ = db.delete(&path).await;
                Ok::<String, String>(report)
            })
        }
        "--playit" => {
            let secret = playit::load_secret(paths).ok_or("playit.gg no está conectado.")?;
            let data = tauri::async_runtime::block_on(playit::run_data(&secret)).map_err(|e| e.explain())?;
            Ok(serde_json::to_string_pretty(&data).unwrap())
        }
        "--diagnosticar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let context = diagnostico::CrashContext { since_ms: 0, exit_code: None, ram_gb: instance.ram_gb.unwrap_or(settings.ram_gb), system_ram_gb: system_ram_gb() };
            let diagnosis = diagnostico::after_crash(&instance, &context);
            Ok(serde_json::to_string_pretty(&diagnosis).unwrap())
        }
        "--arreglar" => {
            let id = args.get(1).ok_or("Falta la instancia.")?.clone();
            let config = config::Config::load(paths);
            let started = std::time::Instant::now();
            let port = tauri::async_runtime::block_on(sesion_local::start(paths.logs.join("sesion-local.log"))).map_err(|e| e.to_string())?;
            let session = sesion_local::jvm_arguments(port);
            let report = tauri::async_runtime::block_on(auto_fix(&id, paths, &config, None, settings, session, |_| {}, |stage, _| eprintln!("{stage}")))?;
            Ok(format!("{}\nen {:.0} s", serde_json::to_string_pretty(&report).unwrap(), started.elapsed().as_secs_f64()))
        }
        "--verificar" => {
            let instance = instances::find(paths, args.get(1).ok_or("Falta la instancia.")?).ok_or("No existe esa instancia.")?;
            let deep = args.get(2).is_none_or(|a| a != "rapido");
            let started = std::time::Instant::now();
            let config = config::Config::load(paths);
            let report = tauri::async_runtime::block_on(ensure_complete(&instance, paths, &config, None, deep, |stage, _| eprintln!("{stage}")));
            Ok(format!(
                "{} archivos revisados en {:.0} s\nRepuestos ({}): {}\nSin reponer ({}, {} mods):\n{}",
                report.checked,
                started.elapsed().as_secs_f64(),
                report.repaired.len(),
                report.repaired.join(", "),
                report.failed.len(),
                report.missing_mods.len(),
                report.failed.join("\n")
            ))
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

enum LauncherCopy {
    /// Este es el único abierto con esta carpeta (el archivo queda bloqueado mientras viva).
    First(std::fs::File),
    /// Ya hay otro abierto.
    Second,
    /// No se pudo saber (sin permiso para crear el candado): se abre igual.
    Unknown,
}

/// Un solo Parabellum por carpeta. Con dos abiertos (uno escondido junto al reloj y otro nuevo), cada
/// uno publicaba su propia dirección P2P en la misma presencia y los amigos quedaban rechazados; además
/// se duplicaban las conexiones a Firebase y el exe viejo no dejaba actualizar. Al reiniciarse tras
/// actualizar, la versión anterior todavía se está cerrando: se espera hasta 20 s.
fn single_instance(paths: &Paths, restarting: bool) -> LauncherCopy {
    let Ok(lock) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(paths.data.join("parabellum.lock")) else {
        return LauncherCopy::Unknown;
    };
    let tries = if restarting { 40 } else { 1 };
    for attempt in 0..tries {
        match lock.try_lock() {
            Ok(()) => return LauncherCopy::First(lock),
            Err(std::fs::TryLockError::WouldBlock) if attempt + 1 < tries => std::thread::sleep(std::time::Duration::from_millis(500)),
            Err(std::fs::TryLockError::WouldBlock) => return LauncherCopy::Second,
            Err(std::fs::TryLockError::Error(_)) => return LauncherCopy::Unknown,
        }
    }
    LauncherCopy::Second
}

fn main() {
    let paths = Paths::detect();
    let _ = paths.ensure_created();
    let mut settings = Settings::load(&paths);
    // La primera vez, Parabellum habla el idioma de Windows; después, el que se elija en Ajustes.
    if settings.language.is_empty() {
        settings.language = idioma::detect().to_string();
        let _ = settings.save(&paths);
    }
    idioma::set(&settings.language);
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
    // Un solo Parabellum por carpeta (se suelta solo al cerrarse el proceso).
    let _instance = match single_instance(&paths, restarting) {
        LauncherCopy::First(lock) => Some(lock),
        LauncherCopy::Unknown => None,
        LauncherCopy::Second => {
            // El que ya está abierto (quizá junto al reloj) ve esta marca y se muestra.
            let _ = std::fs::write(paths.data.join("mostrar-ventana"), b"");
            return;
        }
    };
    catalog::cleanup_previous_version();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Iconos de mods, resource packs y mundos (http://icono.localhost/…): se leen cuando se ven.
        .register_asynchronous_uri_scheme_protocol("icono", |ctx, request, responder| {
            let paths = ctx.app_handle().state::<AppState>().paths.clone();
            let path = request.uri().path().to_string();
            std::thread::spawn(move || {
                let response = match content::icon_for_path(&paths, &path) {
                    Some(bytes) => tauri::http::Response::builder()
                        .header("Content-Type", "image/png")
                        .header("Cache-Control", "max-age=31536000, immutable")
                        .body(bytes),
                    None => tauri::http::Response::builder().status(404).body(Vec::new()),
                };
                responder.respond(response.unwrap());
            });
        })
        .manage(AppState {
            config: config::Config::load(&paths),
            paths,
            settings: Mutex::new(settings),
            games: Mutex::default(),
            stopped: Mutex::default(),
            catalog: Mutex::default(),
            social: cuenta::SocialState::default(),
            hosting: hosting::Hosting::default(),
            drive_publisher: Default::default(),
            p2p: Default::default(),
            puente: Default::default(),
        })
        .setup(|app| {
            // El tamaño de la interfaz que eligió el jugador (Ajustes › Apariencia).
            let scale = app.state::<AppState>().settings.lock().unwrap().ui_scale;
            if (scale - 1.0).abs() > f64::EPSILON {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_zoom(scale.clamp(0.8, 1.5));
                }
            }
            adopt_running_games(app.handle());
            {
                let state = app.state::<AppState>();
                juego::write_fonts(&state.paths);
                let running: Vec<String> = state.games.lock().unwrap().keys().cloned().collect();
                for instance in instances::list(&state.paths).iter().filter(|i| !running.contains(&i.id)) {
                    juego::remove(instance);
                }
            }
            avisos::register(&app.state::<AppState>().paths.data.join("cache"));
            cuenta::create_tray(app.handle())?;
            cuenta::startup(app.handle());
            hosting::init(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            // Con la sesión iniciada, la X deja Parabellum junto al reloj (los amigos te siguen viendo).
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if cuenta::should_hide_on_close(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                    cuenta::hid_to_tray(window.app_handle());
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            estado_inicial,
            guardar_ajustes,
            panel_lateral,
            sonido_mensaje,
            convertir_imagen,
            estado_servidores,
            mejor_ruta,
            conexiones_servidor,
            firewall_servidores,
            abrir_firewall_servidores,
            favorito_chat,
            favorito_mod,
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
            cuenta::estado_social,
            cuenta::iniciar_sesion_google,
            cuenta::cerrar_sesion_google,
            cuenta::agregar_amigo,
            cuenta::aceptar_solicitud,
            cuenta::rechazar_solicitud,
            cuenta::eliminar_amigo,
            cuenta::reconectar_amigos,
            cuenta::chats,
            cuenta::abrir_chat,
            cuenta::cerrar_chat,
            cuenta::marcar_leido,
            cuenta::enviar_mensaje,
            cuenta::enviar_imagen,
            cuenta::imagen_chat,
            cuenta::editar_mensaje,
            cuenta::borrar_mensaje,
            cuenta::chat_con_amigo,
            cuenta::crear_grupo,
            cuenta::agregar_al_grupo,
            cuenta::renombrar_grupo,
            cuenta::salir_del_chat,
            cuenta::leer_imagen,
            instalar_modpack,
            instalar_datapack,
            mostrar_archivo,
            catalogo,
            instalar_de_catalogo,
            verificar_instancia,
            revisar_instancia,
            diagnosticar,
            probar_y_arreglar,
            aplicar_arreglo,
            accesos_directos,
            acceso_directo,
            actualizacion_launcher,
            aplicar_actualizacion,
            carpeta_exportar,
            elegir_carpeta_exportar,
            abrir_carpeta_paquetes,
            publicar_launcher,
            drive_publicar,
            herramientas_creador,
            conectar_drive,
            desconectar_drive,
            subir_paquete_drive,
            subir_launcher_drive,
            registro,
            abrir_registro,
            listar_java,
            abrir_carpeta,
            hosting_estado,
            conectar_playit,
            cancelar_playit,
            reintentar_playit,
            cerrar_parabellum,
            instalar_compartida,
            cuenta::compartir_instancia,
            desconectar_playit,
            compartir_mundo,
            preparar_servidor,
            mods_de_tunel,
            crear_servidor,
            iniciar_servidor,
            detener_servidor,
            reiniciar_servidor,
            opciones_servidor,
            cambiar_opcion_servidor,
            icono_servidor,
            comando_servidor,
            consola_servidor,
            ajustes_servidor,
            guardar_servidor,
            invitados_servidor,
            eliminar_servidor,
            sincronizar_servidor,
            carpeta_servidor,
            arreglar_servidor,
            salir_de_servidor
        ])
        .build(tauri::generate_context!())
        .expect("no se pudo abrir Parabellum")
        .run(|app, event| {
            // Al salir del todo: los amigos te ven desconectado al momento.
            if let tauri::RunEvent::Exit = event {
                // Los servidores guardan el mundo y se apagan antes de salir.
                hosting::shutdown(app);
                cuenta::shutdown(app);
            }
        });
}
