//! Formato .mrpack de Modrinth (el estándar que entienden Modrinth App, Prism y otros launchers):
//! un zip con modrinth.index.json (qué descargar, con sus hashes) y overrides/ (todo lo demás).
//! Es ligero porque los mods se bajan de Modrinth: sirve con internet, no en la escuela.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use crate::download::{self, FileDownload};
use crate::instances::Instance;
use crate::modrinth;
use crate::packs::{self, ExportOptions, ZipOut};

const INDEX: &str = "modrinth.index.json";
/// Únicos sitios desde los que la especificación permite descargar archivos de un .mrpack.
const ALLOWED_HOSTS: [&str; 4] = ["https://cdn.modrinth.com/", "https://github.com/", "https://raw.githubusercontent.com/", "https://gitlab.com/"];

// ================= Exportar

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Exported {
    pub file: String,
    /// Archivos que se descargarán de Modrinth al instalarlo.
    pub downloads: usize,
    /// Archivos que van dentro del .mrpack (configuración y lo que no está en Modrinth).
    pub included: usize,
    pub warning: Option<String>,
}

pub async fn export(
    instance: &Instance,
    out_file: &Path,
    options: &ExportOptions,
    progress: impl Fn(String, f64) + Send + Sync + 'static,
) -> Result<Exported, String> {
    progress("Buscando qué archivos están en Modrinth…".into(), 0.0);
    let installed = modrinth::identify(instance).await;
    let known: Vec<&modrinth::InstalledFile> = installed.iter().filter(|f| f.project_id.is_some() && !f.file.ends_with(".disabled")).collect();
    let mut warning = None;
    let remote = match modrinth::files_by_hash(&known.iter().map(|f| f.sha1.clone()).collect::<Vec<_>>()).await {
        Ok(found) => found,
        Err(_) if !known.is_empty() => {
            warning = Some("Sin conexión con Modrinth: los mods van dentro del .mrpack, así que pesa más.".to_string());
            HashMap::new()
        }
        Err(_) => HashMap::new(),
    };
    // "mods/sodium.jar" -> dónde descargarlo
    let downloadable: HashMap<String, (&modrinth::RemoteFile, &str)> =
        known.iter().filter_map(|f| Some((format!("{}/{}", f.kind, f.file), (remote.get(&f.sha1)?, f.kind)))).collect();

    let mut files = Vec::new();
    let mut overrides = Vec::new();
    for path in packs::selected_files(instance, &options.include) {
        let Ok(relative) = path.strip_prefix(&instance.game_dir) else { continue };
        let relative = relative.to_string_lossy().replace('\\', "/");
        // Los mods desactivados no van: otro launcher los dejaría desactivados igual.
        if relative.ends_with(".disabled") {
            continue;
        }
        match downloadable.get(&relative) {
            Some((remote, kind)) => files.push(json!({
                "path": relative,
                "hashes": { "sha1": remote.sha1, "sha512": remote.sha512 },
                "env": { "client": "required", "server": if *kind == "mods" { "required" } else { "unsupported" } },
                "downloads": [remote.url],
                "fileSize": remote.size,
            })),
            None => overrides.push((path.clone(), format!("overrides/{relative}"))),
        }
    }

    let mut dependencies = json!({ "minecraft": instance.minecraft_version });
    if let Some(version) = &instance.loader_version {
        let key = match instance.loader.as_str() {
            "Fabric" => Some("fabric-loader"),
            "Quilt" => Some("quilt-loader"),
            "Forge" => Some("forge"),
            "NeoForge" => Some("neoforge"),
            _ => None,
        };
        if let Some(key) = key {
            dependencies[key] = json!(version);
        }
    }
    let name = Some(options.name.trim()).filter(|n| !n.is_empty()).unwrap_or(&instance.name).to_string();
    let mut index = json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": Some(options.version.trim()).filter(|v| !v.is_empty()).unwrap_or("1.0.0"),
        "name": name,
        "files": files,
        "dependencies": dependencies,
    });
    if !options.description.trim().is_empty() {
        index["summary"] = json!(options.description.trim());
    }

    let out = out_file.to_path_buf();
    let downloads = files.len();
    let included = overrides.len();
    let total: u64 = overrides.iter().filter_map(|(p, _)| p.metadata().ok()).map(|m| m.len()).sum::<u64>().max(1);
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let temp = out.with_extension("mrpack.tmp");
        let mut zip = ZipOut::create(&temp)?;
        zip.add_bytes(INDEX, &serde_json::to_vec_pretty(&index).unwrap())?;
        for (file, name) in &overrides {
            zip.add_file(file, name)?;
            progress(format!("Empaquetando {:.0} de {:.0} MB…", zip.written as f64 / 1048576.0, total as f64 / 1048576.0), zip.written as f64 / total as f64);
        }
        zip.finish()?;
        std::fs::rename(&temp, &out).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(Exported { file: out_file.display().to_string(), downloads, included, warning })
}

// ================= Importar

#[derive(Debug, Clone)]
pub struct MrpackFile {
    pub path: PathBuf,
    pub url: String,
    pub sha1: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Info {
    pub name: String,
    pub version: String,
    pub minecraft: String,
    /// ("Fabric", "0.19.3"); None en vanilla.
    pub loader: Option<(String, String)>,
    pub files: Vec<MrpackFile>,
}

/// Una ruta del índice solo puede ser relativa y quedarse dentro de la instancia.
pub(crate) fn safe_relative(path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    let valid = !path.is_empty() && !path.contains(':') && candidate.components().all(|c| matches!(c, Component::Normal(_)));
    valid.then(|| candidate.to_path_buf())
}

/// Lee el índice de un .mrpack. None si el zip no es un .mrpack.
pub fn read(zip_path: &Path) -> Result<Option<Info>, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("No se pudo abrir el archivo: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| "El archivo no es un zip válido.".to_string())?;
    let Ok(mut entry) = zip.by_name(INDEX) else { return Ok(None) };
    let mut text = String::new();
    entry.read_to_string(&mut text).map_err(|e| e.to_string())?;
    let index: Value = serde_json::from_str(&text).map_err(|e| format!("El índice del modpack está dañado: {e}"))?;
    if index["formatVersion"].as_u64() != Some(1) || index["game"].as_str() != Some("minecraft") {
        return Err("Este .mrpack usa un formato que Parabellum todavía no conoce.".into());
    }
    let dependencies = &index["dependencies"];
    let minecraft = dependencies["minecraft"].as_str().ok_or("El modpack no dice qué versión de Minecraft usa.")?.to_string();
    let loader = [("fabric-loader", "Fabric"), ("quilt-loader", "Quilt"), ("neoforge", "NeoForge"), ("forge", "Forge")]
        .iter()
        .find_map(|(key, name)| Some((name.to_string(), dependencies[*key].as_str()?.to_string())));
    let mut files = Vec::new();
    for item in index["files"].as_array().into_iter().flatten() {
        // Lo que dice no usarse en el cliente no se descarga.
        if item.pointer("/env/client").and_then(Value::as_str) == Some("unsupported") {
            continue;
        }
        let raw_path = item["path"].as_str().unwrap_or_default();
        let path = safe_relative(raw_path).ok_or(format!("El modpack trae una ruta no permitida: {raw_path}"))?;
        let url = item["downloads"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find(|u| ALLOWED_HOSTS.iter().any(|h| u.starts_with(h)))
            .ok_or(format!("{raw_path} no se descarga de un sitio permitido."))?
            .to_string();
        let sha1 = item.pointer("/hashes/sha1").and_then(Value::as_str).filter(|h| h.len() == 40).ok_or(format!("{raw_path} no trae su SHA-1."))?;
        files.push(MrpackFile { path, url, sha1: sha1.to_string(), size: item["fileSize"].as_u64() });
    }
    Ok(Some(Info {
        name: index["name"].as_str().unwrap_or("Modpack").to_string(),
        version: index["versionId"].as_str().unwrap_or_default().to_string(),
        minecraft,
        loader,
        files,
    }))
}

/// Descarga los archivos del índice (verificando su SHA-1) y copia overrides/ y client-overrides/.
pub async fn install_files(
    info: &Info,
    zip_path: &Path,
    game_dir: &Path,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<(), String> {
    let downloads: Vec<FileDownload> = info
        .files
        .iter()
        .map(|f| FileDownload { url: f.url.clone(), path: game_dir.join(&f.path), sha1: Some(f.sha1.clone()), size: f.size })
        .collect();
    let report = progress.clone();
    download::download_all(downloads, move |count, total, bytes, total_bytes| {
        report(
            format!("Descargando mods… {count} de {total} ({:.0} de {:.0} MB)", bytes as f64 / 1048576.0, total_bytes as f64 / 1048576.0),
            0.9 * bytes as f64 / total_bytes.max(1) as f64,
        );
    })
    .await?;
    let (zip_path, game_dir) = (zip_path.to_path_buf(), game_dir.to_path_buf());
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        progress("Copiando la configuración del modpack…".into(), 0.92);
        let mut zip = zip::ZipArchive::new(std::fs::File::open(&zip_path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        // Primero overrides y después client-overrides, que tienen prioridad.
        for prefix in ["overrides", "client-overrides"] {
            for i in 0..zip.len() {
                let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
                let Some(name) = entry.enclosed_name() else { continue };
                let Ok(relative) = name.strip_prefix(prefix) else { continue };
                if entry.is_dir() || relative.as_os_str().is_empty() {
                    continue;
                }
                let target = game_dir.join(relative);
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut out).map_err(|e| format!("El modpack está dañado ({}): {e}", name.display()))?;
            }
        }
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rutas_seguras() {
        assert!(safe_relative("mods/sodium.jar").is_some());
        assert!(safe_relative("config/a/b.toml").is_some());
        assert!(safe_relative("../fuera.jar").is_none());
        assert!(safe_relative("mods/../../fuera.jar").is_none());
        assert!(safe_relative("C:/Windows/x.dll").is_none());
        assert!(safe_relative("/etc/passwd").is_none());
        assert!(safe_relative("").is_none());
    }
}
