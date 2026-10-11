//! Base de datos de mods: la API de Modrinth (abierta, sin clave). Buscar, ver un proyecto, elegir
//! la versión compatible con la instancia, resolver dependencias, instalar y actualizar.
//!
//! Para saber qué hay instalado se calcula el SHA-1 de cada archivo y se pregunta a Modrinth a qué
//! versión pertenece (igual que su app); los resultados se guardan en .lanzador/modrinth.json.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::Path;

use crate::compat::{self, ModMeta, Problem};
use crate::content;
use crate::download::{self, FileDownload};
use crate::instances::Instance;

const API: &str = "https://api.modrinth.com/v2";
/// Modrinth pide un User-Agent que identifique al programa.
const USER_AGENT: &str = concat!("juanrojasgarcialin/parabellum/", env!("CARGO_PKG_VERSION"));
const CDN: &str = "https://cdn.modrinth.com/";
const INDEX_FILE: &str = "modrinth.json";
pub fn unavailable() -> String {
    t!(
        "No se puede conectar con Modrinth (en algunas redes, como la de la escuela, está bloqueado).",
        "Cannot connect to Modrinth (on some networks, like the school's, it is blocked)."
    )
}

// ================= HTTP

async fn send(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request.header(reqwest::header::USER_AGENT, USER_AGENT).send().await.map_err(|_| unavailable())?;
    match response.status() {
        status if status.is_success() => response.json().await.map_err(|e| t!("Modrinth respondió algo inesperado: {e}", "Modrinth answered something unexpected: {e}")),
        reqwest::StatusCode::NOT_FOUND => Err(t!("Modrinth no tiene ese proyecto o versión.", "Modrinth does not have that project or version.")),
        reqwest::StatusCode::TOO_MANY_REQUESTS => Err(t!("Modrinth pide esperar un momento. Vuelve a intentarlo en un minuto.", "Modrinth asks to wait a moment. Try again in a minute.")),
        status => Err(t!("Modrinth respondió {status}. Vuelve a intentarlo más tarde.", "Modrinth answered {status}. Try again later.")),
    }
}

async fn get(path: &str, query: &[(&str, String)]) -> Result<Value, String> {
    send(download::client().get(format!("{API}{path}")).query(query)).await
}

async fn post(path: &str, body: &Value) -> Result<Value, String> {
    send(download::client().post(format!("{API}{path}")).json(body)).await
}

/// Los ids y slugs de Modrinth solo llevan letras, números, guiones y puntos.
fn checked_id(id: &str) -> Result<&str, String> {
    if !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) && id != "." && id != ".." {
        Ok(id)
    } else {
        Err(t!("Identificador de Modrinth no válido.", "Invalid Modrinth identifier."))
    }
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn texts(value: &Value) -> Vec<String> {
    value.as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect()
}

// ================= Tipos de contenido

/// Lo que se puede instalar en una instancia y la categoría (carpeta) donde va.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentType {
    Mod,
    ResourcePack,
    Shader,
}

impl ContentType {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "mod" | "mods" => Some(Self::Mod),
            "resourcepack" | "resourcepacks" => Some(Self::ResourcePack),
            "shader" | "shaderpacks" => Some(Self::Shader),
            _ => None,
        }
    }

    /// Categoría de content.rs ("mods", "resourcepacks", "shaderpacks").
    pub fn kind(self) -> &'static str {
        match self {
            Self::Mod => "mods",
            Self::ResourcePack => "resourcepacks",
            Self::Shader => "shaderpacks",
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Mod => ".jar",
            _ => ".zip",
        }
    }

    const ALL: [ContentType; 3] = [Self::Mod, Self::ResourcePack, Self::Shader];
}

/// Loaders de Modrinth que sirven en la instancia; el primero es el suyo (se prefiere). Quilt también
/// carga mods de Fabric, y en Minecraft 1.20.1 NeoForge todavía era Forge por dentro: ahí los mods de
/// uno sirven en el otro. Los shaders (Iris, OptiFine…) no se filtran por loader.
pub fn instance_loaders(instance: &Instance, kind: ContentType) -> Result<Vec<&'static str>, String> {
    let shared = instance.minecraft_version == "1.20.1";
    match kind {
        ContentType::Mod => match instance.loader.as_str() {
            "Fabric" => Ok(vec!["fabric"]),
            "Quilt" => Ok(vec!["quilt", "fabric"]),
            "Forge" if shared => Ok(vec!["forge", "neoforge"]),
            "Forge" => Ok(vec!["forge"]),
            "NeoForge" if shared => Ok(vec!["neoforge", "forge"]),
            "NeoForge" => Ok(vec!["neoforge"]),
            _ => Err(t!("Las instancias vanilla no cargan mods. Crea una con un loader (Fabric, Quilt, Forge o NeoForge).", "Vanilla instances do not load mods. Create one with a loader (Fabric, Quilt, Forge or NeoForge).")),
        },
        ContentType::ResourcePack => Ok(vec!["minecraft"]),
        ContentType::Shader => Ok(vec![]),
    }
}

// ================= Búsqueda

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    #[serde(default)]
    pub query: String,
    /// "mod", "resourcepack" o "shader"
    pub kind: String,
    #[serde(default)]
    pub loaders: Vec<String>,
    pub game_version: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    /// relevance, downloads, follows, newest o updated
    #[serde(default)]
    pub sort: String,
    #[serde(default)]
    pub offset: u32,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: Option<String>,
    pub downloads: u64,
    pub follows: u64,
    pub categories: Vec<String>,
    pub updated: String,
    pub client_side: String,
    pub server_side: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub hits: Vec<Hit>,
    pub total: u64,
}

pub const PAGE_SIZE: u32 = 20;

pub async fn search(query: &SearchQuery) -> Result<SearchResults, String> {
    let project_type = search_type(&query.kind).ok_or(t!("Tipo de contenido desconocido.", "Unknown content type."))?;
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{project_type}")]];
    let loaders: Vec<String> = query.loaders.iter().filter(|l| !l.trim().is_empty()).map(|l| format!("categories:{l}")).collect();
    if !loaders.is_empty() {
        // Dentro de un mismo grupo es "o": Quilt muestra mods de Quilt o de Fabric.
        facets.push(loaders);
    }
    if let Some(version) = query.game_version.as_ref().filter(|v| !v.trim().is_empty()) {
        facets.push(vec![format!("versions:{version}")]);
    }
    for category in query.categories.iter().filter(|c| !c.trim().is_empty()) {
        facets.push(vec![format!("categories:{category}")]);
    }
    let index = match query.sort.as_str() {
        "downloads" | "follows" | "newest" | "updated" => query.sort.as_str(),
        _ => "relevance",
    };
    let result = get(
        "/search",
        &[
            ("query", query.query.trim().to_string()),
            ("facets", serde_json::to_string(&facets).unwrap()),
            ("index", index.to_string()),
            ("offset", query.offset.to_string()),
            ("limit", PAGE_SIZE.to_string()),
        ],
    )
    .await?;
    let hits = result["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|hit| Hit {
            id: text(hit, "project_id"),
            slug: text(hit, "slug"),
            title: text(hit, "title"),
            description: text(hit, "description"),
            author: text(hit, "author"),
            icon_url: hit["icon_url"].as_str().filter(|u| !u.is_empty()).map(String::from),
            downloads: hit["downloads"].as_u64().unwrap_or(0),
            follows: hit["follows"].as_u64().unwrap_or(0),
            categories: texts(&hit["display_categories"]),
            updated: text(hit, "date_modified"),
            client_side: text(hit, "client_side"),
            server_side: text(hit, "server_side"),
        })
        .collect();
    Ok(SearchResults { hits, total: result["total_hits"].as_u64().unwrap_or(0) })
}

#[derive(Serialize, Debug)]
pub struct Category {
    pub name: String,
    /// Icono SVG como data URL (se muestra con <img>, así no puede ejecutar nada).
    pub icon: String,
    pub header: String,
}

pub async fn categories(kind: &str) -> Result<Vec<Category>, String> {
    // Los data packs usan las categorías de los mods.
    let kind = search_type(kind).map(|k| if k == "datapack" { "mod" } else { k }).ok_or(t!("Tipo de contenido desconocido.", "Unknown content type."))?;
    let list = get("/tag/category", &[]).await?;
    Ok(list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["project_type"] == kind)
        .map(|c| {
            // Como imagen suelta, un SVG sin xmlns no se dibuja.
            let svg = text(c, "icon");
            let svg = if svg.contains("xmlns=") { svg } else { svg.replacen("<svg", r#"<svg xmlns="http://www.w3.org/2000/svg""#, 1) };
            (c, svg)
        })
        .map(|(c, svg)| Category {
            name: text(c, "name"),
            icon: format!("data:image/svg+xml;base64,{}", base64::Engine::encode(&base64::engine::general_purpose::STANDARD, svg)),
            header: text(c, "header"),
        })
        .collect())
}

// ================= Proyecto y versiones

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    /// Descripción larga en Markdown (la interfaz la limpia antes de mostrarla).
    pub body: String,
    pub icon_url: Option<String>,
    pub project_type: String,
    pub author: String,
    pub downloads: u64,
    pub followers: u64,
    pub categories: Vec<String>,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub client_side: String,
    pub server_side: String,
    pub license: String,
    pub updated: String,
    pub published: String,
    pub links: Vec<(String, String)>,
    pub gallery: Vec<GalleryImage>,
}

#[derive(Serialize, Debug)]
pub struct GalleryImage {
    pub url: String,
    pub title: String,
    pub description: String,
}

pub async fn project(id: &str) -> Result<Project, String> {
    let id = checked_id(id)?;
    let (project_path, members_path) = (format!("/project/{id}"), format!("/project/{id}/members"));
    let (project, members) = futures::join!(get(&project_path, &[]), get(&members_path, &[]));
    let p = project?;
    // Autor: la organización si la tiene; si no, el equipo.
    let author = match p["organization"].as_str() {
        Some(organization) => get(&format!("/organization/{}", checked_id(organization)?), &[]).await.ok().map(|o| text(&o, "name")),
        None => None,
    }
    .filter(|n| !n.is_empty())
    .unwrap_or_else(|| {
        members.ok().and_then(|m| m.as_array().cloned()).unwrap_or_default().iter().take(3).map(|m| text(&m["user"], "username")).collect::<Vec<_>>().join(", ")
    });
    let mut links = Vec::new();
    for (label, key) in [(t!("Código fuente", "Source code"), "source_url"), (t!("Reportar errores", "Report issues"), "issues_url"), ("Wiki".into(), "wiki_url"), ("Discord".into(), "discord_url")] {
        if let Some(url) = p[key].as_str().filter(|u| u.starts_with("https://")) {
            links.push((label.to_string(), url.to_string()));
        }
    }
    let mut categories = texts(&p["categories"]);
    categories.extend(texts(&p["additional_categories"]));
    let mut gallery: Vec<(i64, GalleryImage)> = p["gallery"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|g| {
            (
                g["ordering"].as_i64().unwrap_or(0),
                GalleryImage { url: text(g, "url"), title: text(g, "title"), description: text(g, "description") },
            )
        })
        .collect();
    gallery.sort_by_key(|(order, _)| *order);
    Ok(Project {
        id: text(&p, "id"),
        slug: text(&p, "slug"),
        title: text(&p, "title"),
        description: text(&p, "description"),
        body: text(&p, "body"),
        icon_url: p["icon_url"].as_str().filter(|u| !u.is_empty()).map(String::from),
        project_type: text(&p, "project_type"),
        author,
        downloads: p["downloads"].as_u64().unwrap_or(0),
        followers: p["followers"].as_u64().unwrap_or(0),
        categories,
        loaders: texts(&p["loaders"]),
        game_versions: texts(&p["game_versions"]),
        client_side: text(&p, "client_side"),
        server_side: text(&p, "server_side"),
        license: p["license"]["name"].as_str().filter(|n| !n.is_empty()).or(p["license"]["id"].as_str()).unwrap_or_default().to_string(),
        updated: text(&p, "updated"),
        published: text(&p, "published"),
        links,
        gallery: gallery.into_iter().map(|(_, g)| g).collect(),
    })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    /// required, optional, incompatible o embedded
    pub kind: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    /// release, beta o alpha
    pub version_type: String,
    pub published: String,
    pub downloads: u64,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub filename: String,
    #[serde(skip)]
    pub url: String,
    #[serde(skip)]
    pub sha1: String,
    pub size: u64,
    pub dependencies: Vec<Dependency>,
}

fn parse_version(v: &Value) -> Option<Version> {
    let files = v["files"].as_array()?;
    let file = files.iter().find(|f| f["primary"] == true).or(files.first())?;
    Some(Version {
        id: v["id"].as_str()?.into(),
        project_id: v["project_id"].as_str()?.into(),
        name: text(v, "name"),
        version_number: text(v, "version_number"),
        version_type: text(v, "version_type"),
        published: text(v, "date_published"),
        downloads: v["downloads"].as_u64().unwrap_or(0),
        loaders: texts(&v["loaders"]),
        game_versions: texts(&v["game_versions"]),
        filename: file["filename"].as_str()?.into(),
        url: file["url"].as_str()?.into(),
        sha1: file["hashes"]["sha1"].as_str()?.into(),
        size: file["size"].as_u64().unwrap_or(0),
        dependencies: v["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|d| Dependency {
                project_id: d["project_id"].as_str().map(String::from),
                version_id: d["version_id"].as_str().map(String::from),
                kind: text(d, "dependency_type"),
            })
            .collect(),
    })
}

/// Versiones de un proyecto (de la más nueva a la más vieja), filtradas si se indica.
pub async fn project_versions(id: &str, loaders: &[&str], game_versions: &[&str]) -> Result<Vec<Version>, String> {
    let mut query = Vec::new();
    if !loaders.is_empty() {
        query.push(("loaders", serde_json::to_string(loaders).unwrap()));
    }
    if !game_versions.is_empty() {
        query.push(("game_versions", serde_json::to_string(game_versions).unwrap()));
    }
    let list = get(&format!("/project/{}/version", checked_id(id)?), &query).await?;
    Ok(list.as_array().into_iter().flatten().filter_map(parse_version).collect())
}

async fn version(id: &str) -> Result<Version, String> {
    parse_version(&get(&format!("/version/{}", checked_id(id)?), &[]).await?).ok_or_else(|| t!("La versión no tiene archivos.", "The version has no files."))
}

/// La versión a instalar: la estable más nueva; si no hay, la beta más nueva; si no, la alpha.
pub fn best(versions: &[Version]) -> Option<&Version> {
    ["release", "beta", "alpha"].iter().find_map(|kind| versions.iter().find(|v| v.version_type == *kind)).or(versions.first())
}

/// Como `best`, pero primero entre las versiones del loader de la instancia (el primero de
/// `loaders`): en NeoForge 1.20.1 gana la versión para NeoForge aunque haya una de Forge más nueva.
pub fn preferred<'a>(versions: &'a [Version], loaders: &[&str]) -> Option<&'a Version> {
    let native: Vec<Version> = versions.iter().filter(|v| loaders.first().is_none_or(|l| v.loaders.iter().any(|x| x == l))).cloned().collect();
    match best(&native) {
        Some(chosen) => versions.iter().find(|v| v.id == chosen.id),
        None => best(versions),
    }
}

/// Las versiones de la lista ordenadas como se prueban: primero las del loader de la instancia.
fn native_first(mut versions: Vec<Version>, loaders: &[&str]) -> Vec<Version> {
    versions.sort_by_key(|v| loaders.first().is_some_and(|l| !v.loaders.iter().any(|x| x == l)));
    versions
}

/// Una versión de Modrinth que dice servir para el loader y la versión de Minecraft de la instancia.
fn version_fits(version: &Version, loaders: &[&str], minecraft: &str) -> bool {
    (loaders.is_empty() || version.loaders.iter().any(|l| loaders.contains(&l.as_str()))) && version.game_versions.iter().any(|g| g == minecraft)
}

fn loader_names(loaders: &[String]) -> String {
    let names: Vec<String> = loaders
        .iter()
        .map(|l| match l.as_str() {
            "neoforge" => "NeoForge".to_string(),
            other => {
                let mut chars = other.chars();
                chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
            }
        })
        .collect();
    names.join(", ")
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub project_type: String,
}

async fn summaries(ids: &[String]) -> HashMap<String, ProjectSummary> {
    let mut result = HashMap::new();
    for chunk in ids.chunks(100) {
        let Ok(list) = get("/projects", &[("ids", serde_json::to_string(chunk).unwrap())]).await else { continue };
        for p in list.as_array().into_iter().flatten() {
            let summary = ProjectSummary {
                id: text(p, "id"),
                title: text(p, "title"),
                icon_url: p["icon_url"].as_str().filter(|u| !u.is_empty()).map(String::from),
                project_type: text(p, "project_type"),
            };
            result.insert(summary.id.clone(), summary);
        }
    }
    result
}

// ================= Lo instalado (índice por SHA-1)

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    sha1: String,
    size: u64,
    modified: i64,
    /// Ya se preguntó a Modrinth por este archivo (aunque no sea de Modrinth).
    #[serde(default)]
    checked: bool,
    project_id: Option<String>,
    version_id: Option<String>,
    published: Option<String>,
    /// release, beta o alpha: las actualizaciones no bajan de estabilidad.
    #[serde(default)]
    version_type: Option<String>,
    /// Proyectos con los que esta versión dice ser incompatible.
    #[serde(default)]
    incompatible: Vec<String>,
    /// Lo que declara el mod en su fabric.mod.json (solo mods).
    #[serde(default)]
    mods: Option<Vec<ModMeta>>,
}

#[derive(Serialize, Deserialize, Default, Debug)]
struct Index {
    /// Clave: "mods/sodium.jar" (sin ".disabled", así desactivar no pierde la información).
    files: HashMap<String, IndexEntry>,
    /// Cómo se leyó lo que declara cada mod: 2 = también los mods.toml de Forge y NeoForge.
    #[serde(default)]
    meta_version: u32,
}

const META_VERSION: u32 = 4;

fn index_path(instance: &Instance) -> std::path::PathBuf {
    instance.state_dir().join(INDEX_FILE)
}

fn load_index(instance: &Instance) -> Index {
    let mut index: Index = std::fs::read_to_string(index_path(instance)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    // Leído con una versión anterior: se vuelve a leer lo que declara cada mod.
    if index.meta_version < META_VERSION {
        for entry in index.files.values_mut() {
            entry.mods = None;
        }
        index.meta_version = META_VERSION;
    }
    index
}

fn save_index(instance: &Instance, index: &Index) {
    let _ = std::fs::create_dir_all(instance.state_dir());
    let file = index_path(instance);
    let temp = file.with_extension("tmp");
    if std::fs::write(&temp, serde_json::to_vec(index).unwrap()).is_ok() {
        let _ = std::fs::rename(temp, file);
    }
}

/// Un archivo de mods, resource packs o shaders de la instancia.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct InstalledFile {
    pub kind: &'static str,
    /// Nombre real del archivo (puede terminar en .disabled).
    pub file: String,
    pub sha1: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    #[serde(skip)]
    published: Option<String>,
    #[serde(skip)]
    version_type: Option<String>,
    #[serde(skip)]
    incompatible: Vec<String>,
    #[serde(skip)]
    pub mods: Vec<ModMeta>,
}

fn sha1_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn modified_seconds(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Calcula el SHA-1 de los archivos nuevos o cambiados (los demás vienen del índice).
fn scan(instance: &Instance) -> (Index, Vec<(String, String)>) {
    let mut index = load_index(instance);
    let mut present = Vec::new();
    for kind in ContentType::ALL {
        let Some(folder) = content::folder(instance, kind.kind()) else { continue };
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            let base = file.strip_suffix(".disabled").unwrap_or(&file);
            if !base.to_lowercase().ends_with(kind.extension()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            let key = format!("{}/{base}", kind.kind());
            let modified = modified_seconds(&meta);
            let fresh = index.files.get(&key).is_some_and(|e| e.size == meta.len() && e.modified == modified);
            if !fresh {
                let Ok(sha1) = sha1_file(&entry.path()) else { continue };
                index.files.insert(key.clone(), IndexEntry { sha1, size: meta.len(), modified, ..Default::default() });
            }
            if kind == ContentType::Mod {
                if let Some(entry_info) = index.files.get_mut(&key).filter(|e| e.mods.is_none()) {
                    entry_info.mods = Some(std::fs::File::open(entry.path()).map(compat::read_jar).unwrap_or_default());
                }
            }
            present.push((key, file));
        }
    }
    let keys: HashSet<&String> = present.iter().map(|(k, _)| k).collect();
    index.files.retain(|key, _| keys.contains(key));
    (index, present)
}

/// Qué hay instalado y, si hay conexión, de qué proyecto de Modrinth es cada archivo.
pub async fn identify(instance: &Instance) -> Vec<InstalledFile> {
    let scanned = instance.clone();
    let Ok((mut index, present)) = tauri::async_runtime::spawn_blocking(move || scan(&scanned)).await else { return Vec::new() };
    // Sin consultar todavía (o de un índice anterior que no guardaba el tipo de versión).
    let unknown: Vec<String> =
        index.files.values().filter(|e| !e.checked || (e.project_id.is_some() && e.version_type.is_none())).map(|e| e.sha1.clone()).collect();
    for chunk in unknown.chunks(500) {
        let Ok(found) = post("/version_files", &json!({ "hashes": chunk, "algorithm": "sha1" })).await else { break };
        for entry in index.files.values_mut().filter(|e| chunk.contains(&e.sha1)) {
            entry.checked = true;
            if let Some(version) = found.get(&entry.sha1).and_then(parse_version) {
                entry.project_id = Some(version.project_id.clone());
                entry.version_id = Some(version.id.clone());
                entry.published = Some(version.published.clone());
                entry.version_type = Some(version.version_type.clone());
                entry.incompatible =
                    version.dependencies.iter().filter(|d| d.kind == "incompatible").filter_map(|d| d.project_id.clone()).collect();
            }
        }
    }
    save_index(instance, &index);
    present
        .into_iter()
        .filter_map(|(key, file)| {
            let entry = index.files.get(&key)?;
            Some(InstalledFile {
                kind: ContentType::ALL.into_iter().find(|k| key.starts_with(&format!("{}/", k.kind())))?.kind(),
                file,
                sha1: entry.sha1.clone(),
                project_id: entry.project_id.clone(),
                version_id: entry.version_id.clone(),
                published: entry.published.clone(),
                version_type: entry.version_type.clone(),
                incompatible: entry.incompatible.clone(),
                mods: entry.mods.clone().unwrap_or_default(),
            })
        })
        .collect()
}

/// Si cada proyecto funciona en un servidor dedicado: id -> "required", "optional" o "unsupported".
pub async fn server_sides(ids: &[String]) -> HashMap<String, String> {
    let mut sides = HashMap::new();
    let mut unique: Vec<&String> = ids.iter().collect();
    unique.sort();
    unique.dedup();
    for chunk in unique.chunks(200) {
        let Ok(found) = get("/projects", &[("ids", serde_json::to_string(chunk).unwrap())]).await else { break };
        for project in found.as_array().into_iter().flatten() {
            if let (Some(id), Some(side)) = (project["id"].as_str(), project["server_side"].as_str()) {
                sides.insert(id.to_string(), side.to_string());
            }
        }
    }
    sides
}

/// Proyecto de Modrinth de cada archivo según el índice (sin conexión ni calcular nada).
/// Clave: "mods/sodium.jar" sin ".disabled".
pub fn known_projects(instance: &Instance) -> HashMap<String, String> {
    load_index(instance).files.into_iter().filter_map(|(key, e)| Some((key, e.project_id?))).collect()
}

// ================= Plan de instalación (con dependencias)

/// Un archivo a descargar en la instancia.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlannedFile {
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: Option<String>,
    pub version_number: String,
    /// "mods", "resourcepacks" o "shaderpacks"
    pub kind: String,
    pub filename: String,
    pub url: String,
    pub sha1: String,
    pub size: u64,
    /// true si es una dependencia que se instala sola
    pub dependency: bool,
    /// Archivo actual del mismo proyecto que se reemplaza (actualizar o cambiar de versión).
    pub replaces: Option<String>,
    pub published: String,
    #[serde(default)]
    pub version_type: String,
    #[serde(default)]
    pub incompatible: Vec<String>,
}

impl PlannedFile {
    fn new(version: &Version, kind: ContentType, dependency: bool, replaces: Option<String>) -> Self {
        Self {
            project_id: version.project_id.clone(),
            version_id: version.id.clone(),
            title: String::new(),
            icon_url: None,
            version_number: version.version_number.clone(),
            kind: kind.kind().into(),
            filename: version.filename.clone(),
            url: version.url.clone(),
            sha1: version.sha1.clone(),
            size: version.size,
            dependency,
            replaces,
            published: version.published.clone(),
            version_type: version.version_type.clone(),
            incompatible: version.dependencies.iter().filter(|d| d.kind == "incompatible").filter_map(|d| d.project_id.clone()).collect(),
        }
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub project_id: String,
    pub title: String,
    pub version_number: String,
    /// Lo que se descarga: el proyecto y sus dependencias obligatorias.
    pub files: Vec<PlannedFile>,
    /// Dependencias opcionales que se pueden instalar aparte.
    pub optional: Vec<ProjectSummary>,
    /// Ya está instalada exactamente esa versión.
    pub installed: bool,
    /// Por qué no se puede instalar (dependencia imposible o incompatibilidad).
    pub problem: Option<String>,
    /// El problema es del propio mod pedido: vale la pena probar otras versiones suyas.
    #[serde(skip)]
    check_main: bool,
}

fn describe_target(instance: &Instance, kind: ContentType) -> String {
    match kind {
        ContentType::Mod => format!("{} {}", instance.loader, instance.minecraft_version),
        _ => format!("Minecraft {}", instance.minecraft_version),
    }
}

// ================= Versiones que funcionan juntas (Fabric y Quilt)

/// Carpeta temporal de la instancia donde se bajan los mods candidatos para leer qué necesitan.
/// Al instalar se usan desde aquí en lugar de volver a descargarlos.
fn candidates_dir(instance: &Instance) -> std::path::PathBuf {
    instance.state_dir().join("descargas")
}

fn clean_candidates(instance: &Instance) {
    let day = std::time::Duration::from_secs(86_400);
    for entry in std::fs::read_dir(candidates_dir(instance)).into_iter().flatten().flatten() {
        let old = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_none_or(|age| age > day);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

/// Lo que declara un mod del plan (lo descarga a la carpeta temporal si hace falta).
async fn planned_meta(instance: &Instance, file: &PlannedFile) -> Result<Vec<ModMeta>, String> {
    planned_jar(instance, file).await.map(|(metas, _)| metas)
}

/// Lo que declara un mod del plan y para qué loaders es (lo descarga a la carpeta temporal si hace falta).
async fn planned_jar(instance: &Instance, file: &PlannedFile) -> Result<(Vec<ModMeta>, compat::JarLoaders), String> {
    if !file.url.starts_with(CDN) || file.sha1.len() != 40 || !file.sha1.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(t!("El archivo {} no es válido.", "The file {} is not valid.", file.filename));
    }
    let path = candidates_dir(instance).join(format!("{}.jar", file.sha1));
    download::download_all(
        vec![FileDownload { url: file.url.clone(), path: path.clone(), sha1: Some(file.sha1.clone()), size: Some(file.size) }],
        |_, _, _, _| {},
    )
    .await?;
    tauri::async_runtime::spawn_blocking(move || {
        let metas = std::fs::File::open(&path).map(compat::read_jar).unwrap_or_default();
        let loaders = std::fs::File::open(&path).map(compat::read_loaders).unwrap_or_default();
        (metas, loaders)
    })
    .await
    .map_err(|e| e.to_string())
}

fn same_problem(a: &Problem, b: &Problem) -> bool {
    let key = |p: &Problem| match p {
        Problem::Missing { name, id, .. } => (0, name.clone(), id.clone()),
        Problem::Wrong { name, id, .. } => (1, name.clone(), id.clone()),
        Problem::Breaks { name, id, .. } => (2, name.clone(), id.clone()),
    };
    key(a) == key(b)
}

/// Revisa con los fabric.mod.json que todo el plan funcione junto (y con lo instalado). Si una
/// dependencia no encaja (p. ej. Iris 1.8.8 pide Sodium 0.6.x y la más nueva es 0.8), prueba otras
/// versiones de esa dependencia. Devuelve el problema que quede, explicado.
async fn repair(
    instance: &Instance,
    files: &mut Vec<PlannedFile>,
    installed: &[InstalledFile],
    loaders: &[&str],
) -> Result<Option<String>, String> {
    let minecraft = instance.minecraft_version.as_str();
    let game = compat::Game::of(instance);
    let replaced: HashSet<String> = files.iter().filter_map(|f| f.replaces.clone()).collect();
    let base: Vec<(String, Vec<ModMeta>)> = installed
        .iter()
        .filter(|f| f.kind == "mods" && !f.file.ends_with(".disabled") && !replaced.contains(&f.file))
        .map(|f| (f.file.clone(), f.mods.clone()))
        .collect();
    let mut metas: HashMap<String, Vec<ModMeta>> = HashMap::new();
    let mut tried: HashSet<String> = files.iter().map(|f| f.version_id.clone()).collect();
    let mut versions: HashMap<String, Vec<Version>> = HashMap::new();
    let all = |files: &[PlannedFile], metas: &HashMap<String, Vec<ModMeta>>| -> Vec<(String, Vec<ModMeta>)> {
        let mut all = base.clone();
        all.extend(files.iter().map(|f| (f.filename.clone(), metas.get(&f.sha1).cloned().unwrap_or_default())));
        all
    };

    for _ in 0..10 {
        for file in files.iter() {
            if !metas.contains_key(&file.sha1) {
                metas.insert(file.sha1.clone(), planned_meta(instance, file).await?);
            }
        }
        let current = all(files, &metas);
        let problems: Vec<Problem> =
            compat::check_game(&current, &game).into_iter().filter(|p| files.iter().any(|f| p.involves(&f.filename))).collect();
        let Some(first) = problems.first() else { return Ok(None) };
        let names = compat::names(&current);

        // Un mod que falta y Modrinth no lo dio como dependencia: se busca un proyecto con ese nombre.
        if let Some(Problem::Missing { id, .. }) = problems.iter().find(|p| matches!(p, Problem::Missing { .. })) {
            let mut added = false;
            for slug in [id.clone(), id.replace('_', "-")] {
                let Ok(slug) = checked_id(&slug).map(String::from) else { continue };
                let Ok(list) = project_versions(&slug, loaders, &[minecraft]).await else { continue };
                let Some(found) = preferred(&list, loaders) else { continue };
                let candidate = PlannedFile::new(found, ContentType::Mod, true, None);
                let meta = planned_meta(instance, &candidate).await?;
                // Solo sirve si de verdad es ese mod.
                if meta.iter().any(|m| &m.id == id || m.provides.contains(id)) && !files.iter().any(|f| f.project_id == candidate.project_id) {
                    metas.insert(candidate.sha1.clone(), meta);
                    files.push(candidate);
                    added = true;
                    break;
                }
            }
            if added {
                continue;
            }
        }

        // Una dependencia del plan que se puede cambiar por otra versión.
        let swap = problems.iter().find_map(|problem| {
            let sides: Vec<&str> = match problem {
                Problem::Wrong { owner, found_file, .. } | Problem::Breaks { owner, found_file, .. } => vec![found_file, owner],
                Problem::Missing { .. } => vec![],
            };
            sides.into_iter().find_map(|side| files.iter().position(|f| f.dependency && f.filename == side)).map(|i| (i, problem))
        });
        let Some((index, problem)) = swap else { return Ok(Some(first.describe(&names))) };
        let project = files[index].project_id.clone();
        if !versions.contains_key(&project) {
            versions.insert(project.clone(), project_versions(&project, loaders, &[minecraft]).await?);
        }
        // Primero las versiones cuyo nombre ya dice que cumplen el rango pedido; luego estables y más nuevas.
        let wanted: Option<&Vec<String>> = match problem {
            Problem::Wrong { predicates, found_file, .. } if *found_file == files[index].filename => Some(predicates),
            _ => None,
        };
        let hinted = |v: &Version| {
            wanted.is_some_and(|ranges| compat::embedded_versions(&v.version_number).iter().any(|e| ranges.iter().any(|r| compat::matches(e, r))))
        };
        let type_rank = |v: &Version| match v.version_type.as_str() {
            "release" => 0,
            "beta" => 1,
            _ => 2,
        };
        let mut candidates: Vec<Version> = versions[&project].iter().filter(|v| !tried.contains(&v.id)).cloned().collect();
        candidates.sort_by_key(|v| (!hinted(v), type_rank(v)));
        let mut fixed = false;
        for candidate in candidates.into_iter().take(5) {
            tried.insert(candidate.id.clone());
            let mut replacement = PlannedFile::new(&candidate, ContentType::Mod, true, files[index].replaces.clone());
            replacement.title = files[index].title.clone();
            metas.insert(replacement.sha1.clone(), planned_meta(instance, &replacement).await?);
            let mut trial = files.clone();
            trial[index] = replacement.clone();
            if !compat::check_game(&all(&trial, &metas), &game).iter().any(|p| same_problem(p, problem)) {
                files[index] = replacement;
                fixed = true;
                break;
            }
        }
        if !fixed {
            return Ok(Some(problem.describe(&names)));
        }
    }
    Ok(Some(t!("No se encontró una combinación de versiones que funcione junta.", "No combination of versions that works together was found.")))
}

pub async fn plan(instance: &Instance, project_id: &str, version_id: Option<&str>) -> Result<Plan, String> {
    clean_candidates(instance);
    let mut plan = plan_version(instance, project_id, version_id).await?;
    // Si la versión más nueva del mod no funciona con lo instalado, se prueban otras (también beta).
    if plan.problem.is_some() && version_id.is_none() && plan.check_main {
        let kind = ContentType::Mod;
        let loaders = instance_loaders(instance, kind)?;
        let list = project_versions(&plan.project_id, &loaders, &[instance.minecraft_version.as_str()]).await?;
        let chosen = plan.files.first().map(|f| f.version_id.clone()).unwrap_or_default();
        // Lo instalado suele pedir algo más nuevo: primero las versiones posteriores a la elegida
        // (aunque sean beta), luego las anteriores; la lista ya viene de la más nueva a la más vieja.
        let position = list.iter().position(|v| v.id == chosen).unwrap_or(list.len());
        let others = list[..position].iter().rev().chain(list.iter().skip(position + 1));
        for other in others.take(5) {
            let alternative = plan_version(instance, &plan.project_id, Some(&other.id)).await?;
            if alternative.problem.is_none() {
                plan = alternative;
                break;
            }
        }
    }
    Ok(plan)
}

async fn plan_version(instance: &Instance, project_id: &str, version_id: Option<&str>) -> Result<Plan, String> {
    let project = get(&format!("/project/{}", checked_id(project_id)?), &[]).await?;
    let project_id = text(&project, "id");
    let title = text(&project, "title");
    let kind = ContentType::parse(project["project_type"].as_str().unwrap_or_default()).ok_or_else(|| match project["project_type"].as_str() {
        Some("modpack") => t!("Los modpacks se instalan como una instancia nueva (llega en la siguiente fase).", "Modpacks are installed as a new instance."),
        _ => t!("Este tipo de contenido todavía no se puede instalar.", "This type of content cannot be installed yet."),
    })?;
    let loaders = instance_loaders(instance, kind)?;
    let minecraft = instance.minecraft_version.as_str();
    let target = describe_target(instance, kind);

    let installed = identify(instance).await;
    let installed_by_project: HashMap<&str, &InstalledFile> =
        installed.iter().filter_map(|f| Some((f.project_id.as_deref()?, f))).collect();

    let main = match version_id {
        Some(id) => {
            let chosen = version(id).await?;
            if kind == ContentType::Mod && !version_fits(&chosen, &loaders, minecraft) {
                return Err(t!(
                    "Esa versión de {title} es para {} {}; esta instancia es {target}.",
                    "That version of {title} is for {} {}; this instance is {target}.",
                    loader_names(&chosen.loaders),
                    chosen.game_versions.last().cloned().unwrap_or_default()
                ));
            }
            chosen
        }
        None => preferred(&project_versions(&project_id, &loaders, &[minecraft]).await?, &loaders)
            .cloned()
            .ok_or(t!("{title} no tiene ninguna versión para {target}.", "{title} has no version for {target}."))?,
    };
    let current = installed_by_project.get(project_id.as_str());
    if current.is_some_and(|f| f.version_id.as_deref() == Some(main.id.as_str())) {
        return Ok(Plan {
            project_id,
            title,
            version_number: main.version_number,
            files: Vec::new(),
            optional: Vec::new(),
            installed: true,
            problem: None,
            check_main: false,
        });
    }

    let mut files = vec![PlannedFile::new(&main, kind, false, current.map(|f| f.file.clone()))];
    let mut queued: HashSet<String> = HashSet::from([project_id.clone()]);
    let mut optional: Vec<String> = Vec::new();
    // (proyecto que pide, proyecto que falta / es incompatible)
    let mut missing: Vec<(String, String)> = Vec::new();
    let mut conflicts: Vec<(String, String)> = Vec::new();
    let mut pending = VecDeque::from([main.clone()]);
    while let Some(next) = pending.pop_front() {
        for dependency in &next.dependencies {
            let dependency_project = match (&dependency.project_id, &dependency.version_id) {
                (Some(project), _) => project.clone(),
                (None, Some(version_id)) => match version(version_id).await {
                    Ok(v) => v.project_id,
                    Err(_) => continue,
                },
                _ => continue,
            };
            match dependency.kind.as_str() {
                "incompatible" if installed_by_project.contains_key(dependency_project.as_str()) => {
                    conflicts.push((next.project_id.clone(), dependency_project));
                }
                "required" => {
                    if installed_by_project.contains_key(dependency_project.as_str()) || !queued.insert(dependency_project.clone()) {
                        continue;
                    }
                    // Una dependencia que pide una versión exacta puede pedir la de otro loader (la de
                    // Forge en una instancia de NeoForge): entonces se busca la que sirve.
                    let exact = match &dependency.version_id {
                        Some(id) => version(id).await.ok().filter(|v| kind != ContentType::Mod || version_fits(v, &loaders, minecraft)),
                        None => None,
                    };
                    let chosen = match exact {
                        Some(found) => Some(found),
                        None => project_versions(&dependency_project, &loaders, &[minecraft]).await.ok().and_then(|list| preferred(&list, &loaders).cloned()),
                    };
                    match chosen {
                        Some(found) => {
                            files.push(PlannedFile::new(&found, kind, true, None));
                            pending.push_back(found);
                        }
                        None => missing.push((next.project_id.clone(), dependency_project)),
                    }
                }
                "optional" => {
                    if !installed_by_project.contains_key(dependency_project.as_str()) && !queued.contains(&dependency_project) && !optional.contains(&dependency_project) {
                        optional.push(dependency_project);
                    }
                }
                _ => {}
            }
        }
    }
    // Lo instalado también puede decir que no es compatible con algo del plan.
    for file in &installed {
        for project in &file.incompatible {
            if queued.contains(project) {
                if let Some(owner) = &file.project_id {
                    conflicts.push((project.clone(), owner.clone()));
                }
            }
        }
    }

    // Cada .jar se abre para ver para qué loader es de verdad: si Modrinth lo etiquetó mal, se busca
    // otra versión del mismo proyecto que sí sirva.
    let mut compat_problem = None;
    let mut check_main = false;
    if missing.is_empty() && conflicts.is_empty() && kind == ContentType::Mod {
        let connector = installed.iter().any(|f| f.mods.iter().any(|m| m.id == "connectormod"));
        for index in 0..files.len() {
            let (_, jar) = planned_jar(instance, &files[index]).await?;
            if compat::loader_fits(jar, &instance.loader, minecraft, connector) {
                continue;
            }
            let others = native_first(project_versions(&files[index].project_id, &loaders, &[minecraft]).await?, &loaders);
            let mut replaced = false;
            for other in others.iter().filter(|v| v.id != files[index].version_id).take(6) {
                let candidate = PlannedFile::new(other, kind, files[index].dependency, files[index].replaces.clone());
                if compat::loader_fits(planned_jar(instance, &candidate).await?.1, &instance.loader, minecraft, connector) {
                    files[index] = candidate;
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                compat_problem = Some(t!(
                    "{} es un mod de {} (aunque Modrinth diga otra cosa) y esta instancia es {target}.",
                    "{} is a {} mod (even if Modrinth says otherwise) and this instance is {target}.",
                    if files[index].project_id == project_id { title.clone() } else { files[index].filename.clone() },
                    jar.describe()
                ));
                check_main = index == 0;
                break;
            }
        }
    }
    // Rangos de versión de los propios mods (fabric.mod.json, mods.toml): cambia dependencias si no encajan.
    if compat_problem.is_none() && missing.is_empty() && conflicts.is_empty() && kind == ContentType::Mod {
        let main_file = files[0].filename.clone();
        compat_problem = repair(instance, &mut files, &installed, &loaders).await?;
        check_main = compat_problem.is_some() && files.first().is_some_and(|f| f.filename == main_file);
    }

    // Nombres e iconos de todo lo que aparece
    let mut ids: Vec<String> = files.iter().map(|f| f.project_id.clone()).collect();
    ids.extend(optional.iter().cloned());
    ids.extend(missing.iter().chain(&conflicts).flat_map(|(a, b)| [a.clone(), b.clone()]));
    ids.sort();
    ids.dedup();
    let names = summaries(&ids).await;
    let name = |id: &str| names.get(id).map(|s| s.title.clone()).unwrap_or_else(|| id.to_string());
    for file in &mut files {
        if let Some(summary) = names.get(&file.project_id) {
            file.title = summary.title.clone();
            file.icon_url = summary.icon_url.clone();
        }
        if file.title.is_empty() {
            file.title = if file.project_id == project_id { title.clone() } else { file.filename.clone() };
        }
    }
    let problem = if let Some((who, needs)) = missing.first() {
        Some(t!("{} necesita {}, que no tiene versión para {target}.", "{} needs {}, which has no version for {target}.", name(who), name(needs)))
    } else if let Some((a, b)) = conflicts.first() {
        Some(t!("{} no es compatible con {} (ya instalado).", "{} is not compatible with {} (already installed).", name(a), name(b)))
    } else {
        compat_problem
    };
    Ok(Plan {
        project_id,
        check_main,
        title,
        version_number: main.version_number,
        files,
        optional: optional.iter().filter_map(|id| names.get(id).cloned()).collect(),
        installed: false,
        problem,
    })
}

// ================= Instalar y actualizar

/// Descarga los archivos del plan (verificando el SHA-1), manda a la papelera las versiones que
/// reemplazan y anota todo en el índice.
pub async fn install(
    instance: &Instance,
    files: &[PlannedFile],
    progress: impl Fn(usize, usize, u64, u64) + Send + Sync + 'static,
) -> Result<(), String> {
    let mut downloads = Vec::new();
    let mut targets = Vec::new();
    for file in files {
        let kind = ContentType::parse(&file.kind).ok_or(t!("Categoría desconocida.", "Unknown category."))?;
        // Solo archivos del CDN de Modrinth con un nombre simple de la extensión esperada.
        let safe_name = !file.filename.is_empty()
            && !file.filename.contains(['/', '\\', ':'])
            && !file.filename.starts_with('.')
            && file.filename.to_lowercase().ends_with(kind.extension());
        if !file.url.starts_with(CDN) || !safe_name || file.sha1.len() != 40 {
            return Err(t!("El archivo {} no es válido.", "The file {} is not valid.", file.filename));
        }
        let folder = content::folder(instance, kind.kind()).unwrap();
        // Al actualizar un mod desactivado, la versión nueva también queda desactivada.
        let disabled = file.replaces.as_deref().is_some_and(|r| r.ends_with(".disabled"));
        let name = if disabled { format!("{}.disabled", file.filename) } else { file.filename.clone() };
        downloads.push(FileDownload { url: file.url.clone(), path: folder.join(&name), sha1: Some(file.sha1.clone()), size: Some(file.size) });
        targets.push((file, kind, folder, name));
    }
    // Los que ya se bajaron al revisar la compatibilidad se copian en lugar de descargarse otra vez.
    let candidates = candidates_dir(instance);
    for download in &downloads {
        let cached = candidates.join(format!("{}.jar", download.sha1.as_deref().unwrap_or_default()));
        if cached.exists() && !download.path.exists() {
            let _ = std::fs::create_dir_all(download.path.parent().unwrap());
            let _ = std::fs::copy(&cached, &download.path);
        }
    }
    download::download_all(downloads, progress).await?;
    // Lo que se bajó para probar versiones ya no hace falta.
    let _ = std::fs::remove_dir_all(&candidates);

    let mut index = load_index(instance);
    for (file, kind, folder, name) in targets {
        if let Some(old) = file.replaces.as_deref().filter(|old| *old != name && !old.contains(['/', '\\'])) {
            let _ = trash::delete(folder.join(old));
            crate::integridad::forget(instance, &format!("{}/{old}", kind.kind()));
            index.files.remove(&format!("{}/{}", kind.kind(), old.strip_suffix(".disabled").unwrap_or(old)));
        }
        let Ok(meta) = std::fs::metadata(folder.join(&name)) else { continue };
        index.files.insert(
            format!("{}/{}", kind.kind(), file.filename),
            IndexEntry {
                sha1: file.sha1.clone(),
                size: meta.len(),
                modified: modified_seconds(&meta),
                checked: true,
                project_id: Some(file.project_id.clone()),
                version_id: Some(file.version_id.clone()),
                published: Some(file.published.clone()),
                version_type: Some(file.version_type.clone()).filter(|t| !t.is_empty()),
                incompatible: file.incompatible.clone(),
                mods: None,
            },
        );
    }
    save_index(instance, &index);
    Ok(())
}

/// Versiones más nuevas de lo instalado desde Modrinth (compatibles con la instancia).
pub async fn updates(instance: &Instance) -> Result<Vec<PlannedFile>, String> {
    let installed = identify(instance).await;
    let mut result = Vec::new();
    for kind in ContentType::ALL {
        let Ok(loaders) = instance_loaders(instance, kind) else { continue };
        // Una versión estable solo se actualiza a otra estable; una beta, a beta o estable.
        for allowed in [&["release"][..], &["release", "beta"][..], &[][..]] {
            let files: Vec<&InstalledFile> = installed
                .iter()
                .filter(|f| f.kind == kind.kind() && f.project_id.is_some())
                .filter(|f| match f.version_type.as_deref() {
                    Some("release") => allowed == ["release"],
                    Some("beta") => allowed == ["release", "beta"],
                    _ => allowed.is_empty(),
                })
                .collect();
            if files.is_empty() {
                continue;
            }
            let mut body = json!({
                "hashes": files.iter().map(|f| f.sha1.as_str()).collect::<Vec<_>>(),
                "algorithm": "sha1",
                "game_versions": [instance.minecraft_version],
            });
            if !loaders.is_empty() {
                body["loaders"] = json!(loaders);
            }
            if !allowed.is_empty() {
                body["version_types"] = json!(allowed);
            }
            let latest = post("/version_files/update", &body).await?;
            for file in files {
                let Some(newest) = latest.get(&file.sha1).and_then(parse_version) else { continue };
                let newer = file.published.as_deref().is_none_or(|current| newest.published.as_str() > current);
                if file.version_id.as_deref() != Some(newest.id.as_str()) && newer {
                    result.push(PlannedFile::new(&newest, kind, false, Some(file.file.clone())));
                }
            }
        }
    }
    let ids: Vec<String> = result.iter().map(|f| f.project_id.clone()).collect();
    let names = summaries(&ids).await;
    for file in &mut result {
        if let Some(summary) = names.get(&file.project_id) {
            file.title = summary.title.clone();
            file.icon_url = summary.icon_url.clone();
        }
    }
    Ok(result)
}

/// Una actualización que se dejó fuera porque rompería algo.
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HeldBack {
    pub version_id: String,
    pub title: String,
    pub reason: String,
}

/// Aplica actualizaciones dejando fuera las que romperían algo (por ejemplo, un Sodium nuevo que el
/// Iris instalado no acepta). Devuelve las que se dejaron fuera y por qué.
pub async fn apply_updates(
    instance: &Instance,
    mut files: Vec<PlannedFile>,
    progress: impl Fn(usize, usize, u64, u64) + Send + Sync + 'static,
) -> Result<Vec<HeldBack>, String> {
    let mut held_back = Vec::new();
    // Una actualización que pida otro loader, otra versión del juego u otra dependencia se queda fuera.
    if instance.loader != "Vanilla" && files.iter().any(|f| f.kind == "mods") {
        let installed = identify(instance).await;
        // Una actualización etiquetada para este loader que en realidad es para otro.
        let connector = installed.iter().any(|f| f.mods.iter().any(|m| m.id == "connectormod"));
        let mut index = 0;
        while index < files.len() {
            if files[index].kind == "mods" {
                let (_, jar) = planned_jar(instance, &files[index]).await?;
                if !compat::loader_fits(jar, &instance.loader, &instance.minecraft_version, connector) {
                    let removed = files.remove(index);
                    held_back.push(HeldBack {
                        reason: t!(
                            "Es un mod de {} aunque Modrinth diga {}.",
                            "It is a {} mod even though Modrinth says {}.",
                            jar.describe(),
                            instance.loader
                        ),
                        title: format!("{} {}", removed.title, removed.version_number),
                        version_id: removed.version_id,
                    });
                    continue;
                }
            }
            index += 1;
        }
        loop {
            let replaced: HashSet<&str> = files.iter().filter_map(|f| f.replaces.as_deref()).collect();
            let mut all: Vec<(String, Vec<ModMeta>)> = installed
                .iter()
                .filter(|f| f.kind == "mods" && !f.file.ends_with(".disabled") && !replaced.contains(f.file.as_str()))
                .map(|f| (f.file.clone(), f.mods.clone()))
                .collect();
            for file in files.iter().filter(|f| f.kind == "mods") {
                all.push((file.filename.clone(), planned_meta(instance, file).await?));
            }
            let problems = compat::check_game(&all, &compat::Game::of(instance));
            let Some((index, problem)) = problems.iter().find_map(|p| {
                let sides: Vec<&str> = match p {
                    Problem::Wrong { owner, found_file, .. } | Problem::Breaks { owner, found_file, .. } => vec![found_file, owner],
                    Problem::Missing { owner, .. } => vec![owner],
                };
                sides.into_iter().find_map(|side| files.iter().position(|f| f.filename == side)).map(|i| (i, p))
            }) else {
                break;
            };
            let removed = files.remove(index);
            held_back.push(HeldBack {
                reason: problem.describe(&compat::names(&all)),
                title: format!("{} {}", removed.title, removed.version_number),
                version_id: removed.version_id,
            });
        }
    }
    install(instance, &files, progress).await?;
    Ok(held_back)
}

// ================= Modpacks y data packs

/// Tipos que se pueden buscar: además de lo que va dentro de una instancia, modpacks y data packs.
fn search_type(kind: &str) -> Option<&'static str> {
    match kind {
        "mod" => Some("mod"),
        "resourcepack" => Some("resourcepack"),
        "shader" => Some("shader"),
        "modpack" => Some("modpack"),
        "datapack" => Some("datapack"),
        _ => None,
    }
}

/// Un archivo de Modrinth encontrado por su SHA-1 (para exportar .mrpack con enlaces de descarga).
#[derive(Clone, Debug)]
pub struct RemoteFile {
    pub url: String,
    pub sha1: String,
    pub sha512: String,
    pub size: u64,
}

pub async fn files_by_hash(hashes: &[String]) -> Result<HashMap<String, RemoteFile>, String> {
    let mut result = HashMap::new();
    for chunk in hashes.chunks(500) {
        let found = post("/version_files", &json!({ "hashes": chunk, "algorithm": "sha1" })).await?;
        for (hash, version) in found.as_object().into_iter().flatten() {
            let file = version["files"].as_array().into_iter().flatten().find(|f| f["hashes"]["sha1"].as_str() == Some(hash.as_str()));
            if let Some(file) = file {
                if let (Some(url), Some(sha512)) = (file["url"].as_str(), file["hashes"]["sha512"].as_str()) {
                    result.insert(
                        hash.clone(),
                        RemoteFile { url: url.into(), sha1: hash.clone(), sha512: sha512.into(), size: file["size"].as_u64().unwrap_or(0) },
                    );
                }
            }
        }
    }
    Ok(result)
}

/// Un modpack de Modrinth descargado (su .mrpack) listo para instalarse como instancia.
#[derive(Debug)]
pub struct ModpackDownload {
    pub file: std::path::PathBuf,
    pub title: String,
    pub icon_url: Option<String>,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
}

/// Loaders de los modpacks que se pueden instalar.
pub const MODPACK_LOADERS: &[&str] = &["fabric", "quilt", "forge", "neoforge"];

pub async fn download_modpack(project_id: &str, version_id: Option<&str>, temp_dir: &Path) -> Result<ModpackDownload, String> {
    let project = get(&format!("/project/{}", checked_id(project_id)?), &[]).await?;
    if project["project_type"].as_str() != Some("modpack") {
        return Err(t!("Ese proyecto no es un modpack.", "That project is not a modpack."));
    }
    let title = text(&project, "title");
    let chosen = match version_id {
        Some(id) => version(id).await?,
        None => best(&project_versions(&text(&project, "id"), MODPACK_LOADERS, &[]).await?)
            .cloned()
            .ok_or(t!("{title} no tiene versiones que Parabellum pueda instalar.", "{title} has no versions Parabellum can install."))?,
    };
    if !chosen.filename.to_lowercase().ends_with(".mrpack") || !chosen.url.starts_with(CDN) {
        return Err(t!("Esa versión no trae un archivo .mrpack.", "That version has no .mrpack file."));
    }
    let file = temp_dir.join(format!("{}.mrpack", chosen.sha1));
    download::download_all(
        vec![FileDownload { url: chosen.url.clone(), path: file.clone(), sha1: Some(chosen.sha1.clone()), size: Some(chosen.size) }],
        |_, _, _, _| {},
    )
    .await?;
    Ok(ModpackDownload {
        file,
        title,
        icon_url: project["icon_url"].as_str().filter(|u| u.starts_with(CDN)).map(String::from),
        project_id: text(&project, "id"),
        version_id: chosen.id,
        version_number: chosen.version_number,
    })
}

/// Busca en Modrinth el mod con ese id (el de su fabric.mod.json o mods.toml) en una versión para la
/// instancia que cumpla los rangos. Comprueba el .jar de verdad (el id del proyecto no siempre es el
/// del mod). Devuelve (proyecto, versión), o None si no está en Modrinth.
pub async fn find_by_mod_id(instance: &Instance, mod_id: &str, requirements: &[String]) -> Result<Option<(String, String)>, String> {
    let loaders = instance_loaders(instance, ContentType::Mod)?;
    let minecraft = instance.minecraft_version.clone();
    let mut candidates: Vec<String> = vec![mod_id.to_string(), mod_id.replace('_', "-")];
    let query = SearchQuery {
        query: mod_id.replace(['_', '-'], " "),
        kind: "mod".into(),
        loaders: loaders.iter().map(|l| l.to_string()).collect(),
        game_version: Some(minecraft.clone()),
        categories: Vec::new(),
        sort: "relevance".into(),
        offset: 0,
    };
    if let Ok(results) = search(&query).await {
        candidates.extend(results.hits.into_iter().take(6).map(|h| h.id));
    }
    let mut seen = HashSet::new();
    let mut downloads = 0;
    for project in candidates.into_iter().filter(|c| seen.insert(c.clone())) {
        let Ok(id) = checked_id(&project).map(String::from) else { continue };
        let Ok(versions) = project_versions(&id, &loaders, &[minecraft.as_str()]).await else { continue };
        for version in native_first(versions, &loaders).iter().take(6) {
            if downloads >= 14 {
                return Ok(None);
            }
            downloads += 1;
            let file = PlannedFile::new(version, ContentType::Mod, true, None);
            let Ok((metas, jar)) = planned_jar(instance, &file).await else { break };
            let Some(meta) = metas.iter().find(|m| m.id == mod_id || m.provides.iter().any(|p| p == mod_id)) else {
                // Este proyecto no es ese mod: se prueba el siguiente.
                break;
            };
            if requirements.iter().all(|r| compat::matches(&meta.version, r)) && compat::loader_fits(jar, &instance.loader, &minecraft, false) {
                return Ok(Some((version.project_id.clone(), version.id.clone())));
            }
        }
    }
    Ok(None)
}

/// Icono de un proyecto (solo del CDN de Modrinth y de tamaño razonable).
pub async fn download_icon(url: &str) -> Option<Vec<u8>> {
    if !url.starts_with(CDN) {
        return None;
    }
    let response = download::client().get(url).header(reqwest::header::USER_AGENT, USER_AGENT).send().await.ok()?;
    let bytes = response.error_for_status().ok()?.bytes().await.ok()?;
    (bytes.len() < 2_000_000).then(|| bytes.to_vec())
}

/// Instala un data pack de Modrinth dentro de un mundo. Devuelve (nombre, versión).
pub async fn install_datapack(instance: &Instance, world: &str, project_id: &str, version_id: Option<&str>) -> Result<(String, String), String> {
    let world_dir = content::item_path(instance, "worlds", world).map_err(|_| t!("La instancia no tiene un mundo llamado «{world}».", "The instance has no world named «{world}»."))?;
    let project = get(&format!("/project/{}", checked_id(project_id)?), &[]).await?;
    let title = text(&project, "title");
    let minecraft = instance.minecraft_version.as_str();
    let chosen = match version_id {
        Some(id) => version(id).await?,
        None => best(&project_versions(&text(&project, "id"), &["datapack"], &[minecraft]).await?)
            .cloned()
            .ok_or(t!("{title} no tiene versión de data pack para Minecraft {minecraft}.", "{title} has no data pack version for Minecraft {minecraft}."))?,
    };
    let safe_name = !chosen.filename.contains(['/', '\\', ':']) && chosen.filename.to_lowercase().ends_with(".zip");
    if !chosen.loaders.iter().any(|l| l == "datapack") || !safe_name || !chosen.url.starts_with(CDN) {
        return Err(t!("Esa versión no es un data pack.", "That version is not a data pack."));
    }
    download::download_all(
        vec![FileDownload {
            url: chosen.url.clone(),
            path: world_dir.join("datapacks").join(&chosen.filename),
            sha1: Some(chosen.sha1.clone()),
            size: Some(chosen.size),
        }],
        |_, _, _, _| {},
    )
    .await?;
    Ok((title, chosen.version_number))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(kind: &str, number: &str) -> Version {
        Version {
            id: number.into(),
            project_id: "p".into(),
            name: number.into(),
            version_number: number.into(),
            version_type: kind.into(),
            published: String::new(),
            downloads: 0,
            loaders: vec![],
            game_versions: vec![],
            filename: format!("{number}.jar"),
            url: String::new(),
            sha1: String::new(),
            size: 0,
            dependencies: vec![],
        }
    }

    #[test]
    fn prefiere_la_estable_mas_nueva() {
        let list = vec![version("beta", "3"), version("release", "2"), version("release", "1")];
        assert_eq!(best(&list).unwrap().id, "2");
        let only_betas = vec![version("alpha", "5"), version("beta", "4")];
        assert_eq!(best(&only_betas).unwrap().id, "4");
    }

    #[test]
    fn ids_seguros() {
        assert!(checked_id("P7dR8mSH").is_ok());
        assert!(checked_id("fabric-api").is_ok());
        assert!(checked_id("../version").is_err());
        assert!(checked_id("a/b").is_err());
        assert!(checked_id("").is_err());
    }
}
