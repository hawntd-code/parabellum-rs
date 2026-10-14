//! Minecraft "vanilla" desde los servidores oficiales de Mojang: lista de versiones e instalación
//! (JSON, juego, bibliotecas, assets, configuración de registros y el Java que pide la versión).

use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use sha1::{Digest, Sha1};

use crate::download::{self, FileDownload};
use crate::java;
use crate::paths::Paths;
use crate::version::{rules_allow, Features};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const JAVA_RUNTIMES_URL: &str =
    "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const ASSETS_URL: &str = "https://resources.download.minecraft.net/";
/// Antes de 1.7.10 los assets usan un formato antiguo ("legacy") que no preparamos.
const OLDEST_SUPPORTED: &str = "2014-06-01";

pub fn unavailable() -> String {
    t!(
        "No se puede conectar con los servidores de Mojang (en algunas redes, como la de la escuela, están bloqueados).",
        "Cannot connect to Mojang's servers (on some networks, like the school's, they are blocked)."
    )
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub release_time: String,
    #[serde(skip)]
    pub url: String,
    #[serde(skip)]
    pub sha1: Option<String>,
}

pub async fn list() -> Result<Vec<Release>, String> {
    let text = download::get_text(MANIFEST_URL).await.map_err(|_| unavailable())?;
    let manifest: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(manifest["versions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some(Release {
                id: v["id"].as_str()?.into(),
                kind: v["type"].as_str()?.into(),
                release_time: v["releaseTime"].as_str()?.into(),
                url: v["url"].as_str()?.into(),
                sha1: v["sha1"].as_str().map(String::from),
            })
        })
        .filter(|r| (r.kind == "release" || r.kind == "snapshot") && r.release_time.as_str() >= OLDEST_SUPPORTED)
        .collect())
}

/// Progreso de la instalación: (texto, fracción de 0 a 1).
pub async fn install(release: &Release, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + Clone + 'static) -> Result<(), String> {
    progress(t!("Preparando Minecraft {}…", "Preparing Minecraft {}…", release.id), 0.0);
    let version_text = download::get_text(&release.url).await.map_err(|_| unavailable())?;
    if let Some(expected) = &release.sha1 {
        if !hex::encode(Sha1::digest(version_text.as_bytes())).eq_ignore_ascii_case(expected) {
            return Err(t!("El JSON de la versión {} llegó dañado. Vuelve a intentarlo.", "The JSON of version {} arrived damaged. Try again.", release.id));
        }
    }
    let version: Value = serde_json::from_str(&version_text).map_err(|e| e.to_string())?;
    let mut files = Vec::new();

    // Juego
    let client = version.pointer("/downloads/client").ok_or(t!("Esta versión no tiene cliente descargable.", "This version has no downloadable client."))?;
    files.extend(FileDownload::from_json(client, paths.version_jar(&release.id)));

    // Java que pide la versión (las antiguas no lo dicen: Java 8, "jre-legacy")
    let component = version.pointer("/javaVersion/component").and_then(Value::as_str).unwrap_or("jre-legacy").to_string();
    let major = version.pointer("/javaVersion/majorVersion").and_then(Value::as_u64).unwrap_or(8) as u32;

    // Bibliotecas de este sistema (y los nativos del formato antiguo). Si aquí hacen falta otras (Mac
    // con chip Apple, 1.18), se bajan también, sin quitar las de Mojang: así un paquete exportado de
    // aquí sirve en otro sistema.
    let no_features = Features::new();
    let original = version["libraries"].as_array().cloned().unwrap_or_default();
    let mut libraries = original.clone();
    for library in crate::plataforma::adapt_libraries(original, major) {
        if !libraries.contains(&library) {
            libraries.push(library);
        }
    }
    for library in &libraries {
        if !rules_allow(library.get("rules"), &no_features) {
            continue;
        }
        if let Some(artifact) = library.pointer("/downloads/artifact") {
            if let Some(path) = artifact["path"].as_str() {
                files.extend(FileDownload::from_json(artifact, paths.libraries.join(path)));
            }
        }
        if let Some(classifier) = library.pointer(&format!("/natives/{}", crate::plataforma::MOJANG_OS)).and_then(Value::as_str) {
            let classifier = classifier.replace("${arch}", "64");
            if let Some(native) = library.pointer(&format!("/downloads/classifiers/{classifier}")) {
                if let Some(path) = native["path"].as_str() {
                    files.extend(FileDownload::from_json(native, paths.libraries.join(path)));
                }
            }
        }
    }

    // Configuración de registros (log4j)
    if let Some(logging) = version.pointer("/logging/client/file") {
        if let Some(id) = logging["id"].as_str() {
            files.extend(FileDownload::from_json(logging, paths.assets.join("log_configs").join(id)));
        }
    }

    // Assets (primero el índice, que dice qué archivos hacen falta) y la lista de archivos de Java: las
    // dos a la vez (una después de otra se iban unos segundos esperando respuestas).
    let assets = async {
        let Some(index) = version.get("assetIndex") else { return Ok(Vec::new()) };
        progress(t!("Leyendo la lista de sonidos y texturas…", "Reading the list of sounds and textures…"), 0.01);
        let id = index["id"].as_str().ok_or(t!("Índice de assets sin id.", "Asset index without id."))?;
        let index_file = paths.assets.join("indexes").join(format!("{id}.json"));
        let index_download = FileDownload::from_json(index, index_file.clone()).ok_or(t!("Índice de assets sin URL.", "Asset index without URL."))?;
        download::download_all(vec![index_download], |_, _, _, _| {}).await?;
        let index_json: Value = serde_json::from_str(&std::fs::read_to_string(&index_file).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        let mut objects = Vec::new();
        for object in index_json["objects"].as_object().into_iter().flatten().map(|(_, o)| o) {
            let Some(hash) = object["hash"].as_str().filter(|h| h.len() > 2) else { continue };
            objects.push(FileDownload {
                url: format!("{ASSETS_URL}{}/{hash}", &hash[..2]),
                path: paths.assets.join("objects").join(&hash[..2]).join(hash),
                sha1: Some(hash.into()),
                size: object["size"].as_u64(),
            });
        }
        Ok::<_, String>(objects)
    };
    let runtime = async {
        let runtimes = java::discover(paths);
        if java::select(&runtimes, Some(&component), major).is_some_and(|r| r.major == major) {
            return Ok((Vec::new(), Vec::new()));
        }
        progress(t!("Preparando Java {major}…", "Preparing Java {major}…"), 0.015);
        java_files(&component, major, paths).await
    };
    let (assets, runtime) = futures::join!(assets, runtime);
    files.extend(assets?);
    let (java, java_programs) = runtime?;
    files.extend(java);

    let id = release.id.clone();
    let stage_progress = progress.clone();
    download::download_all(files, move |count, total, bytes, total_bytes| {
        let mb = 1048576.0;
        stage_progress(
            t!("Descargando Minecraft {id}… {count} de {total} archivos ({:.0} de {:.0} MB)", "Downloading Minecraft {id}… {count} of {total} files ({:.0} of {:.0} MB)", bytes as f64 / mb, total_bytes as f64 / mb),
            0.02 + 0.97 * bytes as f64 / total_bytes as f64,
        );
    })
    .await?;
    java_programs.iter().for_each(|program| crate::plataforma::make_executable(program));

    // El JSON va al final: si la descarga se corta, la versión no parece instalada.
    let json_file = paths.version_json(&release.id);
    std::fs::create_dir_all(json_file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&json_file, &version_text).map_err(|e| e.to_string())?;
    progress(t!("Minecraft {} listo", "Minecraft {} ready", release.id), 1.0);
    Ok(())
}

/// Que haya un Java de esa versión mayor en base/java: si falta, se baja el de Mojang que la tenga.
pub async fn ensure_java(major: u32, paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + 'static) -> Result<(), String> {
    if java::discover(paths).iter().any(|r| r.major == major) {
        return Ok(());
    }
    let platform = crate::plataforma::java_runtime_platform(major);
    let all: Value = serde_json::from_str(&download::get_text(JAVA_RUNTIMES_URL).await.map_err(|_| unavailable())?).map_err(|e| e.to_string())?;
    let component = all[platform]
        .as_object()
        .into_iter()
        .flatten()
        .find(|(_, list)| list.pointer("/0/version/name").and_then(Value::as_str).is_some_and(|v| java::major_of(v) == major))
        .map(|(name, _)| name.clone())
        .ok_or(t!("Mojang no ofrece Java {major}.", "Mojang does not offer Java {major}."))?;
    progress(t!("Preparando Java {major}…", "Preparing Java {major}…"), 0.0);
    let (files, programs) = java_files(&component, major, paths).await?;
    download::download_all(files, move |count, total, _, _| {
        progress(t!("Descargando Java {major}… {count} de {total} archivos", "Downloading Java {major}… {count} of {total} files"), count as f64 / total.max(1) as f64)
    })
    .await?;
    programs.iter().for_each(|program| crate::plataforma::make_executable(program));
    Ok(())
}

/// Archivos del Java de Mojang para este sistema (el mismo que usa el launcher oficial) y cuáles son
/// programas (java, jspawnhelper…): después de bajarlos hay que marcarlos como ejecutables. Si Mojang
/// no tiene ese componente para este sistema, sirve otro con la misma versión mayor (en Mac con chip
/// Apple, 1.18 pide «java-runtime-beta», que no hay, y «java-runtime-gamma» también es Java 17).
async fn java_files(component: &str, major: u32, paths: &Paths) -> Result<(Vec<FileDownload>, Vec<PathBuf>), String> {
    let platform = crate::plataforma::java_runtime_platform(major);
    let all: Value = serde_json::from_str(&download::get_text(JAVA_RUNTIMES_URL).await.map_err(|_| unavailable())?)
        .map_err(|e| e.to_string())?;
    let offered = |name: &str| all.pointer(&format!("/{platform}/{name}/0/version/name")).and_then(Value::as_str).map(java::major_of);
    let component = if offered(component).is_some() {
        component.to_string()
    } else {
        all[platform]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, _)| name.clone())
            .find(|name| offered(name) == Some(major))
            .ok_or(t!("Mojang no ofrece {component} para {platform}.", "Mojang does not offer {component} for {platform}."))?
    };
    let listing = all.pointer(&format!("/{platform}/{component}/0/manifest"));
    let manifest_url = listing
        .and_then(|l| l["url"].as_str())
        .ok_or(t!("Mojang no ofrece {component} para {platform}.", "Mojang does not offer {component} for {platform}."))?;
    let manifest_text = download::get_text(manifest_url).await.map_err(|_| unavailable())?;
    // Como el JSON de la versión: de esta lista sale qué se baja y qué se puede ejecutar.
    if let Some(expected) = listing.and_then(|l| l["sha1"].as_str()) {
        if !hex::encode(Sha1::digest(manifest_text.as_bytes())).eq_ignore_ascii_case(expected) {
            return Err(t!("La lista de archivos de Java llegó dañada. Vuelve a intentarlo.", "The list of Java files arrived damaged. Try again."));
        }
    }
    let manifest: Value = serde_json::from_str(&manifest_text).map_err(|e| e.to_string())?;
    let home = paths.java.join(&component);
    let mut downloads = Vec::new();
    let mut programs = Vec::new();
    for (relative, entry) in manifest["files"].as_object().into_iter().flatten().filter(|(_, entry)| entry["type"] == "file") {
        // Solo dentro de la carpeta de este Java (sin «..» ni rutas absolutas).
        let Some(relative) = crate::mrpack::safe_relative(relative) else { continue };
        let Some(download) = entry.pointer("/downloads/raw").and_then(|raw| FileDownload::from_json(raw, home.join(&relative))) else { continue };
        // Solo se marcan como programa los que se bajan (y se comprueban) aquí.
        if entry["executable"] == true {
            programs.push(download.path.clone());
        }
        downloads.push(download);
    }
    Ok((downloads, programs))
}

/// Versiones vanilla completas que ya están en la carpeta base (sin heredar de otra y con su .jar).
pub fn installed_versions(paths: &Paths) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(&paths.versions) else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            let json: Value = serde_json::from_str(&std::fs::read_to_string(paths.version_json(&id)).ok()?).ok()?;
            (json.get("inheritsFrom").is_none() && paths.version_jar(&id).exists()).then_some(id)
        })
        .collect()
}
