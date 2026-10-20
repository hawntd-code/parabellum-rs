//! Identidad de una instalación y su huella de contenido (docs/plan-diciembre.md, sección 4).
//!
//! La pregunta que responde: ¿esta instancia es la misma que el paquete del que viene? Dos instancias
//! son iguales si tienen el mismo Minecraft, el mismo loader y los mismos mods activos **por su
//! contenido** (SHA-1), no por su nombre de archivo. No cuentan las opciones, los mundos, los resource
//! packs, los mods desactivados ni el mod de Parabellum (cambian por persona o por sistema).
//!
//! Al instalar, actualizar o reparar un paquete, y al exportarlo, se guarda la referencia
//! (`.lanzador/referencia.json`): la huella de lo que trae el paquete. Después, `check` compara la
//! instancia con esa referencia y dice qué cambió. Las huellas de cada archivo se guardan en
//! `.lanzador/huellas.json` (por tamaño y fecha): solo se recalcula lo que cambió.

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::instances::Instance;

pub const INSTALLATION_FILE: &str = "instalacion.json";
pub const REFERENCE_FILE: &str = "referencia.json";
const CACHE_FILE: &str = "huellas.json";

/// Un mod activo, por su contenido.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub file: String,
    pub sha1: String,
}

/// Lo que importa para la compatibilidad de una instancia, con su huella.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Content {
    pub minecraft: String,
    /// «Fabric 0.20.0», «Vanilla».
    pub loader: String,
    /// Ordenados por SHA-1.
    pub mods: Vec<ModFile>,
    /// «sha256:…» del JSON canónico (Minecraft, loader y los SHA-1 ordenados).
    pub content_hash: String,
}

/// La huella del paquete del que viene la instancia, guardada al instalarlo o exportarlo.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Reference {
    pub format: u32,
    /// Id del paquete en el catálogo (manifest.id) y su versión.
    pub package: String,
    pub version: String,
    /// «instalado» (vino del paquete) o «publicado» (esta instancia es la original).
    pub origin: String,
    pub at: i64,
    #[serde(flatten)]
    pub content: Content,
}

/// Cómo está una instancia respecto de su paquete.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    /// «exacta», «modificada» o «desconocida» (no hay con qué comparar).
    pub state: String,
    /// Mods activos que el paquete no trae (por nombre de archivo).
    pub added: Vec<String>,
    /// Mods del paquete que no están activos (quitados, desactivados o de otra versión).
    pub removed: Vec<String>,
    /// (paquete, instancia) si cambió la versión de Minecraft o el loader.
    pub minecraft: Option<(String, String)>,
    pub loader: Option<(String, String)>,
    /// La referencia: de qué paquete y versión, y si la instancia es la original.
    pub package: Option<String>,
    pub version: Option<String>,
    pub origin: Option<String>,
}

impl Diff {
    pub fn modified(&self) -> bool {
        self.state == "modificada"
    }
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

/// El id de esta instalación: nace la primera vez que se pide y no cambia al renombrar la instancia.
pub fn installation_id(instance: &Instance) -> String {
    let file = instance.state_dir().join(INSTALLATION_FILE);
    if let Some(id) = std::fs::read_to_string(&file).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()).and_then(|v| v["installationId"].as_str().map(String::from)) {
        return id;
    }
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("números aleatorios");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let id = format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]);
    let _ = std::fs::create_dir_all(instance.state_dir());
    let _ = std::fs::write(&file, serde_json::to_vec_pretty(&serde_json::json!({ "format": 1, "installationId": id, "created": now_ms() })).unwrap());
    id
}

/// El loader como se guarda en los paquetes: «Fabric 0.20.0» o «Vanilla».
pub fn loader_label(instance: &Instance) -> String {
    match instance.loader_version.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(version) => format!("{} {}", instance.loader, version.trim()),
        None => instance.loader.clone(),
    }
}

/// La huella: SHA-256 del JSON canónico. Da lo mismo en Windows y Mac, y no depende de los nombres.
pub fn hash(minecraft: &str, loader: &str, mods: &[ModFile]) -> String {
    let mut sha1s: Vec<&str> = mods.iter().map(|m| m.sha1.as_str()).collect();
    sha1s.sort_unstable();
    sha1s.dedup();
    let canonical = serde_json::json!({ "format": 1, "loader": loader.trim().to_lowercase(), "minecraft": minecraft.trim(), "mods": sha1s });
    let digest = Sha256::digest(serde_json::to_vec(&canonical).unwrap());
    format!("sha256:{}", digest.iter().map(|b| format!("{b:02x}")).collect::<String>())
}

fn build(minecraft: &str, loader: &str, mut mods: Vec<ModFile>) -> Content {
    mods.sort_by(|a, b| a.sha1.cmp(&b.sha1).then(a.file.cmp(&b.file)));
    mods.dedup_by(|a, b| a.sha1 == b.sha1);
    Content { content_hash: hash(minecraft, loader, &mods), minecraft: minecraft.trim().to_string(), loader: loader.trim().to_string(), mods }
}

/// ¿Es un mod activo que cuenta para la huella? Un .jar (no .disabled) que no sea el de Parabellum.
pub fn counts(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    lower.ends_with(".jar") && !crate::juego::is_ours(file_name)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Cached {
    size: u64,
    modified: i64,
    sha1: String,
}

fn modified_ms(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub fn sha1_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buffer).ok()?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Some(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// SHA-1 de varios archivos de la instancia, con la caché de huellas (solo se lee lo que cambió).
fn sha1s(instance: &Instance, files: &[PathBuf]) -> Vec<ModFile> {
    let cache_path = instance.state_dir().join(CACHE_FILE);
    let mut cache: HashMap<String, Cached> = std::fs::read(&cache_path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let mut changed = false;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for path in files {
        let Ok(meta) = path.metadata() else { continue };
        let key = path.strip_prefix(&instance.folder).unwrap_or(path).to_string_lossy().replace('\\', "/").to_lowercase();
        seen.insert(key.clone());
        let modified = modified_ms(&meta);
        let sha1 = match cache.get(&key).filter(|c| c.size == meta.len() && c.modified == modified) {
            Some(hit) => hit.sha1.clone(),
            None => {
                let Some(sha1) = sha1_file(path) else { continue };
                cache.insert(key, Cached { size: meta.len(), modified, sha1: sha1.clone() });
                changed = true;
                sha1
            }
        };
        out.push(ModFile { file: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), sha1 });
    }
    // Lo que ya no está no se guarda más.
    let before = cache.len();
    cache.retain(|k, _| seen.contains(k) || !k.contains("/mods/") && !k.starts_with("mods/"));
    if changed || cache.len() != before {
        let _ = std::fs::create_dir_all(instance.state_dir());
        let _ = std::fs::write(&cache_path, serde_json::to_vec(&cache).unwrap_or_default());
    }
    out
}

/// Los mods activos de la instancia (game_dir/mods, sin subcarpetas: el juego no las carga).
pub fn active_mods(instance: &Instance) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(instance.game_dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().is_some_and(|n| counts(&n.to_string_lossy())))
        .collect();
    files.sort();
    files
}

/// La huella de la instancia como está ahora.
pub fn content(instance: &Instance) -> Content {
    build(&instance.minecraft_version, &loader_label(instance), sha1s(instance, &active_mods(instance)))
}

/// La huella de ciertos mods de la instancia (los que trae un paquete), con su Minecraft y loader.
pub fn content_of(instance: &Instance, mods: &[PathBuf]) -> Content {
    let files: Vec<PathBuf> = mods.iter().filter(|p| p.file_name().is_some_and(|n| counts(&n.to_string_lossy()))).cloned().collect();
    build(&instance.minecraft_version, &loader_label(instance), sha1s(instance, &files))
}

/// De las rutas de un paquete («instancia/mods/sodium.jar»), las de los mods de esta instancia.
pub fn package_mods<'a>(instance: &Instance, entries: impl IntoIterator<Item = &'a str>) -> Vec<PathBuf> {
    let mods_dir = instance.game_dir.join("mods");
    entries
        .into_iter()
        .filter_map(|name| name.strip_prefix("instancia/"))
        .filter_map(crate::mrpack::safe_relative)
        .map(|relative| instance.folder.join(relative))
        .filter(|path| path.parent().is_some_and(|p| p == mods_dir) && path.file_name().is_some_and(|n| counts(&n.to_string_lossy())))
        .collect()
}

pub fn reference(instance: &Instance) -> Option<Reference> {
    serde_json::from_slice(&std::fs::read(instance.state_dir().join(REFERENCE_FILE)).ok()?).ok()
}

/// Guarda la referencia de la instancia: la huella de lo que trae su paquete.
pub fn save_reference(instance: &Instance, package: &str, version: &str, origin: &str, content: Content) -> Reference {
    let reference = Reference { format: 1, package: package.to_string(), version: version.to_string(), origin: origin.to_string(), at: now_ms(), content };
    let _ = std::fs::create_dir_all(instance.state_dir());
    let _ = std::fs::write(instance.state_dir().join(REFERENCE_FILE), serde_json::to_vec_pretty(&reference).unwrap());
    reference
}

/// Compara la instancia (`local`) con su paquete (`expected`).
pub fn compare(local: &Content, expected: &Content) -> Diff {
    let mut diff = Diff { state: "exacta".into(), ..Default::default() };
    if local.content_hash == expected.content_hash {
        return diff;
    }
    let have: HashSet<&str> = local.mods.iter().map(|m| m.sha1.as_str()).collect();
    let want: HashSet<&str> = expected.mods.iter().map(|m| m.sha1.as_str()).collect();
    diff.added = local.mods.iter().filter(|m| !want.contains(m.sha1.as_str())).map(|m| m.file.clone()).collect();
    diff.removed = expected.mods.iter().filter(|m| !have.contains(m.sha1.as_str())).map(|m| m.file.clone()).collect();
    diff.added.sort_by_key(|f| f.to_lowercase());
    diff.removed.sort_by_key(|f| f.to_lowercase());
    if local.minecraft != expected.minecraft {
        diff.minecraft = Some((expected.minecraft.clone(), local.minecraft.clone()));
    }
    if local.loader.to_lowercase() != expected.loader.to_lowercase() {
        diff.loader = Some((expected.loader.clone(), local.loader.clone()));
    }
    diff.state = "modificada".into();
    diff
}

/// Instancias instaladas de un paquete antes de que existiera la referencia: se arma con lo que guardó
/// integridad.rs al instalarlo (cada mod del paquete con su CRC32 y tamaño). Lo que ya no coincide con
/// eso (o falta) queda con una huella que no puede coincidir: cuenta como quitado, que es lo que pasó.
fn migrated_reference(instance: &Instance) -> Option<Reference> {
    let manifest = crate::packs::installed_manifest(instance)?;
    let record = crate::integridad::load(&instance.game_dir)?;
    let mut mods = Vec::new();
    for tracked in record {
        let parts: Vec<&str> = tracked.path.split('/').collect();
        if parts.len() != 2 || !parts[0].eq_ignore_ascii_case("mods") || !counts(parts[1]) {
            continue;
        }
        let path = instance.game_dir.join(&tracked.path);
        let disabled = PathBuf::from(format!("{}.disabled", path.display()));
        let candidate = if path.is_file() { path } else { disabled };
        let same = candidate.metadata().is_ok_and(|m| Some(m.len()) == tracked.size) && tracked.crc32.is_some() && crate::remote_zip::file_crc32(&candidate) == tracked.crc32;
        let sha1 = if same { sha1_file(&candidate) } else { None };
        let sha1 = sha1.unwrap_or_else(|| format!("crc32:{:08x}:{}", tracked.crc32.unwrap_or(0), tracked.size.unwrap_or(0)));
        mods.push(ModFile { file: parts[1].to_string(), sha1 });
    }
    if mods.is_empty() {
        return None;
    }
    Some(save_reference(instance, &manifest.id, &manifest.version, "instalado", build(&instance.minecraft_version, &loader_label(instance), mods)))
}

/// ¿Cómo está la instancia respecto del paquete del que viene? «desconocida» si no hay referencia.
pub fn check(instance: &Instance) -> Diff {
    let Some(reference) = reference(instance).or_else(|| migrated_reference(instance)) else { return Diff { state: "desconocida".into(), ..Default::default() } };
    let mut diff = compare(&content(instance), &reference.content);
    diff.package = Some(reference.package);
    diff.version = Some(reference.version);
    diff.origin = Some(reference.origin);
    diff
}

/// Después de instalar, actualizar o reparar desde un paquete: la referencia son sus mods.
pub fn after_package_install(paths: &crate::paths::Paths, folder: &str, manifest: &crate::packs::Manifest, entries: &[String]) {
    let Some(instance) = crate::instances::find(paths, folder) else { return };
    let mods = package_mods(&instance, entries.iter().map(String::as_str));
    save_reference(&instance, &manifest.id, &manifest.version, "instalado", content_of(&instance, &mods));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(file: &str, sha1: &str) -> ModFile {
        ModFile { file: file.into(), sha1: sha1.into() }
    }

    #[test]
    fn la_huella_no_depende_del_orden_ni_de_los_nombres() {
        let a = build("26.3", "Fabric 0.20.0", vec![m("sodium.jar", "aa"), m("lithium.jar", "bb")]);
        let b = build("26.3", "fabric 0.20.0", vec![m("lithium-renombrado.jar", "bb"), m("Sodium.jar", "aa")]);
        assert_eq!(a.content_hash, b.content_hash);
        assert!(a.content_hash.starts_with("sha256:"));
        let otro_loader = build("26.3", "Fabric 0.19.5", vec![m("sodium.jar", "aa"), m("lithium.jar", "bb")]);
        assert_ne!(a.content_hash, otro_loader.content_hash);
        let otro_minecraft = build("26.2", "Fabric 0.20.0", vec![m("sodium.jar", "aa"), m("lithium.jar", "bb")]);
        assert_ne!(a.content_hash, otro_minecraft.content_hash);
    }

    #[test]
    fn diferencias_con_el_paquete() {
        let paquete = build("26.3", "Fabric 0.20.0", vec![m("sodium.jar", "aa"), m("lithium.jar", "bb"), m("api.jar", "cc")]);
        let igual = build("26.3", "Fabric 0.20.0", vec![m("api.jar", "cc"), m("sodium.jar", "aa"), m("lithium.jar", "bb")]);
        assert_eq!(compare(&igual, &paquete).state, "exacta");
        // Un mod de más, uno quitado y uno de otra versión (otro SHA-1 con el mismo nombre).
        let cambiada = build("26.3", "Fabric 0.20.0", vec![m("sodium.jar", "aa"), m("lithium.jar", "b2"), m("xray.jar", "dd")]);
        let diff = compare(&cambiada, &paquete);
        assert!(diff.modified());
        assert_eq!(diff.added, vec!["lithium.jar", "xray.jar"]);
        assert_eq!(diff.removed, vec!["api.jar", "lithium.jar"]);
        assert_eq!(diff.minecraft, None);
        let otro_loader = build("26.3", "Fabric 0.19.5", paquete.mods.clone());
        assert_eq!(compare(&otro_loader, &paquete).loader, Some(("Fabric 0.20.0".into(), "Fabric 0.19.5".into())));
    }

    #[test]
    fn cuentan_los_jar_activos_menos_el_de_parabellum() {
        assert!(counts("sodium-fabric-0.6.jar"));
        assert!(counts("Sodium.JAR"));
        assert!(!counts("sodium.jar.disabled"));
        assert!(!counts(crate::juego::FILE_NAME));
        assert!(!counts("readme.txt"));
    }

    #[test]
    fn huella_de_una_instancia_en_disco() {
        let root = std::env::temp_dir().join(format!("parabellum-identidad-{}", now_ms()));
        let mods = root.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("a.jar"), b"mod a").unwrap();
        std::fs::write(mods.join("b.jar"), b"mod b").unwrap();
        std::fs::write(mods.join("c.jar.disabled"), b"mod c").unwrap();
        std::fs::write(mods.join(crate::juego::FILE_NAME), b"parabellum").unwrap();
        let instance = Instance { id: "prueba".into(), minecraft_version: "26.3".into(), loader: "Fabric".into(), loader_version: Some("0.20.0".into()), folder: root.clone(), game_dir: root.clone(), ..Default::default() };
        let before = content(&instance);
        assert_eq!(before.mods.len(), 2);
        let reference = save_reference(&instance, "pack", "1", "instalado", content_of(&instance, &package_mods(&instance, ["instancia/mods/a.jar", "instancia/mods/b.jar", "instancia/options.txt"])));
        assert_eq!(reference.content.content_hash, before.content_hash);
        assert_eq!(check(&instance).state, "exacta");
        // Se agrega un mod y se desactiva otro.
        std::fs::write(mods.join("x.jar"), b"mod x").unwrap();
        std::fs::rename(mods.join("b.jar"), mods.join("b.jar.disabled")).unwrap();
        let diff = check(&instance);
        assert!(diff.modified());
        assert_eq!(diff.added, vec!["x.jar"]);
        assert_eq!(diff.removed, vec!["b.jar"]);
        assert_eq!(diff.package.as_deref(), Some("pack"));
        // El id de la instalación no cambia.
        let id = installation_id(&instance);
        assert_eq!(id.len(), 36);
        assert_eq!(installation_id(&instance), id);
        let _ = std::fs::remove_dir_all(&root);
    }
}
