//! Compatibilidad entre mods. Cada mod declara qué necesita y con qué choca, con rangos de versión:
//!  - Fabric (y Quilt, que los carga): fabric.mod.json, "depends"/"breaks", rangos como "0.6.x" o ">=1.8.13".
//!  - Forge y NeoForge: META-INF/mods.toml (o neoforge.mods.toml), [[dependencies.<mod>]], rangos de
//!    Maven como "[47.4.23,)".
//! Modrinth no guarda esos rangos, así que se leen de los .jar.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};

/// Lo que declara un mod (o una biblioteca incluida dentro de otro mod).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
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
    /// Viene de mods.toml (Forge o NeoForge) y no de fabric.mod.json.
    #[serde(default)]
    pub forge: bool,
}

/// Mods de un .jar: el principal y los que lleva dentro (META-INF/jars), como los módulos de Fabric API.
pub fn read_jar(bytes_or_file: impl Read + Seek) -> Vec<ModMeta> {
    let mut result = Vec::new();
    read_into(bytes_or_file, 0, &mut result);
    result
}

fn read_into(reader: impl Read + Seek, depth: u32, result: &mut Vec<ModMeta>) {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else { return };
    // Los anidados se revisan siempre: hay .jar que no declaran nada por fuera y traen el mod dentro
    // (Sinytra Connector, Kotlin for Forge).
    let nested_dirs = ["META-INF/jars/", "META-INF/jarjar/"];
    if let Some(meta) = read_entry(&mut zip, "fabric.mod.json", 2_000_000).and_then(|bytes| parse_fabric(&bytes)) {
        result.push(meta);
    }
    let toml = read_entry(&mut zip, "META-INF/neoforge.mods.toml", 2_000_000).or_else(|| read_entry(&mut zip, "META-INF/mods.toml", 2_000_000));
    if let Some(bytes) = toml {
        let manifest = read_entry(&mut zip, "META-INF/MANIFEST.MF", 200_000).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        let jar_version = manifest.lines().find_map(|l| l.strip_prefix("Implementation-Version:")).map(|v| v.trim().to_string());
        result.extend(parse_forge(&bytes, jar_version.as_deref()));
    }
    // Mods incluidos dentro de otro (los módulos de Fabric API, las bibliotecas de Forge con Jar-in-Jar).
    let nested: Vec<String> = if depth < 2 {
        zip.file_names().filter(|n| n.ends_with(".jar") && nested_dirs.iter().any(|d| n.starts_with(d))).map(String::from).collect()
    } else {
        Vec::new()
    };
    for name in nested {
        if let Some(bytes) = read_entry(&mut zip, &name, 50_000_000) {
            read_into(Cursor::new(bytes), depth + 1, result);
        }
    }
}

/// mods.toml de Forge/NeoForge: puede declarar varios mods, cada uno con sus dependencias.
fn parse_forge(bytes: &[u8], jar_version: Option<&str>) -> Vec<ModMeta> {
    let text = String::from_utf8_lossy(bytes);
    let Ok(doc) = text.parse::<toml::Table>() else { return Vec::new() };
    let dependencies = doc.get("dependencies").and_then(|d| d.as_table());
    let text_of = |value: Option<&toml::Value>| value.and_then(|v| v.as_str()).map(str::to_string);
    doc.get("mods")
        .and_then(|m| m.as_array())
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let id = text_of(entry.get("modId"))?;
            let mut version = text_of(entry.get("version")).unwrap_or_default();
            if version.contains("${") {
                version = jar_version.unwrap_or_default().to_string();
            }
            let mut depends = Vec::new();
            let mut breaks = Vec::new();
            for dependency in dependencies.and_then(|d| d.get(&id)).and_then(|d| d.as_array()).into_iter().flatten() {
                let Some(dep_id) = text_of(dependency.get("modId")) else { continue };
                // Lo que solo hace falta en el servidor no cuenta en el cliente.
                if text_of(dependency.get("side")).is_some_and(|s| s.eq_ignore_ascii_case("SERVER")) {
                    continue;
                }
                let range = text_of(dependency.get("versionRange")).unwrap_or_default();
                let predicate = if range.trim().starts_with(['[', '(']) { range.trim().to_string() } else { "*".to_string() };
                let kind = text_of(dependency.get("type")).map(|k| k.to_lowercase());
                let mandatory = dependency.get("mandatory").and_then(|m| m.as_bool());
                match (kind.as_deref(), mandatory) {
                    (Some("required"), _) | (None, Some(true)) => depends.push((dep_id, vec![predicate])),
                    (Some("incompatible"), _) => breaks.push((dep_id, vec![predicate])),
                    _ => {}
                }
            }
            Some(ModMeta {
                name: text_of(entry.get("displayName")).unwrap_or_else(|| id.clone()),
                id,
                version,
                provides: Vec::new(),
                depends,
                breaks,
                forge: true,
            })
        })
        .collect()
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
        forge: false,
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
/// Los rangos de Maven (Forge, NeoForge) empiezan con "[" o "(".
pub fn matches(version: &str, predicate: &str) -> bool {
    let predicate = predicate.trim();
    if predicate.is_empty() || predicate == "*" {
        return true;
    }
    if predicate.starts_with(['[', '(']) {
        return matches_maven(version, predicate);
    }
    predicate.split_whitespace().all(|term| matches_term(version, term))
}

/// Rangos de Maven: "[1.0,2.0)", "[47.4.23,)", "(,1.0]", "[1.0]" (exacta) o varios separados por comas.
fn matches_maven(version: &str, range: &str) -> bool {
    use crate::loaders::compare_versions;
    let mut rest = range;
    let mut any = false;
    while let Some(start) = rest.find(['[', '(']) {
        let Some(length) = rest[start..].find([']', ')']) else { break };
        let group = &rest[start..=start + length];
        rest = &rest[start + length + 1..];
        any = true;
        let (lower_inclusive, upper_inclusive) = (group.starts_with('['), group.ends_with(']'));
        let inner = &group[1..group.len() - 1];
        let inside = match inner.split_once(',') {
            None => compare_versions(version, inner.trim()) == Ordering::Equal,
            Some((low, high)) => {
                let (low, high) = (low.trim(), high.trim());
                let above = low.is_empty() || {
                    let order = compare_versions(version, low);
                    order == Ordering::Greater || (lower_inclusive && order == Ordering::Equal)
                };
                let below = high.is_empty() || {
                    let order = compare_versions(version, high);
                    order == Ordering::Less || (upper_inclusive && order == Ordering::Equal)
                };
                above && below
            }
        };
        if inside {
            return true;
        }
    }
    !any
}

/// La versión más baja que acepta un rango, si la dice ("[47.4.23,)" -> 47.4.23, ">=0.16.10" -> 0.16.10).
pub fn minimum(predicate: &str) -> Option<String> {
    let predicate = predicate.trim();
    if let Some(inner) = predicate.strip_prefix('[') {
        let low = inner.split([',', ']']).next()?.trim();
        return (!low.is_empty()).then(|| low.to_string());
    }
    predicate.split_whitespace().find_map(|term| term.strip_prefix(">=").or_else(|| term.strip_prefix('^')).or_else(|| term.strip_prefix('~')).map(str::to_string))
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
            let list: Vec<String> = predicates
                .iter()
                .map(|p| {
                    if p == "*" {
                        return t!("cualquier versión", "any version");
                    }
                    // "[47.4.23,)" se lee mejor como "47.4.23 o más nueva".
                    match minimum(p) {
                        Some(low) if p.starts_with('[') && p.trim_end().ends_with(",)") => t!("{low} o más nueva", "{low} or newer"),
                        _ => p.clone(),
                    }
                })
                .collect();
            list.join(t!(" o ", " or ").as_str())
        };
        match self {
            Problem::Missing { name, id, predicates, .. } => {
                t!("{name} necesita {} ({}), que no está instalado.", "{name} needs {} ({}), which is not installed.", show(id), range(predicates))
            }
            Problem::Wrong { name, id, predicates, found, .. } => {
                t!("{name} necesita {} {}, pero la versión es {found}.", "{name} needs {} {}, but the version is {found}.", show(id), range(predicates))
            }
            Problem::Breaks { name, id, found, .. } => t!("{name} no funciona con {} {found}.", "{name} does not work with {} {found}.", show(id)),
        }
    }
}

/// Ids que no son mods de la carpeta: se comprueban aparte (juego y loader) o los trae el loader.
const BUILT_IN: [&str; 10] = ["minecraft", "java", "fabricloader", "quilt_loader", "forge", "neoforge", "fml", "javafml", "lowcodefml", "mixinextras"];

/// La instancia contra la que se revisan los mods.
#[derive(Clone, Debug)]
pub struct Game {
    pub minecraft: String,
    /// "Fabric", "Quilt", "Forge", "NeoForge" o "Vanilla".
    pub loader: String,
    pub loader_version: Option<String>,
}

impl Game {
    pub fn of(instance: &crate::instances::Instance) -> Self {
        Self { minecraft: instance.minecraft_version.clone(), loader: instance.loader.clone(), loader_version: instance.loader_version.clone() }
    }

    fn forge_like(&self) -> bool {
        matches!(self.loader.as_str(), "Forge" | "NeoForge")
    }

    /// La versión que tiene la instancia de algo que no es un mod de la carpeta (juego o loader).
    fn built_in_version(&self, id: &str) -> Option<&str> {
        match id {
            "minecraft" => parse_semver(&self.minecraft).map(|_| self.minecraft.as_str()),
            "fabricloader" if matches!(self.loader.as_str(), "Fabric" | "Quilt") => self.loader_version.as_deref(),
            "quilt_loader" if self.loader == "Quilt" => self.loader_version.as_deref(),
            "forge" if self.loader == "Forge" => self.loader_version.as_deref(),
            "neoforge" if self.loader == "NeoForge" => self.loader_version.as_deref(),
            _ => None,
        }
    }
}

/// Lo que carga el loader de la instancia: en Forge/NeoForge, los mods.toml; en Fabric/Quilt, los fabric.mod.json.
fn relevant<'a>(mods: &'a [(String, Vec<ModMeta>)], game: &Game) -> Vec<(&'a str, Vec<&'a ModMeta>)> {
    mods.iter().map(|(file, metas)| (file.as_str(), metas.iter().filter(|m| m.forge == game.forge_like()).collect())).collect()
}

/// Mods que están dos veces (el mismo id en dos archivos de la carpeta): (id, nombre, archivos).
pub fn duplicates(mods: &[(String, Vec<ModMeta>)], game: &Game) -> Vec<(String, String, Vec<String>)> {
    let mut by_id: Vec<(String, String, Vec<String>)> = Vec::new();
    for (file, metas) in relevant(mods, game) {
        // Solo el mod principal de cada archivo: los incluidos dentro pueden repetirse sin problema.
        let Some(main) = metas.first() else { continue };
        match by_id.iter_mut().find(|(id, _, _)| *id == main.id) {
            Some((_, _, files)) => files.push(file.to_string()),
            None => by_id.push((main.id.clone(), main.name.clone(), vec![file.to_string()])),
        }
    }
    by_id.into_iter().filter(|(_, _, files)| files.len() > 1).collect()
}

/// Revisa un conjunto de mods (archivo -> lo que declara) para Fabric o Quilt con esa versión del loader.
#[cfg(test)]
pub fn check(mods: &[(String, Vec<ModMeta>)], loader_version: Option<&str>) -> Vec<Problem> {
    let game = Game { minecraft: String::new(), loader: "Fabric".into(), loader_version: loader_version.map(String::from) };
    check_game(mods, &game)
}

/// Revisa un conjunto de mods contra la instancia: dependencias, versiones del juego y del loader, choques.
pub fn check_game(mods: &[(String, Vec<ModMeta>)], game: &Game) -> Vec<Problem> {
    let mods: Vec<(String, Vec<ModMeta>)> =
        relevant(mods, game).into_iter().map(|(file, metas)| (file.to_string(), metas.into_iter().cloned().collect())).collect();
    let mods = mods.as_slice();
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
                if BUILT_IN.contains(&id.as_str()) {
                    // El juego y el loader: se comparan con la instancia (found_file vacío).
                    if let Some(have) = game.built_in_version(id) {
                        if !predicates.iter().any(|p| matches(have, p)) {
                            problems.push(Problem::Wrong {
                                owner: file.clone(),
                                name: meta.name.clone(),
                                id: id.clone(),
                                predicates: predicates.clone(),
                                found: have.to_string(),
                                found_file: String::new(),
                            });
                        }
                    }
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
    let mut names = HashMap::from([
        ("fabricloader".to_string(), "Fabric Loader".to_string()),
        ("quilt_loader".to_string(), "Quilt Loader".to_string()),
        ("forge".to_string(), "Forge".to_string()),
        ("neoforge".to_string(), "NeoForge".to_string()),
        ("minecraft".to_string(), "Minecraft".to_string()),
    ]);
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
            forge: false,
        }
    }

    #[test]
    fn rangos_de_maven() {
        assert!(matches("47.4.23", "[47.4.23,)"));
        assert!(!matches("47.4.20", "[47.4.23,)"));
        assert!(matches("1.20.1", "[1.20,1.21)"));
        assert!(!matches("1.21", "[1.20,1.21)"));
        assert!(matches("1.21", "[1.20,1.21]"));
        assert!(matches("2.0", "[1.0]") == false && matches("1.0", "[1.0]"));
        assert!(matches("0.5", "(,1.0]"));
        assert!(matches("3.1", "[1.0,2.0),[3.0,)"));
        assert!(!matches("2.5", "[1.0,2.0),[3.0,)"));
        assert_eq!(minimum("[47.4.23,)").as_deref(), Some("47.4.23"));
        assert_eq!(minimum(">=0.16.10 <0.17").as_deref(), Some("0.16.10"));
        assert_eq!(minimum("*"), None);
    }

    #[test]
    fn lee_mods_toml_de_forge() {
        let text = r#"
modLoader="javafml"
loaderVersion="[47,)"
[[mods]]
modId="parcool"
version="${file.jarVersion}"
displayName="ParCool!"
[[dependencies.parcool]]
    modId="forge"
    mandatory=true
    versionRange="[47.4.23,)"
    side="BOTH"
[[dependencies.parcool]]
    modId="minecraft"
    mandatory=true
    versionRange="[1.20.1,1.21)"
    side="BOTH"
[[dependencies.parcool]]
    modId="curios"
    mandatory=false
    versionRange="[5,)"
    side="BOTH"
"#;
        let metas = parse_forge(text.as_bytes(), Some("4.0.1.2"));
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].id, "parcool");
        assert_eq!(metas[0].version, "4.0.1.2");
        assert_eq!(metas[0].depends.len(), 2);
        let mods = vec![("ParCool.jar".to_string(), metas)];
        let old_forge = Game { minecraft: "1.20.1".into(), loader: "Forge".into(), loader_version: Some("47.4.20".into()) };
        let problems = check_game(&mods, &old_forge);
        assert_eq!(problems.len(), 1);
        assert!(matches!(&problems[0], Problem::Wrong { id, found_file, .. } if id == "forge" && found_file.is_empty()));
        let new_forge = Game { loader_version: Some("47.4.23".into()), ..old_forge.clone() };
        assert!(check_game(&mods, &new_forge).is_empty());
        let other_game = Game { minecraft: "1.21.1".into(), ..new_forge };
        assert_eq!(check_game(&mods, &other_game).len(), 1);
    }

    #[test]
    fn duplicados() {
        let mods = vec![
            ("jei-1.jar".to_string(), vec![meta("jei", "1", &[], &[])]),
            ("jei-2.jar".to_string(), vec![meta("jei", "2", &[], &[])]),
            ("otro.jar".to_string(), vec![meta("otro", "1", &[], &[])]),
        ];
        let game = Game { minecraft: "1.21.1".into(), loader: "Fabric".into(), loader_version: Some("0.16.0".into()) };
        let found = duplicates(&mods, &game);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].2, ["jei-1.jar", "jei-2.jar"]);
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
