//! CurseForge: buscar, ver e instalar mods, resource packs y shaders, e importar sus modpacks.
//!
//! Parabellum no lleva la clave de la API: el exe se reparte y cualquiera podría sacarla. Le pregunta al
//! puente (`puente-curseforge/`, una función de Google Cloud), que la tiene. Los archivos bajan directo del
//! CDN de CurseForge (forgecdn.net), que pasa el filtro de la escuela.
//!
//! Los ids van con «cf:» delante ("cf:394468", "cf:5012345" para un archivo) y salen con las mismas formas
//! que los de Modrinth (`modrinth::Hit`, `Project`, `Version`, `Plan`…): la interfaz los muestra igual y los
//! comandos saben a quién preguntar por el prefijo.
//!
//! Algunos autores no dejan bajar sus archivos fuera de CurseForge (`downloadUrl` vacío). Las condiciones
//! de la API piden respetarlo: esos se abren en su página para bajarlos a mano.

use crate::compat;
use crate::download::{self, FileDownload};
use crate::instances::Instance;
use crate::modrinth::{self, ContentType, Hit, Plan, PlannedFile, Project, ProjectSummary, SearchQuery, SearchResults, Version};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// El puente de CurseForge. `PARABELLUM_CURSEFORGE_PUENTE` lo cambia (para probar uno local).
const BRIDGE: &str = "https://curseforge-86460886589.us-central1.run.app";
pub const PREFIX: &str = "cf:";
const GAME: u64 = 432;
const CLASS_MODS: u64 = 6;
const CLASS_RESOURCE_PACKS: u64 = 12;
const CLASS_SHADERS: u64 = 6552;
const CLASS_MODPACKS: u64 = 4471;
/// De dónde se bajan los archivos (los mismos que acepta integridad.rs).
const CDN_HOSTS: [&str; 3] = ["edge.forgecdn.net", "media.forgecdn.net", "mediafilez.forgecdn.net"];
const INDEX_FILE: &str = "curseforge.json";
const MANIFEST: &str = "manifest.json";

pub fn unavailable() -> String {
    t!(
        "No se puede conectar con CurseForge: el puente de Parabellum no respondió. Vuelve a intentarlo en un momento.",
        "Cannot connect to CurseForge: the Parabellum bridge did not answer. Try again in a moment."
    )
}

/// ¿Es un id de CurseForge («cf:…»)?
pub fn is_curseforge(id: &str) -> bool {
    id.starts_with(PREFIX)
}

fn numeric(id: &str) -> Result<u64, String> {
    id.strip_prefix(PREFIX).unwrap_or(id).parse().map_err(|_| t!("Identificador de CurseForge no válido.", "Invalid CurseForge identifier."))
}

fn cf(id: u64) -> String {
    format!("{PREFIX}{id}")
}

// ================= HTTP

fn base() -> String {
    std::env::var("PARABELLUM_CURSEFORGE_PUENTE").ok().filter(|b| !b.trim().is_empty()).unwrap_or_else(|| BRIDGE.to_string()).trim().trim_end_matches('/').to_string()
}

async fn send(request: reqwest::RequestBuilder) -> Result<Value, String> {
    let response = request.header(reqwest::header::ACCEPT, "application/json").send().await.map_err(|_| unavailable())?;
    match response.status() {
        status if status.is_success() => {
            let body: Value = response.json().await.map_err(|e| t!("CurseForge respondió algo inesperado: {e}", "CurseForge answered something unexpected: {e}"))?;
            Ok(body.get("data").cloned().unwrap_or(body))
        }
        reqwest::StatusCode::NOT_FOUND => Err(t!("CurseForge no tiene ese proyecto o archivo.", "CurseForge does not have that project or file.")),
        reqwest::StatusCode::TOO_MANY_REQUESTS => Err(t!("CurseForge pide esperar un momento. Vuelve a intentarlo en un minuto.", "CurseForge asks to wait a moment. Try again in a minute.")),
        reqwest::StatusCode::SERVICE_UNAVAILABLE => Err(t!("El puente de CurseForge todavía no está listo.", "The CurseForge bridge is not ready yet.")),
        status => Err(t!("CurseForge respondió {status}. Vuelve a intentarlo más tarde.", "CurseForge answered {status}. Try again later.")),
    }
}

async fn get(path: &str, query: &[(&str, String)]) -> Result<Value, String> {
    send(download::client().get(format!("{}{path}", base())).query(query)).await
}

async fn post(path: &str, body: &Value) -> Result<Value, String> {
    send(download::client().post(format!("{}{path}", base())).json(body)).await
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

/// CurseForge manda las descargas como número con decimales.
fn count(value: &Value) -> u64 {
    value.as_u64().or_else(|| value.as_f64().map(|f| f as u64)).unwrap_or(0)
}

// ================= Clases y loaders

fn class_of(kind: &str) -> Option<u64> {
    match kind {
        "mod" | "mods" => Some(CLASS_MODS),
        "resourcepack" | "resourcepacks" => Some(CLASS_RESOURCE_PACKS),
        "shader" | "shaderpacks" => Some(CLASS_SHADERS),
        "modpack" => Some(CLASS_MODPACKS),
        _ => None,
    }
}

fn project_type(class: u64) -> &'static str {
    match class {
        CLASS_RESOURCE_PACKS => "resourcepack",
        CLASS_SHADERS => "shader",
        CLASS_MODPACKS => "modpack",
        _ => "mod",
    }
}

/// La carpeta de la instancia donde va un archivo de esa clase (los mods, si no se sabe).
fn folder_of(class: u64) -> &'static str {
    match class {
        CLASS_RESOURCE_PACKS => "resourcepacks",
        CLASS_SHADERS => "shaderpacks",
        _ => "mods",
    }
}

/// El número de loader de CurseForge (`modLoaderType`).
fn loader_type(loader: &str) -> Option<u64> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// Los loaders y las versiones de Minecraft que dice un archivo (CurseForge los mezcla en `gameVersions`,
/// junto con «Client», «Server» o «Java 21»).
fn split_game_versions(list: &Value) -> (Vec<String>, Vec<String>) {
    let mut loaders = Vec::new();
    let mut games = Vec::new();
    for item in list.as_array().into_iter().flatten().filter_map(Value::as_str) {
        let lower = item.to_lowercase();
        if ["forge", "fabric", "quilt", "neoforge"].contains(&lower.as_str()) {
            loaders.push(lower);
        } else if item.starts_with(|c: char| c.is_ascii_digit()) && item.contains('.') {
            games.push(item.to_string());
        }
    }
    (loaders, games)
}

// ================= Búsqueda y categorías

pub async fn search(query: &SearchQuery) -> Result<SearchResults, String> {
    let class = class_of(&query.kind).ok_or(t!("Este tipo de contenido no está en CurseForge.", "This type of content is not on CurseForge."))?;
    let sort = match query.sort.as_str() {
        "downloads" => 6,
        "updated" => 3,
        "newest" => 11,
        _ => 2,
    };
    let mut params: Vec<(&str, String)> = vec![
        ("gameId", GAME.to_string()),
        ("classId", class.to_string()),
        ("sortField", sort.to_string()),
        ("sortOrder", "desc".into()),
        ("pageSize", modrinth::PAGE_SIZE.to_string()),
        ("index", query.offset.to_string()),
    ];
    if !query.query.trim().is_empty() {
        params.push(("searchFilter", query.query.trim().to_string()));
    }
    if let Some(version) = query.game_version.as_ref().filter(|v| !v.trim().is_empty()) {
        params.push(("gameVersion", version.trim().to_string()));
    }
    let loaders: Vec<u64> = query.loaders.iter().filter_map(|l| loader_type(l)).collect();
    match loaders.as_slice() {
        [] => {}
        [only] => params.push(("modLoaderType", only.to_string())),
        many => params.push(("modLoaderTypes", format!("[{}]", many.iter().map(u64::to_string).collect::<Vec<_>>().join(",")))),
    }
    let categories: Vec<u64> = query.categories.iter().filter_map(|c| c.strip_prefix(PREFIX)?.parse().ok()).collect();
    match categories.as_slice() {
        [] => {}
        [only] => params.push(("categoryId", only.to_string())),
        many => params.push(("categoryIds", format!("[{}]", many.iter().map(u64::to_string).collect::<Vec<_>>().join(",")))),
    }
    let response = send(download::client().get(format!("{}/v1/mods/search", base())).query(&params)).await;
    // `send` devuelve solo `data`; la paginación se pide aparte con el total.
    let list = response?;
    let hits: Vec<Hit> = list.as_array().into_iter().flatten().map(hit).collect();
    // CurseForge no da más de 10 000 resultados por búsqueda: si llega una página llena, hay más.
    let total = if hits.len() as u32 >= modrinth::PAGE_SIZE { (query.offset + modrinth::PAGE_SIZE * 2) as u64 } else { (query.offset as usize + hits.len()) as u64 };
    Ok(SearchResults { hits, total })
}

fn hit(m: &Value) -> Hit {
    Hit {
        id: cf(m["id"].as_u64().unwrap_or(0)),
        slug: text(m, "slug"),
        title: text(m, "name"),
        description: text(m, "summary"),
        author: m["authors"].as_array().and_then(|a| a.first()).map(|a| text(a, "name")).unwrap_or_default(),
        icon_url: m["logo"]["thumbnailUrl"].as_str().filter(|u| u.starts_with("https://")).map(String::from),
        downloads: count(&m["downloadCount"]),
        follows: count(&m["thumbsUpCount"]),
        categories: m["categories"].as_array().into_iter().flatten().map(|c| text(c, "name")).filter(|n| !n.is_empty()).collect(),
        updated: text(m, "dateModified"),
        client_side: String::new(),
        server_side: String::new(),
    }
}

pub async fn categories(kind: &str) -> Result<Vec<modrinth::Category>, String> {
    let class = class_of(kind).ok_or(t!("Este tipo de contenido no está en CurseForge.", "This type of content is not on CurseForge."))?;
    let list = get("/v1/categories", &[("gameId", GAME.to_string()), ("classId", class.to_string())]).await?;
    let mut result: Vec<modrinth::Category> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["isClass"] != true && c["id"].as_u64().is_some())
        .map(|c| modrinth::Category {
            name: cf(c["id"].as_u64().unwrap_or(0)),
            icon: c["iconUrl"].as_str().filter(|u| u.starts_with("https://")).unwrap_or_default().to_string(),
            header: "categories".into(),
            label: Some(text(c, "name")),
        })
        .collect();
    result.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(result)
}

// ================= Proyecto y versiones

pub async fn project(id: &str) -> Result<Project, String> {
    let id = numeric(id)?;
    let (project_path, description_path) = (format!("/v1/mods/{id}"), format!("/v1/mods/{id}/description"));
    let (project, description) = futures::join!(get(&project_path, &[]), get(&description_path, &[]));
    let m = project?;
    let class = m["classId"].as_u64().unwrap_or(CLASS_MODS);
    let mut loaders = Vec::new();
    let mut games = Vec::new();
    for index in m["latestFilesIndexes"].as_array().into_iter().flatten() {
        if let Some(name) = index["modLoader"].as_u64().and_then(|n| [(1, "forge"), (4, "fabric"), (5, "quilt"), (6, "neoforge")].iter().find(|(k, _)| *k == n)).map(|(_, v)| v.to_string()) {
            if !loaders.contains(&name) {
                loaders.push(name);
            }
        }
        let game = text(index, "gameVersion");
        if !game.is_empty() && !games.contains(&game) {
            games.push(game);
        }
    }
    let mut links = Vec::new();
    for (label, key) in [(t!("Código fuente", "Source code"), "sourceUrl"), (t!("Reportar errores", "Report issues"), "issuesUrl"), ("Wiki".into(), "wikiUrl")] {
        if let Some(url) = m["links"][key].as_str().filter(|u| u.starts_with("https://")) {
            links.push((label, url.to_string()));
        }
    }
    Ok(Project {
        id: cf(id),
        slug: text(&m, "slug"),
        title: text(&m, "name"),
        description: text(&m, "summary"),
        body: description.ok().and_then(|d| d.as_str().map(String::from)).unwrap_or_default(),
        html: true,
        icon_url: m["logo"]["thumbnailUrl"].as_str().filter(|u| u.starts_with("https://")).map(String::from),
        project_type: project_type(class).into(),
        author: m["authors"].as_array().into_iter().flatten().take(3).map(|a| text(a, "name")).collect::<Vec<_>>().join(", "),
        downloads: count(&m["downloadCount"]),
        followers: count(&m["thumbsUpCount"]),
        categories: m["categories"].as_array().into_iter().flatten().map(|c| text(c, "name")).filter(|n| !n.is_empty()).collect(),
        loaders,
        game_versions: games,
        client_side: String::new(),
        server_side: String::new(),
        license: String::new(),
        updated: text(&m, "dateModified"),
        published: text(&m, "dateReleased"),
        page_url: m["links"]["websiteUrl"].as_str().filter(|u| u.starts_with("https://")).unwrap_or_default().to_string(),
        links,
        gallery: m["screenshots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| s["url"].as_str().is_some_and(|u| u.starts_with("https://")))
            .map(|s| modrinth::GalleryImage { url: text(s, "url"), title: text(s, "title"), description: text(s, "description") })
            .collect(),
    })
}

/// Un archivo de CurseForge con la forma de una versión de Modrinth. Sin `url` si su autor no deja bajarlo
/// fuera de CurseForge.
fn version(file: &Value) -> Option<Version> {
    let (loaders, game_versions) = split_game_versions(&file["gameVersions"]);
    let sha1 = file["hashes"].as_array().into_iter().flatten().find(|h| h["algo"] == 1).map(|h| text(h, "value")).unwrap_or_default();
    let display = text(file, "displayName");
    Some(Version {
        id: cf(file["id"].as_u64()?),
        project_id: cf(file["modId"].as_u64()?),
        name: display.clone(),
        version_number: display,
        version_type: match file["releaseType"].as_u64() {
            Some(2) => "beta",
            Some(3) => "alpha",
            _ => "release",
        }
        .into(),
        published: text(file, "fileDate"),
        downloads: count(&file["downloadCount"]),
        loaders,
        game_versions,
        filename: text(file, "fileName"),
        url: file["downloadUrl"].as_str().filter(|u| cdn_url(u)).unwrap_or_default().to_string(),
        sha1,
        size: count(&file["fileLength"]),
        dependencies: file["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|d| {
                let kind = match d["relationType"].as_u64()? {
                    1 => "embedded",
                    2 => "optional",
                    3 => "required",
                    5 => "incompatible",
                    _ => return None,
                };
                Some(modrinth::Dependency { project_id: Some(cf(d["modId"].as_u64()?)), version_id: None, kind: kind.into() })
            })
            .collect(),
    })
}

fn cdn_url(url: &str) -> bool {
    url.strip_prefix("https://").and_then(|rest| rest.split('/').next()).is_some_and(|host| CDN_HOSTS.contains(&host.to_lowercase().as_str()))
}

/// Archivos de un proyecto (del más nuevo al más viejo), de la versión de Minecraft y los loaders que se
/// pidan. CurseForge da 50 por página: con tres alcanza para elegir.
pub async fn project_versions(id: &str, loaders: &[&str], game_versions: &[&str]) -> Result<Vec<Version>, String> {
    let id = numeric(id)?;
    let mut query: Vec<(&str, String)> = vec![("pageSize", "50".into())];
    if let [only] = game_versions {
        query.push(("gameVersion", only.to_string()));
    }
    // Con un solo loader lo filtra CurseForge; con varios (Quilt también usa los de Fabric), aquí.
    let types: Vec<u64> = loaders.iter().filter_map(|l| loader_type(l)).collect();
    if let [only] = types.as_slice() {
        query.push(("modLoaderType", only.to_string()));
    }
    let mut files = Vec::new();
    for page in 0..3 {
        let mut paged = query.clone();
        paged.push(("index", (page * 50).to_string()));
        let list = get(&format!("/v1/mods/{id}/files"), &paged).await?;
        let found = list.as_array().cloned().unwrap_or_default();
        let full = found.len() == 50;
        files.extend(found);
        if !full {
            break;
        }
    }
    let wanted_loaders: Vec<String> = loaders.iter().filter(|l| loader_type(l).is_some()).map(|l| l.to_string()).collect();
    let mut versions: Vec<Version> = files
        .iter()
        .filter(|f| f["isServerPack"] != true && f["isAvailable"] != false)
        .filter_map(version)
        .filter(|v| wanted_loaders.is_empty() || v.loaders.is_empty() || v.loaders.iter().any(|l| wanted_loaders.contains(l)))
        .filter(|v| game_versions.is_empty() || v.game_versions.iter().any(|g| game_versions.contains(&g.as_str())))
        .collect();
    versions.sort_by(|a, b| b.published.cmp(&a.published));
    Ok(versions)
}

async fn file(project: u64, file: u64) -> Result<Version, String> {
    version(&get(&format!("/v1/mods/{project}/files/{file}"), &[]).await?).ok_or_else(|| t!("Ese archivo no se puede leer.", "That file cannot be read."))
}

/// Nombre, icono, clase y página de varios proyectos a la vez.
async fn mods(ids: &[u64]) -> HashMap<u64, Value> {
    let mut result = HashMap::new();
    for chunk in ids.chunks(500) {
        let Ok(list) = post("/v1/mods", &json!({ "modIds": chunk, "filterPcOnly": true })).await else { continue };
        for m in list.as_array().into_iter().flatten() {
            if let Some(id) = m["id"].as_u64() {
                result.insert(id, m.clone());
            }
        }
    }
    result
}

fn summary(m: &Value) -> ProjectSummary {
    ProjectSummary {
        id: cf(m["id"].as_u64().unwrap_or(0)),
        title: text(m, "name"),
        icon_url: m["logo"]["thumbnailUrl"].as_str().filter(|u| u.starts_with("https://")).map(String::from),
        project_type: project_type(m["classId"].as_u64().unwrap_or(CLASS_MODS)).into(),
    }
}

/// La página del archivo en curseforge.com, para bajarlo a mano.
fn file_page(m: &Value, file_id: u64) -> String {
    match m["links"]["websiteUrl"].as_str().filter(|u| u.starts_with("https://")) {
        Some(site) => format!("{}/files/{file_id}", site.trim_end_matches('/')),
        None => format!("https://www.curseforge.com/projects/{}", m["id"].as_u64().unwrap_or(0)),
    }
}

// ================= Lo instalado desde CurseForge

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    project_id: u64,
    file_id: u64,
    sha1: String,
}

/// `.lanzador/curseforge.json`: de qué proyecto y archivo es lo que se instaló desde CurseForge. Clave:
/// "mods/jei.jar" (sin ".disabled", como en modrinth.json).
#[derive(Serialize, Deserialize, Default, Debug)]
struct Index {
    files: HashMap<String, IndexEntry>,
}

fn load_index(instance: &Instance) -> Index {
    std::fs::read_to_string(instance.state_dir().join(INDEX_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_index(instance: &Instance, index: &Index) {
    save_index_in(&instance.game_dir, index);
}

fn save_index_in(game_dir: &Path, index: &Index) {
    let state = game_dir.join(crate::instances::STATE_DIR);
    let _ = std::fs::create_dir_all(&state);
    let file = state.join(INDEX_FILE);
    let temp = file.with_extension("tmp");
    if std::fs::write(&temp, serde_json::to_vec(index).unwrap()).is_ok() {
        let _ = std::fs::rename(temp, file);
    }
}

/// Lo instalado desde CurseForge que sigue en la instancia: (clave, archivo real, entrada).
fn present(instance: &Instance) -> Vec<(String, String, IndexEntry)> {
    load_index(instance)
        .files
        .into_iter()
        .filter_map(|(key, entry)| {
            let (kind, name) = key.split_once('/')?;
            let folder = crate::content::folder(instance, kind)?;
            let real = [name.to_string(), format!("{name}.disabled")].into_iter().find(|n| folder.join(n).is_file())?;
            if crate::integridad::sha1_file(&folder.join(&real)).as_deref() != Some(entry.sha1.as_str()) {
                return None;
            }
            Some((key, real, entry))
        })
        .collect()
}

/// Proyectos y archivos de CurseForge instalados («cf:…»), para marcar «Instalado».
pub fn installed(instance: &Instance) -> (Vec<String>, Vec<String>) {
    present(instance).into_iter().map(|(_, _, e)| (cf(e.project_id), cf(e.file_id))).unzip()
}

// ================= Plan de instalación (con dependencias)

fn content_type(class: u64) -> Result<ContentType, String> {
    match class {
        CLASS_MODS => Ok(ContentType::Mod),
        CLASS_RESOURCE_PACKS => Ok(ContentType::ResourcePack),
        CLASS_SHADERS => Ok(ContentType::Shader),
        CLASS_MODPACKS => Err(t!("Los modpacks se instalan como una instancia nueva: búscalo en la pestaña Modpacks.", "Modpacks are installed as a new instance: look for it in the Modpacks tab.")),
        _ => Err(t!("Este tipo de contenido todavía no se puede instalar.", "This type of content cannot be installed yet.")),
    }
}

/// Los loaders que sirven para buscar archivos de esa clase en la instancia (sin «minecraft»: CurseForge no
/// lo usa).
fn wanted_loaders(instance: &Instance, kind: ContentType) -> Result<Vec<&'static str>, String> {
    Ok(modrinth::instance_loaders(instance, kind)?.into_iter().filter(|l| loader_type(l).is_some()).collect())
}

fn planned(version: &Version, kind: ContentType, title: &str, icon: Option<String>, dependency: bool, replaces: Option<String>) -> PlannedFile {
    PlannedFile {
        project_id: version.project_id.clone(),
        version_id: version.id.clone(),
        title: title.to_string(),
        icon_url: icon,
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
        incompatible: Vec::new(),
    }
}

/// Qué se bajaría al instalar un proyecto de CurseForge (con sus dependencias obligatorias) o por qué no
/// se puede. Una dependencia que ya está en la instancia (de CurseForge, de Modrinth o puesta a mano) no
/// se vuelve a bajar: se compara por el id de mod que trae el .jar.
pub async fn plan(instance: &Instance, project_id: &str, version_id: Option<&str>) -> Result<Plan, String> {
    let project = numeric(project_id)?;
    let main_info = mods(&[project]).await.remove(&project).ok_or_else(|| t!("CurseForge no tiene ese proyecto.", "CurseForge does not have that project."))?;
    let title = text(&main_info, "name");
    let kind = content_type(main_info["classId"].as_u64().unwrap_or(CLASS_MODS))?;
    let loaders = wanted_loaders(instance, kind)?;
    let minecraft = instance.minecraft_version.clone();
    let target = match kind {
        ContentType::Mod => format!("{} {minecraft}", instance.loader),
        _ => format!("Minecraft {minecraft}"),
    };
    let mut plan = Plan::new(cf(project), title.clone());
    let chosen = match version_id {
        Some(id) => Some(file(project, numeric(id)?).await?),
        None => modrinth::preferred(&project_versions(project_id, &loaders, &[minecraft.as_str()]).await?, &loaders).cloned(),
    };
    let Some(chosen) = chosen else {
        plan.problem = Some(t!("{title} no tiene una versión para {target} en CurseForge.", "{title} has no version for {target} on CurseForge."));
        return Ok(plan);
    };
    plan.version_number = chosen.version_number.clone();
    if !compatible_file(&chosen, &cf(project), &loaders, &minecraft) {
        plan.problem = Some(t!(
            "La versión elegida de {title} no sirve para {target}. Elige una versión compatible.",
            "The selected version of {title} does not work with {target}. Choose a compatible version."
        ));
        return Ok(plan);
    }
    let already = present(instance);
    if already.iter().any(|(_, _, e)| cf(e.file_id) == chosen.id) {
        plan.installed = true;
        return Ok(plan);
    }
    if chosen.url.is_empty() {
        plan.problem = Some(t!(
            "Su autor no deja bajar {title} fuera de CurseForge. Ábrelo en su página, bájalo y suéltalo en la pestaña Contenido de la instancia.",
            "Its author does not allow downloading {title} outside CurseForge. Open its page, download it and drop it on the instance's Content tab."
        ));
        plan.manual_url = Some(file_page(&main_info, numeric(&chosen.id)?));
        return Ok(plan);
    }
    let replaces = |project: &str| already.iter().find(|(_, _, e)| cf(e.project_id) == project).map(|(_, real, _)| real.clone());
    let icon = |m: &Value| m["logo"]["thumbnailUrl"].as_str().filter(|u| u.starts_with("https://")).map(String::from);
    plan.files.push(planned(&chosen, kind, &title, icon(&main_info), false, replaces(&chosen.project_id)));
    if kind != ContentType::Mod {
        return Ok(plan);
    }

    // Las dependencias obligatorias (y las de ellas), una vez cada una.
    let installed_ids = installed_mod_ids(instance).await;
    let candidates = modrinth::candidates_dir(instance);
    let mut seen: HashSet<String> = HashSet::from([chosen.project_id.clone()]);
    let mut pending: Vec<Version> = vec![chosen.clone()];
    let mut optional: Vec<u64> = Vec::new();
    while let Some(current) = pending.pop() {
        let required: Vec<u64> = current.dependencies.iter().filter(|d| d.kind == "required").filter_map(|d| numeric(d.project_id.as_deref()?).ok()).collect();
        optional.extend(current.dependencies.iter().filter(|d| d.kind == "optional").filter_map(|d| numeric(d.project_id.as_deref()?).ok()));
        let new: Vec<u64> = required.into_iter().filter(|id| seen.insert(cf(*id))).collect();
        if new.is_empty() {
            continue;
        }
        let infos = mods(&new).await;
        for id in new {
            if plan.files.len() > 40 {
                plan.problem = Some(t!("{title} pide demasiadas dependencias para instalarlas con seguridad. Usa su modpack.", "{title} requests too many dependencies to install safely. Use its modpack."));
                return Ok(plan);
            }
            let info = infos.get(&id).cloned().unwrap_or(Value::Null);
            let name = text(&info, "name");
            if already.iter().any(|(_, _, e)| e.project_id == id) {
                continue;
            }
            let versions = project_versions(&cf(id), &loaders, &[minecraft.as_str()]).await.unwrap_or_default();
            let Some(dependency) = modrinth::preferred(&versions, &loaders).cloned() else {
                // Sin versión para la instancia: quizá ya está puesto a mano o de Modrinth (por su nombre).
                if slug_installed(&info, &installed_ids) {
                    continue;
                }
                plan.problem = Some(t!("{title} necesita {name}, que no tiene versión para {target} en CurseForge.", "{title} needs {name}, which has no version for {target} on CurseForge."));
                return Ok(plan);
            };
            if dependency.url.is_empty() {
                if slug_installed(&info, &installed_ids) {
                    continue;
                }
                plan.problem = Some(t!(
                    "{title} necesita {name}, y su autor no deja bajarlo fuera de CurseForge. Bájalo de su página, suéltalo en la pestaña Contenido y vuelve a instalar {title}.",
                    "{title} needs {name}, and its author does not allow downloading it outside CurseForge. Download it from its page, drop it on the Content tab and install {title} again."
                ));
                plan.manual_url = Some(file_page(&info, numeric(&dependency.id)?));
                return Ok(plan);
            }
            // El .jar dice qué mod es: si ya hay uno con ese id (de Modrinth o a mano), no se repite.
            let jar = candidates.join(format!("{}.jar", dependency.sha1));
            if !jar.is_file() {
                download::download_all(
                    vec![FileDownload { url: dependency.url.clone(), path: jar.clone(), sha1: Some(dependency.sha1.clone()), size: Some(dependency.size) }],
                    |_, _, _, _| {},
                )
                .await?;
            }
            let ids: Vec<String> = std::fs::File::open(&jar).map(compat::read_jar).unwrap_or_default().into_iter().flat_map(|m| std::iter::once(m.id).chain(m.provides)).collect();
            if !ids.is_empty() && ids.iter().any(|i| installed_ids.contains(i)) {
                continue;
            }
            plan.files.push(planned(&dependency, kind, &name, icon(&info), true, None));
            pending.push(dependency);
        }
    }
    let optional: Vec<u64> = optional.into_iter().filter(|id| !seen.contains(&cf(*id)) && !already.iter().any(|(_, _, e)| e.project_id == *id)).collect();
    if !optional.is_empty() {
        let infos = mods(&optional).await;
        plan.optional = optional.iter().filter_map(|id| infos.get(id)).map(summary).collect();
    }
    Ok(plan)
}

/// Los ids de mod de los .jar de la instancia (lo que declaran sus fabric.mod.json o mods.toml).
async fn installed_mod_ids(instance: &Instance) -> HashSet<String> {
    modrinth::identify(instance).await.into_iter().filter(|f| f.kind == "mods").flat_map(|f| f.mods.into_iter().flat_map(|m| std::iter::once(m.id).chain(m.provides))).collect()
}

/// También al elegir un archivo a mano se comprueban su proyecto, Minecraft y loader.
fn compatible_file(version: &Version, project: &str, loaders: &[&str], minecraft: &str) -> bool {
    version.project_id == project && modrinth::version_fits(version, loaders, minecraft)
}

/// Para lo que no se puede bajar: ¿hay un mod instalado con el id de su página («fabric-api», «jei»)?
fn slug_installed(info: &Value, installed_ids: &HashSet<String>) -> bool {
    let slug = text(info, "slug").to_lowercase();
    !slug.is_empty() && (installed_ids.contains(&slug) || installed_ids.contains(&slug.replace('-', "_")))
}

// ================= Instalar

/// Baja los archivos del plan (del CDN, comprobando el SHA-1), manda a la papelera los que reemplazan y los
/// anota en `.lanzador/curseforge.json`.
pub async fn install(instance: &Instance, files: &[PlannedFile], progress: impl Fn(usize, usize, u64, u64) + Send + Sync + 'static) -> Result<(), String> {
    let mut downloads = Vec::new();
    let mut targets = Vec::new();
    for file in files {
        let kind = ContentType::parse(&file.kind).ok_or(t!("Categoría desconocida.", "Unknown category."))?;
        let extension = if kind == ContentType::Mod { ".jar" } else { ".zip" };
        let safe_name = !file.filename.is_empty() && !file.filename.contains(['/', '\\', ':']) && !file.filename.starts_with('.') && file.filename.to_lowercase().ends_with(extension);
        if !is_curseforge(&file.project_id) || !cdn_url(&file.url) || !safe_name || file.sha1.len() != 40 {
            return Err(t!("El archivo {} no es válido.", "The file {} is not valid.", file.filename));
        }
        let folder = crate::content::folder(instance, kind.kind()).ok_or(t!("Categoría desconocida.", "Unknown category."))?;
        let disabled = file.replaces.as_deref().is_some_and(|r| r.ends_with(".disabled"));
        let name = if disabled { format!("{}.disabled", file.filename) } else { file.filename.clone() };
        let path = folder.join(&name);
        // Los que se bajaron al revisar las dependencias se copian en lugar de bajarse otra vez.
        let cached = modrinth::candidates_dir(instance).join(format!("{}.jar", file.sha1));
        if cached.is_file() && !path.exists() {
            let _ = std::fs::create_dir_all(&folder);
            let _ = std::fs::copy(&cached, &path);
        }
        downloads.push(FileDownload { url: file.url.clone(), path, sha1: Some(file.sha1.clone()), size: Some(file.size) });
        targets.push((file, kind, folder, name));
    }
    download::download_all(downloads, progress).await?;
    let _ = std::fs::remove_dir_all(modrinth::candidates_dir(instance));
    let mut index = load_index(instance);
    for (file, kind, folder, name) in targets {
        if let Some(old) = file.replaces.as_deref().filter(|old| *old != name && !old.contains(['/', '\\'])) {
            let _ = trash::delete(folder.join(old));
            crate::integridad::forget(instance, &format!("{}/{old}", kind.kind()));
            index.files.remove(&format!("{}/{}", kind.kind(), old.strip_suffix(".disabled").unwrap_or(old)));
        }
        index.files.insert(
            format!("{}/{}", kind.kind(), file.filename),
            IndexEntry { project_id: numeric(&file.project_id)?, file_id: numeric(&file.version_id)?, sha1: file.sha1.clone() },
        );
    }
    save_index(instance, &index);
    Ok(())
}

// ================= Modpacks

/// Un modpack de CurseForge (el .zip con manifest.json que sale de «Exportar» en la app de CurseForge).
#[derive(Debug, Clone)]
pub struct Pack {
    pub name: String,
    pub version: String,
    pub minecraft: String,
    /// ("Forge", "47.4.0"); None en vanilla.
    pub loader: Option<(String, String)>,
    /// (proyecto, archivo) de lo que se baja.
    pub files: Vec<(u64, u64)>,
    /// La carpeta del zip que se copia tal cual en la instancia («overrides»).
    pub overrides: String,
}

/// Lo que hay que bajar a mano porque su autor no deja bajarlo fuera de CurseForge.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Manual {
    /// Nombre del proyecto («Just Enough Items»).
    pub name: String,
    pub file_name: String,
    /// La página del archivo en curseforge.com.
    pub url: String,
    /// La carpeta de la instancia donde va («mods»).
    pub folder: String,
}

/// Lee el manifest.json de un modpack de CurseForge. None si el zip no lo es.
pub fn read_modpack(zip_path: &Path) -> Result<Option<Pack>, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| t!("No se pudo abrir el archivo: {e}", "Could not open the file: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| t!("El archivo no es un zip válido.", "The file is not a valid zip."))?;
    let Ok(mut entry) = zip.by_name(MANIFEST) else { return Ok(None) };
    let mut text_content = String::new();
    entry.read_to_string(&mut text_content).map_err(|e| e.to_string())?;
    let Ok(manifest) = serde_json::from_str::<Value>(text_content.trim_start_matches('\u{feff}')) else { return Ok(None) };
    if manifest["manifestType"] != "minecraftModpack" {
        return Ok(None);
    }
    parse_manifest(&manifest).map(Some)
}

fn parse_manifest(manifest: &Value) -> Result<Pack, String> {
    let minecraft = manifest["minecraft"]["version"].as_str().filter(|v| !v.is_empty()).ok_or(t!("El modpack no dice qué versión de Minecraft usa.", "The modpack does not say which Minecraft version it uses."))?.to_string();
    let loaders = manifest["minecraft"]["modLoaders"].as_array().cloned().unwrap_or_default();
    let primary = loaders.iter().find(|l| l["primary"] == true).or(loaders.first());
    let loader = match primary.and_then(|l| l["id"].as_str()) {
        Some(id) => Some(parse_loader(id, &minecraft).ok_or_else(|| t!("Este modpack usa un loader que Parabellum no conoce ({id}).", "This modpack uses a loader Parabellum does not know ({id})."))?),
        None => None,
    };
    let files = manifest["files"]
        .as_array()
        .into_iter()
        .flatten()
        // Lo que no es obligatorio sale desactivado en la app de CurseForge: aquí no se baja.
        .filter(|f| f["required"] != false)
        .filter_map(|f| Some((f["projectID"].as_u64()?, f["fileID"].as_u64()?)))
        .collect();
    let overrides = manifest["overrides"].as_str().filter(|o| crate::mrpack::safe_relative(o).is_some()).unwrap_or("overrides").to_string();
    Ok(Pack {
        name: manifest["name"].as_str().filter(|n| !n.trim().is_empty()).unwrap_or("Modpack").trim().to_string(),
        version: text(manifest, "version"),
        minecraft,
        loader,
        files,
        overrides,
    })
}

/// «forge-47.4.0», «neoforge-21.1.77», «fabric-0.16.10» o «quilt-0.27.1» -> ("Forge", "47.4.0")… El NeoForge de
/// 1.20.1 a veces trae la versión de Minecraft delante («neoforge-1.20.1-47.1.106»).
fn parse_loader(id: &str, minecraft: &str) -> Option<(String, String)> {
    let (name, version) = id.split_once('-')?;
    let version = version.strip_prefix(&format!("{minecraft}-")).unwrap_or(version);
    let name = match name.to_lowercase().as_str() {
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        _ => return None,
    };
    (!version.is_empty()).then(|| (name.to_string(), version.to_string()))
}

/// Un archivo del modpack ya resuelto: dónde va y de dónde bajarlo.
struct Resolved {
    project_id: u64,
    file_id: u64,
    path: String,
    url: String,
    sha1: String,
    size: u64,
}

/// Pregunta a CurseForge por los archivos del modpack: los que se bajan del CDN y los que hay que bajar a
/// mano. La carpeta de cada uno sale de su clase (mods, resource packs o shaders).
async fn resolve(pack: &Pack) -> Result<(Vec<Resolved>, Vec<Manual>), String> {
    let file_ids: Vec<u64> = pack.files.iter().map(|(_, f)| *f).collect();
    let mut files: HashMap<u64, Value> = HashMap::new();
    for chunk in file_ids.chunks(500) {
        let list = post("/v1/mods/files", &json!({ "fileIds": chunk })).await?;
        for f in list.as_array().into_iter().flatten() {
            if let Some(id) = f["id"].as_u64() {
                files.insert(id, f.clone());
            }
        }
    }
    let project_ids: Vec<u64> = pack.files.iter().map(|(p, _)| *p).collect();
    let projects = mods(&project_ids).await;
    let mut downloads = Vec::new();
    let mut manual = Vec::new();
    let mut missing = Vec::new();
    for (project, file_id) in &pack.files {
        let Some(f) = files.get(file_id) else {
            missing.push(projects.get(project).map(|m| text(m, "name")).unwrap_or_else(|| format!("{project}/{file_id}")));
            continue;
        };
        let info = projects.get(project).cloned().unwrap_or(Value::Null);
        let folder = folder_of(info["classId"].as_u64().unwrap_or(CLASS_MODS));
        let name = text(f, "fileName");
        if name.is_empty() || name.contains(['/', '\\', ':']) || name.starts_with('.') {
            return Err(t!("El modpack trae un archivo con un nombre no permitido: {name}", "The modpack contains a file with a name that is not allowed: {name}"));
        }
        let sha1 = f["hashes"].as_array().into_iter().flatten().find(|h| h["algo"] == 1).map(|h| text(h, "value")).filter(|h| h.len() == 40);
        match (f["downloadUrl"].as_str().filter(|u| cdn_url(u)), sha1) {
            (Some(url), Some(sha1)) => downloads.push(Resolved { project_id: *project, file_id: *file_id, path: format!("{folder}/{name}"), url: url.to_string(), sha1, size: count(&f["fileLength"]) }),
            _ => manual.push(Manual {
                name: Some(text(&info, "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| name.clone()),
                file_name: name,
                url: file_page(&info, *file_id),
                folder: folder.into(),
            }),
        }
    }
    if !missing.is_empty() {
        return Err(t!(
            "CurseForge ya no tiene {} archivos de este modpack: {}",
            "CurseForge no longer has {} files of this modpack: {}",
            missing.len(),
            missing.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    Ok((downloads, manual))
}

/// Baja lo del modpack (comprobando el SHA-1), copia sus overrides y anota lo que trae para revisarlo antes
/// de jugar. Devuelve lo que hay que bajar a mano.
pub async fn install_modpack_files(pack: &Pack, zip_path: &Path, game_dir: &Path, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<Vec<Manual>, String> {
    progress(t!("Preguntando a CurseForge por los {} archivos del modpack…", "Asking CurseForge about the {} files of the modpack…", pack.files.len()), 0.0);
    let (resolved, manual) = resolve(pack).await?;
    let downloads: Vec<FileDownload> = resolved
        .iter()
        .map(|r| FileDownload { url: r.url.clone(), path: game_dir.join(&r.path), sha1: Some(r.sha1.clone()), size: Some(r.size) })
        .collect();
    let report = progress.clone();
    download::download_all(downloads, move |count, total, bytes, total_bytes| {
        report(
            t!("Descargando mods… {count} de {total} ({:.0} de {:.0} MB)", "Downloading mods… {count} of {total} ({:.0} of {:.0} MB)", bytes as f64 / 1048576.0, total_bytes as f64 / 1048576.0),
            0.05 + 0.85 * bytes as f64 / total_bytes.max(1) as f64,
        );
    })
    .await?;
    let (zip_copy, game_copy, overrides) = (zip_path.to_path_buf(), game_dir.to_path_buf(), pack.overrides.clone());
    let report = progress.clone();
    let copied = tauri::async_runtime::spawn_blocking(move || copy_overrides(&zip_copy, &game_copy, &overrides, report)).await.map_err(|e| e.to_string())??;
    // Lo que trae el modpack, para comprobar antes de jugar que siga todo. Lo de bajar a mano va sin
    // enlace: si falta, el aviso pide bajarlo.
    let mut tracked: Vec<crate::integridad::Tracked> =
        resolved.iter().map(|r| crate::integridad::Tracked { path: r.path.clone(), sha1: Some(r.sha1.clone()), crc32: None, size: Some(r.size), url: Some(r.url.clone()) }).collect();
    tracked.extend(manual.iter().map(|m| crate::integridad::Tracked { path: format!("{}/{}", m.folder, m.file_name), sha1: None, crc32: None, size: None, url: None }));
    tracked.extend(copied);
    crate::integridad::save(game_dir, tracked.into_iter().filter(|t| crate::integridad::is_tracked(&t.path)).collect());
    // Explorar también reconoce como instalados los mods que llegaron dentro del modpack.
    let index = Index { files: resolved.iter().map(|r| (r.path.clone(), IndexEntry { project_id: r.project_id, file_id: r.file_id, sha1: r.sha1.clone() })).collect() };
    save_index_in(game_dir, &index);
    Ok(manual)
}

/// Copia `overrides/` del zip en la carpeta del juego. Devuelve lo que copió que se vigila (con su CRC32).
fn copy_overrides(zip_path: &Path, game_dir: &Path, prefix: &str, progress: impl Fn(String, f64)) -> Result<Vec<crate::integridad::Tracked>, String> {
    progress(t!("Copiando la configuración del modpack…", "Copying the modpack configuration…"), 0.92);
    let mut zip = zip::ZipArchive::new(std::fs::File::open(zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut copied = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else { continue };
        let Ok(relative) = name.strip_prefix(prefix) else { continue };
        if entry.is_dir() || relative.as_os_str().is_empty() {
            continue;
        }
        let Some(safe) = crate::mrpack::safe_relative(&relative.to_string_lossy()) else { continue };
        let target = game_dir.join(&safe);
        std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| t!("El modpack está dañado ({}): {e}", "The modpack is damaged ({}): {e}", name.display()))?;
        let path = relative.to_string_lossy().replace('\\', "/");
        copied.push(crate::integridad::Tracked { path, sha1: None, crc32: Some(entry.crc32()), size: Some(entry.size()), url: None });
    }
    Ok(copied)
}

/// Un modpack de CurseForge bajado a una carpeta temporal, listo para importar.
pub struct ModpackDownload {
    pub file: PathBuf,
    pub title: String,
    pub icon_url: Option<String>,
    pub project_id: String,
    pub version_id: String,
    pub version_number: String,
}

/// Baja el .zip de un modpack de CurseForge (la versión pedida o la estable más nueva).
pub async fn download_modpack(project_id: &str, version_id: Option<&str>, temp_dir: &Path) -> Result<ModpackDownload, String> {
    let project = numeric(project_id)?;
    let info = mods(&[project]).await.remove(&project).ok_or_else(|| t!("CurseForge no tiene ese proyecto.", "CurseForge does not have that project."))?;
    if info["classId"].as_u64() != Some(CLASS_MODPACKS) {
        return Err(t!("Ese proyecto no es un modpack.", "That project is not a modpack."));
    }
    let title = text(&info, "name");
    let chosen = match version_id {
        Some(id) => file(project, numeric(id)?).await?,
        None => modrinth::best(&project_versions(project_id, &[], &[]).await?)
            .cloned()
            .ok_or(t!("{title} no tiene versiones que Parabellum pueda instalar.", "{title} has no versions Parabellum can install."))?,
    };
    if chosen.url.is_empty() {
        return Err(t!(
            "Su autor no deja bajar {title} fuera de CurseForge. Bájalo de su página y suéltalo en la ventana de Parabellum.",
            "Its author does not allow downloading {title} outside CurseForge. Download it from its page and drop it on the Parabellum window."
        ));
    }
    if !chosen.filename.to_lowercase().ends_with(".zip") || chosen.sha1.len() != 40 {
        return Err(t!("Esa versión no trae el .zip del modpack.", "That version has no modpack .zip."));
    }
    let file = temp_dir.join(format!("{}.zip", chosen.sha1));
    download::download_all(vec![FileDownload { url: chosen.url.clone(), path: file.clone(), sha1: Some(chosen.sha1.clone()), size: Some(chosen.size) }], |_, _, _, _| {}).await?;
    Ok(ModpackDownload {
        file,
        title,
        icon_url: info["logo"]["thumbnailUrl"].as_str().filter(|u| u.starts_with("https://")).map(String::from),
        project_id: cf(project),
        version_id: chosen.id,
        version_number: chosen.version_number,
    })
}

/// El icono de un proyecto (solo del CDN de imágenes de CurseForge).
pub async fn download_icon(url: &str) -> Option<Vec<u8>> {
    if !url.starts_with("https://media.forgecdn.net/") {
        return None;
    }
    let bytes = download::client().get(url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    (bytes.len() < 2_000_000).then(|| bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loaders_del_manifiesto() {
        assert_eq!(parse_loader("forge-47.4.0", "1.20.1"), Some(("Forge".into(), "47.4.0".into())));
        assert_eq!(parse_loader("neoforge-21.1.77", "1.21.1"), Some(("NeoForge".into(), "21.1.77".into())));
        assert_eq!(parse_loader("neoforge-1.20.1-47.1.106", "1.20.1"), Some(("NeoForge".into(), "47.1.106".into())));
        assert_eq!(parse_loader("fabric-0.16.10", "1.21.1"), Some(("Fabric".into(), "0.16.10".into())));
        assert_eq!(parse_loader("quilt-0.27.1", "1.21.1"), Some(("Quilt".into(), "0.27.1".into())));
        assert_eq!(parse_loader("liteloader-1.12", "1.12.2"), None);
        assert_eq!(parse_loader("forge", "1.20.1"), None);
    }

    #[test]
    fn manifiesto_de_curseforge() {
        let manifest = json!({
            "minecraft": { "version": "1.20.1", "modLoaders": [{ "id": "forge-47.4.0", "primary": true }] },
            "manifestType": "minecraftModpack", "manifestVersion": 1, "name": " Prueba ", "version": "1.2", "author": "Juan",
            "files": [
                { "projectID": 238222, "fileID": 5101366, "required": true },
                { "projectID": 1, "fileID": 2, "required": false },
                { "projectID": 3, "fileID": 4 }
            ],
            "overrides": "overrides"
        });
        let pack = parse_manifest(&manifest).unwrap();
        assert_eq!(pack.name, "Prueba");
        assert_eq!(pack.loader, Some(("Forge".into(), "47.4.0".into())));
        assert_eq!(pack.files, vec![(238222, 5101366), (3, 4)], "lo que no es obligatorio no se baja");
        assert_eq!(pack.overrides, "overrides");
        // Una carpeta de overrides que sale del zip no se usa.
        let mut bad = manifest.clone();
        bad["overrides"] = json!("../fuera");
        assert_eq!(parse_manifest(&bad).unwrap().overrides, "overrides");
        let mut vanilla = manifest;
        vanilla["minecraft"]["modLoaders"] = json!([]);
        assert_eq!(parse_manifest(&vanilla).unwrap().loader, None);
    }

    #[test]
    fn archivo_como_version() {
        let file = json!({
            "id": 5101366, "modId": 238222, "displayName": "jei-1.20.1-forge-15.2.0.27.jar", "fileName": "jei-1.20.1-forge-15.2.0.27.jar",
            "releaseType": 2, "fileDate": "2024-03-01T00:00:00Z", "fileLength": 1234567, "downloadCount": 1000.0,
            "downloadUrl": "https://edge.forgecdn.net/files/5101/366/jei-1.20.1-forge-15.2.0.27.jar",
            "gameVersions": ["1.20.1", "Forge", "NeoForge", "Client"],
            "hashes": [{ "value": "a".repeat(40), "algo": 1 }, { "value": "b".repeat(32), "algo": 2 }],
            "dependencies": [{ "modId": 306612, "relationType": 3 }, { "modId": 1, "relationType": 2 }, { "modId": 2, "relationType": 4 }]
        });
        let version = version(&file).unwrap();
        assert_eq!(version.id, "cf:5101366");
        assert_eq!(version.project_id, "cf:238222");
        assert_eq!(version.version_type, "beta");
        assert_eq!(version.loaders, vec!["forge", "neoforge"]);
        assert_eq!(version.game_versions, vec!["1.20.1"]);
        assert_eq!(version.sha1, "a".repeat(40));
        assert_eq!(version.size, 1234567);
        assert!(compatible_file(&version, "cf:238222", &["forge"], "1.20.1"));
        assert!(!compatible_file(&version, "cf:238222", &["fabric"], "1.20.1"));
        assert!(!compatible_file(&version, "cf:238222", &["forge"], "1.21.1"));
        assert!(!compatible_file(&version, "cf:306612", &["forge"], "1.20.1"));
        assert_eq!(version.dependencies.iter().map(|d| (d.project_id.clone().unwrap(), d.kind.clone())).collect::<Vec<_>>(), vec![("cf:306612".to_string(), "required".to_string()), ("cf:1".to_string(), "optional".to_string())]);
        // Sin enlace (o de otro sitio): no se baja, se abre su página.
        let mut blocked = file.clone();
        blocked["downloadUrl"] = Value::Null;
        assert!(super::version(&blocked).unwrap().url.is_empty());
        blocked["downloadUrl"] = json!("https://ejemplo.com/jei.jar");
        assert!(super::version(&blocked).unwrap().url.is_empty());
    }

    #[test]
    fn ids_de_curseforge() {
        assert!(is_curseforge("cf:238222"));
        assert!(!is_curseforge("AANobbMI"));
        assert_eq!(numeric("cf:238222"), Ok(238222));
        assert!(numeric("cf:../x").is_err());
        assert!(numeric("cf:").is_err());
    }

    #[test]
    fn reconoce_instalados_solo_si_conservan_su_huella() {
        let root = std::env::temp_dir().join(format!("parabellum-cf-index-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let folder = root.join("mods");
        std::fs::create_dir_all(&folder).unwrap();
        let jar = folder.join("prueba.jar");
        std::fs::write(&jar, b"mod original").unwrap();
        let index = Index { files: HashMap::from([("mods/prueba.jar".into(), IndexEntry {
            project_id: 42, file_id: 84, sha1: crate::integridad::sha1_file(&jar).unwrap(),
        })]) };
        save_index_in(&root, &index);
        let instance = Instance { game_dir: root.clone(), ..Default::default() };
        assert_eq!(installed(&instance), (vec!["cf:42".into()], vec!["cf:84".into()]));
        let disabled = folder.join("prueba.jar.disabled");
        std::fs::rename(jar, &disabled).unwrap();
        assert_eq!(installed(&instance).0, vec!["cf:42"]);
        std::fs::write(disabled, b"otro mod").unwrap();
        assert!(installed(&instance).0.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn overrides_no_escriben_fuera_de_la_instancia() {
        let root = std::env::temp_dir().join(format!("parabellum-cf-overrides-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let archive = root.join("pack.zip");
        let mut zip = crate::packs::ZipOut::create(&archive).unwrap();
        zip.add_bytes("overrides/config/prueba.txt", b"configuracion").unwrap();
        zip.add_bytes("overrides/../fuera.txt", b"no").unwrap();
        zip.add_bytes("overrides/config/archivo:oculto", b"no").unwrap();
        zip.finish().unwrap();
        let game = root.join("juego");
        let copied = copy_overrides(&archive, &game, "overrides", |_, _| {}).unwrap();
        assert_eq!(std::fs::read(game.join("config/prueba.txt")).unwrap(), b"configuracion");
        assert_eq!(copied.len(), 1);
        assert!(!root.join("fuera.txt").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
