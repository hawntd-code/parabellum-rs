// Sin ventana de consola en Windows.
#![windows_subsystem = "windows"]

mod download;
mod instances;
mod java;
mod launch;
mod mojang;
mod paths;
mod settings;
mod version;

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

use instances::Instance;
use paths::Paths;
use settings::Settings;

struct AppState {
    paths: Paths,
    settings: Mutex<Settings>,
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

#[tauri::command]
async fn jugar(state: State<'_, AppState>, id: String, servidor: Option<String>) -> Result<u32, String> {
    let paths = state.paths.clone();
    let settings = state.settings.lock().unwrap().clone();
    if let Some(problem) = settings::validate_username(&settings.username) {
        return Err(format!("{problem} Cámbialo en Ajustes."));
    }
    let instance = instances::find(&paths, &id).ok_or("No se encontró la instancia.")?;
    if launch::is_running(&instance) {
        return Err("Este juego ya está abierto.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut plan = launch::plan(&instance, &settings, &paths)?;
        if let Some(address) = servidor.filter(|s| !s.trim().is_empty()) {
            plan.join_server(address.trim(), &instance.minecraft_version);
        }
        launch::start(&plan, &instance)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn juegos_abiertos(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let paths = state.paths.clone();
    tauri::async_runtime::spawn_blocking(move || {
        instances::list(&paths)
            .into_iter()
            .filter(|i| i.state_dir().join("juego.pid").exists() && launch::is_running(i))
            .map(|i| i.id)
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
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
            Ok(format!("Abierto, PID {}", launch::start(&plan, &instance)?))
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
        .manage(AppState { paths, settings: Mutex::new(settings) })
        .invoke_handler(tauri::generate_handler![
            estado_inicial,
            guardar_ajustes,
            uuid_offline,
            listar_instancias,
            listar_versiones,
            crear_instancia,
            jugar,
            juegos_abiertos,
            listar_java,
            abrir_carpeta
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo abrir Parabellum");
}
