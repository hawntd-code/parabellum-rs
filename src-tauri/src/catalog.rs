//! Catálogo de modpacks y versiones de Minecraft para redes donde Mojang y Modrinth están bloqueados
//! (la escuela): la carpeta pública de Google Drive y la carpeta "paquetes" junto al launcher (por
//! ejemplo, copiada de una USB). Mismo formato y organización que la versión C#:
//!   <carpeta>/<versión>/base-minecraft-<versión>.zip y .json
//!   <carpeta>/<versión>/<Modpack>/<Modpack>.zip, .json y .png
//!   <carpeta>/_parabellum/Parabellum-<versión>.zip y .json   (actualizaciones del launcher)
//! La API de Drive (con clave) solo se usa para LISTAR; los archivos se bajan por el enlace directo
//! de Drive, que no necesita clave, deja reanudar y descargar por partes.

use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use crate::config::Config;
use crate::download;
use crate::instances::{self, Instance, INSTANCE_FILE, STATE_DIR};
use crate::loaders::compare_versions;
use crate::packs::{self, Installed, Manifest};
use crate::paths::Paths;
use crate::remote_zip::{self, RemoteZip};

const API: &str = "https://www.googleapis.com/drive/v3/files";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
pub const LOCAL_FOLDER: &str = "paquetes";
/// Id de las actualizaciones de este launcher (la versión C# usa "parabellum-lanzador").
pub const LAUNCHER_ID: &str = "parabellum";
pub const LAUNCHER_TYPE: &str = "lanzador";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Drive,
    Carpeta,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub manifest: Manifest,
    pub source: Source,
    /// Id del archivo en Drive o ruta local del zip.
    pub archive: String,
    pub size: u64,
    /// Icono como data URL.
    pub icon: Option<String>,
    /// Carpeta donde está ("1.20.1/Better MC BMC4").
    pub location: String,
}

impl Entry {
    pub fn is_launcher(&self) -> bool {
        self.manifest.kind == LAUNCHER_TYPE
    }

    pub fn url(&self) -> Option<String> {
        (self.source == Source::Drive).then(|| download_url(&self.archive))
    }
}

#[derive(Clone, Default)]
pub struct Catalog {
    pub entries: Vec<Entry>,
    pub warnings: Vec<String>,
}

pub fn download_url(file_id: &str) -> String {
    format!("https://drive.usercontent.google.com/download?id={file_id}&export=download&confirm=t")
}

/// Los ids de Drive solo llevan letras, números, guiones y guiones bajos.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() < 200 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

// ================= Leer el catálogo

pub async fn load(paths: &Paths, config: &Config) -> Catalog {
    let mut catalog = Catalog::default();
    let local = paths.root.join(LOCAL_FOLDER);
    if let Ok((entries, warnings)) = tauri::async_runtime::spawn_blocking(move || load_local(&local)).await {
        catalog.entries.extend(entries);
        catalog.warnings.extend(warnings.into_iter().map(|w| t!("Carpeta paquetes: {w}", "Packages folder: {w}")));
    }
    if config.has_drive() {
        match load_drive(paths, config).await {
            Ok((entries, warnings)) => {
                catalog.entries.extend(entries);
                catalog.warnings.extend(warnings.into_iter().map(|w| format!("Google Drive: {w}")));
            }
            Err(error) => catalog.warnings.push(format!("Google Drive: {error}")),
        }
    }
    catalog.entries = merge(std::mem::take(&mut catalog.entries));
    catalog
}

fn image_data_url(bytes: &[u8]) -> Option<String> {
    let kind = instances::image_type(bytes)?;
    (bytes.len() < 1_500_000).then(|| format!("data:{kind};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

fn load_local(folder: &Path) -> (Vec<Entry>, Vec<String>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    for json in packs::walk(folder).into_iter().filter(|f| f.extension().is_some_and(|e| e.eq_ignore_ascii_case("json"))) {
        let Ok(text) = std::fs::read_to_string(&json) else { continue };
        let Ok(manifest) = serde_json::from_str::<Manifest>(&text) else {
            warnings.push(t!("{} no es un paquete válido.", "{} is not a valid package.", json.display()));
            continue;
        };
        let dir = json.parent().unwrap();
        let zip = dir.join(&manifest.archive);
        if manifest.id.is_empty() || manifest.archive.is_empty() || !zip.is_file() {
            continue;
        }
        let icon = manifest.icon.as_ref().and_then(|name| std::fs::read(dir.join(name)).ok()).and_then(|b| image_data_url(&b));
        entries.push(Entry {
            size: zip.metadata().map(|m| m.len()).unwrap_or(0),
            archive: zip.display().to_string(),
            location: dir.strip_prefix(folder).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
            source: Source::Carpeta,
            icon,
            manifest,
        });
    }
    (entries, warnings)
}

async fn load_drive(paths: &Paths, config: &Config) -> Result<(Vec<Entry>, Vec<String>), String> {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    // Raíz / versión / modpack (y un nivel más por si acaso).
    let mut pending = VecDeque::from([(config.drive_folder_id.clone(), String::new(), 0)]);
    while let Some((folder_id, path, depth)) = pending.pop_front() {
        let children = list_children(&folder_id, &config.drive_api_key).await?;
        let mut by_name: HashMap<String, &Value> = HashMap::new();
        for child in &children {
            let name = child["name"].as_str().unwrap_or_default();
            if child["mimeType"] == FOLDER_MIME {
                if depth < 3 && valid_id(child["id"].as_str().unwrap_or_default()) {
                    pending.push_back((child["id"].as_str().unwrap().to_string(), format!("{path}{name}/"), depth + 1));
                }
                continue;
            }
            // Si hay dos archivos con el mismo nombre, el más reciente.
            let newer = by_name.get(&name.to_lowercase()).is_none_or(|old| child["modifiedTime"].as_str() > old["modifiedTime"].as_str());
            if newer {
                by_name.insert(name.to_lowercase(), child);
            }
        }
        for json in by_name.values().filter(|f| f["name"].as_str().is_some_and(|n| n.to_lowercase().ends_with(".json"))) {
            let name = json["name"].as_str().unwrap_or_default();
            let Some(id) = json["id"].as_str().filter(|id| valid_id(id)) else { continue };
            let manifest = match download_text(id).await.and_then(|t| serde_json::from_str::<Manifest>(&t).map_err(|e| e.to_string())) {
                Ok(manifest) => manifest,
                Err(error) => {
                    warnings.push(format!("{path}{name}: {error}"));
                    continue;
                }
            };
            if manifest.id.is_empty() {
                continue;
            }
            let Some(zip) = by_name.get(&manifest.archive.to_lowercase()).filter(|z| z["id"].as_str().is_some_and(valid_id)) else {
                warnings.push(t!("{path}{name}: falta «{}» en la misma carpeta.", "{path}{name}: «{}» is missing from the same folder.", manifest.archive));
                continue;
            };
            let icon = match manifest.icon.as_ref().and_then(|n| by_name.get(&n.to_lowercase())) {
                Some(icon) => cached_icon(icon, paths).await,
                None => None,
            };
            entries.push(Entry {
                archive: zip["id"].as_str().unwrap().to_string(),
                size: zip["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(manifest.archive_size),
                location: path.trim_end_matches('/').to_string(),
                source: Source::Drive,
                icon,
                manifest,
            });
        }
    }
    Ok((entries, warnings))
}

async fn list_children(folder_id: &str, api_key: &str) -> Result<Vec<Value>, String> {
    let mut files = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut query = vec![
            ("q", format!("'{folder_id}' in parents and trashed = false")),
            ("fields", "nextPageToken,files(id,name,size,mimeType,modifiedTime)".into()),
            ("pageSize", "1000".into()),
            ("supportsAllDrives", "true".into()),
            ("includeItemsFromAllDrives", "true".into()),
            ("key", api_key.into()),
        ];
        if let Some(token) = &page_token {
            query.push(("pageToken", token.clone()));
        }
        let response = download::client().get(API).query(&query).send().await.map_err(|_| unavailable())?;
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        if !status.is_success() {
            return Err(drive_error(status, &body));
        }
        files.extend(body["files"].as_array().cloned().unwrap_or_default());
        page_token = body["nextPageToken"].as_str().map(String::from);
        if page_token.is_none() {
            return Ok(files);
        }
    }
}

fn unavailable() -> String {
    t!("No se puede conectar con Google Drive.", "Cannot connect to Google Drive.")
}

/// Los errores de la API de Drive, explicados.
fn drive_error(status: reqwest::StatusCode, body: &Value) -> String {
    let error = &body["error"];
    let reason = error["details"]
        .as_array()
        .and_then(|d| d.iter().find_map(|x| x["reason"].as_str()))
        .or_else(|| error["errors"][0]["reason"].as_str())
        .or_else(|| error["status"].as_str())
        .unwrap_or_default();
    let message = error["message"].as_str().unwrap_or_default();
    match reason {
        "API_KEY_SERVICE_BLOCKED" => t!("La clave de API no tiene permiso para usar la Google Drive API.", "The API key is not allowed to use the Google Drive API."),
        "SERVICE_DISABLED" | "accessNotConfigured" => t!("La Google Drive API no está habilitada en el proyecto de Google Cloud.", "The Google Drive API is not enabled in the Google Cloud project."),
        "API_KEY_INVALID" | "keyInvalid" => t!("La clave de API de Google configurada no es válida.", "The configured Google API key is not valid."),
        "userRateLimitExceeded" | "rateLimitExceeded" => t!("Demasiadas solicitudes a Google Drive. Espera un minuto y vuelve a intentarlo.", "Too many requests to Google Drive. Wait a minute and try again."),
        _ if status == reqwest::StatusCode::NOT_FOUND => {
            t!("No se encontró la carpeta de Drive. Comprueba que esté compartida como «Cualquier persona con el enlace».", "The Drive folder was not found. Check that it is shared as «Anyone with the link».")
        }
        _ => t!("Google Drive respondió {status}: {message}", "Google Drive answered {status}: {message}"),
    }
}

/// Drive responde con una página (y no con el archivo) cuando bloquea la descarga.
async fn check_download(response: reqwest::Response) -> Result<reqwest::Response, String> {
    let html = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|t| t.contains("html"));
    if response.status().is_success() && !html {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default().to_lowercase();
    Err(if body.contains("quota") || body.contains("cuota") {
        t!("Google Drive limitó por hoy las descargas de este archivo por exceso de uso. Prueba más tarde o instálalo desde una USB.", "Google Drive limited downloads of this file for today due to heavy use. Try later or install it from a USB drive.")
    } else if status == reqwest::StatusCode::NOT_FOUND {
        t!("No se encontró el archivo en Google Drive. Comprueba que siga en la carpeta.", "The file was not found in Google Drive. Check that it is still in the folder.")
    } else if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::UNAUTHORIZED {
        t!("Google Drive no permitió descargar el archivo. Comprueba que la carpeta esté compartida como «Cualquier persona con el enlace».", "Google Drive did not allow downloading the file. Check that the folder is shared as «Anyone with the link».")
    } else {
        t!("Google Drive devolvió una página en lugar del archivo (puede que la descarga esté limitada). Prueba más tarde.", "Google Drive returned a page instead of the file (the download may be limited). Try later.")
    })
}

async fn download_text(file_id: &str) -> Result<String, String> {
    let response = download::client().get(download_url(file_id)).send().await.map_err(|_| unavailable())?;
    check_download(response).await?.text().await.map_err(|e| e.to_string())
}

/// Los iconos son pequeños: se guardan por id y fecha de modificación.
async fn cached_icon(file: &Value, paths: &Paths) -> Option<String> {
    let id = file["id"].as_str().filter(|id| valid_id(id))?;
    let stamp: String = file["modifiedTime"].as_str().unwrap_or_default().chars().filter(|c| c.is_ascii_digit()).collect();
    let path = paths.data.join("cache").join("iconos").join(format!("{id}-{stamp}.png"));
    if let Ok(bytes) = std::fs::read(&path) {
        return image_data_url(&bytes);
    }
    let response = download::client().get(download_url(id)).send().await.ok()?;
    let bytes = check_download(response).await.ok()?.bytes().await.ok()?;
    let _ = std::fs::create_dir_all(path.parent()?);
    let _ = std::fs::write(&path, &bytes);
    image_data_url(&bytes)
}

/// Un paquete puede estar en varias fuentes: gana la versión más nueva y, a igual versión, la local.
fn merge(entries: Vec<Entry>) -> Vec<Entry> {
    let mut best: HashMap<String, Entry> = HashMap::new();
    for entry in entries {
        let key = if entry.is_launcher() {
            format!("{}@{}", entry.manifest.id.to_lowercase(), entry.manifest.version)
        } else {
            entry.manifest.id.to_lowercase()
        };
        let replace = match best.get(&key) {
            None => true,
            Some(current) => match entry.manifest.version.cmp(&current.manifest.version) {
                Ordering::Greater => true,
                Ordering::Equal => entry.source == Source::Carpeta && current.source == Source::Drive,
                Ordering::Less => false,
            },
        };
        if replace {
            best.insert(key, entry);
        }
    }
    let mut list: Vec<Entry> = best.into_values().collect();
    list.sort_by_key(|e| e.manifest.name.to_lowercase());
    list
}

// ================= Estado de cada paquete en este PC

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: String,
    /// "modpack" o "base"
    pub kind: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub minecraft_version: String,
    pub loader: String,
    pub mod_count: u32,
    pub size: u64,
    pub installed_size: u64,
    pub exported_at: String,
    pub icon: Option<String>,
    pub source: Source,
    pub location: String,
    /// "nuevo", "instalado" o "actualizar"
    pub status: String,
    /// Instancia donde está instalado.
    pub instance: Option<String>,
}

/// La instancia instalada desde este paquete (o la original desde la que se exportó).
pub fn installed_instance<'a>(manifest: &Manifest, instances: &'a [Instance]) -> Option<(&'a Instance, Option<Manifest>)> {
    for instance in instances {
        if let Some(installed) = packs::installed_manifest(instance).filter(|m| m.id.eq_ignore_ascii_case(&manifest.id)) {
            return Some((instance, Some(installed)));
        }
    }
    instances
        .iter()
        .find(|i| {
            packs::installed_manifest(i).is_none()
                && (manifest.folder.as_deref().is_some_and(|f| f.eq_ignore_ascii_case(&i.id)) || packs::slug(&i.id) == manifest.id)
        })
        .map(|i| (i, None))
}

pub fn view(entry: &Entry, instances: &[Instance], paths: &Paths) -> EntryView {
    let m = &entry.manifest;
    let (status, instance) = if m.is_base() {
        (if packs::base_available(&m.minecraft_version, paths) { "instalado" } else { "nuevo" }, None)
    } else {
        match installed_instance(m, instances) {
            Some((instance, Some(installed))) if m.version > installed.version => ("actualizar", Some(instance.id.clone())),
            Some((instance, _)) => ("instalado", Some(instance.id.clone())),
            None => ("nuevo", None),
        }
    };
    EntryView {
        id: m.id.clone(),
        kind: m.kind.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        version: m.version.clone(),
        minecraft_version: m.minecraft_version.clone(),
        loader: m.loader.clone(),
        mod_count: m.mod_count,
        size: entry.size,
        installed_size: m.installed_size,
        exported_at: m.exported_at.clone(),
        icon: entry.icon.clone(),
        source: entry.source,
        location: entry.location.clone(),
        status: status.into(),
        instance,
    }
}

// ================= Descargar e instalar

/// Zip listo para instalar: el local tal cual o el de Drive descargado (con reanudación y reintentos).
/// Devuelve (ruta, es una copia descargada que se puede borrar después).
pub async fn fetch(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync) -> Result<(PathBuf, bool), String> {
    if entry.source == Source::Carpeta {
        return Ok((PathBuf::from(&entry.archive), false));
    }
    let dir = paths.data.join("descargas");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let finished = dir.join(format!("{}.zip", entry.archive));
    if finished.metadata().is_ok_and(|m| m.len() == entry.size) {
        return Ok((finished, true));
    }
    let partial = dir.join(format!("{}.zip.parte", entry.archive));
    let mut attempt = 1;
    loop {
        match download_part(entry, &partial, &progress).await {
            Ok(()) => break,
            Err((error, retry)) if retry && attempt < 4 => {
                progress(t!("Se cortó la conexión, reintentando ({attempt}/3)…", "The connection dropped, retrying ({attempt}/3)…"), 0.0);
                tokio::time::sleep(std::time::Duration::from_secs(3 * attempt)).await;
                attempt += 1;
                let _ = error;
            }
            Err((error, _)) => return Err(error),
        }
    }
    std::fs::rename(&partial, &finished).map_err(|e| e.to_string())?;
    Ok((finished, true))
}

async fn download_part(entry: &Entry, partial: &Path, progress: &(impl Fn(String, f64) + Send + Sync)) -> Result<(), (String, bool)> {
    let mut existing = partial.metadata().map(|m| m.len()).unwrap_or(0);
    if entry.size > 0 && existing >= entry.size {
        return Ok(());
    }
    let mut request = download::client().get(download_url(&entry.archive));
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    let response = request.send().await.map_err(|e| (t!("No se pudo conectar con Google Drive: {e}", "Could not connect to Google Drive: {e}"), true))?;
    let mut response = check_download(response).await.map_err(|e| (e, false))?;
    if existing > 0 && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        existing = 0; // Drive ignoró el rango: se empieza de nuevo
    }
    let mut file = if existing > 0 {
        tokio::fs::OpenOptions::new().append(true).open(partial).await
    } else {
        tokio::fs::File::create(partial).await
    }
    .map_err(|e| (e.to_string(), false))?;
    let started = std::time::Instant::now();
    let mut done = existing;
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    while let Some(chunk) = response.chunk().await.map_err(|e| (t!("La descarga se cortó: {e}", "The download was interrupted: {e}"), true))? {
        file.write_all(&chunk).await.map_err(|e| (e.to_string(), false))?;
        done += chunk.len() as u64;
        if last.elapsed().as_millis() >= 250 {
            last = std::time::Instant::now();
            let speed = (done - existing) as f64 / started.elapsed().as_secs_f64().max(0.5) / 1048576.0;
            progress(
                t!(
                    "Descargando {}: {:.0} de {:.0} MB · {speed:.1} MB/s", "Downloading {}: {:.0} of {:.0} MB · {speed:.1} MB/s",
                    entry.manifest.name,
                    done as f64 / 1048576.0,
                    entry.size as f64 / 1048576.0
                ),
                done as f64 / entry.size.max(1) as f64,
            );
        }
    }
    file.flush().await.map_err(|e| (e.to_string(), false))?;
    if entry.size > 0 && done != entry.size {
        return Err((t!("La descarga quedó incompleta.", "The download is incomplete."), true));
    }
    Ok(())
}

async fn verify(zip: &Path, manifest: &Manifest) -> Result<(), String> {
    let Some(expected) = manifest.archive_sha256.clone() else { return Ok(()) };
    let file = zip.to_path_buf();
    let actual = tauri::async_runtime::spawn_blocking(move || packs::sha256_file(&file)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(t!("El archivo descargado está dañado (su huella SHA-256 no coincide). Vuelve a intentarlo.", "The downloaded file is damaged (its SHA-256 fingerprint does not match). Try again."));
    }
    Ok(())
}

/// Descarga, verifica e instala un paquete completo.
async fn install_full(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<Installed, String> {
    let (zip, is_copy) = fetch(entry, paths, progress.clone()).await?;
    progress(t!("Comprobando {}…", "Checking {}…", entry.manifest.name), 1.0);
    if let Err(error) = verify(&zip, &entry.manifest).await {
        if is_copy {
            let _ = std::fs::remove_file(&zip);
        }
        return Err(error);
    }
    let (file, paths_copy) = (zip.clone(), paths.clone());
    let result = tauri::async_runtime::spawn_blocking(move || packs::install(&file, &paths_copy, progress)).await.map_err(|e| e.to_string())?;
    if is_copy {
        let _ = std::fs::remove_file(&zip);
    }
    result
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInstall {
    pub id: Option<String>,
    pub name: String,
    pub kind: String,
    pub updated: bool,
    /// Actualización por partes: cuánto se descargó realmente.
    pub downloaded_files: Option<usize>,
    pub downloaded_bytes: Option<u64>,
}

/// Instala un paquete del catálogo (antes, su base si falta). Un modpack ya instalado desde Drive se
/// actualiza bajando solo los archivos que cambiaron; `repair` lo hace aunque la versión sea la misma.
pub async fn install(
    entry: &Entry,
    catalog: &[Entry],
    paths: &Paths,
    instances: &[Instance],
    repair: bool,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<CatalogInstall, String> {
    if entry.is_launcher() {
        return Err(t!("Eso es una actualización del launcher, no un modpack.", "That is a launcher update, not a modpack."));
    }
    let manifest = &entry.manifest;
    if !manifest.is_base() && !packs::base_available(&manifest.minecraft_version, paths) {
        let base_id = manifest.base_id.clone().unwrap_or_else(|| packs::base_id(&manifest.minecraft_version));
        let base = catalog.iter().find(|e| e.manifest.is_base() && e.manifest.id.eq_ignore_ascii_case(&base_id)).ok_or(t!(
            "Este modpack necesita Minecraft {}, pero «base-{base_id}» no está en el catálogo. Súbela a la carpeta de Drive.", "This modpack needs Minecraft {}, but «base-{base_id}» is not in the catalog. Upload it to the Drive folder.",
            manifest.minecraft_version
        ))?;
        install_full(base, paths, progress.clone()).await?;
    }

    // Actualizar (o reparar) por partes: solo modpacks de Drive que ya están instalados desde ese paquete.
    let installed = installed_instance(manifest, instances);
    let previous = installed.as_ref().and_then(|(_, m)| m.as_ref());
    let wants_delta = !manifest.is_base() && entry.source == Source::Drive && previous.is_some_and(|p| repair || manifest.version > p.version);
    if wants_delta {
        let (url, size, manifest_copy, paths_copy, report) = (entry.url().unwrap(), entry.size, manifest.clone(), paths.clone(), progress.clone());
        let result = tauri::async_runtime::spawn_blocking(move || delta_update(&RemoteZip::new(&url, size), &manifest_copy, &paths_copy, &report))
            .await
            .map_err(|e| e.to_string())?;
        match result {
            Ok(stats) => {
                return Ok(CatalogInstall {
                    id: installed.map(|(i, _)| i.id.clone()),
                    name: manifest.name.clone(),
                    kind: "modpack".into(),
                    updated: true,
                    downloaded_files: Some(stats.files),
                    downloaded_bytes: Some(stats.bytes),
                });
            }
            // Si Drive no deja bajar por partes (o cambió casi todo), se baja el zip completo.
            Err(error) if error == remote_zip::NO_RANGES || error == remote_zip::PREFER_FULL => {}
            Err(error) => return Err(error),
        }
    }
    let result = install_full(entry, paths, progress).await?;
    Ok(CatalogInstall { id: result.folder, name: result.name, kind: result.kind, updated: result.updated, downloaded_files: None, downloaded_bytes: None })
}

/// Clave para comparar rutas: mismos separadores y sin distinguir mayúsculas (como Windows).
fn path_key(path: &Path) -> String {
    path.components().collect::<PathBuf>().to_string_lossy().to_lowercase()
}

pub struct DeltaStats {
    pub files: usize,
    pub bytes: u64,
}

/// Actualiza una instancia instalada desde un paquete bajando del zip remoto solo lo que cambió
/// (compara tamaño y CRC32). Conserva lo del jugador como la actualización completa.
fn delta_update(remote: &RemoteZip, manifest: &Manifest, paths: &Paths, progress: &(dyn Fn(String, f64) + Send + Sync)) -> Result<DeltaStats, String> {
    progress(t!("Comparando con la versión de Drive…", "Comparing with the Drive version…"), 0.0);
    let folder_name = manifest.folder.clone().filter(|f| !f.is_empty()).unwrap_or_else(|| manifest.name.clone());
    if !packs::valid_folder_name(&folder_name) {
        return Err(t!("El nombre de carpeta del paquete no es válido.", "The package folder name is not valid."));
    }
    let folder = paths.instances.join(&folder_name);
    let entries = remote.entries()?;
    if !entries.iter().any(|e| e.name == packs::MANIFEST_IN_ARCHIVE) {
        return Err(t!("El zip no es un paquete de Parabellum.", "The zip is not a Parabellum package."));
    }
    let preserved = |rest: &str| {
        let rest = rest.to_lowercase();
        packs::PRESERVED_ON_UPDATE.iter().any(|p| {
            let p = p.to_lowercase();
            rest == p || rest.starts_with(&format!("{p}/"))
        })
    };
    let mut wanted: Vec<(&remote_zip::RemoteEntry, PathBuf)> = Vec::new();
    let mut keep: HashSet<String> = HashSet::new();
    let mut managed_dirs: HashSet<String> = HashSet::new();
    let mut checked = 0;
    for entry in entries.iter().filter(|e| !e.name.ends_with('/')) {
        checked += 1;
        if checked % 200 == 0 {
            progress(t!("Comparando archivos… {checked} de {}", "Comparing files… {checked} of {}", entries.len()), 0.0);
        }
        if let Some(rest) = entry.name.strip_prefix("instancia/") {
            let relative = crate::mrpack::safe_relative(rest).ok_or(t!("El paquete trae una ruta no permitida: {rest}", "The package contains a path that is not allowed: {rest}"))?;
            if preserved(rest) {
                continue;
            }
            let target = folder.join(&relative);
            keep.insert(path_key(&target));
            if let Some((first, _)) = rest.split_once('/') {
                managed_dirs.insert(first.to_lowercase());
            }
            let same = target.metadata().is_ok_and(|m| m.len() == entry.size) && remote_zip::file_crc32(&target) == Some(entry.crc32);
            if !same {
                wanted.push((entry, target));
            }
        } else if let Some(rest) = entry.name.strip_prefix("base/") {
            let Some(relative) = crate::mrpack::safe_relative(rest) else { continue };
            let target = paths.base.join(relative);
            if !target.metadata().is_ok_and(|m| m.len() == entry.size) {
                wanted.push((entry, target));
            }
        }
    }

    // instancia.json viene en el paquete; la memoria y los argumentos de Java propios se conservan.
    let old_settings = folder.join(STATE_DIR).join("instancia-anterior.json");
    let _ = std::fs::create_dir_all(folder.join(STATE_DIR));
    let _ = std::fs::copy(folder.join(INSTANCE_FILE), &old_settings);

    // Si cambió casi todo, una sola descarga del zip completo es más rápida que por partes.
    let compressed: u64 = wanted.iter().map(|(e, _)| e.compressed).sum();
    if compressed * 10 > remote.size() * 7 {
        let _ = std::fs::remove_file(&old_settings);
        return Err(remote_zip::PREFER_FULL.into());
    }
    let total: u64 = wanted.iter().map(|(e, _)| e.size).sum::<u64>().max(1);
    let mut done = 0u64;
    let mut count = 0;
    let entries_only: Vec<&remote_zip::RemoteEntry> = wanted.iter().map(|(e, _)| *e).collect();
    remote.read_many(&entries_only, |index, bytes| {
        let target = &wanted[index].1;
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = PathBuf::from(format!("{}.parte", target.display()));
        std::fs::write(&temp, &bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, target).map_err(|e| e.to_string())?;
        done += bytes.len() as u64;
        count += 1;
        progress(
            t!("Actualizando {}: {count} de {} archivos ({:.0} de {:.0} MB)", "Updating {}: {count} of {} files ({:.0} of {:.0} MB)", manifest.name, wanted.len(), done as f64 / 1048576.0, total as f64 / 1048576.0),
            done as f64 / total as f64,
        );
        Ok(())
    })?;

    // Lo que el paquete ya no trae (por ejemplo, un mod quitado) va a la papelera.
    let mut leftovers = Vec::new();
    let mut managed = 0;
    for dir in &managed_dirs {
        for file in packs::walk(&folder.join(dir)) {
            managed += 1;
            let relative = file.strip_prefix(&folder).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            if !keep.contains(&path_key(&file)) && !preserved(&relative) {
                leftovers.push(file);
            }
        }
    }
    // Seguro: si sobrara más de la mitad, algo no cuadra y no se quita nada.
    if leftovers.len() * 2 <= managed.max(1) {
        for file in &leftovers {
            let _ = trash::delete(file);
        }
    } else {
        progress(t!("Se dejaron {} archivos que el paquete no trae (eran demasiados para quitarlos sin revisar).", "{} files the package no longer has were left in place (too many to remove without checking).", leftovers.len()), 1.0);
    }
    packs::keep_local_settings(&old_settings, &folder.join(INSTANCE_FILE));
    let _ = std::fs::remove_file(&old_settings);
    let listed: Vec<(String, u32, u64)> = entries
        .iter()
        .filter(|e| !e.name.ends_with('/') && e.name.strip_prefix("instancia/").is_some_and(|rest| !preserved(rest)))
        .map(|e| (e.name.clone(), e.crc32, e.size))
        .collect();
    crate::integridad::save_package(&folder, &listed);
    std::fs::write(folder.join(STATE_DIR).join(packs::INSTALLED_MANIFEST), serde_json::to_vec_pretty(manifest).unwrap()).map_err(|e| e.to_string())?;
    progress(t!("Listo", "Done"), 1.0);
    Ok(DeltaStats { files: wanted.len(), bytes: done })
}

// ================= Actualizar el propio launcher

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// La versión más nueva de Parabellum publicada en el catálogo, si es mayor que esta.
pub fn launcher_update(catalog: &[Entry]) -> Option<&Entry> {
    catalog
        .iter()
        .filter(|e| e.is_launcher() && e.manifest.id == LAUNCHER_ID)
        .filter(|e| compare_versions(&e.manifest.version, current_version()) == Ordering::Greater)
        .max_by(|a, b| compare_versions(&a.manifest.version, &b.manifest.version))
}

/// Al actualizarse, el exe anterior queda como .old (Windows deja renombrar un exe abierto, no borrarlo).
pub fn cleanup_previous_version() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(PathBuf::from(format!("{}.old", exe.display())));
    }
}

/// Descarga la versión nueva, la verifica y la deja en lugar del exe actual. Devuelve su ruta.
pub async fn apply_launcher_update(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<PathBuf, String> {
    let (zip, is_copy) = fetch(entry, paths, progress.clone()).await?;
    verify(&zip, &entry.manifest).await?;
    let staging = paths.data.join("actualizacion");
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let fresh = staging.join("Parabellum.nuevo.exe");
    {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&zip).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let executable = crate::plataforma::EXECUTABLE.to_lowercase();
        let name = archive.file_names().find(|n| n.to_lowercase().ends_with(&executable)).map(String::from).ok_or(t!("La actualización no trae el ejecutable.", "The update does not contain the executable."))?;
        let mut source = archive.by_name(&name).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(&fresh).map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut out).map_err(|e| t!("La actualización está dañada: {e}", "The update is damaged: {e}"))?;
    }
    if is_copy {
        let _ = std::fs::remove_file(&zip);
    }
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let old = PathBuf::from(format!("{}.old", current.display()));
    let _ = std::fs::remove_file(&old);
    std::fs::rename(&current, &old).map_err(|_| {
        t!("No hay permiso para cambiar la carpeta del launcher. Cópialo a una carpeta tuya (Escritorio, USB…) y actualiza desde ahí.", "There is no permission to change the launcher folder. Copy it to a folder of yours (Desktop, USB…) and update from there.")
    })?;
    if let Err(error) = std::fs::rename(&fresh, &current).or_else(|_| std::fs::copy(&fresh, &current).map(|_| ())) {
        let _ = std::fs::rename(&old, &current); // deshacer: todo queda como estaba
        return Err(t!("No se pudo instalar la actualización: {error}", "Could not install the update: {error}"));
    }
    Ok(current)
}

/// Prepara la carpeta "_parabellum" con este mismo exe para subirla a Drive (solo la versión final).
pub fn publish_launcher(out_dir: &Path, notes: &str) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return Err(t!("Este Parabellum es una compilación de desarrollo: publica desde el Parabellum.exe final.", "This Parabellum is a development build: publish from the final Parabellum.exe."));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let folder = out_dir.join("_parabellum");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    // Solo la última versión
    for old in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
        if old.file_name().to_string_lossy().starts_with("Parabellum-") {
            let _ = std::fs::remove_file(old.path());
        }
    }
    let version = current_version();
    let zip_name = format!("Parabellum-{version}.zip");
    let zip_path = folder.join(&zip_name);
    let mut zip = packs::ZipOut::create(&zip_path)?;
    zip.add_file(&exe, crate::plataforma::EXECUTABLE)?;
    zip.finish()?;
    let manifest = Manifest {
        format: 1,
        kind: LAUNCHER_TYPE.into(),
        id: LAUNCHER_ID.into(),
        name: "Parabellum".into(),
        description: notes.trim().to_string(),
        version: version.into(),
        archive: zip_name,
        archive_size: zip_path.metadata().map(|m| m.len()).unwrap_or(0),
        archive_sha256: Some(packs::sha256_file(&zip_path).map_err(|e| e.to_string())?),
        exported_at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
        ..Default::default()
    };
    std::fs::write(folder.join(format!("Parabellum-{version}.json")), serde_json::to_vec_pretty(&manifest).unwrap()).map_err(|e| e.to_string())?;
    // Para quien lo baja por primera vez desde Drive (el catálogo solo lee los .json).
    let _ = std::fs::write(folder.join("LEEME.txt"), readme(version));
    Ok(folder)
}

/// Instrucciones de la primera vez: el exe no lleva firma de pago, así que Windows avisa al abrirlo.
fn readme(version: &str) -> String {
    format!(
        "PARABELLUM {version} — CÓMO EMPEZAR\r\n\
         \r\n\
         1. Descarga Parabellum-{version}.zip y descomprímelo en una carpeta tuya (Escritorio, Documentos o una USB).\r\n\
         \x20  No se instala nada: todo queda en esa carpeta.\r\n\
         2. Abre Parabellum.exe. Si Windows muestra «Windows protegió tu PC», pulsa «Más información» y luego\r\n\
         \x20  «Ejecutar de todas formas». Sale porque el programa no tiene una firma de pago; solo pasa la primera vez.\r\n\
         3. Desde ahí Parabellum se actualiza solo cuando se publica una versión nueva en esta carpeta.\r\n\
         \x20  Para tenerlo a mano: Ajustes › Comportamiento › Acceso directo en el Escritorio o en el menú Inicio.\r\n\
         \r\n\
         ---\r\n\
         \r\n\
         PARABELLUM {version} — GETTING STARTED\r\n\
         \r\n\
         1. Download Parabellum-{version}.zip and unzip it into a folder of yours (Desktop, Documents or a USB drive).\r\n\
         \x20  Nothing is installed: everything stays in that folder.\r\n\
         2. Open Parabellum.exe. If Windows shows «Windows protected your PC», click «More info» and then\r\n\
         \x20  «Run anyway». It appears because the program has no paid signature; it only happens the first time.\r\n\
         3. From then on Parabellum updates itself when a new version is published in this folder.\r\n\
         \x20  To keep it handy: Settings › Behavior › Desktop or Start menu shortcut.\r\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_iguales_con_cualquier_separador() {
        let folder = Path::new(r"C:\Juego\instancias\Minecraft++");
        let from_zip = folder.join("mods/Sodium.jar");
        let on_disk = folder.join("mods").join("sodium.jar");
        assert_eq!(path_key(&from_zip), path_key(&on_disk));
        assert_ne!(path_key(&folder.join("mods/a.jar")), path_key(&folder.join("mods/b.jar")));
    }

    #[test]
    fn mezcla_de_fuentes() {
        let entry = |version: &str, source: Source| Entry {
            manifest: Manifest { id: "pack".into(), version: version.into(), ..Default::default() },
            source,
            archive: String::new(),
            size: 0,
            icon: None,
            location: String::new(),
        };
        let merged = merge(vec![entry("202601010000", Source::Drive), entry("202602010000", Source::Drive), entry("202602010000", Source::Carpeta)]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].manifest.version, "202602010000");
        assert_eq!(merged[0].source, Source::Carpeta);
    }
}
