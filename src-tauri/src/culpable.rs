//! «Buscar el mod culpable»: cuando el juego no llega al menú y el registro no dice qué mod lo rompe,
//! se prueba por mitades. Primero sin ningún mod (si tampoco abre, no es un mod); después se desactiva
//! la mitad de los sospechosos, se abre el juego hasta el menú y se sigue con la mitad donde está el
//! problema. Con 150 mods son unas 8 pruebas.
//!
//! Junto con una mitad se desactivan los mods que dependen de ella (si no, el juego fallaría por la
//! dependencia que falta y la prueba diría mal), así que las mitades se arman con mods «hoja» primero.
//!
//! Antes de tocar nada se anota qué se desactivó (`.lanzador/buscar-culpable.json`): si Parabellum se
//! cierra a la mitad, al volver a jugar o a buscar se deja todo como estaba (`restore`).

use std::collections::{BTreeSet, HashMap};

use crate::compat::ModMeta;
use crate::instances::Instance;

const JOURNAL: &str = "buscar-culpable.json";
const DISABLED: &str = ".disabled";

/// Los ids que no da ningún mod (el juego, el loader, Java): no cuentan como dependencia entre mods.
pub struct Mods {
    /// (archivo, nombre para mostrar)
    pub files: Vec<(String, String)>,
    /// Por archivo: los ids que necesita.
    requires: HashMap<String, Vec<String>>,
    /// Por id: los archivos que lo dan.
    providers: HashMap<String, Vec<String>>,
}

impl Mods {
    pub fn new(list: Vec<(String, Vec<ModMeta>)>) -> Self {
        let mut providers: HashMap<String, Vec<String>> = HashMap::new();
        let mut requires = HashMap::new();
        let mut files = Vec::new();
        for (file, metas) in list {
            for meta in &metas {
                for id in std::iter::once(&meta.id).chain(&meta.provides) {
                    providers.entry(id.clone()).or_default().push(file.clone());
                }
            }
            let needs: Vec<String> = metas.iter().filter(|m| !m.nested).flat_map(|m| m.depends.iter().map(|(id, _)| id.clone())).collect();
            requires.insert(file.clone(), needs);
            let name = metas.iter().find(|m| !m.nested).map(|m| m.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| file.clone());
            files.push((file, name));
        }
        files.sort();
        Self { files, requires, providers }
    }

    pub fn name(&self, file: &str) -> String {
        self.files.iter().find(|(f, _)| f == file).map(|(_, n)| n.clone()).unwrap_or_else(|| file.to_string())
    }

    /// `set` y los mods que se quedarían sin algo que piden si se desactiva `set` (de todos los activos).
    pub fn closure(&self, set: &BTreeSet<String>) -> BTreeSet<String> {
        let mut out = set.clone();
        loop {
            let mut changed = false;
            for (file, _) in &self.files {
                if out.contains(file) {
                    continue;
                }
                let orphan = self.requires.get(file).into_iter().flatten().any(|id| {
                    let providers = self.providers.get(id).map(Vec::as_slice).unwrap_or_default();
                    // Lo que da él mismo no cuenta; si lo da otro que sigue activo, tampoco.
                    let others: Vec<&String> = providers.iter().filter(|p| *p != file).collect();
                    !others.is_empty() && others.iter().all(|p| out.contains(*p))
                });
                if orphan {
                    out.insert(file.clone());
                    changed = true;
                }
            }
            if !changed {
                return out;
            }
        }
    }

    /// Qué desactivar en la siguiente prueba: un conjunto cerrado (con sus dependientes) que tenga más
    /// o menos la mitad de los sospechosos, empezando por los que nadie necesita. None si no se pueden
    /// separar (todos dependen de todos).
    pub fn split(&self, candidates: &BTreeSet<String>) -> Option<BTreeSet<String>> {
        if candidates.len() < 2 {
            return None;
        }
        let target = candidates.len().div_ceil(2);
        let mut seeds: Vec<(usize, &String)> = candidates
            .iter()
            .map(|c| (self.closure(&BTreeSet::from([c.clone()])).intersection(candidates).count(), c))
            .collect();
        seeds.sort();
        let mut chosen: BTreeSet<String> = BTreeSet::new();
        for (_, seed) in seeds {
            let next = self.closure(&chosen.iter().cloned().chain([seed.clone()]).collect());
            let inside = next.intersection(candidates).count();
            if inside >= candidates.len() {
                continue;
            }
            chosen = next;
            if inside >= target {
                break;
            }
        }
        let inside = chosen.intersection(candidates).count();
        (inside > 0 && inside < candidates.len()).then_some(chosen)
    }
}

/// Desactiva esos mods (`.jar` → `.jar.disabled`), anotándolo antes para poder deshacerlo.
pub fn disable(instance: &Instance, files: &BTreeSet<String>) -> Result<(), String> {
    let _ = std::fs::create_dir_all(instance.state_dir());
    std::fs::write(instance.state_dir().join(JOURNAL), serde_json::to_vec(files).unwrap()).map_err(|e| e.to_string())?;
    let folder = instance.game_dir.join("mods");
    for file in files {
        let from = folder.join(file);
        if from.is_file() {
            std::fs::rename(&from, folder.join(format!("{file}{DISABLED}"))).map_err(|e| t!("No se pudo desactivar {file}: {e}", "Could not disable {file}: {e}"))?;
        }
    }
    Ok(())
}

/// Deja los mods como estaban antes de una prueba (si quedó una a medias). Devuelve cuántos se activaron.
pub fn restore(instance: &Instance) -> usize {
    let journal = instance.state_dir().join(JOURNAL);
    let Some(files) = std::fs::read(&journal).ok().and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok()) else { return 0 };
    let folder = instance.game_dir.join("mods");
    let mut restored = 0;
    for file in files.iter().filter(|f| !f.is_empty() && !f.contains(['/', '\\'])) {
        let (disabled, enabled) = (folder.join(format!("{file}{DISABLED}")), folder.join(file));
        if disabled.is_file() && !enabled.exists() && std::fs::rename(&disabled, &enabled).is_ok() {
            restored += 1;
        }
    }
    let _ = std::fs::remove_file(journal);
    restored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, depends: &[&str]) -> ModMeta {
        ModMeta {
            id: id.into(),
            name: id.into(),
            version: "1".into(),
            provides: Vec::new(),
            depends: depends.iter().map(|d| (d.to_string(), Vec::new())).collect(),
            breaks: Vec::new(),
            forge: false,
            neo_toml: false,
            nested: false,
        }
    }

    fn mods() -> Mods {
        // api ← a, b; b ← c (c necesita b); d y e sueltos. Todos piden «minecraft», que no da nadie.
        Mods::new(vec![
            ("api.jar".into(), vec![meta("api", &["minecraft"])]),
            ("a.jar".into(), vec![meta("a", &["api", "minecraft"])]),
            ("b.jar".into(), vec![meta("b", &["api"])]),
            ("c.jar".into(), vec![meta("c", &["b"])]),
            ("d.jar".into(), vec![meta("d", &[])]),
            ("e.jar".into(), vec![meta("e", &[])]),
        ])
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn dependientes_se_desactivan_juntos() {
        let m = mods();
        assert_eq!(m.closure(&set(&["api.jar"])), set(&["api.jar", "a.jar", "b.jar", "c.jar"]));
        assert_eq!(m.closure(&set(&["b.jar"])), set(&["b.jar", "c.jar"]));
        assert_eq!(m.closure(&set(&["d.jar"])), set(&["d.jar"]));
    }

    /// Simula la búsqueda: el juego abre solo si el culpable está desactivado.
    fn find(m: &Mods, culprit: &str) -> (Option<String>, u32) {
        let mut candidates: BTreeSet<String> = m.files.iter().map(|(f, _)| f.clone()).collect();
        let mut launches = 0;
        while let Some(disabled) = m.split(&candidates) {
            launches += 1;
            let opens = disabled.contains(culprit);
            candidates = if opens { candidates.intersection(&disabled).cloned().collect() } else { candidates.difference(&disabled).cloned().collect() };
        }
        ((candidates.len() == 1).then(|| candidates.into_iter().next().unwrap()), launches)
    }

    #[test]
    fn encuentra_al_culpable_por_mitades() {
        let m = mods();
        for culprit in ["api.jar", "a.jar", "b.jar", "c.jar", "d.jar", "e.jar"] {
            let (found, launches) = find(&m, culprit);
            assert_eq!(found.as_deref(), Some(culprit), "culpable {culprit}");
            assert!(launches <= 4, "{culprit}: {launches} pruebas");
        }
        // 150 mods sueltos: 8 pruebas.
        let many = Mods::new((0..150).map(|i| (format!("m{i:03}.jar"), vec![meta(&format!("m{i}"), &[])])).collect());
        let (found, launches) = find(&many, "m117.jar");
        assert_eq!(found.as_deref(), Some("m117.jar"));
        assert!(launches <= 8, "{launches}");
    }
}
