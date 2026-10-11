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
    /// Viene de neoforge.mods.toml (lo que lee NeoForge 1.20.5 o más nuevo) y no de mods.toml (lo que
    /// leen Forge y los NeoForge anteriores). Los mods «multiloader» traen los dos con dependencias
    /// distintas: hay que mirar el que lee el loader de la instancia.
    #[serde(default)]
    pub neo_toml: bool,
    /// Viene incluido dentro de otro mod (jar-in-jar), no suelto en la carpeta mods.
    #[serde(default)]
    pub nested: bool,
}

/// Para qué loaders está hecho un .jar según lo que trae dentro. Modrinth y CurseForge a veces lo
/// etiquetan mal (un .jar de Forge marcado también como NeoForge), así que manda el archivo:
///  - fabric.mod.json → Fabric (Quilt también lo carga); quilt.mod.json → Quilt.
///  - META-INF/neoforge.mods.toml → NeoForge 1.20.5 o más nuevo.
///  - META-INF/mods.toml → Forge, o NeoForge 1.20.2–1.20.4 si depende de "neoforge". Sin ninguna de
///    las dos dependencias no se sabe: vale para los dos.
///  - mcmod.info → Forge antiguo (1.12 y anteriores).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[serde(default)]
pub struct JarLoaders {
    pub fabric: bool,
    pub quilt: bool,
    pub forge: bool,
    /// neoforge.mods.toml (NeoForge 1.20.5+).
    pub neoforge: bool,
    /// mods.toml que sirve en NeoForge 1.20.2–1.20.4.
    pub neoforge_old: bool,
}

impl JarLoaders {
    /// Trae algo que diga para qué loader es (las bibliotecas sueltas no traen nada).
    pub fn known(&self) -> bool {
        self.fabric || self.quilt || self.forge || self.neoforge || self.neoforge_old
    }

    /// "Forge", "Fabric y Quilt"…
    pub fn describe(&self) -> String {
        let names: Vec<&str> = [(self.fabric, "Fabric"), (self.quilt, "Quilt"), (self.forge, "Forge"), (self.neoforge || self.neoforge_old, "NeoForge")]
            .into_iter()
            .filter(|(on, _)| *on)
            .map(|(_, name)| name)
            .collect();
        match names.as_slice() {
            [] => String::new(),
            [one] => one.to_string(),
            [rest @ .., last] => format!("{}{}{last}", rest.join(", "), t!(" y ", " and ")),
        }
    }
}

pub fn read_loaders(reader: impl Read + Seek) -> JarLoaders {
    let Ok(mut zip) = zip::ZipArchive::new(reader) else { return JarLoaders::default() };
    let has = |name: &str| zip.index_for_name(name).is_some();
    let mut found = JarLoaders {
        fabric: has("fabric.mod.json"),
        quilt: has("quilt.mod.json"),
        neoforge: has("META-INF/neoforge.mods.toml"),
        forge: has("mcmod.info"),
        neoforge_old: false,
    };
    if let Some(bytes) = read_entry(&mut zip, "META-INF/mods.toml", 2_000_000) {
        let text = String::from_utf8_lossy(&bytes);
        let depends_on = |loader: &str| {
            text.parse::<toml::Table>().ok().is_some_and(|doc| {
                doc.get("dependencies")
                    .and_then(|d| d.as_table())
                    .into_iter()
                    .flat_map(|t| t.values())
                    .filter_map(|v| v.as_array())
                    .flatten()
                    .any(|dependency| dependency.get("modId").and_then(|m| m.as_str()) == Some(loader))
            })
        };
        match (depends_on("forge"), depends_on("neoforge")) {
            (true, false) => found.forge = true,
            (false, true) => found.neoforge_old = true,
            _ => {
                found.forge = true;
                found.neoforge_old = true;
            }
        }
    }
    found
}

/// Si un .jar carga en la instancia. NeoForge para 1.20.1 todavía era Forge por dentro: ahí sirven los
/// mods de los dos. Con Sinytra Connector, Forge y NeoForge también cargan mods de Fabric.
pub fn loader_fits(jar: JarLoaders, loader: &str, minecraft: &str, connector: bool) -> bool {
    use crate::loaders::compare_versions;
    if !jar.known() {
        return true;
    }
    let at_least = |version: &str| compare_versions(minecraft, version) != Ordering::Less;
    let fits = match loader {
        "Fabric" => jar.fabric,
        "Quilt" => jar.quilt || jar.fabric,
        "Forge" => jar.forge,
        "NeoForge" if !at_least("1.20.2") => jar.forge || jar.neoforge_old,
        "NeoForge" if !at_least("1.20.5") => jar.neoforge_old || jar.neoforge,
        "NeoForge" => jar.neoforge,
        _ => true,
    };
    fits || (connector && jar.fabric && matches!(loader, "Forge" | "NeoForge"))
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
    let old_toml = read_entry(&mut zip, "META-INF/mods.toml", 2_000_000);
    let neo_toml = read_entry(&mut zip, "META-INF/neoforge.mods.toml", 2_000_000);
    if old_toml.is_some() || neo_toml.is_some() {
        let manifest = read_entry(&mut zip, "META-INF/MANIFEST.MF", 200_000).map(|b| String::from_utf8_lossy(&b).to_string()).unwrap_or_default();
        let jar_version = manifest.lines().find_map(|l| l.strip_prefix("Implementation-Version:")).map(|v| v.trim().to_string());
        if let Some(bytes) = old_toml {
            result.extend(parse_forge(&bytes, jar_version.as_deref()));
        }
        if let Some(bytes) = neo_toml {
            result.extend(parse_forge(&bytes, jar_version.as_deref()).into_iter().map(|meta| ModMeta { neo_toml: true, ..meta }));
        }
    }
    // Mods incluidos dentro de otro (los módulos de Fabric API, las bibliotecas de Forge con Jar-in-Jar).
    // Hasta cuatro niveles: los mods «universales» (LambDynamicLights) traen bibliotecas dentro de
    // bibliotecas dentro del mod.
    let nested: Vec<String> = if depth < 4 {
        zip.file_names().filter(|n| n.ends_with(".jar") && nested_dirs.iter().any(|d| n.starts_with(d))).map(String::from).collect()
    } else {
        Vec::new()
    };
    for name in nested {
        if let Some(bytes) = read_entry(&mut zip, &name, 50_000_000) {
            let before = result.len();
            read_into(Cursor::new(bytes), depth + 1, result);
            for meta in &mut result[before..] {
                meta.nested = true;
            }
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
                neo_toml: false,
                nested: false,
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
        neo_toml: false,
        nested: false,
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
/// Comparación de versiones como la hace Maven (`ComparableVersion`), que es la que usan Forge y
/// NeoForge con los rangos de mods.toml. No es semver: lo que va tras un guion es otra lista y no una
/// versión previa ("1.20.1-1.5.2-neoforge" es MAYOR que "1.20.1"), y solo los calificadores conocidos
/// van antes de la versión final ("1.0-beta" < "1.0" < "1.0-sp").
pub fn maven_compare(a: &str, b: &str) -> Ordering {
    maven::compare(&maven::parse(a), &maven::parse(b))
}

mod maven {
    use std::cmp::Ordering;

    #[derive(Debug, Clone)]
    pub enum Item {
        Int(u128),
        Text(String),
        List(Vec<Item>),
    }

    const QUALIFIERS: [&str; 7] = ["alpha", "beta", "milestone", "rc", "snapshot", "", "sp"];
    /// La posición de "" (la versión final) en QUALIFIERS.
    const RELEASE: &str = "5";

    fn qualifier(text: &str) -> String {
        match QUALIFIERS.iter().position(|q| *q == text) {
            Some(index) => index.to_string(),
            None => format!("{}-{text}", QUALIFIERS.len()),
        }
    }

    fn text_item(text: &str, followed_by_digit: bool) -> Item {
        let text = match (text, followed_by_digit) {
            ("a", true) => "alpha",
            ("b", true) => "beta",
            ("m", true) => "milestone",
            ("ga" | "final" | "release", _) => "",
            ("cr", _) => "rc",
            (other, _) => other,
        };
        Item::Text(text.to_string())
    }

    fn item(digits: bool, text: &str, followed_by_digit: bool) -> Item {
        if digits {
            Item::Int(text.trim_start_matches('0').parse().unwrap_or(if text.chars().all(|c| c == '0') { 0 } else { u128::MAX }))
        } else {
            text_item(text, followed_by_digit)
        }
    }

    fn is_null(item: &Item) -> bool {
        match item {
            Item::Int(n) => *n == 0,
            Item::Text(text) => qualifier(text) == RELEASE,
            Item::List(list) => list.is_empty(),
        }
    }

    /// Quita los ceros y calificadores de versión final del final ("1.0.0" = "1").
    fn normalize(list: &mut Vec<Item>) {
        while let Some(last) = list.last() {
            if is_null(last) {
                list.pop();
            } else if matches!(last, Item::List(_)) {
                // Maven sigue mirando antes de una sublista no vacía; esta implementación la conserva.
                break;
            } else {
                break;
            }
        }
    }

    pub fn parse(version: &str) -> Item {
        let version = version.trim().to_lowercase();
        let chars: Vec<char> = version.chars().collect();
        let mut stack: Vec<Vec<Item>> = vec![Vec::new()];
        let mut digits = false;
        let mut start = 0;
        let piece = |from: usize, to: usize| chars[from..to].iter().collect::<String>();
        for i in 0..chars.len() {
            let c = chars[i];
            if c == '.' || c == '-' {
                let value = if i == start { Item::Int(0) } else { item(digits, &piece(start, i), false) };
                stack.last_mut().unwrap().push(value);
                start = i + 1;
                if c == '-' {
                    stack.push(Vec::new());
                }
            } else if c.is_ascii_digit() {
                if !digits && i > start {
                    stack.last_mut().unwrap().push(text_item(&piece(start, i), true));
                    start = i;
                    stack.push(Vec::new());
                }
                digits = true;
            } else {
                if digits && i > start {
                    stack.last_mut().unwrap().push(item(true, &piece(start, i), false));
                    start = i;
                    stack.push(Vec::new());
                }
                digits = false;
            }
        }
        if chars.len() > start {
            stack.last_mut().unwrap().push(item(digits, &piece(start, chars.len()), false));
        }
        while stack.len() > 1 {
            let mut child = stack.pop().unwrap();
            normalize(&mut child);
            stack.last_mut().unwrap().push(Item::List(child));
        }
        let mut root = stack.pop().unwrap();
        normalize(&mut root);
        Item::List(root)
    }

    fn compare_to(left: &Item, right: Option<&Item>) -> Ordering {
        match (left, right) {
            (Item::Int(n), None) => {
                if *n == 0 {
                    Ordering::Equal
                } else {
                    Ordering::Greater
                }
            }
            (Item::Int(n), Some(Item::Int(m))) => n.cmp(m),
            (Item::Int(_), Some(_)) => Ordering::Greater,
            (Item::Text(text), None) => qualifier(text).cmp(&RELEASE.to_string()),
            (Item::Text(_), Some(Item::Int(_))) => Ordering::Less,
            (Item::Text(a), Some(Item::Text(b))) => qualifier(a).cmp(&qualifier(b)),
            (Item::Text(_), Some(Item::List(_))) => Ordering::Less,
            (Item::List(list), None) => list.first().map_or(Ordering::Equal, |first| compare_to(first, None)),
            (Item::List(_), Some(Item::Int(_))) => Ordering::Less,
            (Item::List(_), Some(Item::Text(_))) => Ordering::Greater,
            (Item::List(a), Some(Item::List(b))) => {
                for i in 0..a.len().max(b.len()) {
                    let order = match (a.get(i), b.get(i)) {
                        (None, None) => Ordering::Equal,
                        (None, Some(r)) => compare_to(r, None).reverse(),
                        (Some(l), r) => compare_to(l, r),
                    };
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                Ordering::Equal
            }
        }
    }

    pub fn compare(a: &Item, b: &Item) -> Ordering {
        compare_to(a, Some(b))
    }
}

fn matches_maven(version: &str, range: &str) -> bool {
    let compare_versions = maven_compare;
    let mut rest = range;
    let mut any = false;
    while let Some(start) = rest.find(['[', '(']) {
        let Some(length) = rest[start..].find([']', ')']) else { break };
        let group = &rest[start..=start + length];
        rest = &rest[start + length + 1..];
        any = true;
        let (lower_inclusive, upper_inclusive) = (group.starts_with('['), group.ends_with(']'));
        let inner = &group[1..group.len() - 1];
        // Un rango mal escrito ("[1.19.0,1.19.1,1.19.2)", Entity Texture Features): Forge lo acepta
        // igual, así que no se toma como problema.
        if inner.matches(',').count() > 1 {
            return true;
        }
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

    /// NeoForge no hace cumplir al pie de la letra el rango de Minecraft: JEI para 1.21.1 pide
    /// "[1.21, 1.21.1)" y abre sin problema en 1.21.1. Ahí basta con que sea de la misma línea (1.21.x).
    fn tolerates(&self, id: &str, have: &str, predicates: &[String]) -> bool {
        if id != "minecraft" || self.loader != "NeoForge" {
            return false;
        }
        let line: Vec<&str> = have.split('.').take(2).collect();
        line.len() == 2 && predicates.iter().any(|p| matches(&line.join("."), p))
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

/// Lo que carga el loader de la instancia: Forge y los NeoForge anteriores a 1.20.5 leen mods.toml,
/// NeoForge 1.20.5 o más nuevo solo neoforge.mods.toml, y Fabric/Quilt los fabric.mod.json.
fn relevant<'a>(mods: &'a [(String, Vec<ModMeta>)], game: &Game) -> Vec<(&'a str, Vec<&'a ModMeta>)> {
    let new_neoforge = game.loader == "NeoForge" && crate::loaders::compare_versions(&game.minecraft, "1.20.5") != Ordering::Less;
    // Forge antes de 1.13 no lee mods.toml (sus mods dicen lo que necesitan en el código): no hay qué revisar.
    let legacy_forge = game.loader == "Forge" && crate::loaders::compare_versions(&game.minecraft, "1.13") == Ordering::Less;
    let wanted = |meta: &ModMeta| match game.loader.as_str() {
        "Forge" if legacy_forge => false,
        "Forge" => meta.forge && !meta.neo_toml,
        "NeoForge" => meta.forge && meta.neo_toml == new_neoforge,
        _ => !meta.forge,
    };
    mods.iter().map(|(file, metas)| (file.as_str(), metas.iter().filter(|m| wanted(m)).collect())).collect()
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

/// Las correcciones de dependencias que trae el modpack para Fabric/Quilt
/// (config/fabric_loader_dependencies.json): "-depends" quita, "+depends" agrega y "depends" reemplaza
/// (igual con "breaks"). Prominence II quita así el requisito de ftbquests de Create Questing.
pub fn apply_fabric_overrides(mods: &mut [(String, Vec<ModMeta>)], overrides: &Value) {
    let Some(overrides) = overrides["overrides"].as_object() else { return };
    let rules = |value: &Value| -> Vec<(String, Vec<String>)> {
        value.as_object().into_iter().flatten().map(|(id, range)| (id.clone(), vec![range.as_str().unwrap_or("*").to_string()])).collect()
    };
    for (_, metas) in mods.iter_mut() {
        for meta in metas.iter_mut().filter(|m| !m.forge) {
            let Some(changes) = overrides.get(&meta.id).and_then(|c| c.as_object()) else { continue };
            for (key, value) in changes {
                let (operation, field) = match key.chars().next() {
                    Some(c @ ('+' | '-')) => (c, &key[1..]),
                    _ => ('=', key.as_str()),
                };
                let list = match field {
                    "depends" => &mut meta.depends,
                    "breaks" => &mut meta.breaks,
                    _ => continue,
                };
                let given = rules(value);
                match operation {
                    '-' => list.retain(|(id, _)| !given.iter().any(|(g, _)| g == id)),
                    '+' => {
                        list.retain(|(id, _)| !given.iter().any(|(g, _)| g == id));
                        list.extend(given);
                    }
                    _ => *list = given,
                }
            }
        }
    }
}

/// Del mismo mod puede haber varias copias incluidas dentro de otros (jar-in-jar; Statement trae una
/// para Minecraft 1.14–1.15 y otra para 1.16+). El loader carga una sola: la que está suelta en la
/// carpeta si la hay; si no, la más nueva cuyo juego y loader encajan. Las demás no cuentan.
fn chosen_copies(mods: &[(String, Vec<ModMeta>)], game: &Game) -> Vec<(String, Vec<ModMeta>)> {
    let fits = |meta: &ModMeta| {
        meta.depends.iter().filter(|(id, _)| BUILT_IN.contains(&id.as_str())).all(|(id, predicates)| match game.built_in_version(id) {
            Some(have) => predicates.iter().any(|p| matches(have, p)) || game.tolerates(id, have, predicates),
            None => true,
        })
    };
    // Lo que los mods piden de cada id: el loader prefiere la copia que se lo da a todos (Fabric
    // resuelve así: You're in Grave Danger trae Cloth Config 11 y otro mod la 13).
    let mut wanted: HashMap<&str, Vec<&Vec<String>>> = HashMap::new();
    for (_, metas) in mods {
        for meta in metas {
            for (id, predicates) in &meta.depends {
                wanted.entry(id.as_str()).or_default().push(predicates);
            }
        }
    }
    let pleases_all = |meta: &ModMeta| wanted.get(meta.id.as_str()).is_none_or(|list| list.iter().all(|p| p.iter().any(|p| matches(&meta.version, p))));
    // id -> (archivo, posición) de la copia que usa el loader.
    let mut chosen: HashMap<&str, (usize, usize)> = HashMap::new();
    let mut copies: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (f, (_, metas)) in mods.iter().enumerate() {
        for (m, meta) in metas.iter().enumerate() {
            copies.entry(meta.id.as_str()).or_default().push((f, m));
        }
    }
    for (id, list) in &copies {
        if list.len() < 2 {
            continue;
        }
        let meta = |(f, m): (usize, usize)| &mods[f].1[m];
        let rank = |&(f, m): &(usize, usize)| {
            let meta = meta((f, m));
            (!meta.nested, fits(meta), pleases_all(meta), parse_semver(&meta.version))
        };
        let best = list.iter().copied().max_by(|a, b| {
            let (ra, rb) = (rank(a), rank(b));
            ra.0.cmp(&rb.0).then(ra.1.cmp(&rb.1)).then(ra.2.cmp(&rb.2)).then_with(|| match (&ra.3, &rb.3) {
                (Some(x), Some(y)) => compare(x, y),
                _ => Ordering::Equal,
            })
        });
        if let Some(best) = best {
            chosen.insert(id, best);
        }
    }
    mods.iter()
        .enumerate()
        .map(|(f, (file, metas))| {
            let kept = metas.iter().enumerate().filter(|(m, meta)| chosen.get(meta.id.as_str()).is_none_or(|c| *c == (f, *m))).map(|(_, meta)| meta.clone()).collect();
            (file.clone(), kept)
        })
        .collect()
}

/// Revisa un conjunto de mods (archivo -> lo que declara) para Fabric o Quilt con esa versión del loader.
#[cfg(test)]
pub fn check(mods: &[(String, Vec<ModMeta>)], loader_version: Option<&str>) -> Vec<Problem> {
    let game = Game { minecraft: String::new(), loader: "Fabric".into(), loader_version: loader_version.map(String::from) };
    check_game(mods, &game)
}

/// Revisa un conjunto de mods contra la instancia: dependencias, versiones del juego y del loader, choques.
pub fn check_game(mods: &[(String, Vec<ModMeta>)], game: &Game) -> Vec<Problem> {
    let mods: Vec<(String, Vec<ModMeta>)> = chosen_copies(
        &relevant(mods, game).into_iter().map(|(file, metas)| (file.to_string(), metas.into_iter().cloned().collect())).collect::<Vec<_>>(),
        game,
    );
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
                        if !predicates.iter().any(|p| matches(have, p)) && !game.tolerates(id, have, predicates) {
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
                    // Lo que le falta a una biblioteca incluida dentro de otro mod no se da por perdido: su
                    // autor la empaqueta con lo que necesita (a veces más adentro de lo que se lee).
                    None if meta.nested => {}
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
            neo_toml: false,
            nested: false,
        }
    }

    fn jar(entries: &[(&str, &str)]) -> Vec<u8> {
        let mut buffer = Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buffer);
        for (name, content) in entries {
            zip.start_file(*name, zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut zip, content.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
        buffer.into_inner()
    }

    #[test]
    fn para_que_loader_es_cada_jar() {
        let forge_toml = "modLoader=\"javafml\"\n[[mods]]\nmodId=\"parcool\"\n[[dependencies.parcool]]\nmodId=\"forge\"\nmandatory=true\nversionRange=\"[47,)\"\n";
        let neo_old = "modLoader=\"javafml\"\n[[mods]]\nmodId=\"x\"\n[[dependencies.x]]\nmodId=\"neoforge\"\ntype=\"required\"\nversionRange=\"[20.4,)\"\n";
        let forge = read_loaders(Cursor::new(jar(&[("META-INF/mods.toml", forge_toml)])));
        let neo = read_loaders(Cursor::new(jar(&[("META-INF/neoforge.mods.toml", forge_toml)])));
        let old = read_loaders(Cursor::new(jar(&[("META-INF/mods.toml", neo_old)])));
        let fabric = read_loaders(Cursor::new(jar(&[("fabric.mod.json", "{}")])));
        let library = read_loaders(Cursor::new(jar(&[("com/x/A.class", "")])));
        assert_eq!(forge.describe(), "Forge");
        // Un mod de Forge no carga en NeoForge 1.21.1, y uno de NeoForge no carga en Forge.
        assert!(loader_fits(forge, "Forge", "1.21.1", false) && !loader_fits(forge, "NeoForge", "1.21.1", false));
        assert!(loader_fits(neo, "NeoForge", "1.21.1", false) && !loader_fits(neo, "Forge", "1.21.1", false));
        // En 1.20.1 NeoForge carga los mods de Forge.
        assert!(loader_fits(forge, "NeoForge", "1.20.1", false));
        assert!(loader_fits(old, "NeoForge", "1.20.4", false) && !loader_fits(old, "NeoForge", "1.21.1", false) && !loader_fits(old, "Forge", "1.20.4", false));
        assert!(loader_fits(fabric, "Quilt", "1.21.1", false) && !loader_fits(fabric, "NeoForge", "1.21.1", false));
        assert!(loader_fits(fabric, "NeoForge", "1.21.1", true), "con Sinytra Connector");
        assert!(!library.known() && loader_fits(library, "Forge", "1.20.1", false));
    }

    #[test]
    fn de_varias_copias_incluidas_cuenta_la_que_usa_el_loader() {
        // Statement (dentro de Auto Slabs) trae el mismo módulo para 1.14–1.15 y para 1.16+.
        let copy = |version: &str, range: &str| ModMeta {
            id: "statement_vanilla_compatibility".into(),
            name: "Statement Vanilla Compatibility Module".into(),
            version: version.into(),
            depends: vec![("minecraft".into(), vec![range.into()])],
            nested: true,
            ..Default::default()
        };
        let main = ModMeta { id: "autoslabs".into(), name: "Auto Slabs".into(), version: "1.1.3".into(), ..Default::default() };
        let mods = vec![("autoslabs-1.1.3.jar".to_string(), vec![main, copy("1.0.1+1.14.4-1.15.2", ">=1.14.0-0 <1.16.0-0"), copy("1.0.1+1.16.5-1.17", ">=1.16.0-0")])];
        let game = Game { minecraft: "1.20.1".into(), loader: "Fabric".into(), loader_version: Some("0.19.3".into()) };
        assert!(check_game(&mods, &game).is_empty());
        // Si ninguna copia sirve, sí es un problema.
        let old = vec![("viejo.jar".to_string(), vec![copy("1.0.1+1.14.4-1.15.2", ">=1.14.0-0 <1.16.0-0"), copy("1.0.0", ">=1.14.0-0 <1.16.0-0")])];
        assert_eq!(check_game(&old, &game).len(), 1);
    }

    #[test]
    fn correcciones_de_dependencias_del_modpack() {
        let questing = ModMeta {
            id: "create_questing".into(),
            name: "Create Questing".into(),
            version: "1.0.0".into(),
            depends: vec![("ftbquests".into(), vec!["*".into()]), ("fabricloader".into(), vec![">=0.16".into()])],
            ..Default::default()
        };
        let mut mods = vec![("Create_Questing-FABRIC-1.0.1.jar".to_string(), vec![questing])];
        let game = Game { minecraft: "1.20.1".into(), loader: "Fabric".into(), loader_version: Some("0.19.3".into()) };
        assert_eq!(check_game(&mods, &game).len(), 1);
        let overrides: Value = serde_json::from_str(r#"{ "version": 1, "overrides": { "create_questing": { "-depends": { "ftbquests": "IGNORED" } } } }"#).unwrap();
        apply_fabric_overrides(&mut mods, &overrides);
        assert!(check_game(&mods, &game).is_empty());
        assert_eq!(mods[0].1[0].depends.len(), 1, "solo se quitó ftbquests");
    }

    #[test]
    fn la_copia_que_pide_todo_el_mundo() {
        let cloth = |version: &str| ModMeta { id: "cloth-config".into(), name: "Cloth Config v11".into(), version: version.into(), nested: true, ..Default::default() };
        let grave = ModMeta { id: "yigd".into(), name: "You're in Grave Danger".into(), version: "2.0.16".into(), depends: vec![("cloth-config".into(), vec!["^11.0.0".into()])], ..Default::default() };
        let elevator = ModMeta { id: "quartz-elevator".into(), name: "Quartz Elevator".into(), version: "2.2.7".into(), ..Default::default() };
        let mods = vec![
            ("youre-in-grave-danger.jar".to_string(), vec![grave, cloth("11.1.136")]),
            ("quartz-elevator.jar".to_string(), vec![elevator, cloth("13.0.121")]),
        ];
        let game = Game { minecraft: "1.20.1".into(), loader: "Fabric".into(), loader_version: Some("0.16.0".into()) };
        assert!(check_game(&mods, &game).is_empty(), "Fabric usa la 11, que sirve a todos");
        assert!(matches("1.19.2", "[1.19.0,1.19.1,1.19.2)"), "rango mal escrito: no se toma como problema");
    }

    #[test]
    fn versiones_como_maven() {
        use std::cmp::Ordering::*;
        // Lo que va tras el guion es otra lista, no una versión previa (The Aether de BMC4).
        assert_eq!(maven_compare("1.20.1-1.5.2-neoforge", "1.20.1"), Greater);
        assert!(matches("1.20.1-1.5.2-neoforge", "[1.20.1,]"));
        assert_eq!(maven_compare("1.0-beta", "1.0"), Less);
        assert_eq!(maven_compare("1.0-rc1", "1.0-beta2"), Greater);
        assert_eq!(maven_compare("1.0.0", "1"), Equal);
        assert_eq!(maven_compare("1.0-sp", "1.0"), Greater);
        assert_eq!(maven_compare("47.4.20", "47.4.3"), Greater);
        assert_eq!(maven_compare("2.15.1", "2.15.1.0"), Equal);
        assert_eq!(maven_compare("1.20.1", "1.21"), Less);
        assert!(matches("47.4.20", "[47.1.0,)") && !matches("1.21.1", "[1.20.1,1.21)") && matches("1.20.1", "[1.20.1]"));
    }

    #[test]
    fn cada_loader_lee_su_archivo() {
        // Immersive Lanterns trae los dos: para Forge accessories es opcional, para NeoForge 1.21 no.
        let forge = "modLoader=\"javafml\"\n[[mods]]\nmodId=\"immersivelanterns\"\n[[dependencies.\"immersivelanterns\"]]\nmodId=\"accessories\"\nmandatory=false\nversionRange=\"*\"\n";
        let neo = "modLoader=\"javafml\"\n[[mods]]\nmodId=\"immersivelanterns\"\n[[dependencies.\"immersivelanterns\"]]\nmodId=\"accessories\"\nmandatory=true\nversionRange=\"*\"\n";
        let metas = read_jar(Cursor::new(jar(&[("META-INF/mods.toml", forge), ("META-INF/neoforge.mods.toml", neo)])));
        assert_eq!(metas.len(), 2);
        let mods = vec![("immersivelanterns.jar".to_string(), metas)];
        let game = |loader: &str, minecraft: &str| Game { minecraft: minecraft.into(), loader: loader.into(), loader_version: None };
        assert!(check_game(&mods, &game("Forge", "1.20.1")).is_empty(), "en Forge no falta nada");
        assert!(check_game(&mods, &game("NeoForge", "1.20.1")).is_empty(), "NeoForge 1.20.1 lee mods.toml");
        assert_eq!(check_game(&mods, &game("NeoForge", "1.21.1")).len(), 1, "NeoForge 1.21 lee neoforge.mods.toml y ahí accessories es obligatorio");
    }

    #[test]
    fn neoforge_no_exige_el_rango_de_minecraft_al_pie_de_la_letra() {
        let jei = ModMeta {
            id: "jei".into(),
            name: "Just Enough Items".into(),
            version: "19.52.0.422".into(),
            depends: vec![("minecraft".into(), vec!["[1.21, 1.21.1)".into()])],
            forge: true,
            ..Default::default()
        };
        let mods = vec![("jei.jar".to_string(), vec![jei])];
        let game = |loader: &str, minecraft: &str| Game { minecraft: minecraft.into(), loader: loader.into(), loader_version: None };
        assert!(check_game(&mods, &game("NeoForge", "1.21.1")).is_empty());
        assert_eq!(check_game(&mods, &game("Forge", "1.21.1")).len(), 1);
        assert_eq!(check_game(&mods, &game("NeoForge", "1.20.4")).len(), 1, "otra línea de Minecraft sí es un problema");
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
