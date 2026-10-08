//! Convierte una instancia en el comando de Java exacto y abre el juego sin consola,
//! como un proceso independiente (el launcher se puede cerrar sin cerrar el juego).

use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::instances::Instance;
use crate::java::{self, JavaRuntime};
use crate::paths::Paths;
use crate::settings::Settings;
use crate::version::{rules_allow, Features, Maven, ResolvedVersion};

pub const LAUNCHER_NAME: &str = "Parabellum";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

pub struct LaunchPlan {
    pub arguments: Vec<String>,
    pub java: Option<JavaRuntime>,
    pub missing: Vec<String>,
    pub natives_dir: PathBuf,
    pub native_archives: Vec<PathBuf>,
    /// Posición de la clase principal: antes van los argumentos de Java, después los del juego.
    main_class_index: usize,
}

impl LaunchPlan {
    pub fn can_launch(&self) -> bool {
        self.missing.is_empty() && self.java.is_some()
    }

    pub fn add_jvm_arguments(&mut self, arguments: impl IntoIterator<Item = String>) {
        let list: Vec<String> = arguments.into_iter().collect();
        let count = list.len();
        self.arguments.splice(self.main_class_index..self.main_class_index, list);
        self.main_class_index += count;
    }

    /// Entra directo a un servidor al abrir el juego: Quick Play desde 1.20; antes, --server y --port.
    pub fn join_server(&mut self, address: &str, minecraft_version: &str) {
        if supports_quick_play(minecraft_version) {
            self.arguments.extend(["--quickPlayMultiplayer".to_string(), address.to_string()]);
        } else {
            let (host, port) = address.rsplit_once(':').unwrap_or((address, "25565"));
            self.arguments.extend(["--server", host, "--port", port].map(String::from));
        }
    }
}

fn supports_quick_play(version: &str) -> bool {
    let mut parts = version.split('.');
    match parts.next().and_then(|p| p.parse::<u32>().ok()) {
        Some(1) => parts.next().and_then(|p| p.parse::<u32>().ok()).is_some_and(|minor| minor >= 20),
        Some(_) => true,
        None => true,
    }
}

pub fn plan(instance: &Instance, settings: &Settings, paths: &Paths) -> Result<LaunchPlan, String> {
    let version = ResolvedVersion::resolve(&instance.version_id, paths)?;
    let natives_dir = paths.natives.join(&version.id);
    let mut missing = Vec::new();
    let mut native_archives = Vec::new();
    let features: Features = HashMap::from([
        ("is_demo_user", false),
        ("has_custom_resolution", true),
        ("has_quick_plays_support", false),
        ("is_quick_play_singleplayer", false),
        ("is_quick_play_multiplayer", false),
        ("is_quick_play_realms", false),
    ]);

    // Bibliotecas y classpath
    let mut classpath: Vec<PathBuf> = Vec::new();
    for library in &version.libraries {
        if !rules_allow(library.get("rules"), &features) {
            continue;
        }
        let Some(maven) = library["name"].as_str().and_then(Maven::parse) else { continue };
        if let Some(classifier) = library.pointer("/natives/windows").and_then(Value::as_str) {
            // Formato antiguo (hasta 1.18): los nativos vienen en un .jar que se descomprime.
            let classifier = classifier.replace("${arch}", "64");
            let relative = library
                .pointer(&format!("/downloads/classifiers/{classifier}/path"))
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| Maven { classifier: Some(classifier.clone()), ..maven.clone() }.relative_path());
            let file = paths.libraries.join(relative);
            if file.exists() {
                native_archives.push(file);
            } else {
                missing.push(format!("Biblioteca: {}", file.display()));
            }
            if library.get("downloads").is_some() && library.pointer("/downloads/artifact").is_none() {
                continue;
            }
        }
        let relative = library
            .pointer("/downloads/artifact/path")
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| maven.relative_path());
        let file = paths.libraries.join(relative);
        if !file.exists() {
            missing.push(format!("Biblioteca: {}", file.display()));
        }
        if !classpath.contains(&file) {
            classpath.push(file);
        }
    }
    let client_jar = paths.version_jar(&version.client_jar_id);
    if !client_jar.exists() {
        missing.push(format!("Juego: {}", client_jar.display()));
    }
    classpath.push(client_jar);

    // Assets
    let asset_index = version.asset_index.clone().unwrap_or_else(|| "legacy".into());
    let index_file = paths.assets.join("indexes").join(format!("{asset_index}.json"));
    match std::fs::read_to_string(&index_file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        None => missing.push(format!("Índice de sonidos y texturas {asset_index}")),
        Some(index) => {
            let objects = index["objects"].as_object().map(|o| o.len()).unwrap_or(0);
            let absent = index["objects"]
                .as_object()
                .into_iter()
                .flatten()
                .filter_map(|(_, o)| o["hash"].as_str())
                .filter(|h| h.len() > 2 && !paths.assets.join("objects").join(&h[..2]).join(h).exists())
                .count();
            if absent > 0 {
                missing.push(format!("Faltan {absent} de {objects} sonidos y texturas"));
            }
        }
    }

    // Java
    let runtimes = java::discover(paths);
    let java = java::select(&runtimes, version.java_component.as_deref(), version.java_major).cloned();
    if java.is_none() {
        missing.push(format!("Java {}", version.java_major));
    }

    // Variables de los argumentos
    let classpath_text = classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(";");
    let vars: HashMap<&str, String> = HashMap::from([
        ("auth_player_name", settings.username.clone()),
        ("version_name", version.id.clone()),
        ("game_directory", instance.game_dir.display().to_string()),
        ("assets_root", paths.assets.display().to_string()),
        ("game_assets", paths.assets.display().to_string()),
        ("assets_index_name", asset_index.clone()),
        ("auth_uuid", settings.effective_uuid()),
        ("auth_access_token", "null".into()),
        ("auth_session", "null".into()),
        ("clientid", "null".into()),
        ("auth_xuid", "null".into()),
        ("user_type", "legacy".into()),
        ("user_properties", "{}".into()),
        ("version_type", version.version_type.clone()),
        ("natives_directory", natives_dir.display().to_string()),
        ("launcher_name", LAUNCHER_NAME.into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", classpath_text.clone()),
        ("classpath_separator", ";".into()),
        ("library_directory", paths.libraries.display().to_string()),
        ("resolution_width", settings.window_width.to_string()),
        ("resolution_height", settings.window_height.to_string()),
    ]);
    let expand = |text: &str| -> String {
        let mut result = text.to_string();
        for (key, value) in &vars {
            result = result.replace(&format!("${{{key}}}"), value);
        }
        result
    };
    let add = |target: &mut Vec<String>, source: &[Value]| {
        for node in source {
            match node {
                Value::String(text) => target.push(expand(text)),
                Value::Object(conditional) if rules_allow(conditional.get("rules"), &features) => match &conditional["value"] {
                    Value::String(text) => target.push(expand(text)),
                    Value::Array(values) => target.extend(values.iter().filter_map(Value::as_str).map(expand)),
                    _ => {}
                },
                _ => {}
            }
        }
    };

    // Java (JVM)
    let ram = instance.ram_gb.unwrap_or(settings.ram_gb).max(1);
    let mut arguments = vec![
        "-Xms1G".to_string(),
        format!("-Xmx{ram}G"),
        "-XX:+UseG1GC".into(),
        "-XX:+UnlockExperimentalVMOptions".into(),
        "-XX:G1NewSizePercent=20".into(),
        "-XX:G1ReservePercent=20".into(),
        "-XX:MaxGCPauseMillis=50".into(),
        "-XX:G1HeapRegionSize=32M".into(),
    ];
    arguments.extend(instance.jvm_args.iter().cloned());
    // Las versiones antiguas no traen argumentos de Java (y un loader encima puede traer solo los suyos).
    let declares_classpath = version.jvm_arguments.iter().any(|a| a.to_string().contains("${classpath}"));
    add(&mut arguments, &version.jvm_arguments);
    if !declares_classpath {
        arguments.extend([format!("-Djava.library.path={}", natives_dir.display()), "-cp".into(), classpath_text]);
    }
    if let Some(logging) = version.logging.as_ref().and_then(|l| l.get("client")) {
        if let (Some(id), Some(argument)) = (logging.pointer("/file/id").and_then(Value::as_str), logging["argument"].as_str()) {
            let file = paths.assets.join("log_configs").join(id);
            if file.exists() {
                arguments.push(argument.replace("${path}", &file.display().to_string()));
            }
        }
    }
    let main_class_index = arguments.len();
    arguments.push(version.main_class.clone());

    // Juego
    if !version.game_arguments.is_empty() {
        add(&mut arguments, &version.game_arguments);
    } else if let Some(legacy) = &version.legacy_arguments {
        arguments.extend(legacy.split_whitespace().map(expand));
        arguments.extend(["--width".into(), vars["resolution_width"].clone(), "--height".into(), vars["resolution_height"].clone()]);
    }

    Ok(LaunchPlan { arguments, java, missing, natives_dir, native_archives, main_class_index })
}

/// Abre el juego. La salida de Java va a un archivo (no a una tubería), así el juego no depende del launcher.
/// Devuelve el proceso para poder seguirlo y detenerlo.
pub fn start(plan: &LaunchPlan, instance: &Instance) -> Result<std::process::Child, String> {
    let java = plan.java.as_ref().ok_or("No hay Java para esta versión.")?;
    if !plan.missing.is_empty() {
        return Err(format!("Faltan archivos:\n{}", plan.missing.iter().take(5).cloned().collect::<Vec<_>>().join("\n")));
    }
    extract_natives(&plan.native_archives, &plan.natives_dir)?;
    let state = instance.state_dir();
    std::fs::create_dir_all(&state).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&instance.game_dir).map_err(|e| e.to_string())?;

    // Java lee los argumentos de un archivo: evita el límite de longitud de la línea de comandos de Windows.
    let argfile = state.join("java-argumentos.txt");
    let mut content = String::new();
    for argument in &plan.arguments {
        content.push('"');
        content.push_str(&argument.replace('\\', "\\\\").replace('"', "\\\""));
        content.push_str("\"\n");
    }
    std::fs::write(&argfile, content).map_err(|e| e.to_string())?;

    let output = std::fs::File::create(state.join("salida-java.log")).map_err(|e| e.to_string())?;
    let child = Command::new(java.executable())
        .arg(format!("@{}", argfile.display()))
        .current_dir(&instance.game_dir)
        .stdin(Stdio::null())
        .stdout(output.try_clone().map_err(|e| e.to_string())?)
        .stderr(output)
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("No se pudo abrir Java: {e}"))?;
    let now = chrono::Utc::now().timestamp_millis();
    let _ = std::fs::write(state.join("ultima-partida.txt"), now.to_string());
    let _ = std::fs::write(state.join("juego.pid"), child.id().to_string());
    Ok(child)
}

fn extract_natives(archives: &[PathBuf], target: &Path) -> Result<(), String> {
    if archives.is_empty() {
        return Ok(());
    }
    std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for archive in archives {
        let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
        let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(name) = entry.enclosed_name() else { continue };
            if entry.is_dir() || name.starts_with("META-INF") {
                continue;
            }
            let destination = target.join(name);
            if destination.metadata().is_ok_and(|m| m.len() == entry.size()) {
                continue;
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&destination).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            out.flush().map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// PID de un juego que siga abierto desde antes de abrir el launcher (por el archivo juego.pid).
pub fn running_pid(instance: &Instance) -> Option<u32> {
    let pid = std::fs::read_to_string(instance.state_dir().join("juego.pid")).ok()?.trim().parse::<u32>().ok()?;
    process_alive(pid).then_some(pid)
}

/// Cierra el juego y los procesos que haya abierto.
pub fn kill(pid: u32) {
    let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).creation_flags(CREATE_NO_WINDOW).output();
}

/// Últimas líneas del registro del juego (latest.log; si aún no existe, la salida de Java).
pub fn read_log(instance: &Instance, max_bytes: u64) -> (String, String) {
    let latest = instance.game_dir.join("logs").join("latest.log");
    let java = instance.state_dir().join("salida-java.log");
    // Si Java falló antes de que Minecraft creara su registro, el motivo está en la salida de Java.
    let newest = [&latest, &java]
        .into_iter()
        .filter_map(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()).map(|t| (p, t)))
        .max_by_key(|(_, t)| *t)
        .map(|(p, _)| p.clone());
    let Some(path) = newest else { return (String::new(), String::new()) };
    let text = read_tail(&path, max_bytes);
    (path.file_name().unwrap_or_default().to_string_lossy().to_string(), text)
}

fn read_tail(path: &Path, max_bytes: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else { return String::new() };
    let length = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = length.saturating_sub(max_bytes);
    let _ = file.seek(SeekFrom::Start(start));
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).to_string();
    // Empieza en una línea completa
    if start > 0 { text.split_once('\n').map(|(_, rest)| rest.to_string()).unwrap_or(text) } else { text }
}

/// Explica por qué el juego se cerró con error: el informe de fallo de Minecraft si lo hay,
/// o la última excepción de Java.
pub fn crash_reason(instance: &Instance, since_ms: i64) -> Option<String> {
    let reports = instance.game_dir.join("crash-reports");
    // Sin carpeta de informes se sigue buscando en los registros (no se sale de la función).
    let newest = std::fs::read_dir(&reports)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let modified = e.metadata().ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64;
            (modified >= since_ms).then_some((e.path(), modified))
        })
        .max_by_key(|(_, m)| *m);
    if let Some((path, _)) = newest {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let description = text.lines().find_map(|l| l.strip_prefix("Description: ")).unwrap_or("Minecraft dejó un informe de fallo");
        let exception = text.lines().skip_while(|l| !l.starts_with("Description:")).skip(1).find(|l| !l.trim().is_empty());
        return Some(match exception {
            Some(e) => format!("{description}\n{}", e.trim()),
            None => description.to_string(),
        });
    }
    // Si Minecraft no llegó a escribir su registro, Java falló al arrancar: lo explica su propia salida.
    let modified = |p: &Path| {
        std::fs::metadata(p).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as i64)
    };
    let latest = instance.game_dir.join("logs").join("latest.log");
    if modified(&latest).is_none_or(|m| m < since_ms) {
        let output = read_tail(&instance.state_dir().join("salida-java.log"), 20_000);
        let lines: Vec<&str> = output.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if !lines.is_empty() {
            return Some(lines[lines.len().saturating_sub(4)..].join("\n"));
        }
    }
    None.or_else(|| {
        let (_, log) = read_log(instance, 200_000);
        log.lines()
            .rev()
            .find(|l| l.contains("Exception") || l.contains("Error:") || l.contains("/FATAL]") || l.contains("/ERROR]"))
            .map(|l| l.trim().chars().take(400).collect())
    })
}

fn process_alive(pid: u32) -> bool {
    // tasklist evita depender de APIs de Windows; se llama como mucho cada pocos segundos.
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
        .unwrap_or(false)
}
