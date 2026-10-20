//! Espejo de Minecraft en Google Drive (pedido de Juan, 2026-10-17): las versiones de Minecraft, sus
//! loaders (Fabric, Quilt, Forge y NeoForge) y sus servidores, para crear instancias y servidores en
//! redes donde solo Google está permitido (la escuela).
//!
//! En la carpeta _espejo del catálogo:
//! - capa-001.zip, capa-002.zip…: cada archivo una sola vez, por su SHA-1 («o/<sha1>»). Los sonidos y
//!   las bibliotecas que comparten las versiones no se repiten, así que todo cabe en unos pocos GB.
//! - espejo.indice: los paquetes (una versión de Minecraft, un Java, un loader o un servidor) y en qué
//!   capa está la lista de cada uno («listas/….json»: qué archivo va en qué ruta). No termina en .json
//!   para que el catálogo de las versiones anteriores de Parabellum no lo lea como un paquete.
//!
//! Quien publica (Juan) lo arma con `Builder`: baja cada cosa como siempre (Mojang, los servidores de
//! cada loader), guarda en la capa lo que el espejo no tenía y sube el índice al terminar cada capa, así
//! lo publicado siempre está completo. Los demás bajan por partes (remote_zip) solo lo que le falta a su
//! PC.

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::loaders::{self, compare_versions, Loader};
use crate::paths::Paths;
use crate::remote_zip::{RemoteEntry, RemoteZip};

pub const FOLDER: &str = "_espejo";
pub const INDEX_FILE: &str = "espejo.indice";
const FORMAT: u32 = 1;
/// Tamaño de cada capa: Drive limita las descargas de cada archivo, así se reparten.
const LAYER_LIMIT: u64 = 2_000_000_000;

// ================= Modo escuela

static SCHOOL: AtomicBool = AtomicBool::new(false);

/// Modo escuela (el botón de arriba): todo se baja de Google Drive y no se intenta Mojang ni los
/// servidores de los loaders, que en la escuela están bloqueados (y cada intento tarda en fallar).
pub fn school_mode() -> bool {
    SCHOOL.load(Ordering::Relaxed)
}

pub fn set_school_mode(on: bool) {
    SCHOOL.store(on, Ordering::Relaxed);
}

// ================= Índice

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub format: u32,
    pub updated: String,
    pub layers: Vec<Layer>,
    pub packages: BTreeMap<String, Package>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    /// minecraft, java, loader o servidor.
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minecraft: Option<String>,
    /// Cuándo salió esa versión de Minecraft (para ordenarlas como Mojang).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loader_version: Option<String>,
    /// La versión que se lanza (la del loader: «fabric-loader-0.19.5-1.21.1»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    /// El Java que usa (minecraft): el nombre de su carpeta («java-runtime-gamma»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub java: Option<String>,
    /// Cómo arrancar el servidor (lo que va después de los argumentos de Java).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub launch: Vec<String>,
    /// La capa con su lista.
    pub list: String,
    pub files: usize,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Listed {
    path: String,
    sha1: String,
    size: u64,
}

/// El sistema, para lo que depende de él (los nativos y el Java): «windows-x86_64», «macos-aarch64».
pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn minecraft_id(version: &str) -> String {
    format!("minecraft/{version}@{}", platform())
}

fn java_id(component: &str) -> String {
    format!("java/{component}@{}", platform())
}

pub fn loader_id(loader: Loader, minecraft: &str, version: &str) -> String {
    format!("{}/{minecraft}/{version}", loader.name().to_lowercase())
}

pub fn server_id(minecraft: &str, loader: Option<(Loader, &str)>) -> String {
    match loader {
        Some((loader, version)) => format!("servidor/{minecraft}/{}/{version}", loader.name().to_lowercase()),
        None => format!("servidor/{minecraft}/vanilla"),
    }
}

fn list_entry(id: &str) -> String {
    format!("listas/{}.json", id.replace(['/', '@'], "_"))
}

fn object_entry(sha1: &str) -> String {
    format!("o/{sha1}")
}

fn damaged() -> String {
    t!("El espejo de Google Drive está incompleto. Avísale a quien publica.", "The Google Drive mirror is incomplete. Tell whoever publishes.")
}

// ================= Leer el espejo (todos)

/// Dónde está el espejo en Drive (lo encuentra el catálogo al listar): el índice y los archivos de su
/// carpeta (nombre → id y tamaño).
#[derive(Clone, Debug, Default)]
pub struct Location {
    pub index: String,
    pub modified: String,
    pub files: HashMap<String, (String, u64)>,
}

pub struct Remote {
    pub index: Index,
    location: Location,
    /// SHA-1 → (capa, archivo), de todas las capas.
    objects: OnceLock<HashMap<String, (String, RemoteEntry)>>,
}

static REMOTE: Mutex<Option<(String, Arc<Remote>)>> = Mutex::new(None);

/// El espejo de Drive. Ok(None) si el catálogo no tiene uno.
pub async fn remote(paths: &Paths) -> Result<Option<Arc<Remote>>, String> {
    let catalog = crate::catalog::current(paths).await;
    let Some(location) = catalog.mirror.clone() else { return Ok(None) };
    from_location(location).await.map(Some)
}

async fn from_location(location: Location) -> Result<Arc<Remote>, String> {
    let key = format!("{}@{}", location.index, location.modified);
    if let Some((cached, remote)) = REMOTE.lock().unwrap().clone() {
        if cached == key {
            return Ok(remote);
        }
    }
    let text = crate::catalog::download_text(&location.index).await?;
    let index: Index = serde_json::from_str(&text).map_err(|_| damaged())?;
    if index.format > FORMAT {
        return Err(t!("El espejo de Drive es de una versión más nueva de Parabellum: actualízalo.", "The Drive mirror is from a newer Parabellum version: update it."));
    }
    let remote = Arc::new(Remote { index, location, objects: OnceLock::new() });
    *REMOTE.lock().unwrap() = Some((key, remote.clone()));
    Ok(remote)
}

fn entries_cache() -> &'static Mutex<HashMap<String, Arc<HashMap<String, RemoteEntry>>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Arc<HashMap<String, RemoteEntry>>>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

impl Remote {
    fn zip(&self, layer: &str) -> Result<RemoteZip, String> {
        let (id, size) = self.location.files.get(layer).ok_or_else(damaged)?;
        Ok(RemoteZip::new(&crate::catalog::download_url(id), *size))
    }

    /// Los archivos de una capa (su directorio central). Una capa no cambia: se guarda en datos/espejo.
    fn entries(&self, layer: &str, paths: &Paths) -> Result<Arc<HashMap<String, RemoteEntry>>, String> {
        let size = self.location.files.get(layer).map(|(_, size)| *size).ok_or_else(damaged)?;
        let key = format!("{layer}-{size}");
        if let Some(found) = entries_cache().lock().unwrap().get(&key) {
            return Ok(found.clone());
        }
        let saved = paths.data.join("espejo").join(format!("{key}.json"));
        let list: Vec<RemoteEntry> = match std::fs::read(&saved).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
            Some(list) => list,
            None => {
                let list = self.zip(layer)?.entries()?;
                let _ = std::fs::create_dir_all(saved.parent().unwrap());
                let _ = std::fs::write(&saved, serde_json::to_vec(&list).unwrap_or_default());
                list
            }
        };
        let map = Arc::new(list.into_iter().map(|e| (e.name.clone(), e)).collect::<HashMap<_, _>>());
        entries_cache().lock().unwrap().insert(key, map.clone());
        Ok(map)
    }

    fn objects(&self, paths: &Paths) -> Result<&HashMap<String, (String, RemoteEntry)>, String> {
        if let Some(objects) = self.objects.get() {
            return Ok(objects);
        }
        let mut objects = HashMap::new();
        for layer in &self.index.layers {
            for (name, entry) in self.entries(&layer.name, paths)?.iter() {
                if let Some(sha1) = name.strip_prefix("o/") {
                    objects.entry(sha1.to_string()).or_insert_with(|| (layer.name.clone(), entry.clone()));
                }
            }
        }
        Ok(self.objects.get_or_init(|| objects))
    }

    fn list(&self, id: &str, paths: &Paths) -> Result<Vec<Listed>, String> {
        let package = self.index.packages.get(id).ok_or_else(damaged)?;
        let entries = self.entries(&package.list, paths)?;
        let entry = entries.get(&list_entry(id)).ok_or_else(damaged)?;
        serde_json::from_slice(&self.zip(&package.list)?.read(entry)?).map_err(|_| damaged())
    }

    /// Baja a `root` los archivos del paquete que faltan (o tienen otro tamaño). Con `verify`, también los
    /// que tienen el tamaño bueno pero otra huella (dañados): es lo que hace «Verificar». Lo normal es solo
    /// el tamaño, porque todo lo que se baja ya se comprueba antes de quedar con su nombre (CRC32 del zip y
    /// el «.parte»). Los JSON de las versiones van al final: si se corta, la versión no parece instalada.
    /// Devuelve cuántos archivos bajó.
    fn install(&self, id: &str, root: &Path, paths: &Paths, label: &str, verify: bool, progress: &(dyn Fn(String, f64) + Send + Sync)) -> Result<usize, String> {
        progress(
            if verify { t!("Revisando {label} archivo por archivo…", "Checking {label} file by file…") } else { t!("Comparando {label} con Google Drive…", "Comparing {label} with Google Drive…") },
            0.0,
        );
        let mut wanted: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for file in self.list(id, paths)? {
            let relative = crate::mrpack::safe_relative(&file.path).ok_or_else(damaged)?;
            let target = root.join(relative);
            let same = target.metadata().is_ok_and(|m| m.len() == file.size) && (!verify || content_sha1(&target).is_some_and(|h| h == file.sha1));
            if !same {
                wanted.entry(file.sha1).or_default().push(target);
            }
        }
        if wanted.is_empty() {
            return Ok(0);
        }
        let objects = self.objects(paths)?;
        let mut by_layer: BTreeMap<String, Vec<(RemoteEntry, Vec<PathBuf>)>> = BTreeMap::new();
        for (sha1, targets) in wanted {
            let (layer, entry) = objects.get(&sha1).ok_or_else(damaged)?;
            by_layer.entry(layer.clone()).or_default().push((entry.clone(), targets));
        }
        let total: u64 = by_layer.values().flatten().map(|(e, _)| e.size).sum::<u64>().max(1);
        let count_total: usize = by_layer.values().map(Vec::len).sum();
        let (mut done, mut count) = (0u64, 0usize);
        let mut last = Vec::new();
        for (layer, files) in &by_layer {
            let zip = self.zip(layer)?;
            let entries: Vec<&RemoteEntry> = files.iter().map(|(e, _)| e).collect();
            zip.read_many(&entries, |i, bytes| {
                for target in &files[i].1 {
                    if is_version_json(target) {
                        last.push((target.clone(), bytes.clone()));
                    } else {
                        write(target, &bytes)?;
                    }
                }
                done += bytes.len() as u64;
                count += 1;
                progress(
                    t!(
                        "Descargando {label} de Google Drive… {count} de {count_total} archivos ({:.0} de {:.0} MB)",
                        "Downloading {label} from Google Drive… {count} of {count_total} files ({:.0} of {:.0} MB)",
                        done as f64 / 1048576.0,
                        total as f64 / 1048576.0
                    ),
                    done as f64 / total as f64,
                );
                Ok(())
            })?;
        }
        for (target, bytes) in last {
            write(&target, &bytes)?;
        }
        Ok(count)
    }
}

/// SHA-1 del contenido (siempre leído: para verificar no vale el nombre de los sonidos).
pub fn content_sha1(file: &Path) -> Option<String> {
    let mut source = std::fs::File::open(file).ok()?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = source.read(&mut buffer).ok()?;
        if read == 0 {
            return Some(hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// versions/<id>/<id>.json
fn is_version_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
        && path.parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).is_some_and(|n| n == "versions")
}

fn write(target: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    let temp = PathBuf::from(format!("{}.parte", target.display()));
    std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, target).map_err(|e| e.to_string())
}

/// Corre `work` en un hilo aparte dentro de la misma tarea de Descargas (velocidad, límite, cancelar).
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    let scope = crate::descargas::current();
    tauri::async_runtime::spawn_blocking(move || crate::descargas::enter(scope, work)).await.map_err(|e| e.to_string())?
}

/// Las versiones de Minecraft del espejo para este sistema, de la más nueva a la más vieja.
pub fn minecraft_versions(remote: &Remote) -> Vec<crate::mojang::Release> {
    let suffix = format!("@{}", platform());
    let mut versions: Vec<crate::mojang::Release> = remote
        .index
        .packages
        .iter()
        .filter(|(id, p)| p.kind == "minecraft" && id.ends_with(&suffix))
        .filter_map(|(_, p)| {
            Some(crate::mojang::Release {
                id: p.minecraft.clone()?,
                kind: "release".into(),
                release_time: p.released.clone().unwrap_or_default(),
                url: String::new(),
                sha1: None,
            })
        })
        .collect();
    versions.sort_by(|a, b| b.release_time.cmp(&a.release_time));
    versions
}

/// Las versiones de un loader para `minecraft` que tiene el espejo, de la más nueva a la más vieja.
pub fn loader_versions(remote: &Remote, loader: Loader, minecraft: &str) -> Vec<String> {
    let prefix = format!("{}/{minecraft}/", loader.name().to_lowercase());
    let mut versions: Vec<String> = remote.index.packages.keys().filter_map(|k| k.strip_prefix(&prefix)).map(String::from).collect();
    versions.sort_by(|a, b| compare_versions(b, a));
    versions
}

/// Instala Minecraft `version` y su Java desde el espejo. Ok(false): el espejo no la tiene (para este
/// sistema).
pub async fn install_minecraft(version: &str, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + 'static) -> Result<bool, String> {
    let Some(remote) = remote(paths).await? else { return Ok(false) };
    let id = minecraft_id(version);
    let Some(package) = remote.index.packages.get(&id).cloned() else { return Ok(false) };
    let (paths, version) = (paths.clone(), version.to_string());
    blocking(move || {
        if let Some(java) = package.java.as_deref().filter(|j| remote.index.packages.contains_key(&java_id(j))) {
            remote.install(&java_id(java), &paths.base, &paths, &format!("Java ({java})"), false, &progress)?;
            // Completo: sin la marca de que se estaba bajando ni «.parte» (ver java::package_installed).
            crate::java::package_installed(&paths, [format!("base/java/{java}/").as_str()]);
        }
        remote.install(&id, &paths.base, &paths, &format!("Minecraft {version}"), false, &progress)?;
        Ok(true)
    })
    .await
}

/// «Verificar» el Java `component` contra el espejo (por huella). Ok(None): el espejo no lo tiene para
/// este sistema. Devuelve cuántos archivos repuso.
pub async fn verify_java(component: &str, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + 'static) -> Result<Option<usize>, String> {
    let Some(remote) = remote(paths).await? else { return Ok(None) };
    let id = java_id(component);
    if !remote.index.packages.contains_key(&id) {
        return Ok(None);
    }
    let (paths, component) = (paths.clone(), component.to_string());
    let fixed = blocking(move || remote.install(&id, &paths.base, &paths, &format!("Java ({component})"), true, &progress)).await?;
    Ok(Some(fixed))
}

/// Instala un loader desde el espejo (la versión pedida o la más nueva que tenga). Devuelve (id de la
/// versión, versión del loader); None si el espejo no lo tiene.
pub async fn install_loader(
    loader: Loader,
    minecraft: &str,
    requested: Option<&str>,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + 'static,
) -> Result<Option<(String, String)>, String> {
    let Some(remote) = remote(paths).await? else { return Ok(None) };
    let version = match requested.map(str::trim).filter(|v| !v.is_empty()) {
        Some(version) if remote.index.packages.contains_key(&loader_id(loader, minecraft, version)) => version.to_string(),
        // Esa versión exacta no está (un modpack de Modrinth pide la suya y el espejo guarda la recomendada
        // de cada Minecraft): la más nueva que tenga, como con los servidores. Los mods piden una mínima.
        _ => match loader_versions(&remote, loader, minecraft).into_iter().next() {
            Some(version) => version,
            None => return Ok(None),
        },
    };
    let id = loader_id(loader, minecraft, &version);
    let Some(version_id) = remote.index.packages.get(&id).and_then(|p| p.version_id.clone()) else { return Ok(None) };
    let paths = paths.clone();
    let label = format!("{} {version}", loader.name());
    blocking(move || remote.install(&id, &paths.base, &paths, &label, false, &progress)).await?;
    Ok(Some((version_id, version)))
}

/// Instala el software de un servidor desde el espejo en `folder`. Si el espejo no tiene esa versión del
/// loader, la más nueva que tenga para esa versión de Minecraft. Devuelve (cómo arrancarlo, versión del
/// loader); None si el espejo no tiene ese servidor.
pub async fn install_server(
    minecraft: &str,
    loader: Option<(Loader, &str)>,
    folder: &Path,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + 'static,
) -> Result<Option<(Vec<String>, Option<String>)>, String> {
    let Some(remote) = remote(paths).await? else { return Ok(None) };
    let loader = match loader {
        Some((loader, version)) if !remote.index.packages.contains_key(&server_id(minecraft, Some((loader, version)))) => {
            let prefix = format!("servidor/{minecraft}/{}/", loader.name().to_lowercase());
            let mut versions: Vec<&str> = remote.index.packages.keys().filter_map(|k| k.strip_prefix(&prefix)).collect();
            versions.sort_by(|a, b| compare_versions(b, a));
            match versions.first() {
                Some(newest) => Some((loader, newest.to_string())),
                None => return Ok(None),
            }
        }
        other => other.map(|(l, v)| (l, v.to_string())),
    };
    let loader_ref = loader.as_ref().map(|(l, v)| (*l, v.as_str()));
    let id = server_id(minecraft, loader_ref);
    let Some(package) = remote.index.packages.get(&id).cloned() else { return Ok(None) };
    let (paths, folder_copy) = (paths.clone(), folder.to_path_buf());
    let label = match &loader {
        Some((loader, version)) => t!("el servidor de {} {version}", "the {} {version} server", loader.name()),
        None => t!("el servidor de Minecraft {minecraft}", "the Minecraft {minecraft} server"),
    };
    blocking(move || remote.install(&id, &folder_copy, &paths, &label, false, &progress)).await?;
    // Forge y NeoForge: el archivo de argumentos de este sistema (win_args.txt o unix_args.txt).
    let launch = match &loader {
        Some((Loader::Forge | Loader::NeoForge, version)) => crate::servidor::forge_launch(folder, version).unwrap_or(package.launch),
        _ => package.launch,
    };
    Ok(Some((launch, loader.map(|(_, v)| v))))
}

// ================= Armar el espejo (quien publica)

/// Lo que se agrega al espejo.
#[derive(Clone, Debug)]
pub enum Item {
    Minecraft(String),
    /// Un loader para esa versión de Minecraft (la versión pedida o la recomendada).
    Loader(Loader, String, Option<String>),
    /// El servidor vanilla o con un loader (la versión pedida o la recomendada).
    Server(String, Option<(Loader, Option<String>)>),
}

impl Item {
    pub fn describe(&self) -> String {
        let version = |v: &Option<String>| v.as_ref().map(|v| format!(" {v}")).unwrap_or_default();
        match self {
            Item::Minecraft(v) => format!("Minecraft {v}"),
            Item::Loader(l, v, lv) => t!("{}{} para {v}", "{}{} for {v}", l.name(), version(lv)),
            Item::Server(v, None) => t!("servidor {v}", "server {v}"),
            Item::Server(v, Some((l, lv))) => t!("servidor {}{} {v}", "{}{} server {v}", l.name(), version(lv)),
        }
    }
}

/// (loader, versión del loader) de un id de versión como «fabric-loader-0.18.4-1.21.1»,
/// «1.20.1-forge-47.4.20» o «neoforge-21.1.228» (los de los modpacks del catálogo).
pub fn loader_of(version_id: &str, minecraft: &str) -> Option<(Loader, String)> {
    let suffix = format!("-{minecraft}");
    if let Some(version) = version_id.strip_prefix("fabric-loader-").and_then(|r| r.strip_suffix(&suffix)) {
        return Some((Loader::Fabric, version.to_string()));
    }
    if let Some(version) = version_id.strip_prefix("quilt-loader-").and_then(|r| r.strip_suffix(&suffix)) {
        return Some((Loader::Quilt, version.to_string()));
    }
    if let Some(version) = version_id.strip_prefix("neoforge-") {
        return Some((Loader::NeoForge, version.to_string()));
    }
    let forge = format!("{minecraft}-forge-");
    version_id.strip_prefix(&forge).or_else(|| version_id.strip_prefix("forge-")).map(|v| (Loader::Forge, v.to_string()))
}

/// La versión pedida del loader o la recomendada.
async fn loader_version(loader: Loader, minecraft: &str, requested: &Option<String>) -> Result<String, String> {
    if let Some(version) = requested {
        return Ok(version.clone());
    }
    loaders::recommended(&loaders::loader_versions(loader, minecraft).await?)
        .map(|v| v.version.clone())
        .ok_or(format!("{} no tiene versiones para {minecraft}", loader.name()))
}

/// Qué subir: todas las versiones (release) de Minecraft que Parabellum abre, o solo `only`, con cada
/// loader que las tenga (su versión recomendada) y sus servidores. De la más nueva a la más vieja: así lo
/// de las más jugadas queda junto en las capas. `extra_servers`: los de los modpacks del catálogo (su
/// versión exacta del loader).
pub async fn plan(only: &[String], loaders_too: bool, servers: bool, extra_servers: &[(String, Loader, String)]) -> Result<Vec<Item>, String> {
    let releases: Vec<String> = crate::mojang::list().await?.into_iter().filter(|r| r.kind == "release").map(|r| r.id).collect();
    let wanted: Vec<String> = if only.is_empty() { releases } else { releases.into_iter().filter(|r| only.contains(r)).collect() };
    let mut games: HashMap<&'static str, Vec<String>> = HashMap::new();
    if loaders_too {
        for loader in [Loader::Fabric, Loader::Quilt, Loader::Forge, Loader::NeoForge] {
            games.insert(loader.name(), loaders::game_versions(loader).await.unwrap_or_default());
        }
    }
    let mut items = Vec::new();
    for version in wanted {
        items.push(Item::Minecraft(version.clone()));
        let available: Vec<Loader> =
            [Loader::Fabric, Loader::Quilt, Loader::Forge, Loader::NeoForge].into_iter().filter(|l| games.get(l.name()).is_some_and(|g| g.contains(&version))).collect();
        items.extend(available.iter().map(|l| Item::Loader(*l, version.clone(), None)));
        if servers {
            items.push(Item::Server(version.clone(), None));
            items.extend(available.iter().map(|l| Item::Server(version.clone(), Some((*l, None)))));
            for (minecraft, loader, loader_version) in extra_servers.iter().filter(|(m, _, _)| *m == version) {
                items.push(Item::Server(minecraft.clone(), Some((*loader, Some(loader_version.clone())))));
            }
        }
    }
    Ok(items)
}

struct LayerWriter {
    name: String,
    path: PathBuf,
    zip: zip::ZipWriter<std::fs::File>,
    size: u64,
}

/// Lo que no vale la pena comprimir (ya viene comprimido).
fn stored(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(e.to_ascii_lowercase().as_str(), "jar" | "zip" | "ogg" | "png" | "jpg" | "lzma" | "gz" | "xz" | "mp3" | "dll" | "so" | "dylib" | "jsa")
    })
}

pub struct Builder {
    paths: Paths,
    publisher: Arc<crate::drive::Publisher>,
    /// La carpeta donde se arman las capas (se borran al subirlas).
    out: PathBuf,
    index: Index,
    known: HashSet<String>,
    writer: Option<LayerWriter>,
    /// Paquetes de la capa abierta: entran al índice cuando se sube.
    pending: Vec<(String, Package)>,
    releases: Vec<crate::mojang::Release>,
    log: Box<dyn Fn(String) + Send + Sync>,
}

impl Builder {
    /// Lee lo que ya tiene el espejo en Drive (el índice y qué archivos guarda cada capa).
    pub async fn open(paths: &Paths, publisher: Arc<crate::drive::Publisher>, log: impl Fn(String) + Send + Sync + 'static) -> Result<Self, String> {
        let config = crate::config::Config::load(paths);
        let catalog = crate::catalog::load(paths, &config).await;
        let mut index = Index { format: FORMAT, ..Default::default() };
        let mut known = HashSet::new();
        if let Some(location) = catalog.mirror.clone() {
            let remote = from_location(location).await?;
            index = remote.index.clone();
            let paths = paths.clone();
            let remote_copy = remote.clone();
            known = tauri::async_runtime::spawn_blocking(move || remote_copy.objects(&paths).map(|o| o.keys().cloned().collect::<HashSet<_>>()))
                .await
                .map_err(|e| e.to_string())??;
            log(t!("El espejo ya tiene {} paquetes, {} capas y {} archivos.", "The mirror already has {} packages, {} layers and {} files.", index.packages.len(), index.layers.len(), known.len()));
        }
        let releases = crate::mojang::list().await?;
        Ok(Self {
            out: paths.data.join("espejo-salida"),
            paths: paths.clone(),
            publisher,
            index,
            known,
            writer: None,
            pending: Vec::new(),
            releases,
            log: Box::new(log),
        })
    }

    pub fn has(&self, id: &str) -> bool {
        self.index.packages.contains_key(id) || self.pending.iter().any(|(p, _)| p == id)
    }

    /// Baja, empaqueta y (al llenarse la capa) sube un elemento. Lo que ya está en el espejo se salta.
    pub async fn add(&mut self, item: &Item) -> Result<(), String> {
        let log_progress = |_: String, _: f64| {};
        match item {
            Item::Minecraft(version) => {
                let id = minecraft_id(version);
                if self.has(&id) {
                    return Ok(());
                }
                self.ensure_minecraft(version).await?;
                let files = crate::launch::version_files(version, &self.paths)?;
                let java = files.own_java().cloned().ok_or(t!("Falta el Java de Minecraft {version}.", "Java for Minecraft {version} is missing."))?;
                let java_package = java_id(&java.name);
                if !self.has(&java_package) {
                    let java_files: Vec<PathBuf> = crate::packs::walk(&java.home).into_iter().filter(|f| !f.extension().is_some_and(|e| e == "parte")).collect();
                    let package = Package { kind: "java".into(), ..Default::default() };
                    let base = self.paths.base.clone();
                    self.add_package(&java_package, package, &base, java_files)?;
                }
                if let Some(missing) = files.files.iter().find(|f| !f.exists()) {
                    return Err(format!("Minecraft {version}: falta {}", missing.display()));
                }
                let released = self.releases.iter().find(|r| &r.id == version).map(|r| r.release_time.clone());
                let package = Package { kind: "minecraft".into(), minecraft: Some(version.clone()), released, java: Some(java.name.clone()), ..Default::default() };
                let base = self.paths.base.clone();
                self.add_package(&id, package, &base, files.files)?;
            }
            Item::Loader(loader, minecraft, requested) => {
                let version = loader_version(*loader, minecraft, requested).await?;
                let id = loader_id(*loader, minecraft, &version);
                if self.has(&id) {
                    return Ok(());
                }
                self.ensure_minecraft(minecraft).await?;
                let version_id = loaders::install(*loader, minecraft, &version, &self.paths, log_progress).await?;
                let vanilla: HashSet<String> =
                    crate::launch::version_files(minecraft, &self.paths)?.files.iter().map(|f| f.to_string_lossy().to_lowercase()).collect();
                let files: Vec<PathBuf> = crate::launch::version_files(&version_id, &self.paths)?
                    .files
                    .into_iter()
                    .filter(|f| !vanilla.contains(&f.to_string_lossy().to_lowercase()))
                    .collect();
                if let Some(missing) = files.iter().find(|f| !f.exists()) {
                    return Err(format!("{} {version}: falta {}", loader.name(), missing.display()));
                }
                let package = Package {
                    kind: "loader".into(),
                    minecraft: Some(minecraft.clone()),
                    loader: Some(loader.name().into()),
                    loader_version: Some(version.clone()),
                    version_id: Some(version_id),
                    ..Default::default()
                };
                let base = self.paths.base.clone();
                self.add_package(&id, package, &base, files)?;
            }
            Item::Server(minecraft, wanted) => {
                let loader = wanted.as_ref().map(|(l, _)| *l);
                let version = match wanted {
                    Some((loader, requested)) => Some(loader_version(*loader, minecraft, requested).await?),
                    None => None,
                };
                let id = server_id(minecraft, loader.zip(version.as_deref()));
                if self.has(&id) {
                    return Ok(());
                }
                self.ensure_minecraft(minecraft).await?;
                let folder = self.paths.data.join("espejo-servidores").join(id.replace(['/', '@'], "_"));
                let _ = std::fs::remove_dir_all(&folder);
                std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
                let launch = crate::servidor::install_software_only(minecraft, loader, version.as_deref(), &folder, &self.paths).await?;
                if loader == Some(Loader::Fabric) {
                    // El lanzador de Fabric baja sus bibliotecas al arrancar por primera vez: se arranca una vez
                    // (sin aceptar el EULA, Minecraft se cierra solo) para que vayan en el paquete.
                    crate::servidor::warm_up(minecraft, &folder, &launch, &self.paths).await?;
                }
                let files: Vec<PathBuf> = crate::packs::walk(&folder).into_iter().filter(|f| server_file(&folder, f)).collect();
                let package = Package {
                    kind: "servidor".into(),
                    minecraft: Some(minecraft.clone()),
                    loader: loader.map(|l| l.name().to_string()),
                    loader_version: version,
                    launch,
                    ..Default::default()
                };
                self.add_package(&id, package, &folder, files)?;
                let _ = std::fs::remove_dir_all(&folder);
            }
        }
        if self.writer.as_ref().is_some_and(|w| w.size >= LAYER_LIMIT) {
            self.flush().await?;
        }
        Ok(())
    }

    async fn ensure_minecraft(&self, version: &str) -> Result<(), String> {
        let release = self.releases.iter().find(|r| r.id == version).ok_or(format!("Mojang no tiene {version}"))?;
        crate::mojang::install(release, &self.paths, |_, _| {}).await
    }

    fn add_package(&mut self, id: &str, mut package: Package, root: &Path, files: Vec<PathBuf>) -> Result<(), String> {
        let mut listed = Vec::with_capacity(files.len());
        for file in files {
            let relative = file.strip_prefix(root).map_err(|_| format!("{} no está en {}", file.display(), root.display()))?.to_string_lossy().replace('\\', "/");
            let size = std::fs::metadata(&file).map_err(|e| format!("{}: {e}", file.display()))?.len();
            let sha1 = hash(&file)?;
            if self.known.insert(sha1.clone()) {
                self.write_object(&sha1, &file, size)?;
            }
            listed.push(Listed { path: relative, sha1, size });
        }
        package.files = listed.len();
        package.size = listed.iter().map(|f| f.size).sum();
        let writer = self.writer()?;
        writer.zip.start_file(list_entry(id), zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated)).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&listed).unwrap();
        writer.zip.write_all(&bytes).map_err(|e| e.to_string())?;
        writer.size += bytes.len() as u64;
        package.list = writer.name.clone();
        (self.log)(format!("  + {id}: {} archivos, {:.0} MB", package.files, package.size as f64 / 1048576.0));
        self.pending.push((id.to_string(), package));
        Ok(())
    }

    fn writer(&mut self) -> Result<&mut LayerWriter, String> {
        if self.writer.is_none() {
            let number = self.index.layers.len() + 1;
            let name = format!("capa-{number:03}.zip");
            let path = self.out.join(FOLDER).join(&name);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            let file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
            self.writer = Some(LayerWriter { name, path, zip: zip::ZipWriter::new(file), size: 0 });
        }
        Ok(self.writer.as_mut().unwrap())
    }

    fn write_object(&mut self, sha1: &str, file: &Path, size: u64) -> Result<(), String> {
        let method = if stored(file) { zip::CompressionMethod::Stored } else { zip::CompressionMethod::Deflated };
        let writer = self.writer()?;
        writer
            .zip
            .start_file(object_entry(sha1), zip::write::SimpleFileOptions::default().compression_method(method).large_file(size > u32::MAX as u64))
            .map_err(|e| e.to_string())?;
        let mut source = std::fs::File::open(file).map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut writer.zip).map_err(|e| e.to_string())?;
        writer.size += size;
        Ok(())
    }

    /// Cierra la capa abierta, la sube y sube el índice con sus paquetes.
    pub async fn flush(&mut self) -> Result<(), String> {
        let Some(writer) = self.writer.take() else { return Ok(()) };
        writer.zip.finish().map_err(|e| e.to_string())?;
        let size = std::fs::metadata(&writer.path).map_err(|e| e.to_string())?.len();
        let (usage, limit) = self.publisher.storage().await?;
        if let Some(limit) = limit {
            if usage + size + 200 * 1048576 > limit {
                return Err(format!(
                    "El Drive no tiene espacio para {} ({:.1} GB; libres {:.1} GB). Quedó en {}.",
                    writer.name,
                    size as f64 / 1e9,
                    limit.saturating_sub(usage) as f64 / 1e9,
                    writer.path.display()
                ));
            }
        }
        (self.log)(t!("Subiendo {} ({:.2} GB)…", "Uploading {} ({:.2} GB)…", writer.name, size as f64 / 1e9));
        let started = std::time::Instant::now();
        let log = &self.log;
        let last = Mutex::new(std::time::Instant::now());
        self.publisher
            .publish(&self.out, std::slice::from_ref(&writer.path), |stage, _| {
                let mut last = last.lock().unwrap();
                if last.elapsed().as_secs() >= 30 {
                    *last = std::time::Instant::now();
                    log(format!("  {stage}"));
                }
            })
            .await?;
        (self.log)(t!("Subida {} en {:.0} s ({:.1} MB/s).", "Uploaded {} in {:.0} s ({:.1} MB/s).", writer.name, started.elapsed().as_secs_f64(), size as f64 / 1048576.0 / started.elapsed().as_secs_f64().max(1.0)));
        self.index.layers.push(Layer { name: writer.name.clone(), size });
        self.index.packages.extend(self.pending.drain(..));
        self.upload_index().await?;
        let _ = std::fs::remove_file(&writer.path);
        Ok(())
    }

    async fn upload_index(&mut self) -> Result<(), String> {
        self.index.format = FORMAT;
        self.index.updated = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        let path = self.out.join(FOLDER).join(INDEX_FILE);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, serde_json::to_vec_pretty(&self.index).unwrap()).map_err(|e| e.to_string())?;
        self.publisher.publish(&self.out, &[path], |_, _| {}).await?;
        (self.log)(t!("Índice subido: {} paquetes en {} capas.", "Index uploaded: {} packages in {} layers.", self.index.packages.len(), self.index.layers.len()));
        Ok(())
    }
}

/// SHA-1 de un archivo. Los sonidos de Mojang ya se llaman como su SHA-1 (assets/objects/ab/ab…).
fn hash(file: &Path) -> Result<String, String> {
    let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let in_objects = file.parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).is_some_and(|n| n == "objects");
    if in_objects && name.len() == 40 && name.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(name.to_lowercase());
    }
    let mut source = std::fs::File::open(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            return Ok(hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// Lo que va en el paquete de un servidor: su software, sin lo que crea al arrancar (mundo, ajustes,
/// registros, el EULA).
fn server_file(folder: &Path, file: &Path) -> bool {
    let Ok(relative) = file.strip_prefix(folder) else { return false };
    let first = relative.components().next().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).unwrap_or_default();
    let skipped = [
        "eula.txt", "server.properties", "logs", "crash-reports", "world", "usercache.json", "ops.json", "whitelist.json", "banned-ips.json",
        "banned-players.json", ".fabric", ".quilt", "mods", "config", "installer.log", "parabellum-servidor.json",
    ];
    !skipped.contains(&first.as_str()) && !first.ends_with(".log") && !file.extension().is_some_and(|e| e == "parte")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_del_espejo() {
        assert!(minecraft_id("1.20.1").starts_with("minecraft/1.20.1@"));
        assert_eq!(loader_id(Loader::NeoForge, "1.21.1", "21.1.228"), "neoforge/1.21.1/21.1.228");
        assert_eq!(server_id("1.20.1", Some((Loader::Forge, "47.4.0"))), "servidor/1.20.1/forge/47.4.0");
        assert_eq!(server_id("26.3", None), "servidor/26.3/vanilla");
        assert_eq!(list_entry("minecraft/1.20.1@windows-x86_64"), "listas/minecraft_1.20.1_windows-x86_64.json");
        assert!(is_version_json(Path::new("base/versions/1.20.1/1.20.1.json")));
        assert_eq!(loader_of("fabric-loader-0.18.4-1.21.1", "1.21.1"), Some((Loader::Fabric, "0.18.4".into())));
        assert_eq!(loader_of("1.20.1-forge-47.4.20", "1.20.1"), Some((Loader::Forge, "47.4.20".into())));
        assert_eq!(loader_of("neoforge-21.1.228", "1.21.1"), Some((Loader::NeoForge, "21.1.228".into())));
        assert_eq!(loader_of("1.21.1", "1.21.1"), None);
        assert!(!is_version_json(Path::new("base/assets/indexes/5.json")));
        let folder = Path::new("s");
        assert!(server_file(folder, Path::new("s/libraries/a/b.jar")));
        assert!(server_file(folder, Path::new("s/server.jar")));
        assert!(!server_file(folder, Path::new("s/eula.txt")));
        assert!(!server_file(folder, Path::new("s/logs/latest.log")));
        assert!(!server_file(folder, Path::new("s/.fabric/remapped/x.jar")));
    }

    #[test]
    fn huella_de_los_sonidos_por_su_nombre() {
        let root = std::env::temp_dir().join(format!("parabellum-espejo-{}", std::process::id()));
        let object = root.join("objects").join("4d").join("4d186321c1a7f0f354b297e8914ab2400000000a");
        std::fs::create_dir_all(object.parent().unwrap()).unwrap();
        std::fs::write(&object, b"no se lee").unwrap();
        assert_eq!(hash(&object).unwrap(), "4d186321c1a7f0f354b297e8914ab2400000000a");
        let other = root.join("hola.txt");
        std::fs::write(&other, b"hola").unwrap();
        assert_eq!(hash(&other).unwrap(), hex::encode(Sha1::digest(b"hola")));
        let _ = std::fs::remove_dir_all(&root);
    }
}
