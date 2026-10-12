//! Forge y NeoForge con su instalador oficial. El instalador trae install_profile.json (bibliotecas y
//! "processors": herramientas de Java que preparan el juego parcheado) y version.json (la versión
//! que hereda de la vanilla). Parabellum hace lo mismo que el instalador para el lado cliente:
//! descarga, ejecuta los processors y escribe la versión al final.

use serde_json::Value;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use crate::plataforma::CommandExt as _;
use std::process::{Command, Stdio};

use crate::download::{self, FileDownload};
use crate::java;
use crate::loaders::{compare_versions, Loader, LoaderVersion};
use crate::paths::Paths;
use crate::version::{rules_allow, Features, Maven, ResolvedVersion};

const FORGE_MAVEN: &str = "https://maven.minecraftforge.net";
const NEOFORGE_MAVEN: &str = "https://maven.neoforged.net/releases";
const NEOFORGE_VERSIONS: &str = "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge";

fn unavailable(loader: Loader) -> String {
    t!("No se puede conectar con los servidores de {} (en algunas redes, como la de la escuela, están bloqueados).", "Cannot connect to the {} servers (on some networks, like the school's, they are blocked).", loader.name())
}

/// (versión de Minecraft, versión del loader) de todo lo publicado.
async fn published(loader: Loader) -> Result<Vec<(String, String)>, String> {
    match loader {
        Loader::NeoForge => {
            let text = download::get_text(NEOFORGE_VERSIONS).await.map_err(|_| unavailable(loader))?;
            let json: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            Ok(json["versions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(|v| Some((neoforge_minecraft(v)?, v.to_string())))
                .collect())
        }
        _ => {
            let url = format!("{FORGE_MAVEN}/net/minecraftforge/forge/maven-metadata.xml");
            let xml = download::get_text(&url).await.map_err(|_| unavailable(loader))?;
            Ok(xml
                .split("<version>")
                .skip(1)
                .filter_map(|chunk| chunk.split_once("</version>"))
                .filter_map(|(version, _)| version.split_once('-'))
                .map(|(minecraft, forge)| (minecraft.to_string(), forge.to_string()))
                .collect())
        }
    }
}

/// NeoForge numera según Minecraft: "21.1.77" es de 1.21.1, "21.0.167" de 1.21 y, desde que Minecraft
/// usa años, "26.1.2.114" es de 26.1.2 y "26.3.0.58-beta" de 26.3.
fn neoforge_minecraft(version: &str) -> Option<String> {
    let main = version.split('-').next()?;
    let parts: Vec<u32> = main.split('.').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    match parts.as_slice() {
        [major, minor, _] if (20..26).contains(major) => Some(if *minor == 0 { format!("1.{major}") } else { format!("1.{major}.{minor}") }),
        [year, drop, hotfix, _] if *year >= 26 => Some(if *hotfix == 0 { format!("{year}.{drop}") } else { format!("{year}.{drop}.{hotfix}") }),
        _ => None,
    }
}

pub async fn game_versions(loader: Loader) -> Result<Vec<String>, String> {
    let mut games: Vec<String> = Vec::new();
    for (minecraft, _) in published(loader).await? {
        if !games.contains(&minecraft) {
            games.push(minecraft);
        }
    }
    Ok(games)
}

pub async fn loader_versions(loader: Loader, minecraft: &str) -> Result<Vec<LoaderVersion>, String> {
    let mut versions: Vec<LoaderVersion> = published(loader)
        .await?
        .into_iter()
        .filter(|(game, _)| game == minecraft)
        .map(|(_, version)| LoaderVersion { stable: !version.contains('-'), version })
        .collect();
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
    versions.dedup_by(|a, b| a.version == b.version);
    Ok(versions)
}

pub fn installer_url(loader: Loader, minecraft: &str, version: &str) -> String {
    match loader {
        Loader::NeoForge => format!("{NEOFORGE_MAVEN}/net/neoforged/neoforge/{version}/neoforge-{version}-installer.jar"),
        _ => format!("{FORGE_MAVEN}/net/minecraftforge/forge/{minecraft}-{version}/forge-{minecraft}-{version}-installer.jar"),
    }
}

/// Instala Forge o NeoForge sobre la vanilla ya instalada. Devuelve el id de la versión creada.
pub async fn install(
    loader: Loader,
    minecraft: &str,
    version: &str,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + 'static,
) -> Result<String, String> {
    let name = loader.name();
    // Ya instalado (por Parabellum, por un paquete de Drive o por CurseForge): no se repite.
    if let Some(id) = installed(loader, minecraft, version, paths) {
        return Ok(id);
    }
    progress(t!("Descargando el instalador de {name} {version}…", "Downloading the {name} {version} installer…"), 0.0);
    let url = installer_url(loader, minecraft, version);
    let sha1 = match download::get_text(&format!("{url}.sha1")).await {
        Ok(text) => Some(text.trim().to_string()).filter(|s| s.len() == 40),
        Err(e) if e.is_status() => return Err(t!("{name} no tiene la versión {version} para Minecraft {minecraft}.", "{name} has no version {version} for Minecraft {minecraft}.")),
        Err(_) => return Err(unavailable(loader)),
    };
    let installer = paths.data.join("descargas").join(url.rsplit('/').next().unwrap_or("instalador.jar"));
    download::download_all(vec![FileDownload { url, path: installer.clone(), sha1, size: None }], |_, _, _, _| {}).await?;

    // Lo que hay que descargar sale del propio instalador.
    let (profile, version_json, files) = {
        let (installer, paths) = (installer.clone(), paths.clone());
        tokio::task::spawn_blocking(move || read_installer(loader, &installer, &paths)).await.map_err(|e| e.to_string())??
    };
    let progress = std::sync::Arc::new(progress);
    let stage = progress.clone();
    let label = format!("{name} {version}");
    download::download_all(files, move |count, total, _, _| {
        stage(t!("Descargando {label}… {count} de {total} archivos", "Downloading {label}… {count} of {total} files"), 0.05 + 0.45 * count as f64 / total.max(1) as f64);
    })
    .await?;

    let id = {
        let (installer, paths, minecraft, progress) = (installer.clone(), paths.clone(), minecraft.to_string(), progress.clone());
        tokio::task::spawn_blocking(move || {
            run_processors(loader, &profile, &minecraft, &installer, &paths, &*progress)?;
            write_version(&version_json, &paths)
        })
        .await
        .map_err(|e| e.to_string())??
    };
    let _ = std::fs::remove_file(&installer);
    Ok(id)
}

/// Id de la versión si ya está completa: su JSON, sus bibliotecas y lo que generan los processors.
fn installed(loader: Loader, minecraft: &str, version: &str, paths: &Paths) -> Option<String> {
    let features = Features::new();
    candidate_ids(loader, minecraft, version).into_iter().find(|id| {
        let Some(json) = std::fs::read_to_string(paths.version_json(id)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else {
            return false;
        };
        let libraries_present = json["libraries"].as_array().into_iter().flatten().filter(|l| rules_allow(l.get("rules"), &features)).all(|l| {
            l.pointer("/downloads/artifact/path")
                .and_then(Value::as_str)
                .map(String::from)
                .or_else(|| Maven::parse(l["name"].as_str()?).map(|m| m.relative_path()))
                .is_some_and(|relative| paths.libraries.join(relative).exists())
        });
        libraries_present && !generated_files(&json, paths).is_empty()
    })
}

/// install_profile.json, version.json y las bibliotecas que faltan (las que trae el instalador
/// dentro, en maven/, se copian directamente).
fn read_installer(loader: Loader, installer: &Path, paths: &Paths) -> Result<(Value, Value, Vec<FileDownload>), String> {
    let damaged = |e: &dyn std::fmt::Display| t!("El instalador de {} está dañado: {e}", "The {} installer is damaged: {e}", loader.name());
    let mut zip = zip::ZipArchive::new(std::fs::File::open(installer).map_err(|e| damaged(&e))?).map_err(|e| damaged(&e))?;
    let profile: Value = serde_json::from_str(&read_entry(&mut zip, "install_profile.json").ok_or_else(|| damaged(&t!("falta install_profile.json", "install_profile.json is missing")))?)
        .map_err(|e| damaged(&e))?;
    if profile.get("versionInfo").is_some() {
        return Err(t!(
            "Esta versión de {} es de las más antiguas y usa otro tipo de instalador que Parabellum no maneja. Elige una más nueva.", "This {} version is one of the oldest and uses another kind of installer that Parabellum does not handle. Choose a newer one.",
            loader.name()
        ));
    }
    let json_name = profile["json"].as_str().unwrap_or("/version.json").trim_start_matches('/');
    let version: Value =
        serde_json::from_str(&read_entry(&mut zip, json_name).ok_or_else(|| damaged(&t!("falta version.json", "version.json is missing")))?).map_err(|e| damaged(&e))?;

    let features = Features::new();
    let mut files = Vec::new();
    let libraries = profile["libraries"].as_array().into_iter().flatten().chain(version["libraries"].as_array().into_iter().flatten());
    for library in libraries.filter(|l| rules_allow(l.get("rules"), &features)) {
        let artifact = library.pointer("/downloads/artifact");
        let Some(relative) = artifact
            .and_then(|a| a["path"].as_str())
            .map(String::from)
            .or_else(|| Maven::parse(library["name"].as_str()?).map(|m| m.relative_path()))
        else {
            continue;
        };
        let target = paths.libraries.join(&relative);
        let url = match artifact {
            Some(artifact) => artifact["url"].as_str().unwrap_or("").to_string(),
            None => format!("{}/{relative}", library["url"].as_str().unwrap_or("https://libraries.minecraft.net").trim_end_matches('/')),
        };
        if url.is_empty() {
            // Va dentro del instalador; si no está, la crea un processor (p. ej. el juego parcheado).
            extract_entry(&mut zip, &format!("maven/{relative}"), &target)?;
        } else {
            files.push(FileDownload {
                url,
                path: target,
                sha1: artifact.and_then(|a| a["sha1"].as_str()).map(String::from),
                size: artifact.and_then(|a| a["size"].as_u64()),
            });
        }
    }
    Ok((profile, version, files))
}

fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
    let mut text = String::new();
    zip.by_name(name).ok()?.read_to_string(&mut text).ok()?;
    Some(text)
}

/// Copia un archivo del instalador si no está ya igual. Devuelve false si el instalador no lo trae.
fn extract_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str, target: &Path) -> Result<bool, String> {
    let Ok(mut entry) = zip.by_name(name) else { return Ok(false) };
    if target.metadata().is_ok_and(|m| m.len() == entry.size()) {
        return Ok(true);
    }
    std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
    let mut out = std::fs::File::create(target).map_err(|e| e.to_string())?;
    std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    Ok(true)
}

fn library(coordinate: &str, paths: &Paths) -> Result<PathBuf, String> {
    Maven::parse(coordinate).map(|m| paths.libraries.join(m.relative_path())).ok_or(t!("Biblioteca con nombre no válido: {coordinate}", "Library with an invalid name: {coordinate}"))
}

/// Sustituye {DATO} por su valor y [grupo:artefacto:versión] por la ruta de la biblioteca.
fn expand(text: &str, data: &HashMap<String, String>, paths: &Paths) -> Result<String, String> {
    if let Some(coordinate) = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) {
        return Ok(library(coordinate, paths)?.display().to_string());
    }
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(length) = rest[start..].find('}') else { break };
        let key = &rest[start + 1..start + length];
        out.push_str(&rest[..start]);
        out.push_str(data.get(key).ok_or(t!("El instalador pide un dato que no conoce: {key}.", "The installer asks for a value it does not know: {key}."))?);
        rest = &rest[start + length + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

fn main_class(jar: &Path) -> Result<String, String> {
    let file = std::fs::File::open(jar).map_err(|_| t!("Falta {}.", "{} is missing.", jar.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let manifest = read_entry(&mut zip, "META-INF/MANIFEST.MF").ok_or(t!("{} no tiene MANIFEST.", "{} has no MANIFEST.", jar.display()))?;
    // Las líneas largas del manifiesto siguen en la siguiente, empezando con un espacio.
    let joined = manifest.replace("\r\n", "\n").replace("\n ", "");
    joined
        .lines()
        .find_map(|l| l.strip_prefix("Main-Class:"))
        .map(|c| c.trim().to_string())
        .ok_or(t!("{} no dice su clase principal.", "{} does not state its main class.", jar.display()))
}

fn sha1_of(path: &Path) -> Option<String> {
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

/// Ejecuta los processors del lado cliente, como el instalador oficial. Se saltan los que ya
/// dejaron sus archivos (con el SHA-1 esperado), así reinstalar es rápido.
fn run_processors(
    loader: Loader,
    profile: &Value,
    minecraft: &str,
    installer: &Path,
    paths: &Paths,
    progress: &(dyn Fn(String, f64) + Send + Sync),
) -> Result<(), String> {
    let name = loader.name();
    let processors: Vec<&Value> = profile["processors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["sides"].as_array().is_none_or(|sides| sides.iter().any(|s| s == "client")))
        .collect();
    if processors.is_empty() {
        return Ok(());
    }

    // Java de la versión de Minecraft (ya instalado con la vanilla)
    let vanilla = ResolvedVersion::resolve(minecraft, paths)?;
    let runtimes = java::discover(paths);
    let runtime = java::select(&runtimes, vanilla.java_component.as_deref(), vanilla.java_major)
        .ok_or(t!("Falta Java {} para instalar {name}.", "Java {} is missing to install {name}.", vanilla.java_major))?;
    let java_exe = crate::plataforma::java_binary(&runtime.home, false);

    let temp = paths.data.join("temporal").join(format!("instalador-{}", name.to_lowercase()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(installer).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    let mut data: HashMap<String, String> = HashMap::from([
        ("SIDE".into(), "client".into()),
        ("MINECRAFT_JAR".into(), paths.version_jar(minecraft).display().to_string()),
        ("MINECRAFT_VERSION".into(), minecraft.to_string()),
        ("ROOT".into(), paths.base.display().to_string()),
        ("INSTALLER".into(), installer.display().to_string()),
        ("LIBRARY_DIR".into(), paths.libraries.display().to_string()),
    ]);
    for (key, value) in profile["data"].as_object().into_iter().flatten() {
        let Some(client) = value["client"].as_str() else { continue };
        let resolved = if let Some(literal) = client.strip_prefix('\'').and_then(|c| c.strip_suffix('\'')) {
            literal.to_string()
        } else if let Some(inner) = client.strip_prefix('/') {
            // Un archivo del instalador (p. ej. los parches binarios)
            let target = temp.join(inner);
            if !extract_entry(&mut zip, inner, &target)? {
                return Err(t!("Al instalador de {name} le falta {inner}.", "The {name} installer is missing {inner}."));
            }
            target.display().to_string()
        } else {
            expand(client, &HashMap::new(), paths)?
        };
        data.insert(key.clone(), resolved);
    }

    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(paths.logs.join(format!("instalador-{}.log", name.to_lowercase())))
        .map_err(|e| e.to_string())?;
    let total = processors.len();
    for (index, processor) in processors.into_iter().enumerate() {
        progress(t!("Preparando {name}… paso {} de {total}", "Preparing {name}… step {} of {total}", index + 1), 0.5 + 0.5 * index as f64 / total as f64);
        let jar = library(processor["jar"].as_str().ok_or_else(|| t!("El instalador trae un paso sin programa.", "The installer has a step without a program."))?, paths)?;
        let mut classpath = vec![jar.clone()];
        for entry in processor["classpath"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            classpath.push(library(entry, paths)?);
        }
        let arguments = processor["args"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|a| expand(a, &data, paths))
            .collect::<Result<Vec<_>, _>>()?;
        let outputs = processor["outputs"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(file, sha)| Ok((PathBuf::from(expand(file, &data, paths)?), expand(sha.as_str().unwrap_or(""), &data, paths)?)))
            .collect::<Result<Vec<(PathBuf, String)>, String>>()?;
        let done = |outputs: &[(PathBuf, String)]| outputs.iter().all(|(file, sha)| sha1_of(file).is_some_and(|s| s.eq_ignore_ascii_case(sha)));
        if !outputs.is_empty() && done(&outputs) {
            continue;
        }

        let main = main_class(&jar)?;
        let _ = writeln!(log, "== {main} {}", arguments.join(" "));
        let _ = log.flush();
        let status = Command::new(&java_exe)
            .arg("-cp")
            .arg(classpath.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(crate::plataforma::CLASSPATH_SEPARATOR))
            .arg(&main)
            .args(&arguments)
            .current_dir(&temp)
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log.try_clone().map_err(|e| e.to_string())?)
            .no_console()
            .status()
            .map_err(|e| t!("No se pudo abrir Java: {e}", "Could not start Java: {e}"))?;
        let step = arguments.iter().position(|a| a == "--task").and_then(|i| arguments.get(i + 1)).cloned().unwrap_or(main);
        if !status.success() {
            return Err(t!(
                "Falló un paso del instalador de {name} ({step}). Si es por la conexión, vuelve a intentarlo; los detalles quedan en datos/logs/instalador-{}.log.", "A step of the {name} installer failed ({step}). If it is the connection, try again; the details are in datos/logs/instalador-{}.log.",
                name.to_lowercase()
            ));
        }
        if !done(&outputs) {
            return Err(t!("El instalador de {name} generó un archivo distinto del esperado ({step}). Vuelve a intentarlo.", "The {name} installer produced a different file than expected ({step}). Try again."));
        }
    }
    let _ = std::fs::remove_dir_all(&temp);
    Ok(())
}

/// El JSON de la versión va al final: si algo falla antes, el loader no parece instalado.
fn write_version(version: &Value, paths: &Paths) -> Result<String, String> {
    let id = version["id"]
        .as_str()
        .filter(|id| !id.is_empty() && !id.contains(['/', '\\']) && !id.contains(".."))
        .ok_or(t!("La versión del instalador no tiene id.", "The installer version has no id."))?
        .to_string();
    let file = paths.version_json(&id);
    std::fs::create_dir_all(file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&file, serde_json::to_vec_pretty(version).unwrap()).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Forge o NeoForge y su versión, a partir del JSON de una versión instalada (sirve también con
/// las de CurseForge, que se llaman "forge-47.4.20").
pub fn describe(id: &str, json: &Value) -> Option<(Loader, String)> {
    let game: Vec<&str> = json.pointer("/arguments/game").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let after = |flag: &str| game.iter().position(|a| *a == flag).and_then(|i| game.get(i + 1)).map(|v| v.to_string());
    if let Some(version) = after("--fml.neoForgeVersion") {
        return Some((Loader::NeoForge, version));
    }
    if let Some(version) = after("--fml.forgeVersion") {
        return Some((Loader::Forge, version));
    }
    if let Some(version) = id.strip_prefix("neoforge-") {
        return Some((Loader::NeoForge, version.to_string()));
    }
    let minecraft = json["inheritsFrom"].as_str().unwrap_or_default();
    id.strip_prefix(&format!("{minecraft}-forge-"))
        .or_else(|| id.strip_prefix("forge-"))
        .map(|version| (Loader::Forge, version.to_string()))
}

/// Ids posibles de una versión instalada: la del instalador oficial y la de CurseForge.
pub fn candidate_ids(loader: Loader, minecraft: &str, version: &str) -> Vec<String> {
    match loader {
        Loader::NeoForge => vec![format!("neoforge-{version}")],
        _ => vec![format!("{minecraft}-forge-{version}"), format!("forge-{version}")],
    }
}

/// Archivos que Forge y NeoForge usan sin nombrarlos en su JSON: el juego parcheado, sus módulos
/// de carga (fmlcore, javafmllanguage…) y los mapeos que dejan los processors. Están en carpetas con
/// la versión del loader ("net/minecraftforge/*/1.20.1-47.4.26", "net/neoforged/*/26.1.2.114").
/// Hacen falta para llevar la versión a otro PC (paquetes para Drive o USB).
pub fn generated_files(json: &Value, paths: &Paths) -> Vec<PathBuf> {
    let Some((loader, version)) = describe(json["id"].as_str().unwrap_or_default(), json) else { return Vec::new() };
    let game: Vec<&str> = json.pointer("/arguments/game").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let after = |flag: &str| game.iter().position(|a| *a == flag).and_then(|i| game.get(i + 1)).copied();
    let minecraft = after("--fml.mcVersion").or(json["inheritsFrom"].as_str()).unwrap_or_default();
    let (group, folder_version) = match loader {
        Loader::NeoForge => ("net/neoforged".to_string(), version),
        _ => (after("--fml.forgeGroup").unwrap_or("net.minecraftforge").replace('.', "/"), format!("{minecraft}-{version}")),
    };
    let mut folders: Vec<PathBuf> = std::fs::read_dir(paths.libraries.join(&group))
        .into_iter()
        .flatten()
        .flatten()
        .map(|artifact| artifact.path().join(&folder_version))
        .filter(|folder| folder.is_dir())
        .collect();
    if let Some(mcp) = after("--fml.mcpVersion").or(after("--fml.neoFormVersion")) {
        folders.push(paths.libraries.join("net/minecraft/client").join(format!("{minecraft}-{mcp}")));
    }
    let mut files = Vec::new();
    for folder in folders {
        for entry in std::fs::read_dir(&folder).into_iter().flatten().flatten() {
            let path = entry.path();
            let keep = path.extension().is_some_and(|e| e == "jar" || e == "txt" || e == "lzma");
            if path.is_file() && keep && !files.contains(&path) {
                files.push(path);
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versiones_de_neoforge() {
        assert_eq!(neoforge_minecraft("21.1.77").as_deref(), Some("1.21.1"));
        assert_eq!(neoforge_minecraft("21.0.167").as_deref(), Some("1.21"));
        assert_eq!(neoforge_minecraft("20.4.237").as_deref(), Some("1.20.4"));
        assert_eq!(neoforge_minecraft("26.1.2.114").as_deref(), Some("26.1.2"));
        assert_eq!(neoforge_minecraft("26.3.0.58-beta").as_deref(), Some("26.3"));
        assert_eq!(neoforge_minecraft("0.25w14craftmine.3-beta"), None);
    }

    #[test]
    fn reconoce_forge_de_curseforge_y_oficial() {
        let curseforge = serde_json::json!({ "inheritsFrom": "1.20.1", "arguments": { "game": ["--launchTarget", "forgeclient", "--fml.forgeVersion", "47.4.20"] } });
        assert_eq!(describe("forge-47.4.20", &curseforge).map(|(l, v)| (l.name(), v)), Some(("Forge", "47.4.20".to_string())));
        let forge26 = serde_json::json!({ "inheritsFrom": "26.1.2", "arguments": { "game": ["--launchTarget", "forge_client"] } });
        assert_eq!(describe("26.1.2-forge-64.1.3", &forge26).map(|(l, v)| (l.name(), v)), Some(("Forge", "64.1.3".to_string())));
        let neo = serde_json::json!({ "inheritsFrom": "26.1.2", "arguments": { "game": ["--fml.neoForgeVersion", "26.1.2.114"] } });
        assert_eq!(describe("neoforge-26.1.2.114", &neo).map(|(l, v)| (l.name(), v)), Some(("NeoForge", "26.1.2.114".to_string())));
        let fabric = serde_json::json!({ "inheritsFrom": "1.21.1" });
        assert!(describe("fabric-loader-0.16.10-1.21.1", &fabric).is_none());
    }

    #[test]
    fn sustituye_datos_y_bibliotecas() {
        let paths = Paths::new(r"C:\P");
        let data = HashMap::from([("SIDE".to_string(), "client".to_string())]);
        assert_eq!(expand("--side={SIDE}", &data, &paths).unwrap(), "--side=client");
        assert_eq!(
            expand("[de.oceanlabs.mcp:mcp_config:1.20.1-20230612.114412:mappings@txt]", &data, &paths).unwrap(),
            r"C:\P\base\libraries\de/oceanlabs/mcp/mcp_config/1.20.1-20230612.114412/mcp_config-1.20.1-20230612.114412-mappings.txt"
        );
        assert!(expand("{NADA}", &data, &paths).is_err());
    }
}
