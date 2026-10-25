//! Catálogo de modpacks y versiones de Minecraft para redes donde Mojang y Modrinth están bloqueados
//! (la escuela): la carpeta pública de Google Drive y la carpeta "paquetes" junto al launcher (por
//! ejemplo, copiada de una USB). Mismo formato y organización que la versión C#:
//!   <carpeta>/<versión>/base-minecraft-<versión>.zip y .json
//!   <carpeta>/<versión>/<Modpack>/<Modpack>.zip, .json y .png
//!   <carpeta>/_parabellum/Parabellum-<versión>.zip y .json   (actualizaciones del launcher en Windows;
//!                                                             en Mac, _parabellum-mac)
//! La API de Drive (con clave) solo se usa para LISTAR; los archivos se bajan por el enlace directo
//! de Drive, que no necesita clave, deja reanudar y descargar por partes.

use base64::Engine;
use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
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
    /// El paquete ligero en Drive (packs::Manifest::lite): su id y su tamaño.
    pub lite: Option<(String, u64)>,
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
    /// El espejo de Minecraft (espejo.rs), si la carpeta de Drive tiene uno.
    pub mirror: Option<crate::espejo::Location>,
}

/// El último catálogo leído: lo usa `current` (instalar Minecraft desde Drive sin volver a leerlo).
static LAST: std::sync::Mutex<Option<std::sync::Arc<Catalog>>> = std::sync::Mutex::new(None);

/// El catálogo ya leído (por la ventana o antes) o, si no, se lee ahora.
pub async fn current(paths: &Paths) -> std::sync::Arc<Catalog> {
    if let Some(cached) = LAST.lock().unwrap().clone() {
        return cached;
    }
    std::sync::Arc::new(load(paths, &Config::load(paths)).await)
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
            Ok((entries, warnings, mirror)) => {
                catalog.mirror = mirror;
                catalog.entries.extend(entries);
                catalog.warnings.extend(warnings.into_iter().map(|w| format!("Google Drive: {w}")));
            }
            Err(error) => catalog.warnings.push(format!("Google Drive: {error}")),
        }
    }
    catalog.entries = merge(std::mem::take(&mut catalog.entries));
    *LAST.lock().unwrap() = Some(std::sync::Arc::new(catalog.clone()));
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
        // El ligero, si está al lado (no se usa para instalar desde la carpeta: ahí el completo ya está).
        let lite = manifest.lite().map(|n| dir.join(n)).and_then(|p| Some((p.display().to_string(), p.metadata().ok()?.len())));
        entries.push(Entry {
            size: zip.metadata().map(|m| m.len()).unwrap_or(0),
            archive: zip.display().to_string(),
            location: dir.strip_prefix(folder).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default(),
            source: Source::Carpeta,
            icon,
            manifest,
            lite,
        });
    }
    (entries, warnings)
}

async fn load_drive(paths: &Paths, config: &Config) -> Result<(Vec<Entry>, Vec<String>, Option<crate::espejo::Location>), String> {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut mirror = None;
    // Raíz / versión / modpack (y un nivel más por si acaso). Las carpetas de un mismo nivel se listan a
    // la vez: Drive tarda ~0.3 s en contestar cada una y, de una en una, el catálogo tardaba ~9 s.
    let mut level = vec![(config.drive_folder_id.clone(), String::new())];
    for depth in 0..=3 {
        if level.is_empty() {
            break;
        }
        let listed = futures::future::join_all(level.iter().map(|(id, _)| list_children(id, &config.drive_api_key))).await;
        let mut next = Vec::new();
        let mut folders = Vec::new();
        for ((_, path), children) in level.iter().zip(listed) {
            let children = children?;
            for child in children.iter().filter(|c| c["mimeType"] == FOLDER_MIME) {
                let (name, id) = (child["name"].as_str().unwrap_or_default(), child["id"].as_str().unwrap_or_default());
                if depth < 3 && valid_id(id) {
                    next.push((id.to_string(), format!("{path}{name}/")));
                }
            }
            folders.push((children, path.clone()));
        }
        // Los .json de todas las carpetas del nivel, también a la vez (Drive tarda ~1 s en dar cada uno).
        for (found, warned, location) in futures::future::join_all(folders.iter().map(|(children, path)| folder_entries(children, path, paths))).await {
            entries.extend(found);
            warnings.extend(warned);
            mirror = mirror.or(location);
        }
        level = next;
    }
    Ok((entries, warnings, mirror))
}

/// Los paquetes de una carpeta de Drive (sus .json, que se bajan a la vez) y, si está ahí, el espejo.
async fn folder_entries(children: &[Value], path: &str, paths: &Paths) -> (Vec<Entry>, Vec<String>, Option<crate::espejo::Location>) {
    let mut entries = Vec::new();
    let mut warnings = Vec::new();
    let mut by_name: HashMap<String, &Value> = HashMap::new();
    for child in children.iter().filter(|c| c["mimeType"] != FOLDER_MIME) {
        let name = child["name"].as_str().unwrap_or_default();
        // Si hay dos archivos con el mismo nombre, el más reciente.
        let newer = by_name.get(&name.to_lowercase()).is_none_or(|old| child["modifiedTime"].as_str() > old["modifiedTime"].as_str());
        if newer {
            by_name.insert(name.to_lowercase(), child);
        }
    }
    // El espejo de Minecraft: su índice y las capas de la misma carpeta.
    let mirror = by_name.get(crate::espejo::INDEX_FILE).filter(|f| f["id"].as_str().is_some_and(valid_id)).map(|index| crate::espejo::Location {
        index: index["id"].as_str().unwrap().to_string(),
        modified: index["modifiedTime"].as_str().unwrap_or_default().to_string(),
        files: by_name
            .values()
            .filter_map(|f| {
                let id = f["id"].as_str().filter(|id| valid_id(id))?;
                Some((f["name"].as_str()?.to_string(), (id.to_string(), f["size"].as_str()?.parse().ok()?)))
            })
            .collect(),
    });
    let jsons: Vec<(&str, &str)> = by_name
        .values()
        .filter(|f| f["name"].as_str().is_some_and(|n| n.to_lowercase().ends_with(".json")))
        .filter_map(|f| Some((f["name"].as_str()?, f["id"].as_str().filter(|id| valid_id(id))?)))
        .collect();
    let texts = futures::future::join_all(jsons.iter().map(|(_, id)| download_text(id))).await;
    for ((name, _), text) in jsons.into_iter().zip(texts) {
        let manifest = match text.and_then(|t| serde_json::from_str::<Manifest>(&t).map_err(|e| e.to_string())) {
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
        let lite = manifest
            .lite()
            .and_then(|n| by_name.get(&n.to_lowercase()))
            .and_then(|f| Some((f["id"].as_str().filter(|id| valid_id(id))?.to_string(), f["size"].as_str()?.parse().ok()?)));
        entries.push(Entry {
            archive: zip["id"].as_str().unwrap().to_string(),
            size: zip["size"].as_str().and_then(|s| s.parse().ok()).unwrap_or(manifest.archive_size),
            location: path.trim_end_matches('/').to_string(),
            source: Source::Drive,
            icon,
            manifest,
            lite,
        });
    }
    (entries, warnings, mirror)
}

/// Solo la versión nueva de Parabellum, para la pantalla de carga: la carpeta del launcher de este
/// sistema (_parabellum o _parabellum-mac) y la carpeta paquetes, sin leer el resto del catálogo.
pub async fn quick_launcher_update(paths: &Paths, config: &Config) -> Option<Entry> {
    let local = paths.root.join(LOCAL_FOLDER);
    let mut entries: Vec<Entry> = tauri::async_runtime::spawn_blocking(move || load_local(&local).0).await.unwrap_or_default();
    if config.has_drive() {
        let root = list_children(&config.drive_folder_id, &config.drive_api_key).await.ok()?;
        // La carpeta estable y, en el canal de pruebas, también la de pruebas.
        let folders: Vec<String> = [Some(crate::canal::folder(false)), crate::canal::on_test().then(|| crate::canal::folder(true))].into_iter().flatten().collect();
        for folder in folders {
            let found = root.iter().find(|c| c["mimeType"] == FOLDER_MIME && c["name"].as_str().is_some_and(|n| n.eq_ignore_ascii_case(&folder)));
            if let Some(id) = found.and_then(|f| f["id"].as_str()).filter(|id| valid_id(id)) {
                let children = list_children(id, &config.drive_api_key).await.ok()?;
                entries.extend(folder_entries(&children, &folder, paths).await.0);
            }
        }
    }
    launcher_update(&merge(entries)).cloned()
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

/// Los .json del catálogo (y el índice del espejo). De vez en cuando Drive contesta uno con una página o
/// corta la conexión al pedir muchos a la vez (2 de 5 lecturas del catálogo perdían alguno, a veces el de
/// la versión nueva de Parabellum): se reintenta dos veces. Si no existe o no está compartido, no.
pub(crate) async fn download_text(file_id: &str) -> Result<String, String> {
    let mut attempt = 0;
    loop {
        let result = async {
            let response = download::client().get(download_url(file_id)).send().await.map_err(|_| (unavailable(), true))?;
            let status = response.status();
            let lasting = matches!(status, reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::FORBIDDEN | reqwest::StatusCode::UNAUTHORIZED);
            let response = check_download(response).await.map_err(|e| (e, !lasting))?;
            response.text().await.map_err(|e| (e.to_string(), true))
        }
        .await;
        match result {
            Ok(text) => return Ok(text),
            Err((error, retry)) if !retry || attempt == 2 => return Err(error),
            Err(_) => {
                attempt += 1;
                tokio::time::sleep(std::time::Duration::from_millis(700 * attempt)).await;
            }
        }
    }
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
        // Una versión del launcher puede estar en el canal estable y en el de pruebas: son dos.
        let key = if entry.is_launcher() {
            format!("{}@{}@{}", entry.manifest.id.to_lowercase(), entry.manifest.version, crate::canal::is_test_location(&entry.location))
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
    fetch_drive(&entry.archive, entry.size, &entry.manifest.name, paths, progress).await.map(|zip| (zip, true))
}

/// Un zip de Drive en datos/descargas (se reanuda donde quedó y se reintenta si se corta o Drive falla).
async fn fetch_drive(file_id: &str, size: u64, name: &str, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync) -> Result<PathBuf, String> {
    let dir = paths.data.join("descargas");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let finished = dir.join(format!("{file_id}.zip"));
    if finished.metadata().is_ok_and(|m| m.len() == size) {
        return Ok(finished);
    }
    let partial = dir.join(format!("{file_id}.zip.parte"));
    let mut attempt = 1;
    loop {
        match download_part(file_id, size, name, &partial, &progress).await {
            Ok(()) => break,
            Err((error, retry)) if retry && attempt < 4 && !crate::descargas::is_cancelled() => {
                progress(t!("Google Drive no respondió bien, reintentando ({attempt}/3)…", "Google Drive did not answer properly, retrying ({attempt}/3)…"), 0.0);
                tokio::time::sleep(std::time::Duration::from_secs(3 * attempt)).await;
                attempt += 1;
                let _ = error;
            }
            Err((error, _)) => return Err(error),
        }
    }
    std::fs::rename(&partial, &finished).map_err(|e| e.to_string())?;
    Ok(finished)
}

async fn download_part(file_id: &str, size: u64, name: &str, partial: &Path, progress: &(impl Fn(String, f64) + Send + Sync)) -> Result<(), (String, bool)> {
    let mut existing = partial.metadata().map(|m| m.len()).unwrap_or(0);
    if size > 0 && existing >= size {
        return Ok(());
    }
    let mut request = download::client().get(download_url(file_id));
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
    }
    // Drive revisa por virus los archivos de menos de 100 MB antes de soltarlos: con un modpack chico tarda
    // ~30 s en empezar. Mientras, se dice qué pasa en lugar de dejar la barra quieta.
    let started = std::time::Instant::now();
    let sending = request.send();
    tokio::pin!(sending);
    let response = loop {
        tokio::select! {
            response = &mut sending => break response,
            _ = tokio::time::sleep(std::time::Duration::from_secs(3)) => {
                if crate::descargas::is_cancelled() {
                    return Err((crate::descargas::cancelled_error(), false));
                }
                progress(
                    t!(
                        "Google Drive está revisando {name} antes de entregarlo (puede tardar hasta 30 s)… {} s",
                        "Google Drive is checking {name} before handing it over (it can take up to 30 s)… {} s",
                        started.elapsed().as_secs()
                    ),
                    existing as f64 / size.max(1) as f64,
                );
            }
        }
    };
    let response = response.map_err(|e| (t!("No se pudo conectar con Google Drive: {e}", "Could not connect to Google Drive: {e}"), true))?;
    // Un 500 de Drive (le pasa a veces justo después de revisarlo) se reintenta; un 404 o un 403, no.
    let failed = response.status().is_server_error();
    let mut response = check_download(response).await.map_err(|e| (e, failed))?;
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
        crate::descargas::check().map_err(|e| (e, false))?;
        file.write_all(&chunk).await.map_err(|e| (e.to_string(), false))?;
        done += chunk.len() as u64;
        crate::descargas::received(chunk.len()).await;
        if last.elapsed().as_millis() >= 250 {
            last = std::time::Instant::now();
            let speed = (done - existing) as f64 / started.elapsed().as_secs_f64().max(0.5) / 1048576.0;
            progress(
                t!(
                    "Descargando {name}: {:.0} de {:.0} MB · {speed:.1} MB/s", "Downloading {name}: {:.0} of {:.0} MB · {speed:.1} MB/s",
                    done as f64 / 1048576.0,
                    size as f64 / 1048576.0
                ),
                done as f64 / size.max(1) as f64,
            );
        }
    }
    file.flush().await.map_err(|e| (e.to_string(), false))?;
    if size > 0 && done != size {
        return Err((t!("La descarga quedó incompleta.", "The download is incomplete."), true));
    }
    Ok(())
}

async fn verify(zip: &Path, manifest: &Manifest) -> Result<(), String> {
    verify_sha256(zip, manifest.archive_sha256.clone()).await
}

async fn verify_sha256(zip: &Path, expected: Option<String>) -> Result<(), String> {
    let Some(expected) = expected else { return Ok(()) };
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

// ================= El paquete ligero: el zip chico de Drive y los mods del CDN de Modrinth

/// Lo que se baja de un paquete ligero va a una misma caché: la instalación y «pedir antes» (`prefetch`)
/// no bajan a la vez. Si la instalación llega mientras se pide antes, lo espera (ya va avanzado).
static LITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
/// Cuánto lleva lo que se pide antes, para que la instalación que lo espera lo muestre.
static PREFETCH_PROGRESS: std::sync::Mutex<Option<(String, f64)>> = std::sync::Mutex::new(None);
/// Lo que se pide antes tiene un tope (pedido en el plan de noviembre): no llenar el disco ni la red sin
/// que nadie lo haya pedido.
pub const PREFETCH_LIMIT: u64 = 200 * 1024 * 1024;

fn modrinth_cache(paths: &Paths) -> PathBuf {
    paths.data.join("descargas").join("modrinth")
}

/// ¿Se instala con el paquete ligero? Solo de Drive, con todos sus archivos de Modrinth (y del maven de
/// Fabric) bien formados.
pub fn uses_lite(entry: &Entry) -> bool {
    let manifest = &entry.manifest;
    entry.source == Source::Drive
        && entry.lite.is_some()
        && !manifest.remote.is_empty()
        && manifest.remote.iter().all(packs::RemoteFile::valid)
        && manifest.remote_base.iter().all(packs::RemoteFile::valid_library)
}

/// De dónde sale cada archivo que el paquete ligero no trae (ruta en minúsculas -> archivo; las bibliotecas
/// del loader, como "base/…") y qué hay que bajar.
struct LitePlan {
    sources: HashMap<String, PathBuf>,
    downloads: Vec<download::FileDownload>,
    bytes: u64,
    /// Si entre lo que se baja hay bibliotecas del maven de Fabric (para el texto del progreso).
    libraries: bool,
}

/// Lo que ya está en este PC no se baja: el mismo archivo en la instancia que se actualiza o en base (por
/// su SHA-1), o en la caché (allí solo llega comprobado).
fn lite_plan(manifest: &Manifest, paths: &Paths) -> LitePlan {
    let folder = manifest.folder.clone().filter(|f| packs::valid_folder_name(f)).map(|f| paths.instances.join(f));
    let cache = modrinth_cache(paths);
    let mut plan = LitePlan { sources: HashMap::new(), downloads: Vec::new(), bytes: 0, libraries: false };
    let mods = manifest.remote.iter().map(|r| (r, r.path.to_lowercase(), folder.as_ref().zip(crate::mrpack::safe_relative(&r.path)).map(|(f, p)| f.join(p)), false));
    let libraries = manifest.remote_base.iter().map(|r| (r, format!("base/{}", r.path.to_lowercase()), crate::mrpack::safe_relative(&r.path).map(|p| paths.base.join(p)), true));
    for (remote, key, local, library) in mods.chain(libraries) {
        let same = |file: &PathBuf| file.metadata().is_ok_and(|m| m.len() == remote.size) && crate::integridad::sha1_file(file).is_some_and(|h| h.eq_ignore_ascii_case(&remote.sha1));
        let cached = cache.join(&remote.sha1);
        let source = match local.filter(same) {
            Some(local) => local,
            None => {
                if !cached.metadata().is_ok_and(|m| m.len() == remote.size) {
                    plan.bytes += remote.size;
                    plan.libraries |= library;
                    plan.downloads.push(download::FileDownload { url: remote.url.clone(), path: cached.clone(), sha1: Some(remote.sha1.clone()), size: Some(remote.size) });
                }
                cached
            }
        };
        plan.sources.insert(key, source);
    }
    plan
}

/// Baja lo que le falta a este PC de un paquete ligero: el zip chico de Drive y, a la vez, los mods del
/// CDN de Modrinth y las bibliotecas del maven de Fabric. Devuelve el zip y de dónde sale cada archivo.
async fn fetch_lite(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<(PathBuf, HashMap<String, PathBuf>), String> {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
    let (lite_id, lite_size) = entry.lite.clone().ok_or_else(|| remote_zip::PREFER_FULL.to_string())?;
    let (manifest, paths_copy) = (entry.manifest.clone(), paths.clone());
    let plan = tauri::async_runtime::spawn_blocking(move || lite_plan(&manifest, &paths_copy)).await.map_err(|e| e.to_string())?;
    let total = (lite_size + plan.bytes).max(1);
    // Los dos avanzan a la vez: la barra es la suma y el texto, el de los mods mientras bajan.
    let zip_bytes = std::sync::Arc::new(AtomicU64::new(0));
    let mods_bytes = std::sync::Arc::new(AtomicU64::new(0));
    let mods_busy = std::sync::Arc::new(AtomicBool::new(!plan.downloads.is_empty()));
    let zip_report = {
        let (progress, zip_bytes, mods_bytes, mods_busy) = (progress.clone(), zip_bytes.clone(), mods_bytes.clone(), mods_busy.clone());
        move |stage: String, fraction: f64| {
            zip_bytes.store((fraction.clamp(0.0, 1.0) * lite_size as f64) as u64, Relaxed);
            if !mods_busy.load(Relaxed) {
                progress(stage, (zip_bytes.load(Relaxed) + mods_bytes.load(Relaxed)) as f64 / total as f64);
            }
        }
    };
    let mods_report = {
        let (progress, zip_bytes, mods_bytes, libraries) = (progress.clone(), zip_bytes.clone(), mods_bytes.clone(), plan.libraries);
        move |count: usize, files: usize, bytes: u64, all: u64| {
            mods_bytes.store(bytes, Relaxed);
            let (done_mb, all_mb) = (bytes as f64 / 1048576.0, all as f64 / 1048576.0);
            let stage = if libraries {
                t!(
                    "Bajando los mods y el loader: {count} de {files} ({done_mb:.0} de {all_mb:.0} MB)…",
                    "Downloading the mods and the loader: {count} of {files} ({done_mb:.0} of {all_mb:.0} MB)…"
                )
            } else {
                t!(
                    "Bajando los mods de Modrinth: {count} de {files} ({done_mb:.0} de {all_mb:.0} MB)…",
                    "Downloading the mods from Modrinth: {count} of {files} ({done_mb:.0} of {all_mb:.0} MB)…"
                )
            };
            progress(stage, (zip_bytes.load(Relaxed) + bytes) as f64 / total as f64)
        }
    };
    let mods = async {
        let result = download::download_all(plan.downloads, mods_report).await;
        mods_busy.store(false, Relaxed);
        result
    };
    // De una carpeta (solo para probar, con --instalar-ligero), el zip ya está.
    let zip = async {
        match entry.source {
            Source::Carpeta => Ok(PathBuf::from(&lite_id)),
            Source::Drive => fetch_drive(&lite_id, lite_size, &entry.manifest.name, paths, zip_report).await,
        }
    };
    let (zip, mods) = tokio::join!(zip, mods);
    mods?;
    Ok((zip?, plan.sources))
}

/// Espera a que termine lo que se está pidiendo antes, mostrando cuánto lleva.
async fn lite_lock(progress: &(impl Fn(String, f64) + Send + Sync)) -> Result<tokio::sync::MutexGuard<'static, ()>, String> {
    loop {
        if let Ok(guard) = LITE_LOCK.try_lock() {
            return Ok(guard);
        }
        crate::descargas::check()?;
        let shown = PREFETCH_PROGRESS.lock().unwrap().clone();
        if let Some((stage, fraction)) = shown {
            progress(stage, fraction);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

/// Instala o actualiza con el paquete ligero. Lo del jugador se conserva como con el completo
/// (packs::install), y los mods que no cambiaron se copian de la instancia en lugar de bajarse.
pub(crate) async fn install_lite(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<Installed, String> {
    let _turn = lite_lock(&progress).await?;
    let started = std::time::Instant::now();
    let (zip, sources) = fetch_lite(entry, paths, progress.clone()).await?;
    progress(t!("Comprobando {}…", "Checking {}…", entry.manifest.name), 1.0);
    let copy = entry.source == Source::Drive;
    if let Err(error) = verify_sha256(&zip, entry.manifest.lite_sha256.clone()).await {
        if copy {
            let _ = std::fs::remove_file(&zip);
        }
        return Err(error);
    }
    let (file, paths_copy, sources_copy) = (zip.clone(), paths.clone(), sources.clone());
    let result = tauri::async_runtime::spawn_blocking(move || packs::install_with(&file, &paths_copy, &sources_copy, progress)).await.map_err(|e| e.to_string())??;
    // Ya está todo en la instancia: lo bajado sobra.
    if copy {
        let _ = std::fs::remove_file(&zip);
    }
    let cache = modrinth_cache(paths);
    let downloaded = sources.values().filter(|s| s.starts_with(&cache)).count();
    for source in sources.values().filter(|s| s.starts_with(&cache)) {
        let _ = std::fs::remove_file(source);
    }
    log(
        paths,
        &format!("{} {}: paquete ligero en {:.1} s ({downloaded} de Modrinth, {} ya estaban)", entry.manifest.name, entry.manifest.version, started.elapsed().as_secs_f64(), sources.len() - downloaded),
    );
    Ok(result)
}

/// datos/logs/paquetes.log: cómo se instaló cada paquete ligero (para medir y para el doctor).
fn log(paths: &Paths, line: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(&paths.logs);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(paths.logs.join("paquetes.log")) {
        let _ = writeln!(file, "{} {line}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    }
}

/// «Pedir antes»: al abrir la comunidad, su paquete oficial empieza a bajar si falta o hay versión
/// nueva, y al pulsar «Jugar» ya va avanzado o terminó. Uno a la vez y hasta PREFETCH_LIMIT. Con el
/// paquete completo (de una versión anterior a la 0.35), solo si no está instalado: la actualización
/// de esos va por partes y no usaría el zip. Devuelve si quedó bajado (o ya lo estaba).
pub async fn prefetch(entry: &Entry, paths: &Paths, instances: &[Instance], progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<bool, String> {
    if entry.source != Source::Drive || entry.manifest.is_base() || entry.is_launcher() {
        return Ok(false);
    }
    let Ok(_turn) = LITE_LOCK.try_lock() else { return Ok(false) };
    let shown = move |stage: String, fraction: f64| {
        *PREFETCH_PROGRESS.lock().unwrap() = Some((stage.clone(), fraction));
        progress(stage, fraction);
    };
    let result = if uses_lite(entry) {
        let (manifest, paths_copy) = (entry.manifest.clone(), paths.clone());
        let plan = tauri::async_runtime::spawn_blocking(move || lite_plan(&manifest, &paths_copy)).await.map_err(|e| e.to_string())?;
        let lite_size = entry.lite.as_ref().map(|(_, size)| *size).unwrap_or(0);
        if lite_size + plan.bytes > PREFETCH_LIMIT {
            return Ok(false);
        }
        clean_cache(paths);
        fetch_lite(entry, paths, shown).await.map(|_| true)
    } else if entry.size <= PREFETCH_LIMIT && installed_instance(&entry.manifest, instances).is_none() {
        fetch_drive(&entry.archive, entry.size, &entry.manifest.name, paths, shown).await.map(|_| true)
    } else {
        Ok(false)
    };
    *PREFETCH_PROGRESS.lock().unwrap() = None;
    result
}

/// Lo que quedó en la caché de Modrinth hace más de dos semanas (de un paquete que ya cambió) se borra.
fn clean_cache(paths: &Paths) {
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(14 * 24 * 3600);
    for entry in std::fs::read_dir(modrinth_cache(paths)).into_iter().flatten().flatten() {
        if entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < old) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

// ================= Minecraft desde el catálogo (pedido de Juan: que todo baje de Google Drive)

/// El paquete base de Minecraft `version` en el catálogo, si lo hay.
pub fn base_for<'a>(entries: &'a [Entry], version: &str) -> Option<&'a Entry> {
    entries.iter().find(|e| e.manifest.is_base() && e.manifest.minecraft_version == version)
}

/// Instala o completa Minecraft desde su paquete base del catálogo. De Drive baja solo lo que falte en
/// este PC (por partes: los sonidos, las bibliotecas y el Java que ya están no se vuelven a bajar); si
/// falta casi todo, el zip completo de una vez (es más rápido). Sirve aunque Mojang esté bloqueado.
pub async fn install_base(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<(), String> {
    if entry.source == Source::Drive {
        let (url, size, manifest, paths_copy, report) = (entry.url().unwrap(), entry.size, entry.manifest.clone(), paths.clone(), progress.clone());
        let scope = crate::descargas::current();
        let result = tauri::async_runtime::spawn_blocking(move || {
            crate::descargas::enter(scope, || base_delta(&RemoteZip::new(&url, size), &manifest, &paths_copy, &report))
        })
        .await
        .map_err(|e| e.to_string())?;
        match result {
            Ok(()) => return Ok(()),
            Err(error) if error == remote_zip::NO_RANGES || error == remote_zip::PREFER_FULL => {}
            Err(error) => return Err(error),
        }
    }
    install_full(entry, paths, progress).await.map(|_| ())
}

/// Lo que le falta a este PC de una base de Drive, leído del zip remoto por partes. El JSON de la
/// versión se escribe al final: si se corta, la versión no parece instalada.
fn base_delta(remote: &RemoteZip, manifest: &Manifest, paths: &Paths, progress: &(dyn Fn(String, f64) + Send + Sync)) -> Result<(), String> {
    progress(t!("Comparando con Minecraft {} en Google Drive…", "Comparing with Minecraft {} on Google Drive…", manifest.minecraft_version), 0.0);
    let entries = remote.entries()?;
    if !entries.iter().any(|e| e.name == packs::MANIFEST_IN_ARCHIVE) {
        return Err(t!("El zip no es un paquete de Parabellum.", "The zip is not a Parabellum package."));
    }
    let wanted: Vec<(&remote_zip::RemoteEntry, PathBuf)> = entries
        .iter()
        .filter(|e| !e.name.ends_with('/'))
        .filter_map(|e| {
            let relative = crate::mrpack::safe_relative(e.name.strip_prefix("base/")?)?;
            let target = paths.base.join(relative);
            (!target.metadata().is_ok_and(|m| m.len() == e.size)).then_some((e, target))
        })
        .collect();
    let compressed: u64 = wanted.iter().map(|(e, _)| e.compressed).sum();
    if compressed * 10 > remote.size() * 7 {
        return Err(remote_zip::PREFER_FULL.into());
    }
    let total: u64 = wanted.iter().map(|(e, _)| e.size).sum::<u64>().max(1);
    let (mut done, mut count) = (0u64, 0);
    let version_json = paths.version_json(&manifest.minecraft_version);
    let mut last = None;
    let entries_only: Vec<&remote_zip::RemoteEntry> = wanted.iter().map(|(e, _)| *e).collect();
    let write = |target: &Path, bytes: &[u8]| -> Result<(), String> {
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = PathBuf::from(format!("{}.parte", target.display()));
        std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, target).map_err(|e| e.to_string())
    };
    remote.read_many(&entries_only, |index, bytes| {
        let target = &wanted[index].1;
        done += bytes.len() as u64;
        count += 1;
        if path_key(target) == path_key(&version_json) {
            last = Some(bytes);
        } else {
            write(target, &bytes)?;
        }
        progress(
            t!(
                "Descargando Minecraft {} de Google Drive… {count} de {} archivos ({:.0} de {:.0} MB)",
                "Downloading Minecraft {} from Google Drive… {count} of {} files ({:.0} of {:.0} MB)",
                manifest.minecraft_version,
                wanted.len(),
                done as f64 / 1048576.0,
                total as f64 / 1048576.0
            ),
            done as f64 / total as f64,
        );
        Ok(())
    })?;
    if let Some(bytes) = last {
        write(&version_json, &bytes)?;
    }
    crate::java::package_installed(paths, entries.iter().map(|e| e.name.as_str()));
    packs::register(paths, &manifest.id, &manifest.version);
    Ok(())
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
        // Minecraft de esa versión: del espejo (solo lo que falte en este PC) o, si no, su base del catálogo.
        if !crate::espejo::install_minecraft(&manifest.minecraft_version, paths, progress.clone()).await.unwrap_or(false) {
            let base_id = manifest.base_id.clone().unwrap_or_else(|| packs::base_id(&manifest.minecraft_version));
            let base = catalog.iter().find(|e| e.manifest.is_base() && e.manifest.id.eq_ignore_ascii_case(&base_id)).ok_or(t!(
                "Este modpack necesita Minecraft {}, que no está en Google Drive. Súbela con «Subir versiones de Minecraft».", "This modpack needs Minecraft {}, which is not on Google Drive. Upload it with «Upload Minecraft versions».",
                manifest.minecraft_version
            ))?;
            install_full(base, paths, progress.clone()).await?;
        }
    }

    // Con el paquete ligero (desde la 0.35): el zip chico de Drive y los mods del CDN de Modrinth (desde la
    // 0.35.1, también las bibliotecas del maven de Fabric). Si no responden (una red que los bloquee) o algo
    // sale mal, se sigue como antes, con el completo.
    if !manifest.is_base() && uses_lite(entry) {
        match install_lite(entry, paths, progress.clone()).await {
            Ok(result) => {
                return Ok(CatalogInstall { id: result.folder, name: result.name, kind: result.kind, updated: result.updated, downloaded_files: None, downloaded_bytes: None });
            }
            Err(error) if crate::descargas::is_cancelled() => return Err(error),
            Err(error) => {
                log(paths, &format!("{}: el paquete ligero falló ({error}); se baja el completo", manifest.name));
                progress(t!("Modrinth o Fabric no respondieron: se baja el paquete completo de Google Drive…", "Modrinth or Fabric did not answer: downloading the full package from Google Drive…"), 0.0);
            }
        }
    }

    // Actualizar (o reparar) por partes: solo modpacks de Drive que ya están instalados desde ese paquete.
    let installed = installed_instance(manifest, instances);
    let previous = installed.as_ref().and_then(|(_, m)| m.as_ref());
    let wants_delta = !manifest.is_base() && entry.source == Source::Drive && previous.is_some_and(|p| repair || manifest.version > p.version);
    if wants_delta {
        let (url, size, manifest_copy, paths_copy, report) = (entry.url().unwrap(), entry.size, manifest.clone(), paths.clone(), progress.clone());
        let scope = crate::descargas::current();
        let result = tauri::async_runtime::spawn_blocking(move || {
            crate::descargas::enter(scope, || delta_update(&RemoteZip::new(&url, size), &manifest_copy, &paths_copy, &report))
        })
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
    // Lo que se reemplaza o se quita queda en un respaldo recuperable (respaldos.rs): con la misma
    // versión es una reparación (se vuelve al paquete oficial); con otra, una actualización.
    let state = folder.join(STATE_DIR);
    let previous = std::fs::read(state.join(packs::INSTALLED_MANIFEST)).ok().and_then(|b| serde_json::from_slice::<Manifest>(&b).ok());
    let label = if previous.as_ref().is_some_and(|p| p.version == manifest.version) {
        t!("Antes de restaurar {} como en Drive", "Before restoring {} as on Drive", manifest.name)
    } else {
        t!("Antes de actualizar {}", "Before updating {}", manifest.name)
    };
    let mut backup = crate::respaldos::Builder::start(paths, &folder_name, &folder, label);
    for file in [packs::INSTALLED_MANIFEST, crate::identidad::REFERENCE_FILE, crate::integridad::RECORD] {
        backup.keep(&state.join(file));
    }
    let entries_only: Vec<&remote_zip::RemoteEntry> = wanted.iter().map(|(e, _)| *e).collect();
    remote.read_many(&entries_only, |index, bytes| {
        let target = &wanted[index].1;
        if target.starts_with(&folder) {
            backup.keep(target);
        }
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
    crate::java::package_installed(paths, entries.iter().map(|e| e.name.as_str()));

    // Lo que el paquete ya no trae (por ejemplo, un mod quitado) va a la papelera.
    let mut leftovers = Vec::new();
    let mut managed = 0;
    for dir in &managed_dirs {
        for file in packs::walk(&folder.join(dir)) {
            managed += 1;
            let relative = file.strip_prefix(&folder).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_default();
            // Lo propio de Parabellum (.lanzador: el registro, la huella, la caché) no es contenido del paquete.
            let own = relative.to_lowercase().starts_with(&format!("{}/", STATE_DIR.to_lowercase()));
            if !keep.contains(&path_key(&file)) && !preserved(&relative) && !own {
                leftovers.push(file);
            }
        }
    }
    // Seguro: si sobrara más de la mitad, algo no cuadra y no se quita nada. Lo que se quita va al
    // respaldo (o, si no se puede mover, a la papelera).
    if leftovers.len() * 2 <= managed.max(1) {
        for file in &leftovers {
            if !backup.take(file) {
                let _ = trash::delete(file);
            }
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
    crate::integridad::save_package(&folder, &listed, &manifest.remote);
    std::fs::write(folder.join(STATE_DIR).join(packs::INSTALLED_MANIFEST), serde_json::to_vec_pretty(manifest).unwrap()).map_err(|e| e.to_string())?;
    // La referencia de la huella: lo que trae el paquete (identidad.rs).
    let names: Vec<String> = entries.iter().filter(|e| !e.name.ends_with('/')).map(|e| e.name.clone()).collect();
    crate::identidad::after_package_install(paths, &folder_name, manifest, &names);
    backup.finish(paths);
    progress(t!("Listo", "Done"), 1.0);
    Ok(DeltaStats { files: wanted.len(), bytes: done })
}

// ================= Actualizar el propio launcher

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// La versión más nueva de Parabellum publicada en el catálogo para este sistema (con su id,
/// plataforma::lanzador::ID), si es mayor que esta y si este sistema se actualiza solo (SELF_UPDATE).
pub fn launcher_update(catalog: &[Entry]) -> Option<&Entry> {
    if !crate::plataforma::SELF_UPDATE {
        return None;
    }
    catalog
        .iter()
        .filter(|e| e.is_launcher() && e.manifest.id == crate::plataforma::lanzador::ID)
        // En estable, solo las de la carpeta estable; en pruebas, las dos (ver canal.rs).
        .filter(|e| crate::canal::on_test() || !crate::canal::is_test_location(&e.location))
        .filter(|e| compare_versions(&e.manifest.version, current_version()) == Ordering::Greater)
        // La que ya no arrancó en este PC no se ofrece otra vez (ver arranque.rs).
        .filter(|e| crate::arranque::failed_version().is_none_or(|failed| failed != e.manifest.version))
        .max_by(|a, b| compare_versions(&a.manifest.version, &b.manifest.version))
}

/// La versión estable más nueva de Parabellum para este sistema (para volver a estable desde una de pruebas).
pub fn newest_stable(catalog: &[Entry]) -> Option<&Entry> {
    catalog
        .iter()
        .filter(|e| e.is_launcher() && e.manifest.id == crate::plataforma::lanzador::ID && !crate::canal::is_test_location(&e.location))
        .max_by(|a, b| compare_versions(&a.manifest.version, &b.manifest.version))
}

/// Descarga la versión nueva, la verifica y la deja en lugar de esta (en Windows el .exe; en Mac la
/// app entera). Devuelve lo que hay que abrir para reiniciar (plataforma::lanzador::restart).
pub async fn apply_launcher_update(entry: &Entry, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<PathBuf, String> {
    let (zip, is_copy) = fetch(entry, paths, progress.clone()).await?;
    verify(&zip, &entry.manifest).await?;
    let staging = paths.data.join("actualizacion");
    std::fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    let installed = crate::plataforma::lanzador::install(&zip, &staging);
    if is_copy {
        let _ = std::fs::remove_file(&zip);
    }
    installed
}

/// La licencia de Parabellum y las de lo que va dentro del exe (MIT, Apache y BSD piden incluir sus
/// avisos al distribuir). Se integran al compilar porque el publicador corre desde el Parabellum
/// instalado, no desde el repo (ver publish_launcher).
const LICENSE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../LICENSE"));
const THIRD_PARTY_NOTICES: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../AVISOS-DE-TERCEROS.md"));

const LICENSE_FILE: &str = "LICENCIA.txt";
const NOTICES_FILE: &str = "AVISOS-DE-TERCEROS.txt";

/// Agrega al zip la licencia y los avisos de terceros, en su raíz (junto al exe o a Parabellum.app).
fn add_licenses(zip: &mut packs::ZipOut) -> Result<(), String> {
    zip.add_bytes(LICENSE_FILE, LICENSE.as_bytes())?;
    zip.add_bytes(NOTICES_FILE, THIRD_PARTY_NOTICES.as_bytes())
}

/// Prepara la carpeta de las versiones de este sistema («_» + su id: _parabellum en Windows,
/// _parabellum-mac en Mac) con este mismo Parabellum para subirla a Drive (solo la versión final).
pub fn publish_launcher(out_dir: &Path, notes: &str, test: bool) -> Result<PathBuf, String> {
    if cfg!(debug_assertions) {
        return Err(t!("Este Parabellum es una compilación de desarrollo: publica desde el Parabellum final (compilado con --release).", "This Parabellum is a development build: publish from the final Parabellum (built with --release)."));
    }
    let launcher = crate::plataforma::lanzador::published_files()?;
    let id = crate::plataforma::lanzador::ID;
    // _parabellum o _parabellum-pruebas (ver canal.rs).
    let folder = out_dir.join(crate::canal::folder(test));
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
    for (source, name) in &launcher {
        zip.add_file(source, name)?;
    }
    add_licenses(&mut zip)?;
    zip.finish()?;
    let manifest = Manifest {
        format: 1,
        kind: LAUNCHER_TYPE.into(),
        id: id.into(),
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
    let _ = std::fs::write(folder.join("LEEME.txt"), crate::plataforma::lanzador::readme(version));
    Ok(folder)
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
            lite: None,
        };
        let merged = merge(vec![entry("202601010000", Source::Drive), entry("202602010000", Source::Drive), entry("202602010000", Source::Carpeta)]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].manifest.version, "202602010000");
        assert_eq!(merged[0].source, Source::Carpeta);
    }

    #[test]
    fn el_zip_lleva_su_licencia_y_los_avisos() {
        let path = std::env::temp_dir().join(format!("parabellum-licencias-{}.zip", std::process::id()));
        let mut zip = packs::ZipOut::create(&path).unwrap();
        add_licenses(&mut zip).unwrap();
        zip.finish().unwrap();
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        let mut read = |name: &str| {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut archive.by_name(name).unwrap(), &mut text).unwrap();
            text
        };
        assert_eq!(read(LICENSE_FILE), LICENSE);
        assert_eq!(read(NOTICES_FILE), THIRD_PARTY_NOTICES);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn actualizacion_del_launcher_segun_el_sistema() {
        let newer = Entry {
            manifest: Manifest { kind: LAUNCHER_TYPE.into(), id: crate::plataforma::lanzador::ID.into(), version: "999.0.0".into(), ..Default::default() },
            source: Source::Drive,
            archive: String::new(),
            size: 0,
            icon: None,
            location: String::new(),
            lite: None,
        };
        // Cada sistema ve solo la suya: la de Windows ("parabellum") no se ofrece en Mac ni al revés.
        assert_eq!(launcher_update(std::slice::from_ref(&newer)).is_some(), crate::plataforma::SELF_UPDATE);
        let other = Entry { manifest: Manifest { id: if crate::plataforma::lanzador::ID == "parabellum" { "parabellum-mac".into() } else { "parabellum".into() }, ..newer.manifest.clone() }, ..newer };
        assert!(launcher_update(&[other]).is_none());
    }
}
