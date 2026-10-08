//! Instancias: cada carpeta de "instancias" con un instancia.json (mismo formato que la versión anterior,
//! así una carpeta portable vieja sigue funcionando).

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::paths::Paths;

pub const INSTANCE_FILE: &str = "instancia.json";
/// Archivos propios del launcher dentro de la instancia (registro de Java, última partida, icono…).
pub const STATE_DIR: &str = ".lanzador";
/// Mismo nombre que en la versión anterior, para conservar los iconos.
const ICON_FILE: &str = "icono.png";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    /// Nombre de la carpeta: identifica la instancia y no cambia al renombrarla.
    pub id: String,
    pub name: String,
    pub version_id: String,
    pub minecraft_version: String,
    pub loader: String,
    /// Versión del loader (None en vanilla).
    pub loader_version: Option<String>,
    #[serde(skip)]
    pub folder: PathBuf,
    #[serde(skip)]
    pub game_dir: PathBuf,
    /// Memoria propia de la instancia; si no hay, se usa la general de Ajustes.
    pub ram_gb: Option<u32>,
    /// Argumentos de Java extra (por ejemplo -XX:+UseZGC).
    pub jvm_args: Vec<String>,
    /// Milisegundos Unix de la última vez que se abrió.
    pub last_played: Option<i64>,
    /// Milisegundos Unix de cuando se creó (fecha de la carpeta).
    pub created: Option<i64>,
    /// Icono propio como data URL (data:image/png;base64,…), si lo tiene.
    pub icon: Option<String>,
    /// De qué modpack viene ("Fabulously Optimized 6.5.0 · Modrinth"), si vino de uno.
    pub modpack: Option<String>,
    /// Proyecto de Modrinth del modpack (para marcarlo como instalado al explorar).
    pub modpack_project: Option<String>,
}

impl Instance {
    pub fn state_dir(&self) -> PathBuf {
        self.game_dir.join(STATE_DIR)
    }
}

pub fn list(paths: &Paths) -> Vec<Instance> {
    let Ok(entries) = std::fs::read_dir(&paths.instances) else { return Vec::new() };
    let mut result: Vec<Instance> = entries
        .flatten()
        .filter_map(|e| {
            ensure_curseforge_version(&e.path(), paths);
            load(&e.path())
        })
        .collect();
    result.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    result
}

pub fn find(paths: &Paths, id: &str) -> Option<Instance> {
    // El id es el nombre de una carpeta: nunca una ruta que salga de "instancias".
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return None;
    }
    ensure_curseforge_version(&paths.instances.join(id), paths);
    load(&paths.instances.join(id))
}

/// Instancias de CurseForge (como las que traen algunos paquetes de la versión C#).
const CURSEFORGE_FILE: &str = "minecraftinstance.json";

fn read_json(folder: &Path) -> Option<Value> {
    std::fs::read_to_string(folder.join(INSTANCE_FILE))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .or_else(|| read_curseforge(folder).map(|(json, _)| json))
}

/// minecraftinstance.json de CurseForge, traducido a los campos de instancia.json. También devuelve
/// el JSON de la versión del loader, que CurseForge guarda como texto dentro del archivo.
fn read_curseforge(folder: &Path) -> Option<(Value, Option<Value>)> {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(folder.join(CURSEFORGE_FILE)).ok()?).ok()?;
    let game_version = json["gameVersion"].as_str()?.to_string();
    let loader = &json["baseModLoader"];
    let embedded = match &loader["versionJson"] {
        Value::String(text) => serde_json::from_str::<Value>(text).ok(),
        object @ Value::Object(_) => Some(object.clone()),
        _ => None,
    };
    let version_id = embedded
        .as_ref()
        .and_then(|v| v["id"].as_str().map(String::from))
        .or_else(|| loader["name"].as_str().map(String::from))
        .unwrap_or_else(|| game_version.clone());
    let mut result = json!({
        "name": folder.file_name()?.to_string_lossy(),
        "versionId": version_id,
        "minecraftVersion": game_version,
    });
    if json["isMemoryOverride"].as_bool() == Some(true) {
        if let Some(megabytes) = json["allocatedMemory"].as_f64() {
            result["ramGB"] = json!(((megabytes / 1024.0).round() as u32).max(2));
        }
    }
    Some((result, embedded))
}

/// Copia (una sola vez) el JSON del loader de una instancia de CurseForge a base/versions, para
/// lanzarla como cualquier otra.
fn ensure_curseforge_version(folder: &Path, paths: &Paths) {
    if folder.join(INSTANCE_FILE).exists() {
        return;
    }
    let Some((json, Some(embedded))) = read_curseforge(folder) else { return };
    let Some(id) = json["versionId"].as_str().filter(|id| !id.contains(['/', '\\']) && !id.contains("..")) else { return };
    let file = paths.version_json(id);
    if !file.exists() && std::fs::create_dir_all(file.parent().unwrap()).is_ok() {
        let _ = std::fs::write(&file, serde_json::to_vec_pretty(&embedded).unwrap());
    }
}

fn load(folder: &Path) -> Option<Instance> {
    let json = read_json(folder)?;
    let version_id = json["versionId"].as_str()?.to_string();
    let id = folder.file_name()?.to_string_lossy().to_string();
    let game_dir = match json["gameDir"].as_str().filter(|s| !s.trim().is_empty()) {
        Some(relative) => folder.join(relative),
        None => folder.to_path_buf(),
    };
    let minecraft_version = json["minecraftVersion"].as_str().unwrap_or(&version_id).to_string();
    let loader = json["loader"].as_str().map(String::from).unwrap_or_else(|| guess_loader(&version_id).into());
    let loader_version = json["loaderVersion"]
        .as_str()
        .map(String::from)
        .or_else(|| loader_version_of(&version_id, &minecraft_version));
    let state = game_dir.join(STATE_DIR);
    let last_played = std::fs::read_to_string(state.join("ultima-partida.txt"))
        .ok()
        .and_then(|t| t.trim().parse::<i64>().ok())
        // La versión anterior guardaba ticks de .NET (100 ns desde el año 1).
        .map(|v| if v > 100_000_000_000_000 { (v - 621_355_968_000_000_000) / 10_000 } else { v });
    let created = std::fs::metadata(folder.join(INSTANCE_FILE))
        .and_then(|m| m.created())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    let icon = std::fs::read(state.join(ICON_FILE))
        .ok()
        .filter(|bytes| bytes.len() < 2_000_000)
        .and_then(|bytes| Some(format!("data:{};base64,{}", image_type(&bytes)?, base64::engine::general_purpose::STANDARD.encode(bytes))));
    // Paquete de Parabellum (.lanzador/pack.json) o modpack de Modrinth/.mrpack (campo "modpack").
    let modpack = std::fs::read_to_string(state.join("pack.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|p| Some(format!("{} · paquete del {}", p["name"].as_str()?, stamp_date(p["version"].as_str().unwrap_or_default()))))
        .or_else(|| {
            let m = &json["modpack"];
            let source = if m["source"] == "modrinth" { "Modrinth" } else { "archivo" };
            Some(format!("{} {} · {source}", m["name"].as_str()?, m["version"].as_str().unwrap_or_default()))
        });
    Some(Instance {
        created,
        icon,
        modpack_project: json["modpack"]["projectId"].as_str().map(String::from),
        modpack,
        name: json["name"].as_str().unwrap_or(&id).to_string(),
        minecraft_version,
        loader,
        loader_version,
        ram_gb: json["ramGB"].as_u64().map(|v| v as u32),
        jvm_args: json["extraJvmArgs"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect(),
        folder: folder.to_path_buf(),
        game_dir,
        last_played,
        version_id,
        id,
    })
}

/// "202610072130" -> "7/10/2026"
fn stamp_date(stamp: &str) -> String {
    if stamp.len() >= 8 && stamp.chars().all(|c| c.is_ascii_digit()) {
        format!("{}/{}/{}", stamp[6..8].trim_start_matches('0'), stamp[4..6].trim_start_matches('0'), &stamp[..4])
    } else {
        stamp.to_string()
    }
}

/// Versión del loader según el id de la versión: "fabric-loader-0.19.5-1.21.1", "1.20.1-forge-47.4.20",
/// "forge-47.4.20" (CurseForge) o "neoforge-21.1.77".
fn loader_version_of(version_id: &str, minecraft: &str) -> Option<String> {
    ["fabric-loader-", "quilt-loader-"]
        .iter()
        .find_map(|prefix| version_id.strip_prefix(prefix)?.strip_suffix(&format!("-{minecraft}")))
        .or_else(|| version_id.strip_prefix(&format!("{minecraft}-forge-")))
        .or_else(|| version_id.strip_prefix("forge-"))
        .or_else(|| version_id.strip_prefix("neoforge-"))
        .map(String::from)
}

/// Instancias de la versión anterior sin el campo "loader": se deduce del id de la versión.
fn guess_loader(version_id: &str) -> &'static str {
    if version_id.contains("fabric") {
        "Fabric"
    } else if version_id.contains("quilt") {
        "Quilt"
    } else if version_id.contains("neoforge") {
        "NeoForge"
    } else if version_id.contains("forge") {
        "Forge"
    } else {
        "Vanilla"
    }
}

/// Qué juego usa una instancia: la versión instalada a lanzar y de qué está hecha.
pub struct GameVersion<'a> {
    pub version_id: &'a str,
    pub minecraft: &'a str,
    /// ("Fabric", "0.19.5"); None en vanilla.
    pub loader: Option<(&'a str, &'a str)>,
}

impl GameVersion<'_> {
    fn write_to(&self, json: &mut Value) {
        json["versionId"] = json!(self.version_id);
        json["minecraftVersion"] = json!(self.minecraft);
        match self.loader {
            Some((loader, version)) => {
                json["loader"] = json!(loader);
                json["loaderVersion"] = json!(version);
            }
            None => {
                json["loader"] = json!("Vanilla");
                json.as_object_mut().map(|o| o.remove("loaderVersion"));
            }
        }
    }
}

/// Carpeta para una instancia nueva con ese nombre (sin caracteres no válidos en Windows).
pub fn folder_for(name: &str, paths: &Paths) -> Result<PathBuf, String> {
    let clean: String = name
        .trim()
        .chars()
        .map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c })
        .collect();
    let clean = clean.trim_matches(['.', ' ']).to_string();
    if clean.is_empty() {
        return Err("Escribe un nombre para la instancia.".into());
    }
    Ok(paths.instances.join(clean))
}

pub fn create(name: &str, game: &GameVersion, paths: &Paths) -> Result<Instance, String> {
    let folder = folder_for(name, paths)?;
    if folder.exists() {
        return Err(format!(
            "Ya tienes una instancia llamada \"{}\". Elige otro nombre.",
            folder.file_name().unwrap().to_string_lossy()
        ));
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut json = json!({ "name": name.trim() });
    game.write_to(&mut json);
    std::fs::write(folder.join(INSTANCE_FILE), serde_json::to_vec_pretty(&json).unwrap()).map_err(|e| e.to_string())?;
    load(&folder).ok_or_else(|| "No se pudo leer la instancia recién creada.".into())
}

/// Cambia nombre visible, memoria propia y argumentos de Java (la carpeta no cambia).
/// Conserva el resto de campos de instancia.json.
pub fn update(instance: &Instance, name: &str, ram_gb: Option<u32>, jvm_args: &[String]) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("El nombre no puede quedar vacío.".into());
    }
    let mut json = read_json(&instance.folder).ok_or("No se pudo leer instancia.json.")?;
    json["name"] = json!(name.trim());
    match ram_gb {
        Some(ram) => json["ramGB"] = json!(ram.clamp(1, 64)),
        None => {
            json.as_object_mut().map(|o| o.remove("ramGB"));
        }
    }
    let args: Vec<&String> = jvm_args.iter().filter(|a| !a.trim().is_empty()).collect();
    if args.is_empty() {
        json.as_object_mut().map(|o| o.remove("extraJvmArgs"));
    } else {
        json["extraJvmArgs"] = json!(args);
    }
    write_json(instance, &json)
}

/// Cambia la versión del juego o del loader (por ejemplo, Fabric 0.16 → 0.19).
pub fn set_version(instance: &Instance, game: &GameVersion) -> Result<(), String> {
    let mut json = read_json(&instance.folder).ok_or("No se pudo leer instancia.json.")?;
    game.write_to(&mut json);
    write_json(instance, &json)
}

fn write_json(instance: &Instance, json: &Value) -> Result<(), String> {
    let file = instance.folder.join(INSTANCE_FILE);
    let temp = file.with_extension("tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(json).unwrap()).map_err(|e| e.to_string())?;
    std::fs::rename(temp, file).map_err(|e| e.to_string())
}

/// Tipo de imagen por su firma (los iconos de Modrinth suelen ser WebP).
pub fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else {
        None
    }
}

/// Icono descargado (por ejemplo, el de un modpack de Modrinth). Se guarda con el nombre de siempre.
pub fn set_icon_bytes(instance: &Instance, bytes: &[u8]) -> Result<(), String> {
    image_type(bytes).ok_or("La imagen no es válida.")?;
    std::fs::create_dir_all(instance.state_dir()).map_err(|e| e.to_string())?;
    std::fs::write(instance.state_dir().join(ICON_FILE), bytes).map_err(|e| e.to_string())
}

/// Un nombre libre para una instancia nueva: "X", "X (2)", "X (3)"…
pub fn unique_name(name: &str, paths: &Paths) -> Result<String, String> {
    let base = name.trim();
    let mut candidate = base.to_string();
    let mut number = 2;
    while folder_for(&candidate, paths)?.exists() {
        candidate = format!("{base} ({number})");
        number += 1;
    }
    Ok(candidate)
}

/// Guarda un dato extra en instancia.json (por ejemplo, de qué modpack viene).
pub fn set_field(instance: &Instance, key: &str, value: Value) -> Result<(), String> {
    let mut json = read_json(&instance.folder).ok_or("No se pudo leer instancia.json.")?;
    json[key] = value;
    write_json(instance, &json)
}

/// Guarda (PNG en base64) o quita (None) el icono propio.
pub fn set_icon(instance: &Instance, png_base64: Option<&str>) -> Result<(), String> {
    let file = instance.state_dir().join(ICON_FILE);
    match png_base64 {
        Some(data) => {
            let data = data.split_once(',').map(|(_, d)| d).unwrap_or(data);
            let bytes = base64::engine::general_purpose::STANDARD.decode(data).map_err(|_| "La imagen no es válida.")?;
            if !bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
                return Err("La imagen debe ser PNG.".into());
            }
            std::fs::create_dir_all(instance.state_dir()).map_err(|e| e.to_string())?;
            std::fs::write(file, bytes).map_err(|e| e.to_string())
        }
        None => match std::fs::remove_file(file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
    }
}

/// Copia completa de la instancia (mundos, mods, opciones) con el nombre "X (copia)".
pub fn duplicate(instance: &Instance, paths: &Paths) -> Result<Instance, String> {
    let mut name = format!("{} (copia)", instance.name);
    let mut number = 2;
    while folder_for(&name, paths)?.exists() {
        name = format!("{} (copia {number})", instance.name);
        number += 1;
    }
    let target = folder_for(&name, paths)?;
    copy_dir(&instance.folder, &target).map_err(|e| format!("No se pudo copiar la instancia: {e}"))?;
    // Lo que describe la partida anterior no se copia.
    for file in ["juego.pid", "ultima-partida.txt", "salida-java.log", "java-argumentos.txt"] {
        let _ = std::fs::remove_file(target.join(STATE_DIR).join(file));
    }
    let copy = load(&target).ok_or("No se pudo leer la copia.")?;
    update(&copy, &name, copy.ram_gb, &copy.jvm_args)?;
    load(&target).ok_or_else(|| "No se pudo leer la copia.".into())
}

pub fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Manda la carpeta a la papelera de reciclaje (se puede recuperar).
pub fn delete(instance: &Instance) -> Result<(), String> {
    trash::delete(&instance.folder).map_err(|e| format!("No se pudo mandar a la papelera: {e}"))
}
