//! Copias recuperables (docs/plan-diciembre.md, mejora 1): antes de actualizar, reparar o restaurar el
//! paquete de una instancia, lo que se va a reemplazar o quitar se guarda aquí, y se puede volver a
//! poner con «Restaurar». Los datos del jugador (mundos, opciones, capturas) no se tocan al actualizar,
//! así que no hace falta copiarlos.
//!
//! `datos/respaldos/<instancia>/<id>/`: `respaldo.json` y `archivos/` (las rutas relativas a la carpeta
//! de la instancia). Se guardan las 3 más recientes de cada instancia y, entre todas, como mucho 4 GB
//! (la más reciente de cada instancia se queda siempre).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::paths::Paths;

const FOLDER: &str = "respaldos";
const INFO_FILE: &str = "respaldo.json";
const FILES: &str = "archivos";
const KEEP_PER_INSTANCE: usize = 3;
const MAX_TOTAL: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Info {
    pub id: String,
    pub instance: String,
    /// Para qué se hizo («Antes de actualizar RETORCIDOS»), en el idioma de cuando se hizo.
    pub label: String,
    pub created: i64,
    pub files: usize,
    pub bytes: u64,
    /// Lo que el cambio agregó (rutas relativas): al restaurar se quita.
    pub added: Vec<String>,
}

fn root(paths: &Paths) -> PathBuf {
    paths.data.join(FOLDER)
}

fn instance_dir(paths: &Paths, instance: &str) -> PathBuf {
    root(paths).join(sanitize(instance))
}

/// El id de la instancia es el nombre de su carpeta (ya es un nombre válido); por si acaso, sin separadores.
fn sanitize(name: &str) -> String {
    name.chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect()
}

fn relative(base: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(base).ok()?.to_string_lossy().replace('\\', "/");
    crate::mrpack::safe_relative(&rel).map(|_| rel)
}

/// Un respaldo en preparación: se llena con `keep`, `take` y `added`, y se cierra con `finish`.
pub struct Builder {
    dir: PathBuf,
    folder: PathBuf,
    info: Info,
}

impl Builder {
    pub fn start(paths: &Paths, instance: &str, folder: &Path, label: String) -> Builder {
        let created = chrono::Utc::now().timestamp_millis();
        let id = format!("{created}");
        let dir = instance_dir(paths, instance).join(&id);
        Builder { dir, folder: folder.to_path_buf(), info: Info { id, instance: instance.to_string(), label, created, ..Default::default() } }
    }

    fn target(&self, rel: &str) -> PathBuf {
        self.dir.join(FILES).join(rel)
    }

    fn already(&self, rel: &str) -> bool {
        self.target(rel).exists()
    }

    /// Copia un archivo de la instancia antes de que se reemplace (si existe; la primera vez vale).
    pub fn keep(&mut self, file: &Path) {
        let Some(rel) = relative(&self.folder, file) else { return };
        if !file.is_file() || self.already(&rel) {
            if !file.exists() && !self.info.added.contains(&rel) {
                self.info.added.push(rel);
            }
            return;
        }
        let target = self.target(&rel);
        if std::fs::create_dir_all(target.parent().unwrap()).is_ok() && std::fs::copy(file, &target).is_ok() {
            self.info.files += 1;
            self.info.bytes += file.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }

    /// Mueve un archivo de la instancia al respaldo (en lugar de borrarlo). Si no se puede mover, false.
    pub fn take(&mut self, file: &Path) -> bool {
        let Some(rel) = relative(&self.folder, file) else { return false };
        // Ya se había copiado antes de cambiarlo: basta con quitarlo.
        if self.already(&rel) {
            return std::fs::remove_file(file).is_ok();
        }
        let target = self.target(&rel);
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        let moved = std::fs::create_dir_all(target.parent().unwrap()).is_ok()
            && (std::fs::rename(file, &target).is_ok() || (std::fs::copy(file, &target).is_ok() && std::fs::remove_file(file).is_ok()));
        if moved {
            self.info.files += 1;
            self.info.bytes += size;
        }
        moved
    }

    /// Toma como respaldo una carpeta entera que ya se apartó (la versión anterior de la instancia):
    /// sus archivos pasan tal cual; lo que la versión nueva trae y aquella no, cuenta como agregado.
    pub fn adopt(&mut self, old: &Path, new: &Path) -> std::io::Result<()> {
        let files = crate::packs::walk(old);
        std::fs::create_dir_all(&self.dir)?;
        std::fs::rename(old, self.dir.join(FILES))?;
        self.info.files += files.len();
        self.info.bytes += files.iter().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum::<u64>();
        let preserved = |rel: &str| {
            let lower = rel.to_lowercase();
            crate::packs::PRESERVED_ON_UPDATE.iter().any(|p| {
                let p = p.to_lowercase();
                lower == p || lower.starts_with(&format!("{p}/"))
            })
        };
        for file in crate::packs::walk(new) {
            if let Some(rel) = relative(new, &file) {
                if !preserved(&rel) && !self.already(&rel) {
                    self.info.added.push(rel);
                }
            }
        }
        Ok(())
    }

    /// Cierra el respaldo. Sin nada guardado ni agregado no queda nada. Después se aplica la retención.
    pub fn finish(self, paths: &Paths) -> Option<Info> {
        if self.info.files == 0 && self.info.added.is_empty() {
            let _ = std::fs::remove_dir_all(&self.dir);
            return None;
        }
        let _ = std::fs::create_dir_all(&self.dir);
        std::fs::write(self.dir.join(INFO_FILE), serde_json::to_vec_pretty(&self.info).ok()?).ok()?;
        prune(paths);
        Some(self.info)
    }
}

/// Los respaldos de una instancia, del más nuevo al más viejo.
pub fn list(paths: &Paths, instance: &str) -> Vec<Info> {
    let mut list: Vec<Info> = std::fs::read_dir(instance_dir(paths, instance))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| serde_json::from_slice::<Info>(&std::fs::read(e.path().join(INFO_FILE)).ok()?).ok())
        .collect();
    list.sort_by_key(|i| std::cmp::Reverse(i.created));
    list
}

/// Las 3 más recientes de cada instancia y 4 GB en total (sin tocar la más reciente de cada una).
pub fn prune(paths: &Paths) {
    let mut all: Vec<(Info, PathBuf, bool)> = Vec::new();
    for entry in std::fs::read_dir(root(paths)).into_iter().flatten().flatten() {
        let instance = entry.file_name().to_string_lossy().into_owned();
        for (index, info) in list(paths, &instance).into_iter().enumerate() {
            let dir = entry.path().join(&info.id);
            if index >= KEEP_PER_INSTANCE {
                let _ = std::fs::remove_dir_all(&dir);
            } else {
                all.push((info, dir, index == 0));
            }
        }
        // Carpetas sin respaldo.json (un respaldo que no terminó): fuera.
        for leftover in std::fs::read_dir(entry.path()).into_iter().flatten().flatten() {
            if leftover.path().is_dir() && !leftover.path().join(INFO_FILE).exists() {
                let age = leftover.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| t.elapsed().ok()).map(|d| d.as_secs()).unwrap_or(0);
                if age > 3600 {
                    let _ = std::fs::remove_dir_all(leftover.path());
                }
            }
        }
    }
    let mut total: u64 = all.iter().map(|(i, _, _)| i.bytes).sum();
    all.sort_by_key(|(i, _, _)| i.created);
    for (info, dir, newest) in all {
        if total <= MAX_TOTAL {
            break;
        }
        if !newest {
            let _ = std::fs::remove_dir_all(&dir);
            total = total.saturating_sub(info.bytes);
        }
    }
}

/// Vuelve a poner un respaldo en la instancia. Antes guarda otro con lo que va a reemplazar (así
/// restaurar también se puede deshacer). Devuelve cuántos archivos se repusieron.
pub fn restore(paths: &Paths, instance: &crate::instances::Instance, id: &str) -> Result<usize, String> {
    let dir = instance_dir(paths, &instance.id).join(sanitize(id));
    let info: Info = serde_json::from_slice(&std::fs::read(dir.join(INFO_FILE)).map_err(|_| t!("Ese respaldo ya no está.", "That backup is no longer there."))?).map_err(|e| e.to_string())?;
    let files_dir = dir.join(FILES);
    let saved: Vec<(PathBuf, String)> = crate::packs::walk(&files_dir).into_iter().filter_map(|f| Some((f.clone(), relative(&files_dir, &f)?))).collect();
    let when = chrono::DateTime::from_timestamp_millis(info.created).map(|d| d.with_timezone(&chrono::Local).format("%d/%m %H:%M").to_string()).unwrap_or_default();
    let mut undo = Builder::start(paths, &instance.id, &instance.folder, t!("Antes de volver a la copia del {when}", "Before going back to the backup from {when}"));
    for (_, rel) in &saved {
        undo.keep(&instance.folder.join(rel));
    }
    // Lo que había agregado aquel cambio se aparta (va al respaldo nuevo, no se borra).
    for rel in &info.added {
        let Some(safe) = crate::mrpack::safe_relative(rel) else { continue };
        let file = instance.folder.join(safe);
        if file.is_file() {
            undo.take(&file);
        }
    }
    let mut restored = 0;
    for (from, rel) in &saved {
        let Some(safe) = crate::mrpack::safe_relative(rel) else { continue };
        let to = instance.folder.join(safe);
        std::fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::copy(from, &to).map_err(|e| t!("No se pudo reponer {rel}: {e}", "Could not put back {rel}: {e}"))?;
        restored += 1;
    }
    undo.finish(paths);
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(root: &Path) -> Paths {
        Paths::new(root)
    }

    #[test]
    fn respaldar_y_restaurar() {
        let root = std::env::temp_dir().join(format!("parabellum-respaldos-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        let paths = paths_in(&root);
        let folder = paths.instances.join("Prueba");
        std::fs::create_dir_all(folder.join("mods")).unwrap();
        std::fs::write(folder.join("mods/a.jar"), b"a v1").unwrap();
        std::fs::write(folder.join("mods/viejo.jar"), b"viejo").unwrap();

        // Una actualización: a.jar cambia, viejo.jar se quita y nuevo.jar llega.
        let mut backup = Builder::start(&paths, "Prueba", &folder, "Antes de actualizar".into());
        backup.keep(&folder.join("mods/a.jar"));
        std::fs::write(folder.join("mods/a.jar"), b"a v2").unwrap();
        assert!(backup.take(&folder.join("mods/viejo.jar")));
        backup.keep(&folder.join("mods/nuevo.jar"));
        std::fs::write(folder.join("mods/nuevo.jar"), b"nuevo").unwrap();
        let info = backup.finish(&paths).unwrap();
        assert_eq!(info.files, 2);
        assert_eq!(info.added, vec!["mods/nuevo.jar"]);
        assert!(!folder.join("mods/viejo.jar").exists());

        let instance = crate::instances::Instance { id: "Prueba".into(), folder: folder.clone(), game_dir: folder.clone(), ..Default::default() };
        assert_eq!(restore(&paths, &instance, &info.id).unwrap(), 2);
        assert_eq!(std::fs::read(folder.join("mods/a.jar")).unwrap(), b"a v1");
        assert_eq!(std::fs::read(folder.join("mods/viejo.jar")).unwrap(), b"viejo");
        assert!(!folder.join("mods/nuevo.jar").exists());
        // Restaurar dejó su propio respaldo (con nuevo.jar y la a.jar v2).
        let all = list(&paths, "Prueba");
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|i| i.label == "Antes de actualizar"));
        let undo = all.iter().find(|i| i.label != "Antes de actualizar").unwrap();
        assert_eq!(undo.files, 2, "la a.jar v2 y la nuevo.jar apartada");
        assert!(undo.label.contains("copia del") || undo.label.contains("backup from"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn se_quedan_las_tres_mas_nuevas() {
        let root = std::env::temp_dir().join(format!("parabellum-retencion-{}", chrono::Utc::now().timestamp_nanos_opt().unwrap()));
        let paths = paths_in(&root);
        let folder = paths.instances.join("Prueba");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("x.txt"), b"x").unwrap();
        for i in 0..5 {
            let mut b = Builder::start(&paths, "Prueba", &folder, format!("copia {i}"));
            b.info.id = format!("{}", 1000 + i);
            b.info.created = 1000 + i;
            b.dir = instance_dir(&paths, "Prueba").join(&b.info.id);
            b.keep(&folder.join("x.txt"));
            b.finish(&paths);
        }
        let ids: Vec<String> = list(&paths, "Prueba").into_iter().map(|i| i.id).collect();
        assert_eq!(ids, vec!["1004", "1003", "1002"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
