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
    /// Proyecto de Modrinth, si se sabe (para abrir su página y buscar actualizaciones).
    pub project_id: Option<String>,
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
                project_id: None,
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
            let datapacks = std::fs::read_dir(entry.path().join("datapacks")).into_iter().flatten().flatten().count();
            ContentItem {
                name: file.clone(),
                version: (datapacks > 0).then(|| format!("{datapacks} data pack{}", if datapacks == 1 { "" } else { "s" })),
                authors: None,
                enabled: true,
                size: 0,
                modified: meta.as_ref().and_then(modified_ms),
                icon,
                project_id: None,
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
pub fn item_path(instance: &Instance, kind: &str, file: &str) -> Result<PathBuf, String> {
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
    trash::delete(item_path(instance, kind, file)?).map_err(|e| format!("No se pudo mandar a la papelera: {e}"))?;
    // Si venía con un modpack, ya no se repone al jugar.
    if kind != "worlds" {
        crate::integridad::forget(instance, &format!("{kind}/{file}"));
    }
    Ok(())
}

/// Resultado de agregar archivos: cuántos entraron en cada categoría y cuáles no (con el motivo).
#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Added {
    pub mods: usize,
    pub resourcepacks: usize,
    pub shaderpacks: usize,
    pub worlds: usize,
    pub skipped: Vec<String>,
}

/// Agrega archivos de la PC a la instancia. Cada uno va a su categoría según lo que contiene:
/// un .jar es un mod; un .zip o carpeta con pack.mcmeta, un resource pack; con shaders/, un shader;
/// con level.dat, un mundo. `hint` (la categoría abierta) decide lo que no se reconoce.
pub fn add(instance: &Instance, sources: &[PathBuf], hint: Option<&str>) -> Added {
    let mut added = Added::default();
    for source in sources {
        let name = source.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match add_one(instance, source, hint) {
            Ok("mods") => added.mods += 1,
            Ok("resourcepacks") => added.resourcepacks += 1,
            Ok("shaderpacks") => added.shaderpacks += 1,
            Ok(_) => added.worlds += 1,
            Err(reason) => added.skipped.push(format!("{name}: {reason}")),
        }
    }
    added
}

fn add_one(instance: &Instance, source: &Path, hint: Option<&str>) -> Result<&'static str, String> {
    let name = source.file_name().ok_or("no es un archivo")?.to_string_lossy().to_string();
    if !source.exists() {
        return Err("no se encontró".into());
    }
    let kind = classify(source).or_else(|| match hint {
        Some("resourcepacks") if is_zip(source) => Some(Kind::Pack("resourcepacks")),
        Some("shaderpacks") if is_zip(source) => Some(Kind::Pack("shaderpacks")),
        _ => None,
    });
    match kind.ok_or("no es un mod, resource pack, shader ni mundo")? {
        Kind::Modpack => Err("es un modpack: para instalarlo suéltalo en Inicio o usa Crear instancia › Subir un modpack".into()),
        Kind::Datapack => Err("es un data pack: va dentro de un mundo (instálalo desde Explorar › Data packs o cópialo a la carpeta datapacks del mundo)".into()),
        Kind::Mod(loaders) => {
            check_loader(instance, &loaders)?;
            copy_new(source, &folder(instance, "mods").unwrap().join(&name))?;
            Ok("mods")
        }
        Kind::Pack(kind) => {
            let target = folder(instance, kind).unwrap().join(&name);
            if source.is_dir() {
                copy_dir_new(source, &target)?;
            } else {
                copy_new(source, &target)?;
            }
            Ok(kind)
        }
        Kind::WorldFolder => {
            copy_dir_new(source, &folder(instance, "worlds").unwrap().join(&name))?;
            Ok("worlds")
        }
        Kind::WorldZip(prefix) => {
            let world = match prefix.trim_end_matches('/') {
                "" => pretty_name(&name, &[".zip"]),
                folder_name => folder_name.to_string(),
            };
            extract_world(source, &prefix, &folder(instance, "worlds").unwrap().join(world))?;
            Ok("worlds")
        }
    }
}

enum Kind {
    /// Un mod y los loaders para los que está hecho (vacío si no lo dice).
    Mod(Vec<&'static str>),
    Pack(&'static str),
    WorldFolder,
    /// Mundo comprimido; la carpeta interna donde está level.dat ("" si está en la raíz).
    WorldZip(String),
    Modpack,
    Datapack,
}

fn is_zip(path: &Path) -> bool {
    path.is_file() && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"))
}

fn classify(path: &Path) -> Option<Kind> {
    if path.is_dir() {
        return if path.join("level.dat").is_file() {
            Some(Kind::WorldFolder)
        } else if path.join("pack.mcmeta").is_file() {
            Some(Kind::Pack("resourcepacks"))
        } else if path.join("shaders").is_dir() {
            Some(Kind::Pack("shaderpacks"))
        } else {
            None
        };
    }
    let extension = path.extension()?.to_string_lossy().to_lowercase();
    if extension == "mrpack" {
        return Some(Kind::Modpack);
    }
    if extension != "jar" && extension != "zip" {
        return None;
    }
    let zip = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let names: Vec<&str> = zip.file_names().collect();
    let has = |name: &str| names.iter().any(|n| *n == name);
    if extension == "jar" {
        let mut loaders = Vec::new();
        if has("fabric.mod.json") {
            loaders.push("Fabric");
        }
        if has("quilt.mod.json") {
            loaders.push("Quilt");
        }
        if has("META-INF/neoforge.mods.toml") {
            loaders.push("NeoForge");
        }
        if has("META-INF/mods.toml") {
            loaders.push("Forge");
        }
        return Some(Kind::Mod(loaders));
    }
    if has("modrinth.index.json") || has("lanzador-pack.json") {
        return Some(Kind::Modpack);
    }
    if has("pack.mcmeta") {
        // Un data pack trae data/ y no assets/.
        let data = names.iter().any(|n| n.starts_with("data/"));
        let assets = names.iter().any(|n| n.starts_with("assets/"));
        return Some(if data && !assets { Kind::Datapack } else { Kind::Pack("resourcepacks") });
    }
    if names.iter().any(|n| n.starts_with("shaders/")) {
        return Some(Kind::Pack("shaderpacks"));
    }
    if has("level.dat") {
        return Some(Kind::WorldZip(String::new()));
    }
    // Mundo dentro de una carpeta: "Mi mundo/level.dat"
    names
        .iter()
        .find_map(|n| n.strip_suffix("/level.dat").filter(|folder| !folder.contains('/')))
        .map(|folder| Kind::WorldZip(format!("{folder}/")))
}

/// Un mod de otro loader no cargaría: mejor avisar que copiarlo.
fn check_loader(instance: &Instance, loaders: &[&str]) -> Result<(), String> {
    let accepted: &[&str] = match instance.loader.as_str() {
        "Vanilla" => return Err("esta instancia es vanilla y no carga mods (crea una con Fabric)".into()),
        // Quilt también carga los mods de Fabric.
        "Quilt" => &["Quilt", "Fabric"],
        "Fabric" => &["Fabric"],
        // NeoForge de 1.20.1 todavía carga mods de Forge.
        "NeoForge" => &["NeoForge", "Forge"],
        "Forge" => &["Forge"],
        _ => return Ok(()),
    };
    if loaders.is_empty() || loaders.iter().any(|l| accepted.contains(l)) {
        Ok(())
    } else {
        Err(format!("es un mod de {} y esta instancia es {}", loaders.join(" / "), instance.loader))
    }
}

fn copy_new(source: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err("ya está en la instancia".into());
    }
    std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::copy(source, target).map(|_| ()).map_err(|e| format!("no se pudo copiar ({e})"))
}

fn copy_dir_new(source: &Path, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err("ya está en la instancia".into());
    }
    crate::instances::copy_dir(source, target).map_err(|e| format!("no se pudo copiar ({e})"))
}

fn extract_world(source: &Path, prefix: &str, target: &Path) -> Result<(), String> {
    if target.exists() {
        return Err("ya hay un mundo con ese nombre".into());
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(source).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        // enclosed_name descarta rutas que intenten salir de la carpeta ("../").
        let Some(path) = entry.enclosed_name() else { continue };
        let Ok(relative) = path.strip_prefix(prefix.trim_end_matches('/')) else { continue };
        if entry.is_dir() || relative.as_os_str().is_empty() {
            continue;
        }
        let destination = target.join(relative);
        std::fs::create_dir_all(destination.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(&destination).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    Ok(())
}
