//! Convierte una instancia en el comando de Java exacto y abre el juego sin consola,
//! como un proceso independiente (el launcher se puede cerrar sin cerrar el juego).

use serde_json::Value;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::instances::Instance;
use crate::java::{self, JavaRuntime};
use crate::paths::Paths;
use crate::plataforma::{self, CommandExt as _};
use crate::settings::Settings;
use crate::version::{rules_allow, Features, Maven, ResolvedVersion};

pub const LAUNCHER_NAME: &str = "Parabellum";

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
            // «[2806::1]:25565» (IPv6) o «host:puerto».
            let (host, port) = match address.strip_prefix('[').and_then(|rest| rest.split_once(']')) {
                Some((host, rest)) => (host, rest.strip_prefix(':').unwrap_or("25565")),
                None => address.rsplit_once(':').unwrap_or((address, "25565")),
            };
            self.arguments.extend(["--server", host, "--port", port].map(String::from));
        }
    }
}

fn uses_ignore_list(version: &ResolvedVersion) -> bool {
    version.jvm_arguments.iter().any(|a| a.as_str().is_some_and(|s| s.starts_with("-DignoreList=")))
}

fn supports_quick_play(version: &str) -> bool {
    let mut parts = version.split('.');
    match parts.next().and_then(|p| p.parse::<u32>().ok()) {
        Some(1) => parts.next().and_then(|p| p.parse::<u32>().ok()).is_some_and(|minor| minor >= 20),
        Some(_) => true,
        None => true,
    }
}

/// Rutas de una biblioteca: el .jar de nativos del formato antiguo (hasta 1.18) y el .jar normal, si los tiene.
fn library_paths(library: &Value, paths: &Paths) -> (Option<PathBuf>, Option<PathBuf>) {
    let Some(maven) = library["name"].as_str().and_then(Maven::parse) else { return (None, None) };
    let mut native = None;
    if let Some(classifier) = library.pointer(&format!("/natives/{}", plataforma::MOJANG_OS)).and_then(Value::as_str) {
        let classifier = classifier.replace("${arch}", "64");
        let relative = library
            .pointer(&format!("/downloads/classifiers/{classifier}/path"))
            .and_then(Value::as_str)
            .map(String::from)
            .unwrap_or_else(|| Maven { classifier: Some(classifier.clone()), ..maven.clone() }.relative_path());
        native = Some(paths.libraries.join(relative));
        // Bibliotecas que solo traen nativos
        if library.get("downloads").is_some() && library.pointer("/downloads/artifact").is_none() {
            return (native, None);
        }
    }
    let relative = library.pointer("/downloads/artifact/path").and_then(Value::as_str).map(String::from).unwrap_or_else(|| maven.relative_path());
    (native, Some(paths.libraries.join(relative)))
}

/// Todo lo que usa una versión para jugarse: los JSON de su cadena (loader y vanilla), bibliotecas,
/// el juego, sonidos y texturas y la configuración de registros, más el Java que pide. Sirve para
/// empaquetarla y llevarla a donde Mojang está bloqueado.
pub struct VersionFiles {
    pub files: Vec<PathBuf>,
    pub java: Option<JavaRuntime>,
    /// La versión vanilla de la que hereda (la propia si es vanilla).
    pub root_id: String,
}

pub fn version_files(version_id: &str, paths: &Paths) -> Result<VersionFiles, String> {
    let version = ResolvedVersion::resolve(version_id, paths)?;
    let mut files = Vec::new();
    let mut generated = Vec::new();
    let mut next = Some(version_id.to_string());
    while let Some(id) = next.take().filter(|_| files.len() < 10) {
        let json = paths.version_json(&id);
        let content = std::fs::read_to_string(&json).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
        next = content.as_ref().and_then(|v| v["inheritsFrom"].as_str().filter(|s| !s.is_empty()).map(String::from));
        if let Some(content) = &content {
            generated.extend(crate::forge::generated_files(content, paths));
        }
        files.push(json);
    }
    files.extend(generated);
    let features = Features::new();
    for library in version.libraries.iter().filter(|l| rules_allow(l.get("rules"), &features)) {
        let (native, artifact) = library_paths(library, paths);
        files.extend(native);
        files.extend(artifact);
    }
    files.push(paths.version_jar(&version.client_jar_id));
    let asset_index = version.asset_index.clone().unwrap_or_else(|| "legacy".into());
    let index_file = paths.assets.join("indexes").join(format!("{asset_index}.json"));
    if let Some(index) = std::fs::read_to_string(&index_file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        for hash in index["objects"].as_object().into_iter().flatten().filter_map(|(_, o)| o["hash"].as_str()).filter(|h| h.len() > 2) {
            files.push(paths.assets.join("objects").join(&hash[..2]).join(hash));
        }
    }
    files.push(index_file);
    if let Some(id) = version.logging.as_ref().and_then(|l| l.pointer("/client/file/id")).and_then(Value::as_str) {
        files.push(paths.assets.join("log_configs").join(id));
    }
    let mut seen = std::collections::HashSet::new();
    // La misma ruta puede llegar escrita con / (del JSON) y con \ (de la carpeta): cuenta como una.
    files.retain(|f| seen.insert(f.components().collect::<PathBuf>().to_string_lossy().to_lowercase()));
    let java = java::select(&java::discover(paths), version.java_component.as_deref(), version.java_major).cloned();
    Ok(VersionFiles { files, java, root_id: version.root_id })
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
        let (native, artifact) = library_paths(library, paths);
        if let Some(file) = native {
            if file.exists() {
                native_archives.push(file);
            } else {
                missing.push(format!("Biblioteca: {}", file.display()));
            }
        }
        if let Some(file) = artifact {
            if !file.exists() {
                missing.push(format!("Biblioteca: {}", file.display()));
            }
            if !classpath.contains(&file) {
                classpath.push(file);
            }
        }
    }
    let client_jar = paths.version_jar(&version.client_jar_id);
    if !client_jar.exists() {
        missing.push(t!("Juego: {}", "Game: {}", client_jar.display()));
    }
    // Forge (1.17–1.20) y NeoForge cargan el juego ya parcheado desde libraries y, con -DignoreList,
    // esperan que el .jar original no esté en el classpath (si está, Java lo ve dos veces y no arranca).
    if !uses_ignore_list(&version) {
        classpath.push(client_jar);
    }

    // Assets
    let asset_index = version.asset_index.clone().unwrap_or_else(|| "legacy".into());
    let index_file = paths.assets.join("indexes").join(format!("{asset_index}.json"));
    match std::fs::read_to_string(&index_file).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) {
        None => missing.push(t!("Índice de sonidos y texturas {asset_index}", "Sound and texture index {asset_index}")),
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
                missing.push(t!("Faltan {absent} de {objects} sonidos y texturas", "{absent} of {objects} sounds and textures are missing"));
            }
        }
    }

    // Java
    // La instancia puede pedir otro Java (el doctor lo cambia si un mod necesita uno más nuevo).
    let runtimes = java::discover(paths);
    let (component, major) = match instance.java_major {
        Some(major) => (None, major),
        None => (version.java_component.as_deref(), version.java_major),
    };
    let java = java::select(&runtimes, component, major).filter(|r| instance.java_major.is_none() || r.major == major).cloned();
    if java.is_none() {
        missing.push(format!("Java {major}"));
    }

    // Variables de los argumentos
    let classpath_text = classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(plataforma::CLASSPATH_SEPARATOR);
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
        ("classpath_separator", plataforma::CLASSPATH_SEPARATOR.into()),
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
    let java = plan.java.as_ref().ok_or(t!("No hay Java para esta versión.", "There is no Java for this version."))?;
    if !plan.missing.is_empty() {
        return Err(t!("Faltan archivos:\n{}", "Files are missing:\n{}", plan.missing.iter().take(5).cloned().collect::<Vec<_>>().join("\n")));
    }
    extract_natives(&plan.native_archives, &plan.natives_dir)?;
    crate::actividad::prefer_e4mc_relay(&instance.game_dir);
    let state = instance.state_dir();
    std::fs::create_dir_all(&state).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&instance.game_dir).map_err(|e| e.to_string())?;

    // Java lee los argumentos de un archivo: evita el límite de longitud de la línea de comandos de Windows.
    // Lo lee en la codificación del sistema (en Windows, ANSI y no UTF-8: con UTF-8, «Prominence™» o
    // «José» llegaban rotos y el juego abría otra carpeta, sin mods). Java 8 (Minecraft hasta 1.16) no
    // entiende el archivo: ahí van en la línea de comandos, que en esas versiones es bastante más corta.
    let arguments: Vec<String> = if java.major < 9 {
        plan.arguments.iter().map(|a| plataforma::java_argument(a)).collect::<Result<_, _>>()?
    } else {
        let argfile = state.join("java-argumentos.txt");
        let mut content = Vec::new();
        for argument in &plan.arguments {
            let quoted = format!("\"{}\"\n", plataforma::java_argument(argument)?.replace('\\', "\\\\").replace('"', "\\\""));
            content.extend(plataforma::argfile_bytes(&quoted));
        }
        std::fs::write(&argfile, content).map_err(|e| e.to_string())?;
        vec![format!("@{}", plataforma::java_argument(&argfile.display().to_string())?)]
    };

    let output = std::fs::File::create(state.join("salida-java.log")).map_err(|e| e.to_string())?;
    let child = Command::new(java.executable())
        .args(arguments)
        .current_dir(&instance.game_dir)
        .stdin(Stdio::null())
        .stdout(output.try_clone().map_err(|e| e.to_string())?)
        .stderr(output)
        .detached()
        .spawn()
        .map_err(|e| t!("No se pudo abrir Java: {e}", "Could not start Java: {e}"))?;
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
/// Windows reutiliza los números de proceso: solo cuenta si ese proceso sigue siendo un Java. Si no,
/// el archivo es de una partida que ya terminó y se borra.
pub fn running_pid(instance: &Instance) -> Option<u32> {
    let file = instance.state_dir().join("juego.pid");
    let pid = std::fs::read_to_string(&file).ok()?.trim().parse::<u32>().ok()?;
    if java_alive(pid) {
        Some(pid)
    } else {
        let _ = std::fs::remove_file(&file);
        None
    }
}

/// La partida terminó: su juego.pid ya no sirve.
pub fn forget_pid(instance: &Instance) {
    let _ = std::fs::remove_file(instance.state_dir().join("juego.pid"));
}

/// Cierra el juego y los procesos que haya abierto.
pub fn kill(pid: u32) {
    plataforma::kill_tree(pid);
}

/// Crash Assistant (lo traen modpacks como Better MC) abre su propio Java para vigilar el juego y se
/// relanza suelto, así que no muere con el juego. Si Parabellum cierra el juego (al terminar una
/// prueba o con «Detener»), cree que se cayó y muestra su ventana: también se cierra. Se reconoce por
/// los archivos que deja en local/crash_assistant de la instancia ("<números>_args.info").
pub fn close_helpers(instance: &Instance) {
    let stems: Vec<String> = std::fs::read_dir(instance.game_dir.join("local").join("crash_assistant"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_string_lossy().strip_suffix("_args.info").map(String::from))
        .filter(|stem| !stem.is_empty() && stem.chars().all(|c| c.is_ascii_digit() || c == '_'))
        .collect();
    if stems.is_empty() {
        return;
    }
    for (pid, command) in plataforma::java_command_lines() {
        let tail = command.split_once("crash_assistant").map(|(_, rest)| rest).unwrap_or_default();
        if stems.iter().any(|stem| tail.contains(&format!("{stem}_"))) {
            plataforma::kill_tree(pid);
        }
    }
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
        // Algunos mods escriben caracteres que no son UTF-8: se leen igual.
        let text = String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default()).to_string();
        let description = text
            .lines()
            .find_map(|l| l.strip_prefix("Description: "))
            .map(String::from)
            .unwrap_or_else(|| t!("Minecraft dejó un informe de fallo", "Minecraft left a crash report"));
        let exception = text.lines().skip_while(|l| !l.starts_with("Description:")).skip(1).find(|l| !l.trim().is_empty());
        return Some(match exception {
            Some(e) => format!("{description}\n{}", e.trim()),
            None => description,
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
    let (_, log) = read_log(instance, 200_000);
    let lines: Vec<&str> = log.lines().collect();
    // El último error del registro con su explicación, sin la pila de llamadas ("at ..."). Así se ve,
    // por ejemplo, el "Incompatible mods found!" de Fabric con qué mod necesita qué versión.
    let is_frame = |l: &str| l.trim_start().starts_with("at ") || l.trim_start().starts_with("... ");
    let start = lines
        .iter()
        .rposition(|l| l.contains("/FATAL]") || l.contains("/ERROR]"))
        .or_else(|| lines.iter().rposition(|l| (l.contains("Exception") || l.contains("Error:")) && !is_frame(l)))?;
    let block: Vec<String> = std::iter::once(lines[start])
        .chain(lines[start + 1..].iter().copied().take_while(|l| !l.starts_with('[') && !is_frame(l)))
        .filter(|l| !l.trim().is_empty())
        .take(12)
        .map(|l| l.trim().chars().take(300).collect())
        .collect();
    Some(block.join("
"))
}

fn java_alive(pid: u32) -> bool {
    plataforma::is_java_process(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

}
