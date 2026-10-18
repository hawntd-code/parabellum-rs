//! Paquetes para Drive y USB, con el mismo formato que la versión C# (así las dos se entienden y el
//! catálogo de Drive que ya existe sigue sirviendo):
//!  - modpack: zip con lanzador-pack.json, la carpeta de la instancia (instancia/) y las bibliotecas
//!    propias del loader (base/). Junto al zip va un .json igual al manifiesto con el tamaño y el
//!    SHA-256 del zip (para leer el catálogo sin descargar nada) y el icono (.png).
//!  - base: Minecraft de una versión con sus bibliotecas, sonidos, idiomas y Java (base/). La comparten
//!    todos los modpacks de esa versión, así solo se descarga una vez.
//! Todo lo necesario viaja dentro: se instala sin Mojang, Fabric ni Modrinth (la red de la escuela).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

use crate::instances::{Instance, INSTANCE_FILE, STATE_DIR};
use crate::launch;
use crate::paths::Paths;
use crate::settings::Settings;

pub const MANIFEST_IN_ARCHIVE: &str = "lanzador-pack.json";
/// Manifiesto del paquete instalado, dentro de .lanzador (para saber si hay versión nueva).
pub const INSTALLED_MANIFEST: &str = "pack.json";
const REGISTRY: &str = "paquetes.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Manifest {
    pub format: u32,
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub name: String,
    pub description: String,
    /// Marca de versión ("202610072130"); si cambia, hay actualización.
    pub version: String,
    pub minecraft_version: String,
    /// "Fabric 0.19.5", "Vanilla"…
    pub loader: String,
    pub version_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_id: Option<String>,
    pub mod_count: u32,
    /// Carpeta dentro de "instancias".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    pub archive: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub archive_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_sha256: Option<String>,
    pub installed_size: u64,
    pub exported_at: String,
}

impl Manifest {
    pub fn is_base(&self) -> bool {
        self.kind == "base"
    }
}

pub fn base_id(minecraft: &str) -> String {
    format!("minecraft-{minecraft}")
}

/// Igual que en la versión C#: "Minecraft++" -> "minecraft-plus-plus".
pub fn slug(text: &str) -> String {
    let lowered = text.replace('+', " plus ").to_lowercase();
    let plain: String = lowered
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c if c.is_ascii_alphanumeric() => c,
            _ => '-',
        })
        .collect();
    let joined = plain.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if joined.is_empty() { "pack".into() } else { joined }
}

fn safe_file_name(name: &str) -> String {
    let clean: String = name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim().trim_matches('.').to_string();
    if clean.is_empty() { "modpack".into() } else { clean }
}

// ================= Qué se exporta

/// Nunca se exporta: registros, cachés que el juego vuelve a crear y archivos del launcher.
const ALWAYS_SKIPPED: [&str; 10] = ["logs", "crash-reports", ".mixin.out", STATE_DIR, ".fabric", ".quilt", ".cache", "debug", "natives", INSTANCE_FILE];
/// Se ofrece desmarcado: lo personal del jugador.
const UNCHECKED: [&str; 7] = ["saves", "screenshots", "usercache.json", "usernamecache.json", "command_history.txt", "realms_persistence.json", "downloads"];

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub folder: bool,
    pub size: u64,
    pub checked: bool,
}

/// Lo que hay en la instancia para elegir qué exportar.
pub fn exportable(instance: &Instance) -> Vec<Entry> {
    let mut list: Vec<Entry> = std::fs::read_dir(&instance.game_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if ALWAYS_SKIPPED.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
                return None;
            }
            let folder = entry.file_type().ok()?.is_dir();
            let size = if folder { walk(&entry.path()).iter().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum() } else { entry.metadata().ok()?.len() };
            Some(Entry { checked: !UNCHECKED.iter().any(|u| u.eq_ignore_ascii_case(&name)), name, folder, size })
        })
        .collect();
    list.sort_by_key(|e| (!e.folder, e.name.to_lowercase()));
    list
}

/// Archivos (no carpetas) dentro de una carpeta, recursivo.
pub fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).into_iter().flatten().flatten() {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(entry.path()),
                Ok(kind) if kind.is_file() => files.push(entry.path()),
                _ => {}
            }
        }
    }
    files
}

/// Los archivos elegidos de la instancia (los nombres de `include` son de su primer nivel).
pub fn selected_files(instance: &Instance, include: &[String]) -> Vec<PathBuf> {
    let wanted: HashSet<String> = include.iter().map(|n| n.to_lowercase()).collect();
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&instance.game_dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !wanted.contains(&name.to_lowercase()) || ALWAYS_SKIPPED.iter().any(|s| s.eq_ignore_ascii_case(&name)) {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => files.extend(walk(&entry.path())),
            Ok(kind) if kind.is_file() => files.push(entry.path()),
            _ => {}
        }
    }
    // El mod de Parabellum (está en mods mientras se juega) no se exporta.
    files.retain(|f| !f.file_name().is_some_and(|n| crate::juego::is_ours(&n.to_string_lossy())));
    files
}

// ================= Escribir zips

/// Comprimir archivos que ya vienen comprimidos solo gasta tiempo.
fn already_compressed(path: &Path) -> bool {
    match path.extension().map(|e| e.to_string_lossy().to_lowercase()) {
        Some(e) => ["jar", "zip", "png", "ogg", "jpg", "jpeg", "gz", "dll", "7z", "mrpack", "webp"].contains(&e.as_str()),
        // Los sonidos y texturas de Minecraft no tienen extensión
        None => path.components().any(|c| c.as_os_str() == "objects"),
    }
}

pub(crate) struct ZipOut {
    writer: zip::ZipWriter<std::io::BufWriter<std::fs::File>>,
    pub written: u64,
}

impl ZipOut {
    pub fn create(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let file = std::fs::File::create(path).map_err(|e| t!("No se pudo crear {}: {e}", "Could not create {}: {e}", path.display()))?;
        Ok(Self { writer: zip::ZipWriter::new(std::io::BufWriter::with_capacity(1 << 20, file)), written: 0 })
    }

    pub fn add_bytes(&mut self, name: &str, bytes: &[u8]) -> Result<(), String> {
        let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        self.writer.start_file(name, options).map_err(|e| e.to_string())?;
        self.writer.write_all(bytes).map_err(|e| e.to_string())
    }

    pub fn add_file(&mut self, source: &Path, name: &str) -> Result<(), String> {
        let size = source.metadata().map(|m| m.len()).unwrap_or(0);
        let method = if already_compressed(source) { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        let options = SimpleFileOptions::default().compression_method(method).large_file(size >= u32::MAX as u64);
        self.writer.start_file(name.replace('\\', "/"), options).map_err(|e| e.to_string())?;
        let mut input = std::fs::File::open(source).map_err(|e| t!("No se pudo leer {}: {e}", "Could not read {}: {e}", source.display()))?;
        std::io::copy(&mut input, &mut self.writer).map_err(|e| e.to_string())?;
        self.written += size;
        Ok(())
    }

    pub fn finish(self) -> Result<(), String> {
        let mut inner = self.writer.finish().map_err(|e| e.to_string())?;
        inner.flush().map_err(|e| e.to_string())
    }
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn relative(root: &Path, file: &Path) -> Option<String> {
    Some(file.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/"))
}

fn total_size(files: &[PathBuf]) -> u64 {
    files.iter().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum()
}

// ================= Exportar

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ExportOptions {
    /// Nombres del primer nivel de la instancia que se incluyen (mods, config, options.txt…).
    pub include: Vec<String>,
    pub description: String,
    /// Solo .mrpack: nombre y versión del modpack.
    pub name: String,
    pub version: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// Carpeta que se sube a Drive tal cual (la de la versión de Minecraft).
    pub folder: String,
    pub modpack: String,
    pub base: String,
    /// La base ya estaba en la carpeta de salida y no se volvió a crear.
    pub base_reused: bool,
    pub size: u64,
}

/// Exporta la instancia como paquete para Drive/USB en `<salida>/<versión>/`:
/// base-minecraft-<versión>.zip/.json y <Modpack>/<Modpack>.zip/.json/.png.
pub fn export(instance: &Instance, paths: &Paths, out_dir: &Path, options: &ExportOptions, progress: impl Fn(String, f64)) -> Result<ExportResult, String> {
    progress(t!("Revisando la instancia…", "Checking the instance…"), 0.0);
    let plan = launch::plan(instance, &Settings::default(), paths)?;
    if !plan.missing.is_empty() {
        return Err(t!("A la instancia le faltan archivos y no se puede exportar:\n{}", "The instance is missing files and cannot be exported:\n{}", plan.missing.iter().take(5).cloned().collect::<Vec<_>>().join("\n")));
    }
    let version = launch::version_files(&instance.version_id, paths)?;
    let vanilla = launch::version_files(&version.root_id, paths)?;
    let java = vanilla.own_java().or(version.own_java()).ok_or_else(|| missing_java(vanilla.java_major))?;

    // Base: Minecraft puro + Java. Lo demás que use la versión (el loader) viaja con el modpack.
    let mut base_files: Vec<PathBuf> = vanilla.files.iter().filter(|f| f.is_file()).cloned().collect();
    base_files.extend(java_files(java));
    let base_set: HashSet<String> = base_files.iter().map(|f| f.to_string_lossy().to_lowercase()).collect();
    let loader_files: Vec<PathBuf> =
        version.files.iter().filter(|f| f.is_file() && !base_set.contains(&f.to_string_lossy().to_lowercase())).cloned().collect();

    let stamp = chrono::Local::now();
    let version_stamp = stamp.format("%Y%m%d%H%M").to_string();
    let exported_at = stamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    let base_id = base_id(&version.root_id);
    let version_dir = out_dir.join(safe_file_name(&version.root_id));
    let base_name = format!("base-{base_id}");
    let base_zip = version_dir.join(format!("{base_name}.zip"));
    let base_json = version_dir.join(format!("{base_name}.json"));
    let base_reused = base_zip.exists() && base_json.exists();

    // instancia.json (las de CurseForge traen minecraftinstance.json, que sale con lo elegido).
    let mut instance_files: Vec<PathBuf> = Some(instance.folder.join(INSTANCE_FILE)).filter(|f| f.is_file()).into_iter().collect();
    instance_files.extend(selected_files(instance, &options.include));
    let icon = instance.state_dir().join("icono.png");
    let total = (total_size(&instance_files) + total_size(&loader_files) + if base_reused { 0 } else { total_size(&base_files) }).max(1);
    let mut done = 0u64;
    let mut report = |stage: &str, bytes: u64| {
        done += bytes;
        progress(t!("{stage} {:.0} de {:.0} MB…", "{stage} {:.0} of {:.0} MB…", done as f64 / 1048576.0, total as f64 / 1048576.0), done as f64 / total as f64);
    };

    if !base_reused {
        let manifest = base_manifest(&version.root_id, java.major, total_size(&base_files), &version_stamp, &exported_at);
        let entries: Vec<(PathBuf, String)> =
            base_files.iter().filter_map(|f| Some((f.clone(), format!("base/{}", relative(&paths.base, f)?)))).collect();
        write_archive(&base_zip, &manifest, &entries, |bytes| report(&t!("Empaquetando Minecraft", "Packing Minecraft"), bytes))?;
        write_sidecar(&base_zip, &base_json, manifest)?;
    }

    let pack_name = safe_file_name(&instance.name);
    let pack_dir = version_dir.join(&pack_name);
    let pack_zip = pack_dir.join(format!("{pack_name}.zip"));
    let mut manifest = Manifest {
        format: 1,
        kind: "modpack".into(),
        id: slug(&instance.id),
        name: instance.name.clone(),
        description: options.description.trim().to_string(),
        version: version_stamp,
        minecraft_version: instance.minecraft_version.clone(),
        loader: match &instance.loader_version {
            Some(v) => format!("{} {v}", instance.loader),
            None => instance.loader.clone(),
        },
        version_id: instance.version_id.clone(),
        base_id: Some(base_id),
        mod_count: std::fs::read_dir(instance.game_dir.join("mods"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar"))
            .count() as u32,
        folder: Some(instance.id.clone()),
        archive: format!("{pack_name}.zip"),
        installed_size: total_size(&instance_files) + total_size(&loader_files),
        exported_at,
        ..Default::default()
    };
    let mut entries: Vec<(PathBuf, String)> =
        instance_files.iter().filter_map(|f| Some((f.clone(), format!("instancia/{}", relative(&instance.folder, f)?)))).collect();
    entries.extend(loader_files.iter().filter_map(|f| Some((f.clone(), format!("base/{}", relative(&paths.base, f)?)))));
    std::fs::create_dir_all(&pack_dir).map_err(|e| e.to_string())?;
    if icon.is_file() {
        // El icono va dentro del zip y también junto al .json, para verlo en el catálogo sin descargar.
        std::fs::copy(&icon, pack_dir.join(format!("{pack_name}.png"))).map_err(|e| e.to_string())?;
        manifest.icon = Some(format!("{pack_name}.png"));
        entries.push((icon, format!("instancia/{STATE_DIR}/icono.png")));
    }
    write_archive(&pack_zip, &manifest, &entries, |bytes| report(&t!("Empaquetando el modpack", "Packing the modpack"), bytes))?;
    write_sidecar(&pack_zip, &pack_dir.join(format!("{pack_name}.json")), manifest)?;
    progress(t!("Listo", "Done"), 1.0);
    Ok(ExportResult {
        folder: version_dir.display().to_string(),
        modpack: pack_zip.display().to_string(),
        base: base_zip.display().to_string(),
        base_reused,
        size: std::fs::metadata(&pack_zip).map(|m| m.len()).unwrap_or(0),
    })
}

fn base_manifest(minecraft: &str, java_major: u32, installed_size: u64, version_stamp: &str, exported_at: &str) -> Manifest {
    let id = base_id(minecraft);
    Manifest {
        format: 1,
        kind: "base".into(),
        id: id.clone(),
        name: format!("Minecraft {minecraft} (base)"),
        description: format!("Minecraft {minecraft}, sus bibliotecas, sonidos e idiomas y Java {java_major}."),
        version: version_stamp.to_string(),
        minecraft_version: minecraft.to_string(),
        loader: "Vanilla".into(),
        version_id: minecraft.to_string(),
        archive: format!("base-{id}.zip"),
        installed_size,
        exported_at: exported_at.to_string(),
        ..Default::default()
    }
}

/// El paquete lleva el Java que pide la versión: uno de reemplazo (más nuevo) le llegaría a todos.
fn missing_java(major: u32) -> String {
    t!(
        "Falta Java {major} en este PC (o quedó a medias). Abre una vez esa versión con internet para que se complete.",
        "Java {major} is missing on this PC (or it is incomplete). Open that version once with internet so it is completed."
    )
}

/// Los archivos de un Java para un paquete, sin los «.parte» que pudieran haber quedado sueltos.
fn java_files(java: &crate::java::JavaRuntime) -> impl Iterator<Item = PathBuf> {
    walk(&java.home).into_iter().filter(|f| !f.extension().is_some_and(|e| e == "parte"))
}

fn write_archive(zip_path: &Path, manifest: &Manifest, entries: &[(PathBuf, String)], mut on_file: impl FnMut(u64)) -> Result<(), String> {
    // Se escribe en un temporal: si algo falla no queda un zip a medias con el nombre bueno.
    let temp = zip_path.with_extension("zip.tmp");
    let result = (|| {
        let mut zip = ZipOut::create(&temp)?;
        zip.add_bytes(MANIFEST_IN_ARCHIVE, &serde_json::to_vec_pretty(manifest).unwrap())?;
        for (file, name) in entries {
            // La tecla del panel de quien exporta no viaja: a cada quien le queda la suya (la O).
            match (name.ends_with("instancia/options.txt"), std::fs::read_to_string(file)) {
                (true, Ok(text)) => zip.add_bytes(name, without_panel_key(&text).as_bytes())?,
                _ => zip.add_file(file, name)?,
            }
            on_file(file.metadata().map(|m| m.len()).unwrap_or(0));
        }
        zip.finish()
    })();
    match result {
        Ok(()) => std::fs::rename(&temp, zip_path).map_err(|e| e.to_string()),
        Err(e) => {
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

/// options.txt sin la tecla del panel de Parabellum (lo vio Diego en el modpack «Fabric 26.3»: venía en «[»).
fn without_panel_key(text: &str) -> String {
    text.lines().filter(|l| !l.starts_with("key_key.parabellum.abrir:")).map(|l| format!("{l}\n")).collect()
}

fn write_sidecar(zip_path: &Path, json_path: &Path, mut manifest: Manifest) -> Result<(), String> {
    manifest.archive_size = std::fs::metadata(zip_path).map_err(|e| e.to_string())?.len();
    manifest.archive_sha256 = Some(sha256_file(zip_path).map_err(|e| e.to_string())?);
    std::fs::write(json_path, serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e| e.to_string())
}

// ================= Instalar

pub fn read_manifest(zip_path: &Path) -> Result<Option<Manifest>, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| t!("No se pudo abrir el archivo: {e}", "Could not open the file: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| t!("El archivo no es un zip válido.", "The file is not a valid zip."))?;
    let Ok(mut entry) = zip.by_name(MANIFEST_IN_ARCHIVE) else { return Ok(None) };
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map(Some).map_err(|e| t!("El manifiesto del paquete está dañado: {e}", "The package manifest is damaged: {e}"))
}

/// Si junto al zip está su .json (como en Drive), comprueba que el zip llegó completo (SHA-256).
pub fn verify_sidecar(zip_path: &Path) -> Result<bool, String> {
    let sidecar = zip_path.with_extension("json");
    let Some(expected) = std::fs::read_to_string(&sidecar)
        .ok()
        .and_then(|t| serde_json::from_str::<Manifest>(&t).ok())
        .and_then(|m| m.archive_sha256)
    else {
        return Ok(false);
    };
    let actual = sha256_file(zip_path).map_err(|e| e.to_string())?;
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(t!("El archivo está dañado o incompleto (su huella SHA-256 no coincide). Vuelve a copiarlo o descargarlo.", "The file is damaged or incomplete (its SHA-256 fingerprint does not match). Copy or download it again."));
    }
    Ok(true)
}

fn registry(paths: &Paths) -> HashMap<String, String> {
    std::fs::read_to_string(paths.base.join(REGISTRY)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub(crate) fn register(paths: &Paths, id: &str, version: &str) {
    let mut list = registry(paths);
    list.insert(id.to_string(), version.to_string());
    let _ = std::fs::write(paths.base.join(REGISTRY), serde_json::to_vec_pretty(&list).unwrap());
}

/// ¿Está Minecraft de esa versión en este PC (por un paquete base o descargado de Mojang)?
pub fn base_available(minecraft: &str, paths: &Paths) -> bool {
    registry(paths).contains_key(&base_id(minecraft)) || (paths.version_json(minecraft).exists() && paths.version_jar(minecraft).exists())
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub kind: String,
    pub name: String,
    /// Carpeta de la instancia (modpacks).
    pub folder: Option<String>,
    /// Ya existía y se actualizó conservando mundos y opciones.
    pub updated: bool,
}

/// Lo que un modpack actualizado conserva de la instancia anterior.
pub(crate) const PRESERVED_ON_UPDATE: [&str; 14] = [
    "saves", "screenshots", "schematics", "options.txt", "optionsof.txt", "optionsshaders.txt", "servers.dat", "XaeroWaypoints",
    "XaeroWorldMap", "xaero", "journeymap", "local", ".lanzador/ultima-partida.txt", ".lanzador/ajustes.json",
];

pub fn valid_folder_name(name: &str) -> bool {
    !name.trim().is_empty() && name != "." && name != ".." && !name.chars().any(|c| r#"<>:"/\|?*"#.contains(c) || c.is_control())
}

/// Instala un paquete (base o modpack). `progress`: (texto, fracción).
pub fn install(zip_path: &Path, paths: &Paths, progress: impl Fn(String, f64)) -> Result<Installed, String> {
    let manifest = read_manifest(zip_path)?.ok_or(t!("El zip no es un paquete de Parabellum (no tiene lanzador-pack.json).", "The zip is not a Parabellum package (it has no lanzador-pack.json)."))?;
    if !manifest.is_base() && !base_available(&manifest.minecraft_version, paths) {
        return Err(t!(
            "Falta Minecraft {mc} en este PC. Instala primero «base-minecraft-{mc}.zip» (va junto al modpack) o crea una instancia de {mc} con internet.", "Minecraft {mc} is missing on this PC. First install «base-minecraft-{mc}.zip» (it goes next to the modpack) or create a {mc} instance with internet.",
            mc = manifest.minecraft_version
        ));
    }
    let folder = manifest.folder.clone().filter(|f| !f.is_empty()).unwrap_or_else(|| manifest.name.clone());
    if !manifest.is_base() && !valid_folder_name(&folder) {
        return Err(t!("El nombre de carpeta del paquete no es válido.", "The package folder name is not valid."));
    }

    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let staging = paths.data.join("temporal").join(format!("paquete-{}", std::process::id() as u128 * 1_000_000 + chrono::Utc::now().timestamp_subsec_micros() as u128));
    let total: u64 = (0..zip.len()).filter_map(|i| zip.by_index_raw(i).ok().map(|e| e.size())).sum::<u64>().max(1);
    let mut done = 0u64;
    let mut listed: Vec<(String, u32, u64)> = Vec::new();
    let mut java_names: Vec<String> = Vec::new();
    let result = (|| -> Result<Installed, String> {
        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                continue;
            }
            // enclosed_name rechaza rutas que intenten salir de la carpeta ("../", rutas absolutas).
            let Some(name) = entry.enclosed_name() else { continue };
            let target = if let Ok(rest) = name.strip_prefix("base") {
                paths.base.join(rest)
            } else if let (Ok(rest), false) = (name.strip_prefix("instancia"), manifest.is_base()) {
                staging.join(rest)
            } else {
                continue;
            };
            let size = entry.size();
            if !manifest.is_base() {
                listed.push((name.to_string_lossy().replace('\\', "/"), entry.crc32(), size));
            }
            if target.starts_with(&paths.java) {
                java_names.push(name.to_string_lossy().replace('\\', "/"));
            }
            // Los archivos de base son compartidos: si ya están iguales (mismo tamaño) no se reescriben.
            let shared_present = target.starts_with(&paths.base) && target.metadata().is_ok_and(|m| m.len() == size);
            if !shared_present {
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                let temp = PathBuf::from(format!("{}.parte", target.display()));
                let mut out = std::fs::File::create(&temp).map_err(|e| e.to_string())?;
                // Al leer hasta el final, zip comprueba el CRC32: un archivo dañado da error aquí.
                std::io::copy(&mut entry, &mut out).map_err(|e| t!("El paquete está dañado ({}): {e}", "The package is damaged ({}): {e}", name.display()))?;
                drop(out);
                std::fs::rename(&temp, &target).map_err(|e| e.to_string())?;
            }
            done += size;
            progress(t!("Instalando {:.0} de {:.0} MB…", "Installing {:.0} of {:.0} MB…", done as f64 / 1048576.0, total as f64 / 1048576.0), done as f64 / total as f64);
        }
        crate::java::package_installed(paths, java_names.iter().map(String::as_str));
        if manifest.is_base() {
            register(paths, &manifest.id, &manifest.version);
            return Ok(Installed { kind: "base".into(), name: manifest.name.clone(), folder: None, updated: false });
        }
        let state = staging.join(STATE_DIR);
        std::fs::create_dir_all(&state).map_err(|e| e.to_string())?;
        std::fs::write(state.join(INSTALLED_MANIFEST), serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
        crate::integridad::save_package(&staging, &listed);
        let destination = paths.instances.join(&folder);
        let updated = destination.exists();
        if updated {
            let backup = paths.data.join("temporal").join(format!("respaldo-{}", chrono::Utc::now().timestamp_millis()));
            replace_keeping_player_data(&destination, &staging, &backup).map_err(|e| t!("No se pudo actualizar la instancia: {e}", "Could not update the instance: {e}"))?;
        } else {
            std::fs::create_dir_all(&paths.instances).map_err(|e| e.to_string())?;
            std::fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
        }
        Ok(Installed { kind: "modpack".into(), name: manifest.name.clone(), folder: Some(folder.clone()), updated })
    })();
    let _ = std::fs::remove_dir_all(&staging);
    result
}

/// Actualiza sin arriesgar los mundos: la carpeta vieja pasa a respaldo, sus datos del jugador se
/// mueven a la nueva y solo al final se borra el respaldo. Si algo falla, todo vuelve como estaba.
fn replace_keeping_player_data(destination: &Path, staging: &Path, backup: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(backup.parent().unwrap())?;
    std::fs::rename(destination, backup)?;
    let mut moved: Vec<&str> = Vec::new();
    let result = (|| -> std::io::Result<()> {
        for name in PRESERVED_ON_UPDATE {
            let old = backup.join(name);
            let fresh = staging.join(name);
            if !old.exists() {
                continue;
            }
            if fresh.is_dir() {
                std::fs::remove_dir_all(&fresh)?;
            } else if fresh.exists() {
                std::fs::remove_file(&fresh)?;
            }
            std::fs::create_dir_all(fresh.parent().unwrap())?;
            std::fs::rename(&old, &fresh)?;
            moved.push(name);
        }
        // La memoria y los argumentos de Java propios de este PC también se conservan.
        keep_local_settings(&backup.join(INSTANCE_FILE), &staging.join(INSTANCE_FILE));
        std::fs::rename(staging, destination)
    })();
    if let Err(error) = result {
        for name in moved {
            let _ = std::fs::rename(staging.join(name), backup.join(name));
        }
        if !destination.exists() {
            let _ = std::fs::rename(backup, destination);
        }
        return Err(error);
    }
    let _ = std::fs::remove_dir_all(backup);
    Ok(())
}

pub(crate) fn keep_local_settings(old: &Path, fresh: &Path) {
    let read = |p: &Path| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let (Some(old), Some(mut fresh_json)) = (read(old), read(fresh)) else { return };
    for key in ["ramGB", "extraJvmArgs"] {
        if let Some(value) = old.get(key) {
            fresh_json[key] = value.clone();
        }
    }
    let _ = std::fs::write(fresh, serde_json::to_vec_pretty(&fresh_json).unwrap());
}

/// Manifiesto del paquete con el que se instaló la instancia (si vino de un paquete).
pub fn installed_manifest(instance: &Instance) -> Option<Manifest> {
    serde_json::from_str(&std::fs::read_to_string(instance.state_dir().join(INSTALLED_MANIFEST)).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_como_en_csharp() {
        assert_eq!(slug("Minecraft++"), "minecraft-plus-plus");
        assert_eq!(slug("Better MC BMC4"), "better-mc-bmc4");
        assert_eq!(slug("Mundo Pequeño"), "mundo-pequeno");
        assert_eq!(slug("***"), "pack");
    }

    #[test]
    fn manifiesto_compatible() {
        let text = r#"{"format":1,"type":"modpack","id":"minecraft-plus-plus","name":"Minecraft++","description":"",
            "version":"202610042111","minecraftVersion":"26.1.2","loader":"Fabric 0.19.5","versionId":"fabric-loader-0.19.5-26.1.2",
            "baseId":"minecraft-26.1.2","modCount":47,"folder":"Minecraft++","archive":"Minecraft++.zip",
            "archiveSize":356762003,"archiveSha256":"45f5","installedSize":373093270,"exportedAt":"2026-10-04T21:11:56.8686056-07:00"}"#;
        let manifest: Manifest = serde_json::from_str(text).unwrap();
        assert_eq!(manifest.name, "Minecraft++");
        assert_eq!(manifest.base_id.as_deref(), Some("minecraft-26.1.2"));
        assert!(!manifest.is_base());
        let back = serde_json::to_value(&manifest).unwrap();
        assert_eq!(back["minecraftVersion"], "26.1.2");
        assert_eq!(back["type"], "modpack");
    }
}
