//! Que los modpacks queden completos y sanos:
//!  - Al instalar un modpack se guarda la lista de lo que trae (.lanzador/archivos.json): mods,
//!    resource packs y shaders, con su huella y de dónde volver a bajarlos.
//!  - Algunos modpacks de Modrinth no pueden incluir los mods que solo están en CurseForge y los piden
//!    con Missing Mods Checker (config/missing_mods_checker.json). Sin ellos el juego abre una ventana
//!    para bajarlos a mano y se cierra. Parabellum los baja del CDN de CurseForge.
//!  - Antes de jugar se comprueba que esté todo (rápido: que los archivos existan) y lo que falte se
//!    vuelve a bajar. «Verificar y reparar» comprueba además la huella de cada archivo.
//! Lo que el jugador quita o actualiza desde Contenido deja de vigilarse; lo desactivado cuenta como presente.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::download::{self, FileDownload};
use crate::instances::{Instance, INSTANCE_FILE, STATE_DIR};
use crate::mrpack::{self, safe_relative};

pub const RECORD: &str = "archivos.json";
const CHECKER_CONFIG: &str = "missing_mods_checker.json";
/// Lo que se vigila. La configuración no: el juego y el jugador la cambian.
const TRACKED_DIRS: [&str; 3] = ["mods", "resourcepacks", "shaderpacks"];
const DISABLED: &str = ".disabled";
const FORGE_CDN_HOSTS: [&str; 3] = ["edge.forgecdn.net", "media.forgecdn.net", "mediafilez.forgecdn.net"];

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tracked {
    /// Ruta dentro de la instancia, con "/" ("mods/sodium.jar").
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crc32: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    /// De dónde volver a bajarlo. Sin URL venía dentro del modpack y hace falta el modpack para reponerlo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// ¿Es contenido que se vigila? Los .jar y .zip de mods, resourcepacks y shaderpacks, y los packs que
/// vienen como carpeta. Lo demás de esas carpetas no (Iris guarda ahí los ajustes de cada shader).
pub fn is_tracked(relative: &str) -> bool {
    let relative = relative.replace('\\', "/");
    let parts: Vec<&str> = relative.split('/').collect();
    if parts.len() < 2 || parts.iter().any(|p| p.is_empty()) || !TRACKED_DIRS.iter().any(|d| d.eq_ignore_ascii_case(parts[0])) {
        return false;
    }
    let lower = relative.to_lowercase();
    if parts.len() == 2 {
        lower.ends_with(".jar") || lower.ends_with(".zip")
    } else {
        !parts[0].eq_ignore_ascii_case("mods")
    }
}

fn key(relative: &str) -> String {
    let relative = relative.replace('\\', "/").to_lowercase();
    relative.strip_suffix(DISABLED).map(String::from).unwrap_or(relative)
}

fn record_file(game_dir: &Path) -> PathBuf {
    game_dir.join(STATE_DIR).join(RECORD)
}

pub fn load(game_dir: &Path) -> Option<Vec<Tracked>> {
    serde_json::from_str(&std::fs::read_to_string(record_file(game_dir)).ok()?).ok()
}

/// Guarda la lista (sin repetidos; si un archivo aparece dos veces, vale la última).
pub fn save(game_dir: &Path, files: Vec<Tracked>) {
    let mut seen = HashSet::new();
    let mut unique: Vec<Tracked> = files.into_iter().rev().filter(|f| seen.insert(key(&f.path))).collect();
    unique.reverse();
    let _ = std::fs::create_dir_all(game_dir.join(STATE_DIR));
    let _ = std::fs::write(record_file(game_dir), serde_json::to_vec_pretty(&unique).unwrap());
}

/// El jugador quitó o reemplazó ese archivo desde Contenido: ya no se repone.
pub fn forget(instance: &Instance, relative: &str) {
    let Some(mut files) = load(&instance.game_dir) else { return };
    let gone = key(relative);
    let before = files.len();
    files.retain(|f| key(&f.path) != gone);
    if files.len() != before {
        save(&instance.game_dir, files);
    }
}

/// Lista de lo que trae un .mrpack: lo de su índice (con URL) y lo que viene dentro (overrides).
pub fn from_mrpack(info: &mrpack::Info, zip_path: &Path) -> Vec<Tracked> {
    let mut files: Vec<Tracked> = info
        .files
        .iter()
        .map(|f| (f.path.to_string_lossy().replace('\\', "/"), f))
        .filter(|(path, _)| is_tracked(path))
        .map(|(path, f)| Tracked { path, sha1: Some(f.sha1.clone()), crc32: None, size: f.size, url: Some(f.url.clone()) })
        .collect();
    let Ok(mut zip) = std::fs::File::open(zip_path).map_err(|_| ()).and_then(|f| zip::ZipArchive::new(f).map_err(|_| ())) else { return files };
    // Primero overrides y después client-overrides, que tienen prioridad (save se queda con la última).
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let Ok(entry) = zip.by_index_raw(i) else { continue };
            let Some(rest) = entry.name().strip_prefix(prefix) else { continue };
            if entry.is_dir() || !is_tracked(rest) || safe_relative(rest).is_none() {
                continue;
            }
            files.push(Tracked { path: rest.to_string(), sha1: None, crc32: Some(entry.crc32()), size: Some(entry.size()), url: None });
        }
    }
    files
}

/// Guarda la lista de un paquete de Parabellum instalado en `folder`: sus entradas "instancia/…" con
/// el CRC32 y el tamaño que dice el zip. Las rutas quedan relativas a la carpeta del juego (gameDir).
/// Lo que está en Modrinth (`remote`) lleva además su SHA-1 y su enlace: se repone sin el paquete.
pub fn save_package(folder: &Path, entries: &[(String, u32, u64)], remote: &[crate::packs::RemoteFile]) {
    let remote: HashMap<String, &crate::packs::RemoteFile> = remote.iter().filter(|r| r.valid()).map(|r| (format!("instancia/{}", r.path.to_lowercase()), r)).collect();
    let game = std::fs::read_to_string(folder.join(INSTANCE_FILE))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|j| Some(j["gameDir"].as_str()?.trim().replace('\\', "/").trim_matches('/').to_string()))
        .filter(|g| !g.is_empty() && safe_relative(g).is_some());
    let prefix = match &game {
        Some(g) => format!("instancia/{}/", g.to_lowercase()),
        None => "instancia/".into(),
    };
    let files = entries
        .iter()
        .filter_map(|(name, crc32, size)| {
            let rest = name.get(prefix.len()..).filter(|_| name.to_lowercase().starts_with(&prefix))?;
            let from = remote.get(&name.to_lowercase());
            (is_tracked(rest) && safe_relative(rest).is_some()).then(|| Tracked {
                path: rest.to_string(),
                sha1: from.map(|r| r.sha1.clone()),
                crc32: Some(*crc32),
                size: Some(*size),
                url: from.map(|r| r.url.clone()),
            })
        })
        .collect();
    save(&game.map(|g| folder.join(g)).unwrap_or_else(|| folder.to_path_buf()), files);
}

// ================= Missing Mods Checker

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RequiredMod {
    #[serde(default)]
    display_name: String,
    pattern: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    destination: Option<String>,
}

/// El nombre del archivo, si el patrón es un nombre normal y no una expresión regular.
fn plain_file_name(pattern: &str) -> Option<String> {
    let name = pattern.trim().replace(r"\.", ".");
    let lower = name.to_lowercase();
    let plain = !name.is_empty()
        && !name.starts_with('.')
        && !name.chars().any(|c| r#"\/:*?"<>|^$"#.contains(c) || c.is_control())
        && (lower.ends_with(".jar") || lower.ends_with(".zip"));
    plain.then_some(name)
}

/// Codifica un nombre de archivo para una URL (el CDN de CurseForge no acepta "+" ni espacios tal cual).
fn encode(name: &str) -> String {
    name.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// De dónde bajar un archivo que pide Missing Mods Checker. Los enlaces de curseforge.com son páginas
/// que solo abren en un navegador; el mismo archivo está en su CDN: files/<id / 1000>/<id % 1000>/<nombre>.
fn checker_url(url: &str, file_name: &str) -> Option<String> {
    let rest = url.trim().strip_prefix("https://")?;
    let host = rest.split('/').next()?.to_lowercase();
    if FORGE_CDN_HOSTS.contains(&host.as_str()) || mrpack::ALLOWED_HOSTS.iter().any(|h| url.trim().starts_with(h)) {
        return Some(url.trim().to_string());
    }
    if host != "www.curseforge.com" && host != "curseforge.com" {
        return None;
    }
    static ID: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let id: u64 = ID.get_or_init(|| Regex::new(r"/(?:download|files)/(\d+)(?:/|$|\?)").unwrap()).captures(rest)?[1].parse().ok()?;
    Some(format!("https://mediafilez.forgecdn.net/files/{}/{}/{}", id / 1000, id % 1000, encode(file_name)))
}

/// Como el propio mod: el archivo está si su nombre es igual al patrón o lo cumple como expresión regular.
/// Desactivado también cuenta: fue decisión del jugador.
fn matches_pattern(pattern: &str, file_name: &str) -> bool {
    let name = file_name.strip_suffix(DISABLED).unwrap_or(file_name);
    if name.eq_ignore_ascii_case(pattern.trim()) {
        return true;
    }
    Regex::new(&format!("(?i)^(?:{})$", pattern.trim())).is_ok_and(|r| r.is_match(name))
}

pub struct CheckerFile {
    pub name: String,
    pub path: String,
    pub url: String,
}

/// Lo que pide Missing Mods Checker y falta: (lo que se puede bajar, lo que no se sabe de dónde bajar).
/// Lo que no se sabe de dónde bajar va como (nombre, carpeta de destino).
pub fn checker_missing(game_dir: &Path) -> (Vec<CheckerFile>, Vec<(String, String)>) {
    let Some(list) = std::fs::read_to_string(game_dir.join("config").join(CHECKER_CONFIG))
        .ok()
        .and_then(|t| serde_json::from_str::<Vec<RequiredMod>>(t.trim_start_matches('\u{feff}')).ok())
    else {
        return (Vec::new(), Vec::new());
    };
    let mut folders: HashMap<String, Vec<String>> = HashMap::new();
    let (mut missing, mut unknown) = (Vec::new(), Vec::new());
    for required in list {
        let destination = required.destination.as_deref().map(str::trim).filter(|d| !d.is_empty()).unwrap_or("mods").replace('\\', "/");
        let label = Some(required.display_name.trim()).filter(|n| !n.is_empty()).unwrap_or(&required.pattern).to_string();
        let Some(folder) = safe_relative(destination.trim_end_matches('/')) else {
            unknown.push((label, destination));
            continue;
        };
        let names = folders.entry(destination.to_lowercase()).or_insert_with(|| {
            std::fs::read_dir(game_dir.join(&folder)).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()
        });
        if names.iter().any(|n| matches_pattern(&required.pattern, n)) {
            continue;
        }
        match plain_file_name(&required.pattern).and_then(|name| Some((checker_url(&required.url, &name)?, name))) {
            Some((url, name)) => {
                let path = format!("{}/{name}", folder.to_string_lossy().replace('\\', "/"));
                // Si el mismo archivo aparece dos veces en la lista, se baja una sola vez.
                names.push(name);
                missing.push(CheckerFile { name: label, path, url });
            }
            None => unknown.push((label, format!("{destination}/"))),
        }
    }
    (missing, unknown)
}

// ================= Verificar y reparar

/// ¿Está el archivo? (desactivado también cuenta)
pub fn present(game_dir: &Path, relative: &str) -> bool {
    present_path(game_dir, relative).is_some()
}

fn present_path(game_dir: &Path, relative: &str) -> Option<PathBuf> {
    let path = game_dir.join(relative);
    if path.is_file() {
        return Some(path);
    }
    let disabled = game_dir.join(format!("{relative}{DISABLED}"));
    disabled.is_file().then_some(disabled)
}

pub(crate) fn sha1_file(path: &Path) -> Option<String> {
    use sha1::{Digest, Sha1};
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hex::encode(hasher.finalize()))
}

/// ¿El archivo es el que trajo el modpack? Manda la huella: hay modpacks con tamaños mal anotados
/// en su índice (Better MC BMC2 trae cinco con unos bytes de diferencia y el SHA-1 correcto).
fn intact(path: &Path, tracked: &Tracked) -> bool {
    let Ok(meta) = path.metadata() else { return false };
    if let Some(expected) = &tracked.sha1 {
        return sha1_file(path).is_some_and(|h| h.eq_ignore_ascii_case(expected));
    }
    if tracked.size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    match tracked.crc32 {
        Some(expected) => crate::remote_zip::file_crc32(path) == Some(expected),
        None => true,
    }
}

/// Un .jar o .zip bajado de un sitio sin huella: al menos que sea un zip que se pueda leer.
fn readable_zip(path: &Path) -> bool {
    std::fs::File::open(path).ok().and_then(|f| zip::ZipArchive::new(f).ok()).is_some_and(|z| !z.is_empty())
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Archivos revisados.
    pub checked: usize,
    /// Lo que se bajó o repuso.
    pub repaired: Vec<String>,
    /// Lo que no se pudo reponer (con el motivo).
    pub failed: Vec<String>,
    /// Los mods de `failed`: sin un resource pack el juego abre igual; sin un mod, no.
    #[serde(skip)]
    pub missing_mods: Vec<String>,
    /// Faltan o están dañados y venían dentro del modpack: el que llama decide cómo reponerlos.
    #[serde(skip)]
    pub from_pack: Vec<Tracked>,
}

impl Report {
    /// Anota lo que no se pudo reponer; `relative` es su ruta en la instancia, si se sabe.
    pub fn fail(&mut self, relative: Option<&str>, text: String) {
        if relative.is_some_and(|r| key(r).starts_with("mods/")) {
            self.missing_mods.push(text.clone());
        }
        self.failed.push(text);
    }
}

pub fn file_name(relative: &str) -> String {
    relative.rsplit('/').next().unwrap_or(relative).to_string()
}

/// Revisa la instancia y baja lo que falte (con `deep`, también lo dañado) y lo que pide Missing Mods Checker.
pub async fn repair(game_dir: &Path, deep: bool, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Report {
    let mut report = Report::default();
    let mut record = load(game_dir).unwrap_or_default();
    // (descarga, nombre para mostrar, ruta en la instancia, sin huella)
    let mut downloads: Vec<(FileDownload, String, String, bool)> = Vec::new();

    let total = record.len().max(1);
    for (i, tracked) in record.iter().enumerate() {
        if deep && i % 25 == 0 {
            progress(t!("Verificando archivos… {i} de {total}", "Checking files… {i} of {total}"), 0.5 * i as f64 / total as f64);
        }
        report.checked += 1;
        let Some(relative) = safe_relative(&tracked.path) else { continue };
        let relative = relative.to_string_lossy().replace('\\', "/");
        let target = match present_path(game_dir, &relative) {
            Some(path) if !deep || intact(&path, tracked) => continue,
            // Dañado: se quita para que la descarga no lo dé por bueno (lo reemplaza igual).
            Some(path) => {
                let _ = std::fs::remove_file(&path);
                path
            }
            None => game_dir.join(&relative),
        };
        match &tracked.url {
            Some(url) => downloads.push((
                FileDownload { url: url.clone(), path: target, sha1: tracked.sha1.clone(), size: tracked.size },
                file_name(&relative),
                relative.clone(),
                tracked.sha1.is_none(),
            )),
            None => report.from_pack.push(Tracked { path: relative, ..tracked.clone() }),
        }
    }

    let (checker, unknown) = checker_missing(game_dir);
    // Lo que ya está en la lista (bajado antes para Missing Mods Checker) se repone una sola vez.
    let listed: HashSet<String> = downloads.iter().map(|(_, _, relative, _)| key(relative)).collect();
    for file in &checker {
        if !listed.contains(&key(&file.path)) {
            let path = game_dir.join(&file.path);
            downloads.push((FileDownload { url: file.url.clone(), path, sha1: None, size: None }, file.name.clone(), file.path.clone(), true));
        }
    }
    for (name, folder) in unknown {
        report.fail(Some(&folder), t!("{name}: el modpack lo pide pero no dice de dónde bajarlo", "{name}: the modpack asks for it but does not say where to download it from"));
    }
    if downloads.is_empty() {
        return report;
    }

    let report_progress = progress.clone();
    let files: Vec<FileDownload> = downloads.iter().map(|(d, _, _, _)| d.clone()).collect();
    let result = download::download_all(files, move |count, total, _, _| {
        report_progress(t!("Bajando lo que le falta al modpack… {count} de {total}", "Downloading what the modpack is missing… {count} of {total}"), 0.5 + 0.5 * count as f64 / total.max(1) as f64)
    })
    .await;
    for (download, name, relative, unverified) in downloads {
        if !download.path.is_file() {
            let reason = if result.is_err() { t!("no se pudo descargar (¿hay internet?)", "could not be downloaded (is there internet?)") } else { t!("no se descargó", "was not downloaded") };
            report.fail(Some(&relative), format!("{name}: {reason}"));
            continue;
        }
        if unverified && !readable_zip(&download.path) {
            let _ = std::fs::remove_file(&download.path);
            report.fail(Some(&relative), t!("{name}: no se bajó bien (no es un .jar ni un .zip)", "{name}: did not download correctly (it is not a .jar or a .zip)"));
            continue;
        }
        report.repaired.push(name);
    }
    // Lo bajado para Missing Mods Checker se vigila igual que lo demás.
    for file in &checker {
        let path = game_dir.join(&file.path);
        if path.is_file() {
            let size = path.metadata().ok().map(|m| m.len());
            record.push(Tracked { path: file.path.clone(), sha1: sha1_file(&path), crc32: None, size, url: Some(file.url.clone()) });
        }
    }
    if !checker.is_empty() {
        save(game_dir, record);
    }
    progress(t!("Listo", "Done"), 1.0);
    report
}

/// Repone desde el .mrpack lo que venía dentro de él. Devuelve (lo repuesto, las rutas que el .mrpack no trae).
pub fn restore_from_mrpack(zip_path: &Path, game_dir: &Path, wanted: &[Tracked]) -> Result<(Vec<String>, Vec<String>), String> {
    let targets: HashMap<String, PathBuf> = wanted
        .iter()
        .map(|t| {
            // Si estaba desactivado, vuelve desactivado.
            let disabled = game_dir.join(format!("{}{DISABLED}", t.path));
            (key(&t.path), if disabled.exists() { disabled } else { game_dir.join(&t.path) })
        })
        .collect();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut restored = Vec::new();
    for prefix in ["overrides/", "client-overrides/"] {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            let Some(rest) = entry.name().strip_prefix(prefix).map(String::from) else { continue };
            let Some(target) = targets.get(&key(&rest)) else { continue };
            std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
            let temp = PathBuf::from(format!("{}.parte", target.display()));
            let mut out = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| t!("El modpack está dañado ({rest}): {e}", "The modpack is damaged ({rest}): {e}"))?;
            drop(out);
            std::fs::rename(&temp, target).map_err(|e| e.to_string())?;
            restored.push(file_name(&rest));
        }
    }
    restored.dedup();
    let absent = wanted.iter().filter(|t| present_path(game_dir, &t.path).is_none()).map(|t| t.path.clone()).collect();
    Ok((restored, absent))
}

/// Modpack de Modrinth del que vino la instancia: (proyecto, versión).
pub fn modrinth_source(instance: &Instance) -> Option<(String, String)> {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(instance.folder.join(INSTANCE_FILE)).ok()?).ok()?;
    let modpack = &json["modpack"];
    if modpack["source"] != "modrinth" {
        return None;
    }
    Some((modpack["projectId"].as_str()?.to_string(), modpack["versionId"].as_str()?.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enlaces_de_curseforge_al_cdn() {
        assert_eq!(
            checker_url("https://www.curseforge.com/minecraft/mc-mods/balm-fabric/download/7420614", "balm-fabric-1.20.1-7.3.38.jar").as_deref(),
            Some("https://mediafilez.forgecdn.net/files/7420/614/balm-fabric-1.20.1-7.3.38.jar")
        );
        // El resto de la división va sin ceros a la izquierda; "+", espacios y corchetes van codificados.
        assert_eq!(
            checker_url("https://www.curseforge.com/minecraft/mc-mods/backpacked/download/4905038", "backpacked fix [1.20].zip").as_deref(),
            Some("https://mediafilez.forgecdn.net/files/4905/38/backpacked%20fix%20%5B1.20%5D.zip")
        );
        assert_eq!(
            checker_url("https://www.curseforge.com/minecraft/mc-mods/fabrication/files/7051719", "fabrication-3.6.5+1.20.jar").as_deref(),
            Some("https://mediafilez.forgecdn.net/files/7051/719/fabrication-3.6.5%2B1.20.jar")
        );
        assert!(checker_url("https://edge.forgecdn.net/files/1/2/a.jar", "a.jar").is_some());
        assert!(checker_url("https://cdn.modrinth.com/data/x/versions/y/a.jar", "a.jar").is_some());
        assert!(checker_url("http://www.curseforge.com/minecraft/mc-mods/a/download/1", "a.jar").is_none());
        assert!(checker_url("https://ejemplo.com/a.jar", "a.jar").is_none());
        assert!(checker_url("https://www.curseforge.com/minecraft/mc-mods/a", "a.jar").is_none());
    }

    #[test]
    fn patrones_como_el_mod() {
        assert_eq!(plain_file_name("balm-fabric-1.20.1-7.3.38.jar").as_deref(), Some("balm-fabric-1.20.1-7.3.38.jar"));
        assert_eq!(plain_file_name(r"balm-fabric-1\.20\.1\.jar").as_deref(), Some("balm-fabric-1.20.1.jar"));
        assert_eq!(plain_file_name("Stay Clear V1.1 [1.20.1].zip").as_deref(), Some("Stay Clear V1.1 [1.20.1].zip"));
        assert!(plain_file_name("balm-fabric-.*\\.jar").is_none());
        assert!(plain_file_name("../fuera.jar").is_none());
        assert!(plain_file_name("leeme.txt").is_none());
        // "+" y "[ ]" son de expresión regular, pero el mod también acepta el nombre igual.
        assert!(matches_pattern("fabrication-3.6.5+1.20.jar", "Fabrication-3.6.5+1.20.jar"));
        assert!(matches_pattern("Stay Clear V1.1 [1.20.1].zip", "Stay Clear V1.1 [1.20.1].zip"));
        assert!(matches_pattern("balm-fabric-.*\\.jar", "balm-fabric-1.20.1-7.3.38.jar"));
        assert!(matches_pattern("balm.jar", "balm.jar.disabled"));
        assert!(!matches_pattern("balm.jar", "balm-extra.jar"));
    }

    #[test]
    fn lo_que_falta_se_clasifica_por_ruta() {
        let mut report = Report::default();
        report.fail(Some("mods/balm.jar"), "Balm (Fabric Edition): no se pudo descargar".into());
        report.fail(Some("Mods/viejo.jar.disabled"), "viejo".into());
        report.fail(Some("resourcepacks/a.zip"), "a.zip".into());
        report.fail(None, "No se pudo bajar el modpack".into());
        assert_eq!(report.failed.len(), 4);
        assert_eq!(report.missing_mods, vec!["Balm (Fabric Edition): no se pudo descargar".to_string(), "viejo".to_string()]);
    }

    #[test]
    fn que_se_vigila() {
        assert!(is_tracked("mods/sodium.jar"));
        assert!(is_tracked("Resourcepacks/a.zip"));
        assert!(is_tracked("shaderpacks/BSL/shaders/a.fsh"));
        assert!(!is_tracked("shaderpacks/BSL.zip.txt"));
        assert!(!is_tracked("mods/mcef-libraries/a.dll"));
        assert!(!is_tracked("config/sodium.json"));
        assert!(!is_tracked("options.txt"));
        assert!(!is_tracked("mods/"));
        assert_eq!(key("mods/Sodium.jar.disabled"), "mods/sodium.jar");
    }
}
