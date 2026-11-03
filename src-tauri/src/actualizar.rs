//! Actualizar en la misma instancia un modpack de Modrinth o de CurseForge, como Modrinth App y la app de
//! CurseForge. La versión nueva se instala aparte, igual que de cero (main.rs › actualizar_modpack), y
//! después se le pasa lo que es del jugador:
//!  - sus datos (mundos, opciones, capturas…: `packs::PRESERVED_ON_UPDATE`), que se mueven;
//!  - lo que él agregó (mods, paquetes, ajustes que el modpack no traía), si la versión nueva no lo trae;
//!  - los ajustes del modpack que él cambió, si la versión nueva no los cambió también;
//!  - los mods del modpack que él desactivó, si siguen en la versión nueva.
//! Lo que la versión nueva reemplaza queda en un respaldo recuperable (respaldos.rs), como al actualizar
//! un paquete de Drive.

use crate::instances::{Instance, INSTANCE_FILE, STATE_DIR};
use crate::paths::Paths;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Lo que trae una versión de un modpack, leído de su archivo (.mrpack o .zip de CurseForge).
#[derive(Default, Debug)]
pub struct PackFiles {
    /// Lo que viene dentro del archivo (overrides): ruta en minúsculas → CRC32.
    overrides: HashMap<String, u32>,
    /// Lo que se baja aparte según el índice del .mrpack (rutas en minúsculas).
    downloads: HashSet<String>,
    /// Lo que se baja aparte según el manifiesto de CurseForge: (proyecto, archivo).
    curseforge: HashSet<(u64, u64)>,
}

impl PackFiles {
    pub fn read(file: &Path) -> Result<Self, String> {
        let mut pack = PackFiles::default();
        let prefixes: Vec<String> = if let Some(info) = crate::mrpack::read(file)? {
            pack.downloads = info.files.iter().map(|f| f.path.to_string_lossy().replace('\\', "/").to_lowercase()).collect();
            vec!["overrides/".into(), "client-overrides/".into()]
        } else if let Some(cf) = crate::curseforge::read_modpack(file)? {
            pack.curseforge = cf.files.iter().copied().collect();
            vec![format!("{}/", cf.overrides.trim_end_matches('/'))]
        } else {
            return Err(t!("No es un modpack de Modrinth ni de CurseForge.", "It is not a Modrinth or CurseForge modpack."));
        };
        let mut zip = zip::ZipArchive::new(std::fs::File::open(file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        // Los client-overrides van después: mandan sobre los overrides, como al instalar.
        for prefix in &prefixes {
            for i in 0..zip.len() {
                let Ok(entry) = zip.by_index_raw(i) else { continue };
                let name = entry.name().replace('\\', "/");
                let Some(rest) = name.strip_prefix(prefix.as_str()) else { continue };
                if entry.is_dir() || rest.is_empty() || crate::mrpack::safe_relative(rest).is_none() {
                    continue;
                }
                pack.overrides.insert(rest.to_lowercase(), entry.crc32());
            }
        }
        Ok(pack)
    }

    /// ¿Este archivo de la instancia lo puso el modpack? `origin` es su (proyecto, archivo) de CurseForge.
    fn owns(&self, rel: &str, origin: Option<(u64, u64)>) -> bool {
        self.overrides.contains_key(rel) || self.downloads.contains(rel) || origin.is_some_and(|o| self.curseforge.contains(&o))
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Fate {
    /// Del modpack: manda la versión nueva, lo traiga o no.
    Pack,
    /// Del jugador: pasa a la versión nueva.
    Keep,
    /// Del modpack, y el jugador lo desactivó.
    Disabled,
}

/// Qué hacer con un archivo de la versión instalada. `rel` va en minúsculas y con «/»; `fresh_has` dice si
/// la versión nueva trae esa misma ruta, y `changed(crc)` si el jugador lo cambió respecto de lo que
/// traía el modpack (solo se pregunta cuando hace falta: hay que leer el archivo).
fn fate(rel: &str, origin: Option<(u64, u64)>, old: &PackFiles, new: &PackFiles, fresh_has: bool, changed: impl FnOnce(u32) -> bool) -> Fate {
    if let Some(base) = rel.strip_suffix(".disabled") {
        return if old.owns(base, origin) { Fate::Disabled } else { Fate::Keep };
    }
    if old.owns(rel, origin) {
        // Un ajuste del modpack que el jugador cambió se queda, salvo que la versión nueva también lo cambie.
        if let Some(&crc) = old.overrides.get(rel) {
            if new.overrides.get(rel) == Some(&crc) && changed(crc) {
                return Fate::Keep;
            }
        }
        return Fate::Pack;
    }
    if fresh_has {
        Fate::Pack
    } else {
        Fate::Keep
    }
}

#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    /// La versión instalada ahora.
    pub version: String,
    /// Mods y paquetes que agregó el jugador y siguen.
    pub kept: Vec<String>,
    /// Los que agregó el jugador y ya vienen en la versión nueva: se quedó la del modpack.
    pub replaced: Vec<String>,
    /// Ajustes del jugador que se conservaron.
    pub settings: usize,
    /// Mods del modpack que el jugador tenía desactivados y siguen así.
    pub disabled: usize,
    /// Lo que no se pudo completar (lo de la instalación de la versión nueva).
    pub warning: Option<String>,
    /// Lo de CurseForge que hay que bajar a mano.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub manual: Vec<crate::curseforge::Manual>,
}

/// Lo que no se pasa archivo por archivo: los datos del jugador se mueven aparte, el estado de la
/// instancia se arma después y los registros del juego no hacen falta.
fn skipped(lower: &str) -> bool {
    let first = lower.split('/').next().unwrap_or_default();
    lower == INSTANCE_FILE
        || [STATE_DIR, "logs", "crash-reports"].contains(&first)
        || crate::packs::PRESERVED_ON_UPDATE.iter().any(|p| {
            let p = p.to_lowercase();
            lower == p || lower.starts_with(&format!("{p}/"))
        })
}

/// Los ids de mod que declara un .jar (sin los que lleva dentro).
fn mod_ids(file: &Path) -> HashSet<String> {
    std::fs::File::open(file).map(crate::compat::read_jar).unwrap_or_default().into_iter().filter(|m| !m.nested).map(|m| m.id).collect()
}

/// `.lanzador/curseforge.json` como (ruta en minúsculas → (proyecto, archivo)) y el JSON tal cual.
fn curseforge_index(game_dir: &Path) -> (HashMap<String, (u64, u64)>, Value) {
    let json: Value = std::fs::read_to_string(game_dir.join(STATE_DIR).join("curseforge.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    let map = json["files"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(k, v)| Some((k.to_lowercase(), (v["projectId"].as_u64()?, v["fileId"].as_u64()?))))
        .collect();
    (map, json)
}

/// Pasa lo del jugador de `old` (la instancia) a `fresh` (la versión nueva, armada aparte) y pone `fresh`
/// en el lugar de `old`. Lo que queda de la versión anterior pasa a un respaldo recuperable.
pub fn apply(paths: &Paths, old: &Instance, fresh: &Instance, old_pack: &PackFiles, new_pack: &PackFiles, label: String) -> Result<Summary, String> {
    if old.folder != old.game_dir || fresh.folder != fresh.game_dir {
        return Err(t!(
            "Esta instancia guarda el juego en otra carpeta: instala la versión nueva del modpack aparte.",
            "This instance keeps the game in another folder: install the new modpack version separately."
        ));
    }
    let mut summary = Summary::default();
    let (old_index, old_index_json) = curseforge_index(&old.game_dir);
    let (_, mut fresh_index_json) = curseforge_index(&fresh.game_dir);
    // Los mods de la versión nueva, para no duplicar uno que el jugador agregó y ahora ya viene.
    let fresh_mods: Vec<(std::path::PathBuf, HashSet<String>)> = std::fs::read_dir(fresh.game_dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
        .map(|p| {
            let ids = mod_ids(&p);
            (p, ids)
        })
        .collect();
    let fresh_ids: HashSet<&String> = fresh_mods.iter().flat_map(|(_, ids)| ids).collect();
    for file in crate::packs::walk(&old.game_dir) {
        let Ok(relative) = file.strip_prefix(&old.game_dir) else { continue };
        let rel = relative.to_string_lossy().replace('\\', "/");
        let lower = rel.to_lowercase();
        if skipped(&lower) || crate::mrpack::safe_relative(&rel).is_none() {
            continue;
        }
        let target = fresh.game_dir.join(&rel);
        let origin = old_index.get(lower.strip_suffix(".disabled").unwrap_or(&lower)).copied();
        let changed = |crc| crate::remote_zip::file_crc32(&file).is_some_and(|now| now != crc);
        match fate(&lower, origin, old_pack, new_pack, target.exists(), changed) {
            Fate::Pack => {}
            Fate::Disabled => {
                let base = target.with_file_name(target.file_name().unwrap().to_string_lossy().trim_end_matches(".disabled").to_string());
                // El mismo archivo, o el mismo mod en otra versión.
                let ids = mod_ids(&file);
                let same = if base.is_file() { Some(base) } else { fresh_mods.iter().find(|(_, other)| !ids.is_empty() && !other.is_disjoint(&ids)).map(|(p, _)| p.clone()) };
                if let Some(same) = same.filter(|p| p.is_file()) {
                    let disabled = same.with_file_name(format!("{}.disabled", same.file_name().unwrap().to_string_lossy()));
                    if std::fs::rename(&same, disabled).is_ok() {
                        summary.disabled += 1;
                    }
                }
            }
            Fate::Keep => {
                let content = ["mods/", "resourcepacks/", "shaderpacks/"].iter().any(|d| lower.starts_with(d)) && lower.matches('/').count() == 1;
                let name = file.file_name().unwrap().to_string_lossy().to_string();
                if lower.starts_with("mods/") && (lower.ends_with(".jar") || lower.ends_with(".jar.disabled")) {
                    let ids = mod_ids(&file);
                    if ids.iter().any(|id| fresh_ids.contains(id)) {
                        summary.replaced.push(name);
                        continue;
                    }
                }
                std::fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
                std::fs::copy(&file, &target).map_err(|e| t!("No se pudo pasar {rel} a la versión nueva: {e}", "Could not carry {rel} over to the new version: {e}"))?;
                if content {
                    summary.kept.push(name);
                    // Lo que el jugador instaló de CurseForge sigue reconociéndose (actualizaciones, Explorar).
                    if let Some((key, entry)) = old_index_json["files"].as_object().and_then(|files| {
                        files.iter().find(|(k, _)| k.to_lowercase() == lower.strip_suffix(".disabled").unwrap_or(&lower))
                    }) {
                        fresh_index_json["files"][key] = entry.clone();
                    }
                } else {
                    summary.settings += 1;
                }
            }
        }
    }
    if fresh_index_json["files"].is_object() {
        let state = fresh.game_dir.join(STATE_DIR);
        let _ = std::fs::create_dir_all(&state);
        std::fs::write(state.join("curseforge.json"), serde_json::to_vec(&fresh_index_json).unwrap()).map_err(|e| e.to_string())?;
    }
    // El icono (el del modpack o el que eligió el jugador) se queda.
    let icon = old.state_dir().join("icono.png");
    if icon.is_file() {
        let _ = std::fs::copy(&icon, fresh.state_dir().join("icono.png"));
    }
    // instancia.json: el de antes (nombre, memoria, Java…) con el Minecraft, el loader y el modpack nuevos.
    let read = |folder: &Path| std::fs::read_to_string(folder.join(INSTANCE_FILE)).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let (Some(mut json), Some(new_json)) = (read(&old.folder), read(&fresh.folder)) else {
        return Err(t!("No se pudo leer instancia.json.", "Could not read instancia.json."));
    };
    for key in ["versionId", "minecraftVersion", "loader", "loaderVersion", "modpack"] {
        match new_json.get(key) {
            Some(value) => json[key] = value.clone(),
            None => {
                json.as_object_mut().map(|o| o.remove(key));
            }
        }
    }
    if json.get("ramGB").is_none() {
        if let Some(ram) = new_json.get("ramGB") {
            json["ramGB"] = ram.clone();
        }
    }
    std::fs::write(fresh.folder.join(INSTANCE_FILE), serde_json::to_vec_pretty(&json).unwrap()).map_err(|e| e.to_string())?;
    // El cambio: la versión nueva toma el lugar de la anterior, con los datos del jugador.
    let backup = paths.data.join("temporal").join(format!("respaldo-{}", chrono::Utc::now().timestamp_millis()));
    crate::packs::replace_keeping_player_data(&old.folder, &fresh.folder, &backup).map_err(|e| t!("No se pudo actualizar la instancia: {e}", "Could not update the instance: {e}"))?;
    let mut kept = crate::respaldos::Builder::start(paths, &old.id, &old.folder, label);
    if kept.adopt(&backup, &old.folder).is_ok() {
        kept.finish(paths);
    }
    let _ = std::fs::remove_dir_all(&backup);
    summary.kept.sort();
    summary.replaced.sort();
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn pack(overrides: &[(&str, u32)], downloads: &[&str], curseforge: &[(u64, u64)]) -> PackFiles {
        PackFiles {
            overrides: overrides.iter().map(|(p, c)| (p.to_string(), *c)).collect(),
            downloads: downloads.iter().map(|p| p.to_string()).collect(),
            curseforge: curseforge.iter().copied().collect(),
        }
    }

    #[test]
    fn decide_que_es_del_modpack_y_que_del_jugador() {
        let old = pack(&[("config/a.toml", 1), ("config/b.toml", 2)], &["mods/sodium.jar"], &[(10, 100)]);
        let new = pack(&[("config/a.toml", 1), ("config/b.toml", 3)], &["mods/sodium-2.jar"], &[(10, 101)]);
        let never = |_: u32| -> bool { panic!("no hacía falta leerlo") };
        // Lo del modpack lo decide la versión nueva (aunque ya no lo traiga).
        assert_eq!(fate("mods/sodium.jar", None, &old, &new, false, never), Fate::Pack);
        assert_eq!(fate("mods/jei.jar", Some((10, 100)), &old, &new, false, never), Fate::Pack);
        // Un ajuste que cambió el jugador y la versión nueva no: se queda el del jugador.
        assert_eq!(fate("config/a.toml", None, &old, &new, true, |_| true), Fate::Keep);
        assert_eq!(fate("config/a.toml", None, &old, &new, true, |_| false), Fate::Pack, "sin cambios, da igual");
        // Si la versión nueva también lo cambió, manda la nueva.
        assert_eq!(fate("config/b.toml", None, &old, &new, true, never), Fate::Pack);
        // Lo que agregó el jugador (o generó un mod) pasa, salvo que la versión nueva traiga esa misma ruta.
        assert_eq!(fate("mods/mio.jar", None, &old, &new, false, never), Fate::Keep);
        assert_eq!(fate("config/generado.json", None, &old, &new, false, never), Fate::Keep);
        assert_eq!(fate("config/generado.json", None, &old, &new, true, never), Fate::Pack);
        // Desactivados: los del modpack se recuerdan; los propios pasan tal cual.
        assert_eq!(fate("mods/sodium.jar.disabled", None, &old, &new, false, never), Fate::Disabled);
        assert_eq!(fate("mods/jei.jar.disabled", Some((10, 100)), &old, &new, false, never), Fate::Disabled);
        assert_eq!(fate("mods/mio.jar.disabled", None, &old, &new, false, never), Fate::Keep);
    }

    fn zip(path: &Path, files: &[(&str, &str)]) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        for (name, text) in files {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }

    #[test]
    fn lee_mrpack_y_curseforge() {
        let dir = std::env::temp_dir().join(format!("parabellum-actualizar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let index = r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"P","dependencies":{"minecraft":"1.20.1","fabric-loader":"0.16.10"},
            "files":[{"path":"mods/sodium.jar","hashes":{"sha1":"0000000000000000000000000000000000000000","sha512":"x"},"downloads":["https://cdn.modrinth.com/data/a/versions/b/sodium.jar"],"fileSize":1}]}"#;
        let mrpack = dir.join("p.mrpack");
        zip(&mrpack, &[("modrinth.index.json", index), ("overrides/config/A.toml", "a"), ("client-overrides/config/A.toml", "b")]);
        let read = PackFiles::read(&mrpack).unwrap();
        assert!(read.downloads.contains("mods/sodium.jar"));
        assert_eq!(read.overrides.get("config/a.toml"), Some(&crc32fast::hash(b"b")), "manda el client-override");
        let manifest = r#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.4.0","primary":true}]},"manifestType":"minecraftModpack","name":"C","version":"1","files":[{"projectID":5,"fileID":6,"required":true}],"overrides":"overrides"}"#;
        let curse = dir.join("c.zip");
        zip(&curse, &[("manifest.json", manifest), ("overrides/config/b.toml", "x")]);
        let read = PackFiles::read(&curse).unwrap();
        assert!(read.curseforge.contains(&(5, 6)));
        assert!(read.overrides.contains_key("config/b.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
