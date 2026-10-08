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
    /// Versión de Fabric o Quilt (None en vanilla).
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
    // El id es el nombre de una carpeta: nunca una ruta que salga de "instancias".
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return None;
    }
    load(&paths.instances.join(id))
}

fn read_json(folder: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(folder.join(INSTANCE_FILE)).ok()?).ok()
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
        .or_else(|| ["fabric-loader-", "quilt-loader-"].iter().find_map(|prefix| {
            version_id.strip_prefix(prefix)?.strip_suffix(&format!("-{minecraft_version}")).map(String::from)
        }));
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
        .map(|bytes| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)));
    Some(Instance {
        created,
        icon,
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
