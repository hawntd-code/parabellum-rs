//! Instancias: cada carpeta de "instancias" con un instancia.json (mismo formato que la versión anterior,
//! así una carpeta portable vieja sigue funcionando).

use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::paths::Paths;

pub const INSTANCE_FILE: &str = "instancia.json";
/// Archivos propios del launcher dentro de la instancia (registro de Java, última partida…).
pub const STATE_DIR: &str = ".lanzador";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    /// Nombre de la carpeta: identifica la instancia.
    pub id: String,
    pub name: String,
    pub version_id: String,
    pub minecraft_version: String,
    pub loader: String,
    #[serde(skip)]
    pub folder: PathBuf,
    #[serde(skip)]
    pub game_dir: PathBuf,
    pub ram_gb: Option<u32>,
    /// Milisegundos Unix de la última vez que se abrió.
    pub last_played: Option<i64>,
    /// Milisegundos Unix de cuando se creó (fecha de la carpeta).
    pub created: Option<i64>,
}

impl Instance {
    pub fn state_dir(&self) -> PathBuf {
        self.game_dir.join(STATE_DIR)
    }
}

pub fn list(paths: &Paths) -> Vec<Instance> {
    let Ok(entries) = std::fs::read_dir(&paths.instances) else { return Vec::new() };
    let mut result: Vec<Instance> = entries.flatten().filter_map(|e| load(&e.path())).collect();
    result.sort_by(|a, b| b.last_played.cmp(&a.last_played).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    result
}

pub fn find(paths: &Paths, id: &str) -> Option<Instance> {
    load(&paths.instances.join(id))
}

fn load(folder: &Path) -> Option<Instance> {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(folder.join(INSTANCE_FILE)).ok()?).ok()?;
    let version_id = json["versionId"].as_str()?.to_string();
    let id = folder.file_name()?.to_string_lossy().to_string();
    let game_dir = match json["gameDir"].as_str().filter(|s| !s.trim().is_empty()) {
        Some(relative) => folder.join(relative),
        None => folder.to_path_buf(),
    };
    let loader = if version_id.contains("fabric") {
        "Fabric"
    } else if version_id.contains("neoforge") {
        "NeoForge"
    } else if version_id.contains("forge") {
        "Forge"
    } else {
        "Vanilla"
    };
    let last_played = std::fs::read_to_string(game_dir.join(STATE_DIR).join("ultima-partida.txt"))
        .ok()
        .and_then(|t| t.trim().parse::<i64>().ok())
        // La versión anterior guardaba ticks de .NET (100 ns desde el año 1).
        .map(|v| if v > 100_000_000_000_000 { (v - 621_355_968_000_000_000) / 10_000 } else { v });
    let created = std::fs::metadata(folder.join(INSTANCE_FILE))
        .and_then(|m| m.created())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64);
    Some(Instance {
        created,
        name: json["name"].as_str().unwrap_or(&id).to_string(),
        minecraft_version: json["minecraftVersion"].as_str().unwrap_or(&version_id).to_string(),
        loader: loader.into(),
        ram_gb: json["ramGB"].as_u64().map(|v| v as u32),
        folder: folder.to_path_buf(),
        game_dir,
        last_played,
        version_id,
        id,
    })
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

pub fn create(name: &str, version_id: &str, paths: &Paths) -> Result<Instance, String> {
    let folder = folder_for(name, paths)?;
    if folder.exists() {
        return Err(format!(
            "Ya tienes una instancia llamada \"{}\". Elige otro nombre.",
            folder.file_name().unwrap().to_string_lossy()
        ));
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let json = json!({ "name": name.trim(), "versionId": version_id, "minecraftVersion": version_id });
    std::fs::write(folder.join(INSTANCE_FILE), serde_json::to_vec_pretty(&json).unwrap()).map_err(|e| e.to_string())?;
    load(&folder).ok_or_else(|| "No se pudo leer la instancia recién creada.".into())
}
