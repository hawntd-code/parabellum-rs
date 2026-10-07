//! Contenido de una instancia: mods, resource packs, shaders y mundos.
//! Desactivar un mod = renombrarlo a "*.jar.disabled" (lo mismo que hacen CurseForge, Modrinth y Prism).

use base64::Engine;
use regex::Regex;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::instances::Instance;

const DISABLED: &str = ".disabled";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentItem {
    /// Nombre del archivo o carpeta (identifica el elemento).
    pub file: String,
    pub name: String,
    pub version: Option<String>,
    pub authors: Option<String>,
    pub enabled: bool,
    pub size: u64,
    pub modified: Option<i64>,
    pub icon: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub mods: Vec<ContentItem>,
    pub resourcepacks: Vec<ContentItem>,
    pub shaderpacks: Vec<ContentItem>,
    pub worlds: Vec<ContentItem>,
}

/// Carpeta de cada categoría dentro de la instancia.
pub fn folder(instance: &Instance, kind: &str) -> Option<PathBuf> {
    let name = match kind {
        "mods" => "mods",
        "resourcepacks" => "resourcepacks",
        "shaderpacks" => "shaderpacks",
        "worlds" => "saves",
        _ => return None,
    };
    Some(instance.game_dir.join(name))
}

pub fn list(instance: &Instance) -> Content {
    Content {
        mods: files(instance, "mods", &[".jar"], true),
        resourcepacks: files(instance, "resourcepacks", &[".zip"], false),
        shaderpacks: files(instance, "shaderpacks", &[".zip"], false),
        worlds: worlds(instance),
    }
}

fn modified_ms(meta: &std::fs::Metadata) -> Option<i64> {
    meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as i64)
}

fn files(instance: &Instance, kind: &str, extensions: &[&str], read_mod: bool) -> Vec<ContentItem> {
    let Some(dir) = folder(instance, kind) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut items: Vec<ContentItem> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().to_string();
            let enabled = !file.ends_with(DISABLED);
            let base = file.trim_end_matches(DISABLED);
            let is_folder = entry.file_type().ok()?.is_dir();
            // Resource packs y shaders también pueden ser carpetas sin comprimir.
            if !extensions.iter().any(|e| base.to_lowercase().ends_with(e)) && !(is_folder && !read_mod) {
                return None;
            }
            let meta = entry.metadata().ok()?;
            let info = if read_mod { mod_info(&entry.path()) } else { ModInfo::default() };
            Some(ContentItem {
                name: info.name.unwrap_or_else(|| pretty_name(base, extensions)),
                version: info.version,
                authors: info.authors,
                icon: info.icon,
                enabled,
                size: meta.len(),
                modified: modified_ms(&meta),
                file,
            })
        })
        .collect();
    items.sort_by_key(|i| i.name.to_lowercase());
    items
}

fn worlds(instance: &Instance) -> Vec<ContentItem> {
    let Ok(entries) = std::fs::read_dir(instance.game_dir.join("saves")) else { return Vec::new() };
    let mut items: Vec<ContentItem> = entries
        .flatten()
        .filter(|e| e.path().join("level.dat").exists())
        .map(|entry| {
            let meta = std::fs::metadata(entry.path().join("level.dat")).ok();
            let icon = std::fs::read(entry.path().join("icon.png"))
                .ok()
                .filter(|b| b.len() < 500_000)
                .map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
            let file = entry.file_name().to_string_lossy().to_string();
            ContentItem {
                name: file.clone(),
                version: None,
                authors: None,
                enabled: true,
                size: 0,
                modified: meta.as_ref().and_then(modified_ms),
                icon,
                file,
            }
        })
        .collect();
    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    items
}

fn pretty_name(file: &str, extensions: &[&str]) -> String {
    let mut name = file.to_string();
    for extension in extensions {
        if name.to_lowercase().ends_with(extension) {
            name.truncate(name.len() - extension.len());
        }
    }
    name
}

#[derive(Default)]
struct ModInfo {
    name: Option<String>,
    version: Option<String>,
    authors: Option<String>,
    icon: Option<String>,
}

/// Nombre, versión, autores e icono que declara el propio mod (Fabric, Quilt, Forge o NeoForge).
fn mod_info(path: &Path) -> ModInfo {
    let Ok(file) = std::fs::File::open(path) else { return ModInfo::default() };
    let Ok(mut zip) = zip::ZipArchive::new(file) else { return ModInfo::default() };
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let mut entry = zip.by_name(name).ok()?;
        if entry.size() > 4_000_000 {
            return None;
        }
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer).ok()?;
        Some(buffer)
    };

    let mut info = ModInfo::default();
    let mut icon_path: Option<String> = None;
    if let Some(json) = read("fabric.mod.json").and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        info.name = json["name"].as_str().map(String::from);
        info.version = json["version"].as_str().map(String::from);
        info.authors = authors(&json["authors"]);
        icon_path = json["icon"].as_str().map(String::from);
    } else if let Some(json) = read("quilt.mod.json").and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
        let loader = &json["quilt_loader"];
        info.name = loader["metadata"]["name"].as_str().map(String::from);
        info.version = loader["version"].as_str().map(String::from);
        icon_path = loader["metadata"]["icon"].as_str().map(String::from);
    } else if let Some(toml) = read("META-INF/neoforge.mods.toml").or_else(|| read("META-INF/mods.toml")) {
        let text = String::from_utf8_lossy(&toml);
        info.name = toml_value(&text, "displayName");
        info.version = toml_value(&text, "version").filter(|v| !v.contains("${"));
        info.authors = toml_value(&text, "authors");
        icon_path = toml_value(&text, "logoFile");
    }
    info.icon = icon_path
        .and_then(|p| read(p.trim_start_matches('/')))
        .filter(|b| b.len() < 300_000 && b.starts_with(&[0x89, b'P', b'N', b'G']))
        .map(|b| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    info
}

fn authors(value: &serde_json::Value) -> Option<String> {
    let names: Vec<String> = value
        .as_array()?
        .iter()
        .filter_map(|a| a.as_str().map(String::from).or_else(|| a["name"].as_str().map(String::from)))
        .collect();
    (!names.is_empty()).then(|| names.join(", "))
}

fn toml_value(text: &str, key: &str) -> Option<String> {
    static CACHE: OnceLock<std::sync::Mutex<std::collections::HashMap<String, Regex>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let regex = cache
        .lock()
        .unwrap()
        .entry(key.to_string())
        .or_insert_with(|| Regex::new(&format!(r#"(?m)^\s*{key}\s*=\s*"([^"]*)""#)).unwrap())
        .clone();
    regex.captures(text).map(|c| c[1].to_string()).filter(|v| !v.is_empty())
}

/// Ruta segura de un elemento: solo nombres de archivo dentro de la carpeta de su categoría.
fn item_path(instance: &Instance, kind: &str, file: &str) -> Result<PathBuf, String> {
    if file.is_empty() || file.contains(['/', '\\']) || file == "." || file == ".." {
        return Err("Nombre de archivo no válido.".into());
    }
    let path = folder(instance, kind).ok_or("Categoría desconocida.")?.join(file);
    if !path.exists() {
        return Err("Ese archivo ya no existe.".into());
    }
    Ok(path)
}

/// Activa o desactiva un mod, resource pack o shader. Devuelve el nuevo nombre del archivo.
pub fn set_enabled(instance: &Instance, kind: &str, file: &str, enabled: bool) -> Result<String, String> {
    let path = item_path(instance, kind, file)?;
    let new_name = if enabled { file.trim_end_matches(DISABLED).to_string() } else if file.ends_with(DISABLED) { file.to_string() } else { format!("{file}{DISABLED}") };
    if new_name != file {
        std::fs::rename(&path, path.with_file_name(&new_name)).map_err(|e| format!("No se pudo cambiar: {e}"))?;
    }
    Ok(new_name)
}

/// Manda un elemento (o un mundo entero) a la papelera.
pub fn delete(instance: &Instance, kind: &str, file: &str) -> Result<(), String> {
    trash::delete(item_path(instance, kind, file)?).map_err(|e| format!("No se pudo mandar a la papelera: {e}"))
}
