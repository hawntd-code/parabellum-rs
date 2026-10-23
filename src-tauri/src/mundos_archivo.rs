//! Instantáneas de mundos, parentesco de revisiones y restauración preparada fuera del original.
use crate::{identidad, instances::Instance};
use md5::Md5;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const IDENTITY: &str = "parabellum-mundo.json";
const HEADER: &str = "parabellum-revision.json";
const DIMENSIONS: &str = "parabellum-dimensiones";
const MAX_UNPACKED: u64 = 64 * 1024 * 1024 * 1024;
pub const MAX_CLOUD: u64 = 5 * 1024 * 1024 * 1024;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
pub struct Source {
    pub kind: String,
    pub id: String,
    pub world: String,
}
impl Source {
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.kind, self.id, self.world)
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Identity {
    pub world_id: String,
    pub owner: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Recipe {
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub instance_name: String,
    pub package: Option<String>,
    pub package_version: Option<String>,
    pub content_hash: String,
}
impl Recipe {
    pub fn of(instance: &Instance) -> Self {
        let reference = identidad::reference(instance);
        Self {
            minecraft: instance.minecraft_version.clone(),
            loader: instance.loader.clone(),
            loader_version: instance.loader_version.clone(),
            instance_name: instance.name.clone(),
            package: reference.as_ref().map(|r| r.package.clone()),
            package_version: reference.as_ref().map(|r| r.version.clone()),
            content_hash: identidad::content(instance).content_hash,
        }
    }
    pub fn compatible(&self, instance: &Instance) -> bool {
        instance.minecraft_version == self.minecraft
            && instance.loader == self.loader
            && instance.loader_version == self.loader_version
            && identidad::content(instance).content_hash == self.content_hash
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Revision {
    pub id: String,
    pub world_id: String,
    pub owner: String,
    pub name: String,
    pub parents: Vec<String>,
    pub at: i64,
    pub device: String,
    pub player_uuid: String,
    pub recipe: Recipe,
    pub source: Source,
    pub fingerprint: String,
    pub sha256: String,
    pub md5: String,
    pub size: u64,
    /// Paper guarda estas dimensiones junto al mundo, en lugar de dentro de él.
    pub dimensions: Vec<String>,
    /// Una resolución referencia el ZIP confirmado elegido, sin copiar ni perder las otras ramas.
    pub archive_file: Option<String>,
    pub content_revision: Option<String>,
}
impl Revision {
    pub fn data_revision(&self) -> &str {
        self.content_revision.as_deref().unwrap_or(&self.id)
    }
}
pub fn id() -> String {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("identificador aleatorio");
    hex::encode(bytes)
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn component(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 180
        && !name.chars().any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        && !name.ends_with(['.', ' '])
        && !matches!(
            name.split('.').next().unwrap_or("").to_ascii_uppercase().as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
}
pub fn atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let staged = path.with_extension(format!("{}.tmp", id()));
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(&staged).map_err(|e| e.to_string())?;
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    drop(file);
    std::fs::rename(staged, path).map_err(|e| e.to_string())
}
pub fn identity(folder: &Path) -> Option<Identity> {
    serde_json::from_slice(&std::fs::read(folder.join(IDENTITY)).ok()?).ok()
}
pub fn ensure_identity(folder: &Path, owner: &str, duplicate: bool) -> Result<Identity, String> {
    let found = identity(folder).filter(|i| valid_id(&i.world_id) && i.owner == owner && !duplicate);
    let found = found.unwrap_or_else(|| Identity {
        world_id: id(),
        owner: owner.into(),
    });
    atomic(&folder.join(IDENTITY), &found)?;
    Ok(found)
}
fn ignored(name: &str) -> bool {
    matches!(name, "session.lock" | IDENTITY | HEADER | DIMENSIONS)
}
fn files(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        for item in std::fs::read_dir(current).map_err(|e| e.to_string())? {
            let item = item.map_err(|e| e.to_string())?;
            let info = item.file_type().map_err(|e| e.to_string())?;
            if info.is_symlink() {
                return Err(t!(
                    "El mundo contiene un enlace simbólico: no se respalda.",
                    "The world contains a symbolic link: it is not backed up."
                ));
            }
            if item.file_name() == "session.lock" || (item.path().parent() == Some(root) && ignored(&item.file_name().to_string_lossy())) {
                continue;
            }
            if info.is_dir() {
                walk(root, &item.path(), out)?;
            } else if info.is_file() {
                out.push(item.path());
            } else {
                return Err(t!("El mundo contiene un archivo especial.", "The world contains a special file."));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    out.sort();
    Ok(out)
}
pub fn hashes(path: &Path) -> Result<(String, String), String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut sha = Sha256::new();
    let mut md5 = Md5::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        crate::descargas::check()?;
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        sha.update(&buffer[..n]);
        md5.update(&buffer[..n]);
    }
    Ok((hex::encode(sha.finalize()), hex::encode(md5.finalize())))
}
pub fn snapshot(folder: &Path, archive: &Path, mut revision: Revision, progress: impl Fn(f64)) -> Result<Revision, String> {
    if !folder.join("level.dat").is_file() {
        return Err(t!("No se encontró level.dat en el mundo.", "The world's level.dat was not found."));
    }
    let mut roots = vec![(folder.to_path_buf(), String::new())];
    revision.dimensions.clear();
    if revision.source.kind == "servidor" {
        for suffix in ["nether", "the_end"] {
            let sibling = folder.with_file_name(format!("{}_{}", folder.file_name().unwrap().to_string_lossy(), suffix));
            if sibling.is_symlink() {
                return Err(t!(
                    "La dimensión del servidor es un enlace simbólico.",
                    "The server dimension is a symbolic link."
                ));
            }
            if sibling.is_dir() {
                revision.dimensions.push(suffix.into());
                roots.push((sibling, format!("{DIMENSIONS}/{suffix}/")));
            }
        }
    }
    let _locks = roots
        .iter()
        .map(|(root, _)| crate::plataforma::lock_world(&root.join("session.lock")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| t!("Cierra el mundo antes de respaldarlo.", "Close the world before backing it up."))?;
    let mut entries = Vec::new();
    for (root, prefix) in &roots {
        for path in files(root)? {
            let relative = format!("{prefix}{}", path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
            entries.push((path, relative));
        }
    }
    entries.sort_by(|a, b| a.1.cmp(&b.1));
    let total = entries
        .iter()
        .try_fold(0u64, |n, (p, _)| p.metadata().map(|m| n.saturating_add(m.len())))
        .map_err(|e| e.to_string())?;
    if total > MAX_UNPACKED {
        return Err(t!(
            "Este mundo supera el límite de 64 GB por copia.",
            "This world exceeds the 64 GB per-backup limit."
        ));
    }
    let file = std::fs::File::create(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(1))
        .large_file(true)
        .unix_permissions(0o600);
    let mut fingerprint = Sha256::new();
    let mut done = 0u64;
    let mut buffer = vec![0; 1024 * 1024];
    for (path, relative) in entries {
        crate::descargas::check()?;
        let before = path.metadata().map_err(|e| e.to_string())?;
        zip.start_file(&relative, options).map_err(|e| e.to_string())?;
        fingerprint.update((relative.len() as u64).to_le_bytes());
        fingerprint.update(relative.as_bytes());
        let mut digest = Sha256::new();
        let mut source = std::fs::File::open(&path).map_err(|e| e.to_string())?;
        loop {
            crate::descargas::check()?;
            let count = source.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            zip.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
            digest.update(&buffer[..count]);
            done += count as u64;
            progress(done as f64 / total.max(1) as f64);
        }
        let after = path.metadata().map_err(|e| e.to_string())?;
        if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
            return Err(t!(
                "Un archivo del mundo cambió durante la copia. Vuelve a intentarlo con el juego cerrado.",
                "A world file changed during the backup. Try again with the game closed."
            ));
        }
        fingerprint.update(digest.finalize());
    }
    revision.fingerprint = hex::encode(fingerprint.finalize());
    zip.start_file(HEADER, options).map_err(|e| e.to_string())?;
    zip.write_all(&serde_json::to_vec(&revision).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    zip.finish().map_err(|e| e.to_string())?.sync_all().map_err(|e| e.to_string())?;
    let (sha256, md5) = hashes(archive)?;
    revision.sha256 = sha256;
    revision.md5 = md5;
    revision.size = archive.metadata().map_err(|e| e.to_string())?.len();
    Ok(revision)
}
pub fn unpack(archive: &Path, destination: &Path, revision: &Revision) -> Result<(), String> {
    if destination.exists() {
        return Err(t!(
            "La carpeta preparada para restaurar ya existe.",
            "The staging folder for restoration already exists."
        ));
    }
    let (sha256, md5) = hashes(archive)?;
    if sha256 != revision.sha256 || md5 != revision.md5 {
        return Err(t!(
            "La copia descargada no coincide con su huella. No se restaura.",
            "The downloaded backup does not match its checksum. It is not restored."
        ));
    }
    let mut zip = zip::ZipArchive::new(std::fs::File::open(archive).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if zip.len() > 1_000_000 {
        return Err(t!("La copia contiene demasiados archivos.", "The backup contains too many files."));
    }
    let header: Revision = {
        let mut f = zip
            .by_name(HEADER)
            .map_err(|_| t!("La copia no tiene metadatos de Parabellum.", "The backup has no Parabellum metadata."))?;
        if f.size() > 64 * 1024 {
            return Err(t!("Metadatos de copia no válidos.", "Invalid backup metadata."));
        }
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|_| t!("Metadatos de copia no válidos.", "Invalid backup metadata."))?
    };
    if header.world_id != revision.world_id
        || header.id != revision.data_revision()
        || header.recipe != revision.recipe
        || header.fingerprint != revision.fingerprint
        || header.dimensions != revision.dimensions
    {
        return Err(t!("La identidad de la copia no coincide.", "The backup identity does not match."));
    }
    if revision.dimensions.iter().any(|s| !matches!(s.as_str(), "nether" | "the_end"))
        || (!revision.dimensions.is_empty() && revision.source.kind != "servidor")
    {
        return Err(t!("Dimensiones de copia no válidas.", "Invalid backup dimensions."));
    }
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for i in 0..zip.len() {
        crate::descargas::check()?;
        let entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().replace('\\', "/");
        if name == HEADER {
            continue;
        }
        let relative = entry
            .enclosed_name()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| t!("La copia contiene una ruta no válida.", "The backup contains an invalid path."))?
            .to_path_buf();
        if name.split('/').any(|part| !component(part) && !part.is_empty())
            || !seen.insert(name.to_lowercase())
            || entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000)
            || relative.components().any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(t!(
                "La copia contiene rutas duplicadas, enlaces o nombres no válidos.",
                "The backup contains duplicate paths, links or invalid names."
            ));
        }
        if name.split('/').any(|p| p == "session.lock") {
            return Err(t!(
                "La copia incluye un bloqueo de mundo no permitido.",
                "The backup includes a forbidden world lock."
            ));
        }
        if let Some(rest) = name.strip_prefix(&format!("{DIMENSIONS}/")) {
            if !revision.dimensions.iter().any(|s| rest == s || rest.starts_with(&format!("{s}/"))) {
                return Err(t!(
                    "La copia contiene una dimensión no declarada.",
                    "The backup contains an undeclared dimension."
                ));
            }
        }
        total = total
            .checked_add(entry.size())
            .filter(|n| *n <= MAX_UNPACKED)
            .ok_or_else(|| t!("La copia supera el límite permitido.", "The backup exceeds the allowed limit."))?;
        let out = destination.join(&relative);
        if entry.is_dir() {
            std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(out).map_err(|e| e.to_string())?;
        let expected = entry.size();
        let mut limited = entry.take(expected.saturating_add(1));
        let mut copied = 0u64;
        let mut buffer = vec![0; 1024 * 1024];
        loop {
            crate::descargas::check()?;
            let count = limited.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            file.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
            copied += count as u64;
        }
        if copied != expected {
            return Err(t!("El tamaño de un archivo de la copia no coincide.", "A backup file's size does not match."));
        }
        file.sync_all().map_err(|e| e.to_string())?;
    }
    if !destination.join("level.dat").is_file() {
        return Err(t!("La copia no contiene un mundo completo.", "The backup does not contain a complete world."));
    }
    Ok(())
}
/// El original se conserva por renombrado, en el mismo volumen; nunca se borra para restaurar.
pub fn install(staged: &Path, target: &Path, backups: &Path, server: bool) -> Result<Option<PathBuf>, String> {
    let parent = target
        .parent()
        .ok_or_else(|| "Destino sin carpeta".to_string())?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if staged.parent().and_then(|p| p.canonicalize().ok()).as_ref() != Some(&parent) || !staged.is_dir() || staged.is_symlink() || target.is_symlink() {
        return Err(t!("Destino de restauración no válido.", "Invalid restore destination."));
    }
    let mut current = vec![(target.to_path_buf(), String::new())];
    if server {
        for suffix in ["nether", "the_end"] {
            current.push((
                target.with_file_name(format!("{}_{}", target.file_name().unwrap().to_string_lossy(), suffix)),
                suffix.into(),
            ));
        }
    }
    if current.iter().any(|(p, _)| p.is_symlink() || (p.exists() && !p.is_dir())) {
        return Err(t!("Una carpeta del mundo no es válida.", "A world folder is invalid."));
    }
    let _locks = current
        .iter()
        .filter(|(p, _)| p.exists())
        .map(|(p, _)| crate::plataforma::lock_world(&p.join("session.lock")))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| t!("Cierra el mundo antes de restaurarlo.", "Close the world before restoring it."))?;
    let backup = if current.iter().any(|(p, _)| p.exists()) {
        let envelope = backups.join(id());
        std::fs::create_dir_all(&envelope).map_err(|e| e.to_string())?;
        Some(envelope.join("mundo"))
    } else {
        None
    };
    if server
        && ["nether", "the_end"].iter().any(|s| {
            let p = staged.join(DIMENSIONS).join(s);
            p.exists() && (!p.is_dir() || p.is_symlink())
        })
    {
        return Err(t!("Dimensión preparada no válida.", "Invalid staged dimension."));
    }
    // Windows no permite renombrar la carpeta con nuestro propio handle abierto. La reserva del
    // coordinador sigue impidiendo iniciar Minecraft hasta terminar el intercambio.
    #[cfg(windows)]
    drop(_locks);
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let result = (|| -> std::io::Result<()> {
        if let Some(backup) = &backup {
            for (source, suffix) in &current {
                if !source.exists() {
                    continue;
                }
                let destination = if suffix.is_empty() {
                    backup.clone()
                } else {
                    backup.join(DIMENSIONS).join(suffix)
                };
                std::fs::create_dir_all(destination.parent().unwrap())?;
                std::fs::rename(source, &destination)?;
                moved.push((source.clone(), destination));
            }
        }
        if server {
            for suffix in ["nether", "the_end"] {
                let source = staged.join(DIMENSIONS).join(suffix);
                if source.exists() {
                    let destination = target.with_file_name(format!("{}_{}", target.file_name().unwrap().to_string_lossy(), suffix));
                    std::fs::rename(&source, &destination)?;
                    moved.push((source, destination));
                }
            }
        }
        std::fs::rename(staged, target)?;
        Ok(())
    })();
    if let Err(error) = result {
        for (source, destination) in moved.into_iter().rev() {
            if !source.exists() {
                if let Some(parent) = source.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::rename(destination, source);
            }
        }
        return Err(error.to_string());
    }
    Ok(backup)
}
pub fn heads(revisions: &[Revision]) -> Vec<String> {
    let parents: HashSet<&str> = revisions.iter().flat_map(|r| r.parents.iter().map(String::as_str)).collect();
    revisions.iter().filter(|r| !parents.contains(r.id.as_str())).map(|r| r.id.clone()).collect()
}
/// Conserva cinco revisiones por mundo, todas las ramas abiertas y los ZIP referenciados.
pub fn retained(revisions: &[(String, Revision)]) -> HashSet<String> {
    let mut groups: HashMap<&str, Vec<&(String, Revision)>> = HashMap::new();
    for item in revisions {
        groups.entry(&item.1.world_id).or_default().push(item);
    }
    let mut keep = HashSet::new();
    for group in groups.values_mut() {
        group.sort_by_key(|(_, r)| std::cmp::Reverse(r.at));
        let branch_heads = heads(&group.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>());
        for (file, r) in group.iter().filter(|(_, r)| branch_heads.contains(&r.id)).chain(group.iter().take(5)).copied() {
            keep.insert(file.clone());
            if let Some(data) = &r.archive_file {
                keep.insert(data.clone());
            }
        }
    }
    keep
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("pbl-mundos39-{}", id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn world(&self, name: &str) -> PathBuf {
            let p = self.0.join(name);
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("level.dat"), b"nivel").unwrap();
            p
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn revision() -> Revision {
        Revision {
            id: id(),
            world_id: id(),
            owner: "prueba".into(),
            source: Source {
                kind: "instancia".into(),
                id: "prueba".into(),
                world: "world".into(),
            },
            ..Default::default()
        }
    }
    fn put(root: &Path, name: &str, data: &[u8]) {
        let p = root.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, data).unwrap();
    }
    #[test]
    fn copia_dimensiones_inventarios_y_huella_estable() {
        let f = Fixture::new();
        let world = f.world("world");
        for name in [
            "DIM-1/region/r.0.0.mca",
            "DIM1/region/r.0.0.mca",
            "playerdata/jugador.dat",
            "data/mapa.dat",
            "advancements/jugador.json",
        ] {
            put(&world, name, name.as_bytes());
        }
        put(&world, IDENTITY, b"identidad local");
        let zip = f.0.join("copia.zip");
        let r = snapshot(&world, &zip, revision(), |_| {}).unwrap();
        let restored = f.0.join("restaurado");
        unpack(&zip, &restored, &r).unwrap();
        assert_eq!(std::fs::read(restored.join("playerdata/jugador.dat")).unwrap(), b"playerdata/jugador.dat");
        assert!(!restored.join("session.lock").exists());
        assert!(!restored.join(IDENTITY).exists());
        let r2 = snapshot(&world, &f.0.join("otra.zip"), revision(), |_| {}).unwrap();
        assert_eq!(r.fingerprint, r2.fingerprint);
        put(&world, "data/mapa.dat", b"nuevo");
        let r3 = snapshot(&world, &f.0.join("tercera.zip"), revision(), |_| {}).unwrap();
        assert_ne!(r.fingerprint, r3.fingerprint);
        put(&f.0, "corrupta.zip", b"otro contenido");
        assert!(unpack(&f.0.join("corrupta.zip"), &f.0.join("no-instalar"), &r).is_err());
        assert!(!f.0.join("no-instalar").exists());
    }
    #[test]
    fn paper_conserva_dimensiones_externas_y_permite_volver() {
        let f = Fixture::new();
        let world = f.world("world");
        put(&f.0, "world_nether/DIM-1/region/r.mca", b"nether previo");
        put(&f.0, "world_the_end/DIM1/region/r.mca", b"end previo");
        let mut r = revision();
        r.source.kind = "servidor".into();
        let r = snapshot(&world, &f.0.join("paper.zip"), r, |_| {}).unwrap();
        assert_eq!(r.dimensions, vec!["nether", "the_end"]);
        put(&world, "level.dat", b"nivel posterior");
        put(&f.0, "world_nether/DIM-1/region/r.mca", b"nether posterior");
        let stage = f.0.join("stage");
        unpack(&f.0.join("paper.zip"), &stage, &r).unwrap();
        let backup = install(&stage, &world, &f.0.join("backups"), true).unwrap().unwrap();
        assert_eq!(std::fs::read(f.0.join("world_nether/DIM-1/region/r.mca")).unwrap(), b"nether previo");
        assert_eq!(std::fs::read(backup.join("level.dat")).unwrap(), b"nivel posterior");
        let undo = f.0.join("undo");
        std::fs::rename(&backup, &undo).unwrap();
        install(&undo, &world, &f.0.join("backups"), true).unwrap();
        assert_eq!(std::fs::read(f.0.join("world_nether/DIM-1/region/r.mca")).unwrap(), b"nether posterior");
        assert_eq!(std::fs::read(world.join("level.dat")).unwrap(), b"nivel posterior");
    }
    #[test]
    fn restaura_sin_borrar_original_y_rechaza_destino_invalido() {
        let f = Fixture::new();
        let original = f.world("original");
        let stage = f.world("stage");
        put(&stage, "level.dat", b"nuevo");
        let backup = install(&stage, &original, &f.0.join("backups"), false).unwrap().unwrap();
        assert_eq!(std::fs::read(backup.join("level.dat")).unwrap(), b"nivel");
        assert_eq!(std::fs::read(original.join("level.dat")).unwrap(), b"nuevo");
        assert!(install(&f.0.join("inexistente"), &original, &f.0.join("backups"), false).is_err());
        assert_eq!(std::fs::read(original.join("level.dat")).unwrap(), b"nuevo");
    }
    #[cfg(windows)]
    #[test]
    fn fallo_al_instalar_revierte_los_movimientos_del_original() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let original = f.world("original");
        let stage = f.world("stage");
        let _blocked = std::fs::OpenOptions::new().read(true).share_mode(0).open(stage.join("level.dat")).unwrap();
        assert!(install(&stage, &original, &f.0.join("backups"), false).is_err());
        assert_eq!(std::fs::read(original.join("level.dat")).unwrap(), b"nivel");
        assert!(stage.exists());
    }
    #[test]
    fn duplicar_un_mundo_o_cambiar_de_cuenta_crea_identidad_nueva() {
        let f = Fixture::new();
        let world = f.world("world");
        let first = ensure_identity(&world, "a", false).unwrap();
        assert_eq!(first.world_id, ensure_identity(&world, "a", false).unwrap().world_id);
        let copy = ensure_identity(&world, "a", true).unwrap();
        assert_ne!(first.world_id, copy.world_id);
        assert_ne!(copy.world_id, ensure_identity(&world, "b", false).unwrap().world_id);
    }
    #[test]
    fn ramas_y_resoluciones_no_pierden_cabezas_ni_zip_referenciado() {
        let world = id();
        let mut list = Vec::new();
        let mut parent = None;
        for at in 0..9 {
            let r = Revision {
                id: id(),
                world_id: world.clone(),
                parents: parent.clone().into_iter().collect(),
                at,
                ..Default::default()
            };
            parent = Some(r.id.clone());
            list.push((format!("f{at}"), r));
        }
        let branch = Revision {
            id: id(),
            world_id: world,
            parents: vec![list[0].1.id.clone()],
            at: 1,
            ..Default::default()
        };
        list.push(("rama".into(), branch.clone()));
        let rs = list.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>();
        assert_eq!(heads(&rs).len(), 2);
        let keep = retained(&list);
        assert!(keep.contains("rama"));
        assert!(keep.contains("f8"));
        assert!(!keep.contains("f0"));
        let chosen = list[2].1.clone();
        let resolution = Revision {
            id: id(),
            parents: heads(&rs),
            archive_file: Some("f2".into()),
            content_revision: Some(chosen.id.clone()),
            at: 10,
            ..chosen
        };
        list.push(("resolucion".into(), resolution));
        assert_eq!(heads(&list.iter().map(|(_, r)| r.clone()).collect::<Vec<_>>()).len(), 1);
        assert!(retained(&list).contains("f2"));
    }
    #[test]
    fn zip_rechaza_rutas_enlaces_duplicados_y_bloqueos() {
        for names in [
            vec!["../fuera"],
            vec!["C:/fuera"],
            vec!["region/a", "REGION/A"],
            vec!["session.lock"],
            vec!["CON.txt"],
            vec!["parabellum-dimensiones/nether/region/a"],
        ] {
            let f = Fixture::new();
            let zip = f.0.join("malo.zip");
            let mut r = revision();
            r.fingerprint = "f".into();
            let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
            let opt = zip::write::SimpleFileOptions::default();
            writer.start_file(HEADER, opt).unwrap();
            writer.write_all(&serde_json::to_vec(&r).unwrap()).unwrap();
            writer.start_file("level.dat", opt).unwrap();
            writer.write_all(b"nivel").unwrap();
            for name in names {
                writer.start_file(name, opt).unwrap();
                writer.write_all(b"mal").unwrap();
            }
            writer.finish().unwrap();
            let (sha, md5) = hashes(&zip).unwrap();
            r.sha256 = sha;
            r.md5 = md5;
            assert!(unpack(&zip, &f.0.join("stage"), &r).is_err());
            assert!(!f.0.join("fuera").exists());
        }
    }
    #[test]
    fn un_bloqueo_externo_impide_copiar_o_restaurar() {
        let f = Fixture::new();
        let world = f.world("world");
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let _java = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .share_mode(3)
                .open(world.join("session.lock"))
                .unwrap();
            assert!(snapshot(&world, &f.0.join("no.zip"), revision(), |_| {}).is_err());
            let stage = f.world("stage");
            assert!(install(&stage, &world, &f.0.join("backups"), false).is_err());
            assert!(stage.exists());
        }
        #[cfg(not(windows))]
        {
            let _ = world;
        }
    }
    #[test]
    #[ignore = "medición separada de tiempo y memoria, mundo de 2 GB"]
    fn mundo_de_dos_gb_se_procesa_por_bloques() {
        let f = Fixture::new();
        let world = f.world("grande");
        let file = std::fs::File::create(world.join("region-grande.mca")).unwrap();
        file.set_len(2 * 1024 * 1024 * 1024).unwrap();
        drop(file);
        let r = snapshot(&world, &f.0.join("grande.zip"), revision(), |_| {}).unwrap();
        unpack(&f.0.join("grande.zip"), &f.0.join("restaurado"), &r).unwrap();
        assert_eq!(f.0.join("restaurado/region-grande.mca").metadata().unwrap().len(), 2 * 1024 * 1024 * 1024);
    }
}
