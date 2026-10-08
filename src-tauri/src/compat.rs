//! Compatibilidad entre mods de Fabric (y Quilt, que los carga): cada mod declara en su
//! fabric.mod.json qué necesita ("depends") y con qué choca ("breaks"), con rangos de versión
//! como "0.6.x", ">=1.8.13" o "^2.0". Modrinth no guarda esos rangos, así que se leen de los .jar.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};

/// Lo que declara un mod (o una biblioteca incluida dentro de otro mod).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ModMeta {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub provides: Vec<String>,
    /// (id, alternativas): basta con que se cumpla una.
    #[serde(default)]
    pub depends: Vec<(String, Vec<String>)>,
    #[serde(default)]
    pub breaks: Vec<(String, Vec<String>)>,
}

/// Mods de un .jar: el principal y los que lleva dentro (META-INF/jars), como los módulos de Fabric API.
pub fn read_jar(bytes_or_file: impl Read + Seek) -> Vec<ModMeta> {
    let mut result = Vec::new();
    read_into(bytes_or_file, 0, &mut result);
    result
}

fn read_into(reader: impl Read + Seek, depth: u32, result: &mut Vec<ModMeta>) {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else { return };
    let Some(meta) = read_entry(&mut zip, "fabric.mod.json", 2_000_000).and_then(|bytes| parse_fabric(&bytes)) else { return };
    let nested: Vec<String> = if depth < 2 {
        zip.file_names().filter(|n| n.starts_with("META-INF/jars/") && n.ends_with(".jar")).map(String::from).collect()
    } else {
        Vec::new()
    };
    result.push(meta);
    for name in nested {
        if let Some(bytes) = read_entry(&mut zip, &name, 50_000_000) {
            read_into(Cursor::new(bytes), depth + 1, result);
        }
    }
}

fn read_entry<R: Read + Seek>(zip: &mut zip::ZipArchive<R>, name: &str, limit: u64) -> Option<Vec<u8>> {
    let mut entry = zip.by_name(name).ok()?;
    if entry.size() > limit {
        return None;
    }
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

fn parse_fabric(bytes: &[u8]) -> Option<ModMeta> {
    let text = String::from_utf8_lossy(bytes);
    // Algunos mods traen saltos de línea sin escapar dentro de las cadenas: se cambian por espacios.
    let json: Value = serde_json::from_str(&text).or_else(|_| serde_json::from_str(&text.replace(['\n', '\r', '\t'], " "))).ok()?;
    let rules = |key: &str| -> Vec<(String, Vec<String>)> {
        json[key]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(id, value)| {
                let alternatives = match value {
                    Value::String(s) => vec![s.clone()],
                    Value::Array(list) => list.iter().filter_map(Value::as_str).map(String::from).collect(),
                    _ => vec!["*".to_string()],
                };
                (id.clone(), alternatives)
            })
            .collect()
    };
    Some(ModMeta {
        id: json["id"].as_str()?.to_string(),
        name: json["name"].as_str().or(json["id"].as_str()).unwrap_or_default().to_string(),
        version: json["version"].as_str().unwrap_or_default().to_string(),
        provides: json["provides"].as_array().into_iter().flatten().filter_map(Value::as_str).map(String::from).collect(),
        depends: rules("depends"),
        breaks: rules("breaks"),
    })
}

// ================= Versiones y rangos (las reglas de Fabric Loader)

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemVer {
    parts: Vec<u64>,
    pre: Option<String>,
}

fn parse_semver(text: &str) -> Option<SemVer> {
    let text = text.trim().split('+').next()?;
    let (main, pre) = match text.split_once('-') {
        Some((main, pre)) => (main, Some(pre.to_string())),
        None => (text, None),
    };
    let parts = main.split('.').map(|p| p.parse::<u64>().ok()).collect::<Option<Vec<_>>>()?;
    (!parts.is_empty()).then_some(SemVer { parts, pre })
}

fn compare(a: &SemVer, b: &SemVer) -> Ordering {
    for i in 0..a.parts.len().max(b.parts.len()) {
        let order = a.parts.get(i).unwrap_or(&0).cmp(b.parts.get(i).unwrap_or(&0));
        if order != Ordering::Equal {
            return order;
        }
    }
    match (&a.pre, &b.pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(x), Some(y)) => {
            for (p, q) in x.split('.').zip(y.split('.')) {
                let order = match (p.parse::<u64>(), q.parse::<u64>()) {
                    (Ok(m), Ok(n)) => m.cmp(&n),
                    _ => p.cmp(q),
                };
                if order != Ordering::Equal {
                    return order;
                }
            }
            x.split('.').count().cmp(&y.split('.').count())
        }
    }
}

/// ¿La versión cumple el rango? Varios términos separados por espacios deben cumplirse todos.
pub fn matches(version: &str, predicate: &str) -> bool {
    let predicate = predicate.trim();
    if predicate.is_empty() || predicate == "*" {
        return true;
    }
    predicate.split_whitespace().all(|term| matches_term(version, term))
}

fn matches_term(version: &str, term: &str) -> bool {
    let (operator, wanted) = ["^", "~", ">=", "<=", ">", "<", "="]
        .iter()
        .find_map(|op| term.strip_prefix(op).map(|rest| (*op, rest.trim())))
        .unwrap_or(("=", term));
    if wanted == "*" {
        return true;
    }
    // Comodines: "0.6.x" acepta cualquier 0.6.algo.
    if wanted.split('.').any(|p| p == "x" || p == "X" || p == "*") {
        let Some(have) = parse_semver(version) else { return false };
        return wanted
            .split('.')
            .take_while(|p| !matches!(*p, "x" | "X" | "*"))
            .enumerate()
            .all(|(i, p)| p.parse::<u64>().ok() == Some(*have.parts.get(i).unwrap_or(&0)));
    }
    let (Some(have), Some(want)) = (parse_semver(version), parse_semver(wanted)) else {
        // Versiones que no son numéricas: solo sirve la igualdad exacta.
        return operator == "=" && version.trim() == wanted;
    };
    let order = compare(&have, &want);
    let part = |v: &SemVer, i: usize| *v.parts.get(i).unwrap_or(&0);
    match operator {
        ">=" => order != Ordering::Less,
        "<=" => order != Ordering::Greater,
        ">" => order == Ordering::Greater,
        "<" => order == Ordering::Less,
        // En Fabric "^" es la misma versión mayor y "~" la misma menor.
        "^" => order != Ordering::Less && part(&have, 0) == part(&want, 0),
        "~" => order != Ordering::Less && part(&have, 0) == part(&want, 0) && part(&have, 1) == part(&want, 1),
        _ => order == Ordering::Equal,
    }
}

/// Saca de un nombre de versión de Modrinth ("mc1.21.1-0.6.13-fabric") las partes que parecen versiones.
pub fn embedded_versions(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut current = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() || (c == '.' && !current.is_empty()) {
            current.push(c);
        } else {
            let candidate = current.trim_end_matches('.').to_string();
            if candidate.contains('.') {
                found.push(candidate);
            }
            current.clear();
        }
    }
    found
}

// ================= Revisar un conjunto de mods

/// Un problema entre mods. `owner` es el archivo que lo declara.
#[derive(Debug, Clone)]
pub enum Problem {
    /// owner necesita `id` y no está.
    Missing { owner: String, name: String, id: String, predicates: Vec<String> },
    /// owner necesita `id` en otra versión (la que hay es `found` en `found_file`).
    Wrong { owner: String, name: String, id: String, predicates: Vec<String>, found: String, found_file: String },
    /// owner no funciona con `id` en esa versión.
    Breaks { owner: String, name: String, id: String, found: String, found_file: String },
}

impl Problem {
    pub fn owner(&self) -> &str {
        match self {
            Problem::Missing { owner, .. } | Problem::Wrong { owner, .. } | Problem::Breaks { owner, .. } => owner,
        }
    }

    pub fn involves(&self, file: &str) -> bool {
        self.owner() == file
            || matches!(self, Problem::Wrong { found_file, .. } | Problem::Breaks { found_file, .. } if found_file == file)
    }

    /// Explicación para el jugador, con los nombres que da `names` (id del mod -> nombre visible).
    pub fn describe(&self, names: &HashMap<String, String>) -> String {
        let show = |id: &str| names.get(id).cloned().unwrap_or_else(|| id.to_string());
        let range = |predicates: &[String]| {
            let list: Vec<String> = predicates.iter().map(|p| if p == "*" { "cualquier versión".into() } else { p.clone() }).collect();
            list.join(" o ")
        };
        match self {
            Problem::Missing { name, id, predicates, .. } => {
                format!("{name} necesita {} ({}), que no está instalado.", show(id), range(predicates))
            }
            Problem::Wrong { name, id, predicates, found, .. } => {
                format!("{name} necesita {} {}, pero la versión es {found}.", show(id), range(predicates))
            }
            Problem::Breaks { name, id, found, .. } => format!("{name} no funciona con {} {found}.", show(id)),
        }
    }
}

/// Ids que no son mods de la carpeta: se comprueban aparte (loader) o no se pueden comprobar aquí.
const BUILT_IN: [&str; 4] = ["minecraft", "java", "fabricloader", "quilt_loader"];

/// Revisa un conjunto de mods (archivo -> lo que declara). `loader_version` es la de Fabric/Quilt.
pub fn check(mods: &[(String, Vec<ModMeta>)], loader_version: Option<&str>) -> Vec<Problem> {
    // id -> (versión, archivo); si hay varias copias (incluidas en otros mods) gana la más nueva.
    let mut present: HashMap<&str, (&str, &str)> = HashMap::new();
    for (file, metas) in mods {
        for meta in metas {
            for id in std::iter::once(&meta.id).chain(&meta.provides) {
                let newer = present.get(id.as_str()).is_none_or(|(old, _)| {
                    matches!((parse_semver(&meta.version), parse_semver(old)), (Some(a), Some(b)) if compare(&a, &b) == Ordering::Greater)
                });
                if newer {
                    present.insert(id, (&meta.version, file));
                }
            }
        }
    }
    let mut problems = Vec::new();
    for (file, metas) in mods {
        for meta in metas {
            for (id, predicates) in &meta.depends {
                if id == "fabricloader" {
                    if let Some(loader) = loader_version {
                        if !predicates.iter().any(|p| matches(loader, p)) {
                            problems.push(Problem::Wrong {
                                owner: file.clone(),
                                name: meta.name.clone(),
                                id: id.clone(),
                                predicates: predicates.clone(),
                                found: loader.to_string(),
                                found_file: String::new(),
                            });
                        }
                    }
                    continue;
                }
                if BUILT_IN.contains(&id.as_str()) {
                    continue;
                }
                match present.get(id.as_str()) {
                    None => problems.push(Problem::Missing { owner: file.clone(), name: meta.name.clone(), id: id.clone(), predicates: predicates.clone() }),
                    Some((version, found_file)) if !predicates.iter().any(|p| matches(version, p)) => problems.push(Problem::Wrong {
                        owner: file.clone(),
                        name: meta.name.clone(),
                        id: id.clone(),
                        predicates: predicates.clone(),
                        found: version.to_string(),
                        found_file: found_file.to_string(),
                    }),
                    _ => {}
                }
            }
            for (id, predicates) in &meta.breaks {
                if let Some((version, found_file)) = present.get(id.as_str()) {
                    if predicates.iter().any(|p| matches(version, p)) {
                        problems.push(Problem::Breaks {
                            owner: file.clone(),
                            name: meta.name.clone(),
                            id: id.clone(),
                            found: version.to_string(),
                            found_file: found_file.to_string(),
                        });
                    }
                }
            }
        }
    }
    problems
}

/// Nombre visible de cada id de mod (para los mensajes).
pub fn names(mods: &[(String, Vec<ModMeta>)]) -> HashMap<String, String> {
    let mut names = HashMap::from([("fabricloader".to_string(), "Fabric Loader".to_string())]);
    for meta in mods.iter().flat_map(|(_, metas)| metas) {
        names.entry(meta.id.clone()).or_insert_with(|| meta.name.clone());
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rangos_de_fabric() {
        assert!(matches("0.6.13+mc1.21.1", "0.6.x"));
        assert!(!matches("0.8.13+mc1.21.1", "0.6.x"));
        assert!(matches("1.8.14-beta.1+mc1.21.1", ">=1.8.13-"));
        assert!(!matches("1.8.8+mc1.21.1", ">=1.8.13-"));
        assert!(matches("2.3.0", "^2.0.0"));
        assert!(!matches("3.0.0", "^2.0.0"));
        assert!(matches("1.2.9", "~1.2.3"));
        assert!(!matches("1.3.0", "~1.2.3"));
        assert!(matches("0.16.10", ">=0.16.5 <0.17"));
        assert!(!matches("0.17.0", ">=0.16.5 <0.17"));
        assert!(matches("1.0.0", "*"));
        assert!(matches("1.2.3", "1.2.3"));
        assert!(!matches("1.2.4", "1.2.3"));
        assert!(matches("1.21.1", ">=1.21"));
        assert!(!matches("1.0.0-beta.2", ">=1.0.0"));
        assert!(matches("1.0.0-beta.10", ">1.0.0-beta.2"));
    }

    #[test]
    fn versiones_en_nombres_de_modrinth() {
        assert_eq!(embedded_versions("mc1.21.1-0.6.13-fabric"), ["1.21.1", "0.6.13"]);
        assert_eq!(embedded_versions("1.8.8+1.21.1-fabric"), ["1.8.8", "1.21.1"]);
    }

    fn meta(id: &str, version: &str, depends: &[(&str, &str)], breaks: &[(&str, &str)]) -> ModMeta {
        ModMeta {
            id: id.into(),
            name: id.into(),
            version: version.into(),
            provides: vec![],
            depends: depends.iter().map(|(i, p)| (i.to_string(), vec![p.to_string()])).collect(),
            breaks: breaks.iter().map(|(i, p)| (i.to_string(), vec![p.to_string()])).collect(),
        }
    }

    #[test]
    fn detecta_iris_con_sodium_incorrecto() {
        let mods = vec![
            ("iris.jar".to_string(), vec![meta("iris", "1.8.8", &[("sodium", "0.6.x"), ("fabricloader", ">=0.16.0")], &[])]),
            ("sodium.jar".to_string(), vec![meta("sodium", "0.8.13", &[], &[("iris", "<1.8.13-")])]),
        ];
        let problems = check(&mods, Some("0.19.5"));
        assert_eq!(problems.len(), 2);
        assert!(problems.iter().all(|p| p.involves("sodium.jar")));
        let fixed = vec![mods[0].clone(), ("sodium.jar".to_string(), vec![meta("sodium", "0.6.13", &[], &[])])];
        assert!(check(&fixed, Some("0.19.5")).is_empty());
        assert_eq!(check(&fixed, Some("0.15.0")).len(), 1);
    }
}
