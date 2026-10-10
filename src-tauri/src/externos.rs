//! Instancias de otros launchers: CurseForge y Prism (también MultiMC, que usa el mismo formato).
//! Se encuentran solas en sus carpetas de siempre, o en la que elija el jugador, y se copian a
//! Parabellum. El loader lo instala Parabellum, así la copia no depende del otro launcher.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
    /// "CurseForge", "Prism" o "MultiMC".
    pub launcher: String,
    pub name: String,
    pub folder: PathBuf,
    #[serde(skip)]
    pub game_dir: PathBuf,
    pub minecraft: String,
    /// "Vanilla", "Fabric", "Quilt", "Forge" o "NeoForge".
    pub loader: String,
    pub loader_version: Option<String>,
    pub ram_gb: Option<u32>,
    #[serde(skip)]
    pub jvm_args: Vec<String>,
    #[serde(skip)]
    pub icon: Option<PathBuf>,
    pub mods: usize,
    pub worlds: usize,
}

/// Lo que hay en las carpetas de siempre de CurseForge, Prism y MultiMC.
pub fn discover() -> Vec<External> {
    let mut roots = Vec::new();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        roots.push(home.join("curseforge").join("minecraft").join("Instances"));
    }
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        roots.push(appdata.join("PrismLauncher").join("instances"));
        roots.push(appdata.join("MultiMC").join("instances"));
    }
    let mut found: Vec<External> = roots
        .iter()
        .flat_map(|root| std::fs::read_dir(root).into_iter().flatten().flatten())
        .filter_map(|entry| read(&entry.path()))
        .collect();
    found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    found
}

/// Una carpeta de instancia de CurseForge o de Prism/MultiMC.
pub fn read(folder: &Path) -> Option<External> {
    let mut external = read_curseforge(folder).or_else(|| read_prism(folder))?;
    let count = |dir: &str, wanted: fn(&std::fs::DirEntry) -> bool| {
        std::fs::read_dir(external.game_dir.join(dir)).into_iter().flatten().flatten().filter(wanted).count()
    };
    external.mods = count("mods", |e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar"));
    external.worlds = count("saves", |e| e.path().join("level.dat").exists());
    Some(external)
}

fn read_curseforge(folder: &Path) -> Option<External> {
    let json: Value = serde_json::from_str(&std::fs::read_to_string(folder.join("minecraftinstance.json")).ok()?).ok()?;
    let minecraft = json["gameVersion"].as_str().or(json.pointer("/baseModLoader/minecraftVersion").and_then(Value::as_str))?.to_string();
    // "forge-47.4.20", "neoforge-26.1.2.97", "fabric-0.19.5-26.2"…; forgeVersion trae solo la versión.
    let (loader, loader_version) = match json.pointer("/baseModLoader/name").and_then(Value::as_str) {
        None => ("Vanilla", None),
        Some(name) => {
            let (_, loader) = [("neoforge", "NeoForge"), ("forge", "Forge"), ("fabric", "Fabric"), ("quilt", "Quilt")]
                .into_iter()
                .find(|(prefix, _)| name.starts_with(prefix))?;
            (loader, json.pointer("/baseModLoader/forgeVersion").and_then(Value::as_str).map(String::from))
        }
    };
    let ram_gb = (json["isMemoryOverride"] == true)
        .then(|| json["allocatedMemory"].as_u64())
        .flatten()
        .filter(|mb| *mb >= 512)
        .map(|mb| ((mb + 512) / 1024).max(1) as u32);
    Some(External {
        launcher: "CurseForge".into(),
        name: json["name"].as_str().map(String::from).unwrap_or_else(|| folder_name(folder)),
        folder: folder.to_path_buf(),
        game_dir: folder.to_path_buf(),
        minecraft,
        loader: loader.into(),
        loader_version,
        ram_gb,
        jvm_args: json["javaArgsOverride"].as_str().unwrap_or_default().split_whitespace().map(String::from).collect(),
        icon: json["profileImagePath"].as_str().map(PathBuf::from).filter(|p| p.is_file()),
        mods: 0,
        worlds: 0,
    })
}

/// Prism y MultiMC: instance.cfg (formato INI) y mmc-pack.json con los componentes.
fn read_prism(folder: &Path) -> Option<External> {
    let pack: Value = serde_json::from_str(&std::fs::read_to_string(folder.join("mmc-pack.json")).ok()?).ok()?;
    let config = std::fs::read_to_string(folder.join("instance.cfg")).unwrap_or_default();
    let setting = |key: &str| config.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix('=')).map(str::trim).filter(|v| !v.is_empty());
    let components = pack["components"].as_array()?;
    let version_of = |uid: &str| components.iter().find(|c| c["uid"] == uid).and_then(|c| c["version"].as_str()).map(String::from);
    let minecraft = version_of("net.minecraft")?;
    let (loader, loader_version) = [
        ("net.fabricmc.fabric-loader", "Fabric"),
        ("org.quiltmc.quilt-loader", "Quilt"),
        ("net.minecraftforge", "Forge"),
        ("net.neoforged", "NeoForge"),
    ]
    .into_iter()
    .find_map(|(uid, name)| version_of(uid).map(|v| (name, Some(v))))
    .unwrap_or(("Vanilla", None));
    let game_dir = [".minecraft", "minecraft"].iter().map(|d| folder.join(d)).find(|d| d.is_dir()).unwrap_or_else(|| folder.join(".minecraft"));
    let ram_gb = (setting("OverrideMemory") == Some("true"))
        .then(|| setting("MaxMemAlloc")?.parse::<u64>().ok())
        .flatten()
        .filter(|mb| *mb >= 512)
        .map(|mb| ((mb + 512) / 1024).max(1) as u32);
    let jvm_args = if setting("OverrideJavaArgs") == Some("true") {
        setting("JvmArgs").unwrap_or_default().split_whitespace().map(String::from).collect()
    } else {
        Vec::new()
    };
    // Los iconos propios están en la carpeta "icons" del launcher, junto a "instances".
    let icon = setting("iconKey").and_then(|key| {
        let icons = folder.parent()?.parent()?.join("icons");
        ["png", "jpg", "jpeg", "gif", "webp"].iter().map(|ext| icons.join(format!("{key}.{ext}"))).find(|p| p.is_file())
    });
    let launcher = if folder.to_string_lossy().to_lowercase().contains("multimc") { "MultiMC" } else { "Prism" };
    Some(External {
        launcher: launcher.into(),
        name: setting("name").map(String::from).unwrap_or_else(|| folder_name(folder)),
        folder: folder.to_path_buf(),
        game_dir,
        minecraft,
        loader: loader.into(),
        loader_version,
        ram_gb,
        jvm_args,
        icon,
        mods: 0,
        worlds: 0,
    })
}

fn folder_name(folder: &Path) -> String {
    folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// Lo que no se copia: archivos del otro launcher y temporales.
const SKIP: &[&str] = &["minecraftinstance.json", "downloads", "logs", "crash-reports", ".lanzador", "instancia.json"];

/// Copia la carpeta del juego. `progress` recibe (bytes copiados, bytes totales).
pub fn copy_game(from: &Path, to: &Path, progress: impl Fn(u64, u64)) -> Result<(), String> {
    fn walk(dir: &Path, top: bool, files: &mut Vec<(PathBuf, u64)>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            if top && SKIP.iter().any(|s| entry.file_name().to_string_lossy().eq_ignore_ascii_case(s)) {
                continue;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => walk(&entry.path(), false, files),
                Ok(kind) if kind.is_file() => files.push((entry.path(), entry.metadata().map(|m| m.len()).unwrap_or(0))),
                _ => {}
            }
        }
    }
    let mut files = Vec::new();
    walk(from, true, &mut files);
    let total: u64 = files.iter().map(|(_, size)| size).sum::<u64>().max(1);
    let mut done = 0;
    for (index, (file, size)) in files.iter().enumerate() {
        let target = to.join(file.strip_prefix(from).map_err(|e| e.to_string())?);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(file, &target).map_err(|e| t!("No se pudo copiar {}: {e}", "Could not copy {}: {e}", file.display()))?;
        done += size;
        if index % 25 == 0 || index + 1 == files.len() {
            progress(done, total);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lee_instancia_de_prism() {
        let folder = std::env::temp_dir().join(format!("parabellum-prism-{}", std::process::id()));
        std::fs::create_dir_all(folder.join(".minecraft").join("mods")).unwrap();
        std::fs::write(folder.join(".minecraft/mods/sodium.jar"), b"x").unwrap();
        std::fs::write(
            folder.join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.neoforged","version":"21.1.77"}],"formatVersion":1}"#,
        )
        .unwrap();
        std::fs::write(folder.join("instance.cfg"), "[General]\nname=Mi pack\nOverrideMemory=true\nMaxMemAlloc=6144\n").unwrap();
        let found = read(&folder).unwrap();
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!((found.launcher.as_str(), found.name.as_str(), found.minecraft.as_str()), ("Prism", "Mi pack", "1.21.1"));
        assert_eq!((found.loader.as_str(), found.loader_version.as_deref()), ("NeoForge", Some("21.1.77")));
        assert_eq!((found.ram_gb, found.mods), (Some(6), 1));
    }
}
