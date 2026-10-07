// Sin ventana de consola en Windows.
#![windows_subsystem = "windows"]

mod content;
mod download;
mod instances;
mod java;
mod launch;
mod mojang;
mod paths;
mod settings;
mod version;

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

use instances::Instance;
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
    Ok(match mojang::list().await {
        Ok(releases) => Versions { mojang: true, releases, installed, message: None },
        Err(message) => Versions { mojang: false, releases: Vec::new(), installed, message: Some(message) },
    })
}

#[tauri::command]
async fn crear_instancia(app: AppHandle, state: State<'_, AppState>, nombre: String, version: String) -> Result<Instance, String> {
    let paths = state.paths.clone();
    instances::folder_for(&nombre, &paths).and_then(|folder| {
        if folder.exists() { Err(format!("Ya tienes una instancia llamada \"{}\".", nombre.trim())) } else { Ok(()) }
    })?;
    let installed = mojang::installed_versions(&paths).contains(&version);
    match mojang::list().await {
        Ok(releases) => {
            let release = releases.into_iter().find(|r| r.id == version).ok_or(format!("Mojang no tiene la versión {version}."))?;
            let emitter = app.clone();
            // Se ejecuta siempre: si ya estaba descargada termina al momento y completa lo que faltara.
            mojang::install(&release, &paths, move |stage, fraction| {
                let _ = emitter.emit("progreso", Progress { task: "crear".into(), stage, fraction });
            })
            .await?;
        }
        Err(message) if !installed => return Err(message),
        Err(_) => {}
    }
    instances::create(&nombre, &version, &paths)
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
    tauri::async_runtime::spawn_blocking(move || content::list(&instance)).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn activar_contenido(state: State<AppState>, id: String, tipo: String, archivo: String, activo: bool) -> Result<String, String> {
    content::set_enabled(&instance_or_error(&state, &id)?, &tipo, &archivo, activo)
}

#[tauri::command]
fn eliminar_contenido(state: State<AppState>, id: String, tipo: String, archivo: String) -> Result<(), String> {
    content::delete(&instance_or_error(&state, &id)?, &tipo, &archivo)
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
///   --crear <nombre> <versión>          descarga de Mojang y crea la instancia
///   --jugar <instancia> [servidor]      abre el juego (y entra al servidor)
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
            let started = std::time::Instant::now();
            tauri::async_runtime::block_on(async {
                let release = mojang::list().await?.into_iter().find(|r| &r.id == version).ok_or("Mojang no tiene esa versión.")?;
                mojang::install(&release, paths, |_, _| {}).await
            })?;
            let instance = instances::create(name, version, paths)?;
            Ok(format!("Creada {} en {:.0} s", instance.id, started.elapsed().as_secs_f64()))
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
            crear_instancia,
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
            abrir_carpeta_contenido,
            registro,
            abrir_registro,
            listar_java,
            abrir_carpeta
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir Parabellum");
}
