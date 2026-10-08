//! Loaders de mods Fabric y Quilt desde sus servidores (meta.fabricmc.net y meta.quiltmc.org).
//! Cada uno es una versión "hija" (fabric-loader-0.19.5-1.21.1) que hereda de la vanilla con
//! "inheritsFrom": solo añade sus bibliotecas y su clase principal.

use serde::Serialize;
use serde_json::Value;
use std::cmp::Ordering;

use crate::download::{self, FileDownload};
use crate::paths::Paths;
use crate::version::Maven;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loader {
    Fabric,
    Quilt,
}

impl Loader {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "fabric" => Some(Self::Fabric),
            "quilt" => Some(Self::Quilt),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
        }
    }

    fn meta(self) -> &'static str {
        match self {
            Self::Fabric => "https://meta.fabricmc.net/v2",
            Self::Quilt => "https://meta.quiltmc.org/v3",
        }
    }

    /// Los ids de versión empiezan así: "fabric-loader-0.19.5-1.21.1".
    fn prefix(self) -> &'static str {
        match self {
            Self::Fabric => "fabric-loader-",
            Self::Quilt => "quilt-loader-",
        }
    }

    fn unavailable(self) -> String {
        format!("No se puede conectar con los servidores de {} (en algunas redes, como la de la escuela, están bloqueados).", self.name())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LoaderVersion {
    pub version: String,
    /// Las beta, pre y rc llevan guion; las demás son estables.
    pub stable: bool,
}

/// Versiones de Minecraft que el loader soporta (incluidas snapshots).
pub async fn game_versions(loader: Loader) -> Result<Vec<String>, String> {
    let text = download::get_text(&format!("{}/versions/game", loader.meta())).await.map_err(|_| loader.unavailable())?;
    let list: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    Ok(list.as_array().into_iter().flatten().filter_map(|v| v["version"].as_str().map(String::from)).collect())
}

/// Versiones del loader para una versión de Minecraft, de la más nueva a la más vieja.
pub async fn loader_versions(loader: Loader, minecraft: &str) -> Result<Vec<LoaderVersion>, String> {
    let url = format!("{}/versions/loader/{}", loader.meta(), encode(minecraft));
    let text = download::get_text(&url).await.map_err(|_| loader.unavailable())?;
    let list: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut versions: Vec<LoaderVersion> = list
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.pointer("/loader/version").and_then(Value::as_str))
        .map(|version| LoaderVersion { stable: !version.contains('-'), version: version.into() })
        .collect();
    // Quilt las devuelve desordenadas.
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));
    versions.dedup_by(|a, b| a.version == b.version);
    Ok(versions)
}

/// La versión recomendada: la estable más nueva (o la más nueva si no hay estables).
pub fn recommended(versions: &[LoaderVersion]) -> Option<&LoaderVersion> {
    versions.iter().find(|v| v.stable).or(versions.first())
}

/// Instala el loader sobre una vanilla ya instalada. Devuelve el id de la versión creada.
pub async fn install(
    loader: Loader,
    minecraft: &str,
    loader_version: &str,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + 'static,
) -> Result<String, String> {
    let name = loader.name();
    progress(format!("Preparando {name} {loader_version}…"), 0.0);
    let url = format!("{}/versions/loader/{}/{}/profile/json", loader.meta(), encode(minecraft), encode(loader_version));
    let text = download::get_text(&url).await.map_err(|e| {
        if e.is_status() {
            format!("{name} no tiene la versión {loader_version} para Minecraft {minecraft}.")
        } else {
            loader.unavailable()
        }
    })?;
    let mut profile: Value = serde_json::from_str(&text).map_err(|e| format!("El perfil de {name} llegó dañado: {e}"))?;
    let id = profile["id"]
        .as_str()
        .filter(|id| !id.is_empty() && !id.contains(['/', '\\']) && !id.contains(".."))
        .ok_or(format!("El perfil de {name} no tiene id."))?
        .to_string();
    if loader == Loader::Quilt {
        prefer_intermediary(&mut profile);
    }

    let files = library_files(&profile, paths);
    let loader_version = loader_version.to_string();
    download::download_all(files, move |count, total, _, _| {
        progress(format!("Descargando {name} {loader_version}… {count} de {total} archivos"), count as f64 / total.max(1) as f64);
    })
    .await?;

    // El JSON va al final: si la descarga se corta, el loader no parece instalado.
    let json_file = paths.version_json(&id);
    std::fs::create_dir_all(json_file.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&json_file, serde_json::to_vec_pretty(&profile).unwrap()).map_err(|e| e.to_string())?;
    Ok(id)
}

/// Las bibliotecas del perfil vienen como coordenadas Maven más el repositorio donde están.
fn library_files(profile: &Value, paths: &Paths) -> Vec<FileDownload> {
    profile["libraries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|library| {
            // Por si algún perfil usa el formato de Mojang.
            if let Some(artifact) = library.pointer("/downloads/artifact") {
                return FileDownload::from_json(artifact, paths.libraries.join(artifact["path"].as_str()?));
            }
            let relative = Maven::parse(library["name"].as_str()?)?.relative_path();
            let repository = library["url"].as_str().unwrap_or("https://libraries.minecraft.net/");
            Some(FileDownload {
                url: format!("{}/{relative}", repository.trim_end_matches('/')),
                path: paths.libraries.join(&relative),
                sha1: library["sha1"].as_str().map(String::from),
                size: library["size"].as_u64(),
            })
        })
        .collect()
}

/// El perfil de Quilt trae dos mapeos (hashed e intermediary) con el mismo archivo interno; si
/// ambos están en el classpath gana hashed y los mods de Fabric no cargan. Su instalador oficial
/// también se queda solo con intermediary.
fn prefer_intermediary(profile: &mut Value) {
    let Some(libraries) = profile["libraries"].as_array_mut() else { return };
    let named = |library: &Value, prefix: &str| library["name"].as_str().is_some_and(|n| n.starts_with(prefix));
    if libraries.iter().any(|l| named(l, "net.fabricmc:intermediary:")) {
        libraries.retain(|l| !named(l, "org.quiltmc:hashed:"));
    }
}

/// Un loader ya instalado en este PC (sirve sin conexión).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub id: String,
    pub loader: String,
    pub loader_version: String,
    pub minecraft: String,
}

pub fn installed(paths: &Paths) -> Vec<Installed> {
    let Ok(entries) = std::fs::read_dir(&paths.versions) else { return Vec::new() };
    let mut list: Vec<Installed> = entries
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().to_string();
            let (loader, loader_version, minecraft) = describe(&id, paths)?;
            Some(Installed { loader: loader.name().into(), loader_version, minecraft, id })
        })
        .collect();
    list.sort_by(|a, b| compare_versions(&b.loader_version, &a.loader_version));
    list
}

/// (loader, versión del loader, versión de Minecraft) de una versión instalada como "fabric-loader-0.19.5-1.21.1".
pub fn describe(id: &str, paths: &Paths) -> Option<(Loader, String, String)> {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(paths.version_json(id)).ok()?).ok()?;
    let minecraft = json["inheritsFrom"].as_str()?.to_string();
    [Loader::Fabric, Loader::Quilt].into_iter().find_map(|loader| {
        let version = id.strip_prefix(loader.prefix())?.strip_suffix(&format!("-{minecraft}"))?;
        Some((loader, version.to_string(), minecraft.clone()))
    })
}

/// Id de la versión instalada de ese loader, si ya está en este PC.
pub fn installed_id(loader: Loader, minecraft: &str, loader_version: &str, paths: &Paths) -> Option<String> {
    let id = format!("{}{loader_version}-{minecraft}", loader.prefix());
    paths.version_json(&id).exists().then_some(id)
}

/// Compara versiones como "0.16.10", "0.20.0-beta.9" o "0.4.8+build.155" (una beta va antes que su estable).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn split(version: &str) -> (&str, Option<&str>) {
        let version = version.split('+').next().unwrap_or(version);
        match version.split_once('-') {
            Some((main, pre)) => (main, Some(pre)),
            None => (version, None),
        }
    }
    fn parts(a: &str, b: &str) -> Ordering {
        let (mut x, mut y) = (a.split(['.', '-']), b.split(['.', '-']));
        loop {
            let order = match (x.next(), y.next()) {
                (None, None) => return Ordering::Equal,
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (Some(p), Some(q)) => match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    _ => p.cmp(q),
                },
            };
            if order != Ordering::Equal {
                return order;
            }
        }
    }
    let ((main_a, pre_a), (main_b, pre_b)) = (split(a), split(b));
    parts(main_a, main_b).then_with(|| match (pre_a, pre_b) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(p), Some(q)) => parts(p, q),
    })
}

/// Las versiones viejas tienen espacios ("1.14 Pre-Release 5").
fn encode(segment: &str) -> String {
    segment.replace('%', "%25").replace(' ', "%20").replace('+', "%2B").replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordena_versiones() {
        let mut list = vec!["0.20.0-beta.9", "0.24.0", "0.20.0-beta.10", "0.20.0", "0.19.5", "0.20.2-beta.1", "0.4.8+build.155"];
        list.sort_by(|a, b| compare_versions(b, a));
        assert_eq!(list, ["0.24.0", "0.20.2-beta.1", "0.20.0", "0.20.0-beta.10", "0.20.0-beta.9", "0.19.5", "0.4.8+build.155"]);
    }

    #[test]
    fn recomienda_la_estable_mas_nueva() {
        let versions = vec![
            LoaderVersion { version: "0.31.0-beta.4".into(), stable: false },
            LoaderVersion { version: "0.30.1".into(), stable: true },
        ];
        assert_eq!(recommended(&versions).unwrap().version, "0.30.1");
    }

    #[test]
    fn quilt_usa_intermediary() {
        let mut profile = serde_json::json!({ "libraries": [
            { "name": "org.quiltmc:hashed:1.21.1" },
            { "name": "net.fabricmc:intermediary:1.21.1" },
            { "name": "org.quiltmc:quilt-loader:0.30.1" }
        ]});
        prefer_intermediary(&mut profile);
        let names: Vec<&str> = profile["libraries"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
        assert_eq!(names, ["net.fabricmc:intermediary:1.21.1", "org.quiltmc:quilt-loader:0.30.1"]);
    }
}
