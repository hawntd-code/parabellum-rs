//! Lectura de los JSON de versión de Minecraft (incluida la herencia "inheritsFrom" de los loaders),
//! reglas por sistema operativo y coordenadas Maven.

use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};

use crate::paths::Paths;

pub type Features = HashMap<&'static str, bool>;

/// Evalúa las reglas "allow/disallow" de bibliotecas y argumentos para Windows.
pub fn rules_allow(rules: Option<&Value>, features: &Features) -> bool {
    let Some(Value::Array(rules)) = rules else { return true };
    if rules.is_empty() {
        return true;
    }
    let mut allowed = false;
    for rule in rules.iter().filter_map(Value::as_object) {
        if rule_matches(rule, features) {
            allowed = rule.get("action").and_then(Value::as_str) == Some("allow");
        }
    }
    allowed
}

fn rule_matches(rule: &Map<String, Value>, features: &Features) -> bool {
    if let Some(os) = rule.get("os").and_then(Value::as_object) {
        if let Some(name) = os.get("name").and_then(Value::as_str) {
            if name != crate::plataforma::MOJANG_OS {
                return false;
            }
        }
        if let Some(arch) = os.get("arch").and_then(Value::as_str) {
            let current = if cfg!(target_arch = "aarch64") { "arm64" } else if cfg!(target_arch = "x86") { "x86" } else { "x86_64" };
            if arch != current {
                return false;
            }
        }
    }
    if let Some(required) = rule.get("features").and_then(Value::as_object) {
        for (key, wanted) in required {
            let wanted = wanted.as_bool().unwrap_or(false);
            if features.get(key.as_str()).copied().unwrap_or(false) != wanted {
                return false;
            }
        }
    }
    true
}

/// "grupo:artefacto:versión[:clasificador][@extensión]"
#[derive(Clone, Debug)]
pub struct Maven {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl Maven {
    pub fn parse(text: &str) -> Option<Self> {
        let (coordinate, extension) = match text.split_once('@') {
            Some((c, e)) => (c, e.to_string()),
            None => (text, "jar".to_string()),
        };
        let parts: Vec<&str> = coordinate.split(':').collect();
        if parts.len() < 3 {
            return None;
        }
        Some(Self {
            group: parts[0].into(),
            artifact: parts[1].into(),
            version: parts[2].into(),
            classifier: parts.get(3).map(|s| s.to_string()),
            extension,
        })
    }

    pub fn relative_path(&self) -> String {
        let classifier = self.classifier.as_ref().map(|c| format!("-{c}")).unwrap_or_default();
        format!(
            "{}/{}/{}/{}-{}{}.{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            self.artifact,
            self.version,
            classifier,
            self.extension
        )
    }

    /// La misma biblioteca en otra versión cuenta como duplicada (gana la del loader).
    pub fn dedup_key(&self) -> String {
        format!("{}:{}:{}:{}", self.group, self.artifact, self.classifier.as_deref().unwrap_or(""), self.extension)
    }
}

/// Una versión con toda su cadena de herencia ya combinada.
#[derive(Debug)]
pub struct ResolvedVersion {
    pub id: String,
    pub root_id: String,
    pub main_class: String,
    pub libraries: Vec<Value>,
    pub jvm_arguments: Vec<Value>,
    pub game_arguments: Vec<Value>,
    pub legacy_arguments: Option<String>,
    pub asset_index: Option<String>,
    pub logging: Option<Value>,
    pub java_component: Option<String>,
    pub java_major: u32,
    pub client_jar_id: String,
    pub version_type: String,
}

impl ResolvedVersion {
    pub fn resolve(version_id: &str, paths: &Paths) -> Result<Self, String> {
        let mut chain: Vec<Value> = Vec::new();
        let mut ids: Vec<String> = Vec::new();
        let mut next = Some(version_id.to_string());
        while let Some(id) = next {
            if ids.contains(&id) {
                return Err(t!("Herencia circular en la versión {id}.", "Circular inheritance in version {id}."));
            }
            let file = paths.version_json(&id);
            let text = std::fs::read_to_string(&file).map_err(|_| t!("Falta la versión \"{id}\" ({}).", "Version \"{id}\" is missing ({}).", file.display()))?;
            let json: Value = serde_json::from_str(&text).map_err(|e| t!("El JSON de la versión {id} está dañado: {e}", "The JSON of version {id} is damaged: {e}"))?;
            next = json.get("inheritsFrom").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from);
            ids.push(json.get("id").and_then(Value::as_str).unwrap_or(&id).to_string());
            chain.push(json);
        }

        // Bibliotecas: primero las del hijo (loader), luego las del padre, sin duplicados. Las de otro
        // sistema salen antes de buscar duplicados: 1.18.2 trae LWJGL 3.2.1 (solo Mac) y 3.2.2 (los
        // demás) y, si no, la de Mac tapaba a la buena y el juego no arrancaba.
        let mut libraries = Vec::new();
        let mut seen = HashSet::new();
        for version in &chain {
            for library in version.get("libraries").and_then(Value::as_array).into_iter().flatten() {
                if !rules_allow(library.get("rules"), &Features::new()) {
                    continue;
                }
                let Some(name) = library.get("name").and_then(Value::as_str) else { continue };
                let Some(maven) = Maven::parse(name) else { continue };
                // El formato antiguo repite la biblioteca: una entrada con el .jar y otra con los
                // nativos (las .dll); las dos hacen falta.
                let key = if library.get("natives").is_some() { format!("{}#nativos", maven.dedup_key()) } else { maven.dedup_key() };
                if seen.insert(key) {
                    libraries.push(library.clone());
                }
            }
        }

        // Argumentos: los del padre primero y luego los del hijo, como el launcher oficial.
        let mut jvm = Vec::new();
        let mut game = Vec::new();
        for version in chain.iter().rev() {
            if let Some(arguments) = version.get("arguments") {
                jvm.extend(arguments.get("jvm").and_then(Value::as_array).into_iter().flatten().cloned());
                game.extend(arguments.get("game").and_then(Value::as_array).into_iter().flatten().cloned());
            }
        }

        let first_str = |key: &str| chain.iter().find_map(|v| v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from));
        let first_obj = |key: &str| chain.iter().find_map(|v| v.get(key).filter(|o| o.is_object()).cloned());
        let java = first_obj("javaVersion");
        Ok(Self {
            id: ids[0].clone(),
            root_id: ids.last().unwrap().clone(),
            main_class: first_str("mainClass").ok_or(t!("La versión no define mainClass.", "The version does not define mainClass."))?,
            libraries,
            jvm_arguments: jvm,
            game_arguments: game,
            legacy_arguments: first_str("minecraftArguments"),
            asset_index: first_obj("assetIndex")
                .and_then(|a| a.get("id").and_then(Value::as_str).map(String::from))
                .or_else(|| first_str("assets")),
            logging: first_obj("logging"),
            java_component: java.as_ref().and_then(|j| j.get("component").and_then(Value::as_str).map(String::from)),
            java_major: java.as_ref().and_then(|j| j.get("majorVersion").and_then(Value::as_u64)).unwrap_or(8) as u32,
            client_jar_id: first_str("jar").unwrap_or_else(|| ids.last().unwrap().clone()),
            version_type: first_str("type").unwrap_or_else(|| "release".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1.18.2: la misma biblioteca para Mac (3.2.1) y para los demás (3.2.2), cada una con su entrada de
    /// nativos; en Windows quedan las dos de la 3.2.2.
    #[test]
    #[cfg_attr(target_os = "macos", ignore = "en Mac queda la de Mac")]
    fn duplicadas_por_sistema() {
        let dir = std::env::temp_dir().join(format!("parabellum-version-{}", std::process::id()));
        let paths = Paths::new(&dir);
        let json = serde_json::json!({
            "id": "prueba", "mainClass": "net.minecraft.client.main.Main",
            "libraries": [
                { "name": "org.lwjgl:lwjgl:3.2.1", "rules": [{ "action": "allow", "os": { "name": "osx" } }] },
                { "name": "org.lwjgl:lwjgl:3.2.2", "rules": [{ "action": "allow" }, { "action": "disallow", "os": { "name": "osx" } }] },
                { "name": "org.lwjgl:lwjgl:3.2.1", "natives": { "osx": "natives-macos" }, "rules": [{ "action": "allow", "os": { "name": "osx" } }] },
                { "name": "org.lwjgl:lwjgl:3.2.2", "natives": { "windows": "natives-windows" }, "rules": [{ "action": "allow" }, { "action": "disallow", "os": { "name": "osx" } }] }
            ]
        });
        let file = paths.version_json("prueba");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, json.to_string()).unwrap();
        let resolved = ResolvedVersion::resolve("prueba", &paths).unwrap();
        let names: Vec<&str> = resolved.libraries.iter().filter_map(|l| l["name"].as_str()).collect();
        assert_eq!(names, vec!["org.lwjgl:lwjgl:3.2.2", "org.lwjgl:lwjgl:3.2.2"]);
        assert!(resolved.libraries[1].get("natives").is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
