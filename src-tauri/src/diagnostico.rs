//! El doctor de Parabellum: por qué no abre una instancia y cómo arreglarlo.
//!  - Antes de jugar (`check`): lee lo que declara cada mod (fabric.mod.json, mods.toml) y busca
//!    dependencias que faltan, versiones del loader o del juego que no encajan y mods repetidos.
//!  - Después de un fallo (`after_crash`): lee el informe de fallo y los registros y reconoce los
//!    errores conocidos: un mod que pide un argumento de Java, Java equivocado, falta de memoria,
//!    configuración dañada, el mod sospechoso…
//! Cada hallazgo trae arreglos de un clic. «Arreglar todo» aplica los recomendados.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use crate::compat::{self, Game, ModMeta, Problem};
use crate::instances::Instance;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Fix {
    /// Agregar un argumento de Java a la instancia (p. ej. el que pide el propio mod en su error).
    AddJvmArg { arg: String },
    /// Cambiar la versión del loader a una que cumpla todos los rangos.
    SetLoader { requirements: Vec<String> },
    /// Instalar desde Modrinth el mod con ese id (el de su fabric.mod.json o mods.toml), en una
    /// versión que cumpla los rangos. `replaces`: la copia que hay y no sirve (se desactiva).
    InstallMod { id: String, requirements: Vec<String>, replaces: Option<String> },
    /// Desactivar un mod (archivo de la carpeta mods): se puede volver a activar en Contenido.
    DisableMod { file: String },
    /// Volver a activar un mod desactivado (archivo .jar.disabled) que otro necesita.
    EnableMod { file: String },
    /// Mandar a la papelera un archivo de configuración dañado: el mod lo vuelve a crear.
    ResetConfig { path: String },
    SetRam { gb: u32 },
    /// Usar otra versión de Java en esta instancia (se descarga la de Mojang si falta).
    UseJava { major: u32 },
    /// Verificar y reponer los archivos del modpack.
    Repair,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FixOption {
    pub label: String,
    pub fix: Fix,
    pub recommended: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// "error": así no abre; "aviso": puede ser la causa; "info": explicación sin arreglo automático.
    pub severity: &'static str,
    pub title: String,
    pub detail: String,
    /// La línea del registro que lo demuestra.
    pub evidence: Option<String>,
    pub fixes: Vec<FixOption>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Diagnosis {
    pub findings: Vec<Finding>,
    /// Cuántos mods se revisaron.
    pub checked_mods: usize,
}

fn option(label: String, fix: Fix, recommended: bool) -> FixOption {
    FixOption { label, fix, recommended }
}

// ================= Lo que declara cada mod (con caché)

const CACHE_FILE: &str = "dependencias.json";
const CACHE_VERSION: u32 = 3;

#[derive(Serialize, Deserialize, Default)]
struct Cache {
    version: u32,
    files: HashMap<String, CachedJar>,
}

#[derive(Serialize, Deserialize, Clone)]
struct CachedJar {
    size: u64,
    modified: i64,
    metas: Vec<ModMeta>,
    #[serde(default)]
    loaders: compat::JarLoaders,
}

fn modified_seconds(meta: &std::fs::Metadata) -> i64 {
    meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Los mods activos de la instancia con lo que declara cada uno.
pub fn instance_mods(instance: &Instance) -> Vec<(String, Vec<ModMeta>)> {
    all_mods(instance).into_iter().filter(|(file, _)| !file.to_lowercase().ends_with(".disabled")).collect()
}

/// Todos los mods de la carpeta, también los desactivados (.jar.disabled).
fn all_mods(instance: &Instance) -> Vec<(String, Vec<ModMeta>)> {
    all_jars(instance).into_iter().map(|(name, jar)| (name, jar.metas)).collect()
}

/// Cada .jar de la carpeta mods con lo que declara y para qué loaders es. Lo leído se guarda
/// (archivo, tamaño y fecha) para que revisar antes de jugar sea instantáneo; la primera vez se
/// leen en paralelo.
fn all_jars(instance: &Instance) -> Vec<(String, CachedJar)> {
    let folder = instance.game_dir.join("mods");
    let cache_path = instance.state_dir().join(CACHE_FILE);
    let mut cache: Cache = std::fs::read_to_string(&cache_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if cache.version != CACHE_VERSION {
        cache = Cache { version: CACHE_VERSION, ..Default::default() };
    }
    let mut jars: Vec<(String, u64, i64)> = std::fs::read_dir(&folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let meta = e.metadata().ok().filter(|m| m.is_file())?;
            let lower = name.to_lowercase();
            (lower.ends_with(".jar") || lower.ends_with(".jar.disabled")).then(|| (name, meta.len(), modified_seconds(&meta)))
        })
        .collect();
    jars.sort();
    let stale: Vec<String> = jars
        .iter()
        .filter(|(name, size, modified)| !cache.files.get(name).is_some_and(|c| c.size == *size && c.modified == *modified))
        .map(|(name, _, _)| name.clone())
        .collect();
    if !stale.is_empty() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(2, 8);
        let chunk = stale.len().div_ceil(threads).max(1);
        let read: Vec<(String, Vec<ModMeta>, compat::JarLoaders)> = std::thread::scope(|scope| {
            let handles: Vec<_> = stale
                .chunks(chunk)
                .map(|names| {
                    let folder = &folder;
                    scope.spawn(move || {
                        names
                            .iter()
                            .map(|name| {
                                let path = folder.join(name);
                                let metas = std::fs::File::open(&path).map(compat::read_jar).unwrap_or_default();
                                let loaders = std::fs::File::open(&path).map(compat::read_loaders).unwrap_or_default();
                                (name.clone(), metas, loaders)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        for (name, metas, loaders) in read {
            if let Some((_, size, modified)) = jars.iter().find(|(n, _, _)| *n == name) {
                cache.files.insert(name, CachedJar { size: *size, modified: *modified, metas, loaders });
            }
        }
    }
    let present: std::collections::HashSet<&String> = jars.iter().map(|(n, _, _)| n).collect();
    let before = cache.files.len();
    cache.files.retain(|name, _| present.contains(name));
    if !stale.is_empty() || cache.files.len() != before {
        let _ = std::fs::create_dir_all(instance.state_dir());
        let _ = std::fs::write(&cache_path, serde_json::to_vec(&cache).unwrap_or_default());
    }
    jars.into_iter()
        .map(|(name, size, modified)| {
            let jar = cache.files.get(&name).cloned().unwrap_or(CachedJar { size, modified, metas: Vec::new(), loaders: Default::default() });
            (name, jar)
        })
        .collect()
}

/// id de mod -> archivo que lo trae (el principal de cada .jar y los que lleva dentro).
fn files_by_id(mods: &[(String, Vec<ModMeta>)]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (file, metas) in mods {
        for meta in metas {
            map.entry(meta.id.to_lowercase()).or_insert_with(|| file.clone());
        }
    }
    map
}

/// Nombre visible de un archivo de mods (el de su mod principal).
fn file_names(mods: &[(String, Vec<ModMeta>)]) -> HashMap<String, String> {
    mods.iter().map(|(file, metas)| (file.clone(), metas.first().map(|m| m.name.clone()).unwrap_or_else(|| file.clone()))).collect()
}

fn loader_name(id: &str) -> &str {
    match id {
        "fabricloader" => "Fabric Loader",
        "quilt_loader" => "Quilt Loader",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => id,
    }
}

/// "[47.4.23,)" -> "47.4.23 o más nueva"; lo demás tal cual.
fn readable(predicate: &str) -> String {
    let predicate = &predicate.replace(' ', "");
    if predicate == "*" {
        return t!("cualquier versión", "any version");
    }
    match compat::minimum(predicate) {
        Some(low) if predicate.starts_with('[') && predicate.trim_end().ends_with(",)") => t!("{low} o más nueva", "{low} or newer"),
        Some(low) if predicate.trim().starts_with(">=") && !predicate.contains('<') => t!("{low} o más nueva", "{low} or newer"),
        _ => predicate.to_string(),
    }
}

fn list(names: &[String]) -> String {
    let shown: Vec<&str> = names.iter().take(4).map(String::as_str).collect();
    let more = names.len().saturating_sub(shown.len());
    let text = shown.join(", ");
    if more > 0 { t!("{text} y {more} más", "{text} and {more} more") } else { text }
}

// ================= Antes de jugar

/// Revisa los mods de la instancia sin abrir el juego.
pub fn check(instance: &Instance) -> Diagnosis {
    if instance.loader == "Vanilla" {
        return Diagnosis::default();
    }
    let jars = all_jars(instance);
    let connector = jars.iter().any(|(file, jar)| !file.to_lowercase().ends_with(".disabled") && jar.metas.iter().any(|m| m.id == "connectormod"));
    // Mods hechos para otro loader (uno de Forge en NeoForge 1.21, uno de Fabric en Forge…): no cargan.
    let mut findings = Vec::new();
    let mut wrong_loader = std::collections::HashSet::new();
    let fits = |file: &str, jar: &CachedJar| {
        !file.to_lowercase().ends_with(".disabled") && compat::loader_fits(jar.loaders, &instance.loader, &instance.minecraft_version, connector)
    };
    // Mods que ya están en su versión buena (la de otro loader sobra).
    let good: std::collections::HashSet<String> = jars.iter().filter(|(file, jar)| fits(file, jar)).filter_map(|(_, jar)| jar.metas.first().map(|m| m.id.clone())).collect();
    for (file, jar) in &jars {
        if file.to_lowercase().ends_with(".disabled") || fits(file, jar) {
            continue;
        }
        wrong_loader.insert(file.clone());
        let name = jar.metas.first().map(|m| m.name.clone()).unwrap_or_else(|| file.clone());
        let id = jar.metas.first().map(|m| m.id.clone());
        let mut fixes = Vec::new();
        match id {
            Some(id) if good.contains(&id) => fixes.push(option(
                t!("Quitar esta copia (ya está la de {})", "Remove this copy (the {} one is already there)", instance.loader),
                Fix::DisableMod { file: file.clone() },
                true,
            )),
            Some(id) => {
                fixes.push(option(
                    t!("Cambiarlo por el de {} (Modrinth)", "Replace it with the {} one (Modrinth)", instance.loader),
                    Fix::InstallMod { id, requirements: Vec::new(), replaces: Some(file.clone()) },
                    true,
                ));
                fixes.push(option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file: file.clone() }, false));
            }
            None => fixes.push(option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file: file.clone() }, true)),
        }
        findings.push(Finding {
            severity: "error",
            title: t!("{name} es un mod de {}", "{name} is a {} mod", jar.loaders.describe()),
            detail: t!(
                "Esta instancia usa {} {} y ese archivo no carga en ella.",
                "This instance uses {} {} and that file does not load in it.",
                instance.loader,
                instance.minecraft_version
            ),
            evidence: Some(file.clone()),
            fixes,
        });
    }
    let (disabled, mods): (Vec<_>, Vec<_>) = jars
        .into_iter()
        .filter(|(file, _)| !wrong_loader.contains(file))
        .map(|(file, jar)| (file, jar.metas))
        .partition(|(file, _)| file.to_lowercase().ends_with(".disabled"));
    let game = Game::of(instance);
    findings.extend(problems_to_findings(&mods, &compat::check_game(&mods, &game), &disabled));
    for (_, name, files) in compat::duplicates(&mods, &game) {
        // Se queda la copia más nueva (por versión; si no se puede saber, la de nombre más largo).
        let version_of = |file: &String| mods.iter().find(|(f, _)| f == file).and_then(|(_, m)| m.first()).map(|m| m.version.clone()).unwrap_or_default();
        let mut sorted = files.clone();
        sorted.sort_by(|a, b| crate::loaders::compare_versions(&version_of(a), &version_of(b)).then(a.len().cmp(&b.len())));
        let keep = sorted.pop().unwrap_or_default();
        findings.push(Finding {
            severity: "error",
            title: t!("{name} está repetido", "{name} is duplicated"),
            detail: t!(
                "Hay {} copias en la carpeta mods y el loader no abre así. Se queda {keep}.",
                "There are {} copies in the mods folder and the loader will not start like that. {keep} stays.",
                files.len()
            ),
            evidence: Some(files.join(" · ")),
            fixes: sorted
                .iter()
                .map(|file| option(t!("Desactivar la copia {file}", "Disable the copy {file}"), Fix::DisableMod { file: file.clone() }, true))
                .collect(),
        });
    }
    Diagnosis { findings, checked_mods: mods.len() }
}

/// `disabled_mods`: los desactivados de la carpeta (lo que falta puede estar ahí).
fn problems_to_findings(mods: &[(String, Vec<ModMeta>)], problems: &[Problem], disabled_mods: &[(String, Vec<ModMeta>)]) -> Vec<Finding> {
    let disabled = files_by_id(disabled_mods);
    // Los nombres de los desactivados también sirven para explicar lo que falta.
    let mut everything: Vec<(String, Vec<ModMeta>)> = mods.to_vec();
    everything.extend(disabled_mods.iter().cloned());
    let names = compat::names(&everything);
    let show = |id: &str| names.get(id).cloned().unwrap_or_else(|| id.to_string());
    let file_name = file_names(mods);
    let mut findings = Vec::new();

    // Lo que falta, agrupado por mod que falta (varios pueden pedir el mismo).
    let mut missing: Vec<(String, Vec<String>, Vec<String>, Vec<String>)> = Vec::new(); // (id, dueños, archivos, rangos)
    // Versiones del loader que no encajan, agrupadas por loader.
    let mut loader: Vec<(String, Vec<String>, Vec<String>, Vec<String>, String)> = Vec::new(); // (id, dueños, archivos, rangos, tiene)
    for problem in problems {
        match problem {
            Problem::Missing { owner, name, id, predicates } => {
                let first = predicates.first().cloned().unwrap_or_else(|| "*".into());
                match missing.iter_mut().find(|(i, ..)| i == id) {
                    Some((_, owners, files, ranges)) => {
                        owners.push(name.clone());
                        files.push(owner.clone());
                        if first != "*" && !ranges.contains(&first) {
                            ranges.push(first);
                        }
                    }
                    None => missing.push((id.clone(), vec![name.clone()], vec![owner.clone()], if first == "*" { vec![] } else { vec![first] })),
                }
            }
            Problem::Wrong { owner, name, id, predicates, found, found_file } if found_file.is_empty() && id != "minecraft" => {
                let first = predicates.first().cloned().unwrap_or_else(|| "*".into());
                match loader.iter_mut().find(|(i, ..)| i == id) {
                    Some((_, owners, files, ranges, _)) => {
                        owners.push(name.clone());
                        files.push(owner.clone());
                        if !ranges.contains(&first) {
                            ranges.push(first);
                        }
                    }
                    None => loader.push((id.clone(), vec![name.clone()], vec![owner.clone()], vec![first], found.clone())),
                }
            }
            Problem::Wrong { owner, name, predicates, found, found_file, .. } if found_file.is_empty() => {
                // Mod para otra versión de Minecraft.
                let wanted = predicates.iter().map(|p| readable(p)).collect::<Vec<_>>().join(t!(" o ", " or ").as_str());
                findings.push(Finding {
                    severity: "error",
                    title: t!("{name} es para otra versión de Minecraft", "{name} is for another Minecraft version"),
                    detail: t!("Pide Minecraft {wanted} y esta instancia es {found}.", "It asks for Minecraft {wanted} and this instance is {found}."),
                    evidence: Some(owner.clone()),
                    fixes: vec![option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file: owner.clone() }, true)],
                });
            }
            Problem::Wrong { owner, name, id, predicates, found, found_file } => {
                let dep = show(id);
                let wanted = predicates.iter().map(|p| readable(p)).collect::<Vec<_>>().join(t!(" o ", " or ").as_str());
                findings.push(Finding {
                    severity: "error",
                    title: t!("{name} necesita otra versión de {dep}", "{name} needs another version of {dep}"),
                    detail: t!("Pide {dep} {wanted} y la que hay es {found}.", "It asks for {dep} {wanted} and the one installed is {found}."),
                    evidence: Some(format!("{owner} → {found_file}")),
                    fixes: vec![
                        option(
                            t!("Cambiar {dep} por una versión compatible (Modrinth)", "Replace {dep} with a compatible version (Modrinth)"),
                            Fix::InstallMod { id: id.clone(), requirements: predicates.iter().take(1).cloned().collect(), replaces: Some(found_file.clone()) },
                            true,
                        ),
                        option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file: owner.clone() }, false),
                    ],
                });
            }
            Problem::Breaks { owner, name, id, found, found_file } => {
                let dep = show(id);
                let other = file_name.get(found_file).cloned().unwrap_or_else(|| dep.clone());
                findings.push(Finding {
                    severity: "error",
                    title: t!("{name} no funciona con {dep}", "{name} does not work with {dep}"),
                    detail: t!(
                        "{name} dice que choca con {dep} {found}. Hay que quitar uno de los dos.",
                        "{name} says it conflicts with {dep} {found}. One of the two has to go."
                    ),
                    evidence: Some(format!("{owner} ✕ {found_file}")),
                    fixes: vec![
                        option(t!("Desactivar {other}", "Disable {other}"), Fix::DisableMod { file: found_file.clone() }, false),
                        option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file: owner.clone() }, false),
                    ],
                });
            }
        }
    }

    for (id, owners, files, ranges) in missing {
        let dep = show(&id);
        let wanted = if ranges.is_empty() { String::new() } else { format!(" ({})", ranges.iter().map(|r| readable(r)).collect::<Vec<_>>().join(", ")) };
        // Si está en la carpeta pero desactivado, basta con activarlo.
        let mut fixes = match disabled.get(&id.to_lowercase()) {
            Some(file) => vec![option(t!("Activar {dep}", "Enable {dep}"), Fix::EnableMod { file: file.clone() }, true)],
            None => vec![option(
                t!("Instalar {dep} desde Modrinth", "Install {dep} from Modrinth"),
                Fix::InstallMod { id: id.clone(), requirements: ranges.clone(), replaces: None },
                true,
            )],
        };
        for (owner, file) in owners.iter().zip(&files).take(3) {
            fixes.push(option(t!("Desactivar {owner}", "Disable {owner}"), Fix::DisableMod { file: file.clone() }, false));
        }
        findings.push(Finding {
            severity: "error",
            title: t!("Falta {dep}{wanted}", "{dep}{wanted} is missing"),
            detail: t!("Lo necesita: {}.", "Needed by: {}.", list(&owners)),
            evidence: Some(id.clone()),
            fixes,
        });
    }

    for (id, owners, files, ranges, found) in loader {
        let name = loader_name(&id).to_string();
        let wanted = ranges.iter().map(|r| readable(r)).collect::<Vec<_>>().join(", ");
        let mut fixes = vec![option(
            t!("Cambiar {name} a una versión compatible", "Switch {name} to a compatible version"),
            Fix::SetLoader { requirements: ranges.clone() },
            true,
        )];
        for (owner, file) in owners.iter().zip(&files).take(3) {
            fixes.push(option(t!("Desactivar {owner}", "Disable {owner}"), Fix::DisableMod { file: file.clone() }, false));
        }
        findings.push(Finding {
            severity: "error",
            title: t!("Hace falta otra versión de {name}", "Another version of {name} is needed"),
            detail: t!(
                "{} {} {name} {wanted} y la instancia tiene {found}.",
                "{} {} {name} {wanted} and the instance has {found}.",
                list(&owners),
                if owners.len() == 1 { t!("pide", "asks for") } else { t!("piden", "ask for") }
            ),
            evidence: Some(files.join(" · ")),
            fixes,
        });
    }
    findings
}

// ================= Después de un fallo

/// Lo que se sabe del arranque que falló.
pub struct CrashContext {
    /// Milisegundos Unix de cuando se abrió el juego (solo cuentan los registros posteriores).
    pub since_ms: i64,
    pub exit_code: Option<i32>,
    /// Memoria que tenía el juego y la del PC (GB).
    pub ram_gb: u32,
    pub system_ram_gb: u32,
}

/// ¿Escribió el juego un informe de fallo desde ese momento? (Forge muestra su pantalla de error y,
/// al cerrarla, el juego termina con código 0: sin esto parecería que no pasó nada.)
pub fn crash_report_since(instance: &Instance, since_ms: i64) -> bool {
    std::fs::read_dir(instance.game_dir.join("crash-reports"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| modified_ms(&e.path()).is_some_and(|m| m >= since_ms - 2000))
}

/// Cómo terminó «Probar y arreglar».
#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct AutoFix {
    /// Llegó al menú principal.
    pub ok: bool,
    /// Arranques de prueba que hizo.
    pub attempts: u32,
    /// Lo que arregló, en orden.
    pub applied: Vec<String>,
    /// Lo que no se pudo arreglar solo (si no llegó al menú).
    pub diagnosis: Option<Diagnosis>,
}

/// Los arreglos recomendados que todavía no se probaron.
pub fn recommended(diagnosis: &Diagnosis, tried: &[Fix]) -> Vec<(String, Fix)> {
    let mut list: Vec<(String, Fix)> = Vec::new();
    for finding in &diagnosis.findings {
        for option in finding.fixes.iter().filter(|o| o.recommended) {
            if !tried.contains(&option.fix) && !list.iter().any(|(_, f)| *f == option.fix) {
                list.push((option.label.clone(), option.fix.clone()));
            }
        }
    }
    list
}

/// Espera a que el juego llegue al menú principal (o se cierre). Al llegar, lo cierra.
pub fn wait_for_menu(child: &mut std::process::Child, instance: &Instance, started_ms: i64, limit_seconds: u64) -> Result<u64, Option<i32>> {
    let log = instance.game_dir.join("logs").join("latest.log");
    let started = std::time::Instant::now();
    let mut offset = 0u64;
    let mut tail = String::new();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(800));
        if let Ok(Some(status)) = child.try_wait() {
            return Err(status.code());
        }
        if modified_ms(&log).is_some_and(|m| m >= started_ms - 2000) {
            use std::io::{Read, Seek, SeekFrom};
            if let Ok(mut file) = std::fs::File::open(&log) {
                let length = file.metadata().map(|m| m.len()).unwrap_or(0);
                if length < offset {
                    offset = 0;
                }
                let _ = file.seek(SeekFrom::Start(offset));
                let mut bytes = Vec::new();
                let _ = file.read_to_end(&mut bytes);
                offset += bytes.len() as u64;
                tail.push_str(&String::from_utf8_lossy(&bytes));
                // El juego terminó de cargar cuando arranca el sonido o crea los atlas de texturas.
                if tail.contains("Sound engine started") || tail.contains("-atlas") {
                    crate::launch::kill(child.id());
                    let _ = child.wait();
                    return Ok(started.elapsed().as_secs());
                }
                // Forge: los errores al cargar mods se muestran en una pantalla dentro del juego.
                if tail.contains("Mod loading has failed") || tail.contains("Mod Loading has failed") {
                    crate::launch::kill(child.id());
                    let _ = child.wait();
                    return Err(Some(1));
                }
                if tail.len() > 2_000_000 {
                    tail.drain(..tail.len() - 200_000);
                }
            }
        }
        if started.elapsed().as_secs() > limit_seconds {
            crate::launch::kill(child.id());
            let _ = child.wait();
            return Err(None);
        }
    }
}

fn modified_ms(path: &Path) -> Option<i64> {
    std::fs::metadata(path).ok()?.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_millis() as i64)
}

fn read_lossy_tail(path: &Path, max: u64) -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut file) = std::fs::File::open(path) else { return String::new() };
    let length = file.metadata().map(|m| m.len()).unwrap_or(0);
    let _ = file.seek(SeekFrom::Start(length.saturating_sub(max)));
    let mut bytes = Vec::new();
    let _ = file.read_to_end(&mut bytes);
    String::from_utf8_lossy(&bytes).to_string()
}

/// Lo que dejó el juego: el informe de fallo más nuevo, latest.log y la salida de Java.
fn gather(instance: &Instance, since_ms: i64) -> String {
    let mut text = String::new();
    let newest_report = std::fs::read_dir(instance.game_dir.join("crash-reports"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| Some((e.path(), modified_ms(&e.path())?)))
        .filter(|(_, m)| *m >= since_ms - 2000)
        .max_by_key(|(_, m)| *m);
    if let Some((path, _)) = newest_report {
        text += &read_lossy_tail(&path, 400_000);
        text.push('\n');
    }
    let latest = instance.game_dir.join("logs").join("latest.log");
    if modified_ms(&latest).is_some_and(|m| m >= since_ms - 2000) {
        text += &read_lossy_tail(&latest, 600_000);
        text.push('\n');
    }
    text += &read_lossy_tail(&instance.state_dir().join("salida-java.log"), 300_000);
    text
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).unwrap()
}

/// La línea del registro donde está `found` (para mostrarla como prueba), recortada.
fn evidence(text: &str, at: usize) -> String {
    let start = text[..at].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[at..].find('\n').map(|i| at + i).unwrap_or(text.len());
    let line = text[start..end].trim();
    if line.chars().count() > 240 { format!("{}…", line.chars().take(240).collect::<String>()) } else { line.to_string() }
}

/// Explica un arranque fallido. Incluye lo que diga la revisión de mods.
pub fn after_crash(instance: &Instance, context: &CrashContext) -> Diagnosis {
    let text = gather(instance, context.since_ms);
    let mods = instance_mods(instance);
    let by_id = files_by_id(&mods);
    let names = file_names(&mods);
    let mut findings: Vec<Finding> = Vec::new();
    let mut add = |finding: Finding| {
        if !findings.iter().any(|f| f.title == finding.title) {
            findings.push(finding);
        }
    };

    // 1. El propio mod dice qué argumento de Java agregar (CustomSkinLoader y otros).
    static JVM_ARG: OnceLock<Regex> = OnceLock::new();
    let jvm_arg = JVM_ARG.get_or_init(|| regex(r"(?i)add\s+(-D[\w.\-]+(?:=[^\s,;'`]+?)?)\.?\s+to\s+(?:the\s+|your\s+)?JVM\s+arg"));
    for capture in jvm_arg.captures_iter(&text) {
        let arg = capture[1].trim_end_matches(['.', '"', '\'']).to_string();
        if instance.jvm_args.iter().any(|a| a == &arg) {
            continue;
        }
        add(Finding {
            severity: "error",
            title: t!("Un mod pide el ajuste de Java {arg}", "A mod asks for the Java setting {arg}"),
            detail: t!(
                "El propio error dice cómo evitarlo: agregar {arg} a los argumentos de Java de la instancia.",
                "The error itself says how to avoid it: add {arg} to the instance's Java arguments."
            ),
            evidence: Some(evidence(&text, capture.get(0).unwrap().start())),
            fixes: vec![option(t!("Agregar {arg}", "Add {arg}"), Fix::AddJvmArg { arg: arg.clone() }, true)],
        });
    }

    // 2. Un mod compilado para un Java más nuevo.
    static CLASS_VERSION: OnceLock<Regex> = OnceLock::new();
    let class_version = CLASS_VERSION.get_or_init(|| regex(r"class file version (\d+)(?:\.\d+)?\), this version of the Java Runtime only recognizes class file versions up to (\d+)"));
    if let Some(capture) = class_version.captures(&text) {
        let need = capture[1].parse::<u32>().unwrap_or(0).saturating_sub(44);
        let have = capture[2].parse::<u32>().unwrap_or(0).saturating_sub(44);
        if need > have {
            add(Finding {
                severity: "error",
                title: t!("Un mod necesita Java {need}", "A mod needs Java {need}"),
                detail: t!("La instancia se abrió con Java {have}.", "The instance opened with Java {have}."),
                evidence: Some(evidence(&text, capture.get(0).unwrap().start())),
                fixes: vec![option(t!("Usar Java {need} en esta instancia", "Use Java {need} in this instance"), Fix::UseJava { major: need }, true)],
            });
        }
    }

    // 3. Memoria.
    if let Some(at) = text.find("java.lang.OutOfMemoryError") {
        let room = context.system_ram_gb.saturating_sub(3);
        let more = (context.ram_gb + 2).min(room);
        let fixes = if more > context.ram_gb {
            vec![option(t!("Darle {more} GB a la instancia", "Give the instance {more} GB"), Fix::SetRam { gb: more }, true)]
        } else {
            Vec::new()
        };
        add(Finding {
            severity: "error",
            title: t!("Al juego le faltó memoria", "The game ran out of memory"),
            detail: if fixes.is_empty() {
                t!(
                    "Tiene {} GB y el PC no tiene más para darle. Cierra otros programas o quita mods pesados.",
                    "It has {} GB and the PC has no more to give. Close other programs or remove heavy mods.",
                    context.ram_gb
                )
            } else {
                t!("Tenía {} GB.", "It had {} GB.", context.ram_gb)
            },
            evidence: Some(evidence(&text, at)),
            fixes,
        });
    }
    static HEAP: OnceLock<Regex> = OnceLock::new();
    let heap = HEAP.get_or_init(|| regex(r"Could not reserve enough space for .*object heap|Invalid maximum heap size|Initial heap size set to a larger value"));
    if let Some(found) = heap.find(&text) {
        let less = context.ram_gb.saturating_sub(2).max(2);
        add(Finding {
            severity: "error",
            title: t!("Java no pudo apartar la memoria pedida", "Java could not reserve the requested memory"),
            detail: t!(
                "Se le pidieron {} GB y Windows no los dio. Prueba con menos.",
                "{} GB were requested and Windows did not provide them. Try with less.",
                context.ram_gb
            ),
            evidence: Some(evidence(&text, found.start())),
            fixes: if less < context.ram_gb { vec![option(t!("Bajar a {less} GB", "Lower to {less} GB"), Fix::SetRam { gb: less }, true)] } else { Vec::new() },
        });
    }

    // 4. Forge/NeoForge: "Mod x requires y 1.2 or above / Currently, y is not installed".
    static REQUIRES: OnceLock<Regex> = OnceLock::new();
    let requires = REQUIRES.get_or_init(|| regex(r"Mod (\S+) requires (\S+) (\S+) or above\s+Currently, \S+ is (not installed|\S+)"));
    for capture in requires.captures_iter(&text) {
        let (owner_id, dep, low, current) = (&capture[1], capture[2].to_lowercase(), &capture[3], &capture[4]);
        let owner_file = by_id.get(&owner_id.to_lowercase()).cloned();
        let owner_name = owner_file.as_ref().and_then(|f| names.get(f)).cloned().unwrap_or_else(|| owner_id.to_string());
        let range = format!("[{low},)");
        let mut fixes = Vec::new();
        let title;
        if matches!(dep.as_str(), "forge" | "neoforge") {
            title = t!("Hace falta otra versión de {}", "Another version of {} is needed", loader_name(&dep));
            fixes.push(option(t!("Cambiar {} a {low} o más nueva", "Switch {} to {low} or newer", loader_name(&dep)), Fix::SetLoader { requirements: vec![range] }, true));
        } else if dep == "minecraft" {
            title = t!("{owner_name} es para otra versión de Minecraft", "{owner_name} is for another Minecraft version");
        } else {
            title = if current == "not installed" { t!("Falta {dep} ({low} o más nueva)", "{dep} ({low} or newer) is missing") } else { t!("{owner_name} necesita otra versión de {dep}", "{owner_name} needs another version of {dep}") };
            let replaces = (current != "not installed").then(|| by_id.get(&dep).cloned()).flatten();
            fixes.push(option(t!("Instalar {dep} desde Modrinth", "Install {dep} from Modrinth"), Fix::InstallMod { id: dep.clone(), requirements: vec![range], replaces }, true));
        }
        if let Some(file) = owner_file {
            fixes.push(option(t!("Desactivar {owner_name}", "Disable {owner_name}"), Fix::DisableMod { file }, dep == "minecraft"));
        }
        add(Finding {
            severity: "error",
            title,
            detail: t!("{owner_name} pide {dep} {low} o más nueva; ahora: {current}.", "{owner_name} asks for {dep} {low} or newer; now: {current}.", ),
            evidence: Some(evidence(&text, capture.get(0).unwrap().start())),
            fixes,
        });
    }

    // 5. Configuración dañada: se reconoce el archivo y se manda a la papelera (el mod la vuelve a crear).
    static CONFIG: OnceLock<Regex> = OnceLock::new();
    let config = CONFIG.get_or_init(|| {
        regex(r"(?i)(?:Failed loading config file|ParsingException|ParseException|JsonSyntaxException|MalformedJsonException|Failed to (?:load|read|parse) config)[^\n]*?((?:config|defaultconfigs)[\\/][^\s:'`]+?\.(?:toml|json5?|properties|cfg|ya?ml|txt|snbt))")
    });
    static FORGE_CONFIG: OnceLock<Regex> = OnceLock::new();
    let forge_config = FORGE_CONFIG.get_or_init(|| regex(r"Failed loading config file (\S+?\.toml) of type (CLIENT|COMMON|STARTUP) for modid (\S+)"));
    let mut configs: Vec<(String, usize)> = config.captures_iter(&text).map(|c| (c[1].replace('\\', "/"), c.get(0).unwrap().start())).collect();
    configs.extend(forge_config.captures_iter(&text).map(|c| (format!("config/{}", &c[1]), c.get(0).unwrap().start())));
    for (path, at) in configs {
        let relative = path.rsplit_once("/config/").map(|(_, rest)| format!("config/{rest}")).unwrap_or(path.clone());
        if !instance.game_dir.join(&relative).is_file() {
            continue;
        }
        add(Finding {
            severity: "error",
            title: t!("Configuración dañada: {relative}", "Damaged settings: {relative}"),
            detail: t!(
                "Un mod no pudo leer su archivo de configuración. Si se quita, el mod vuelve a crear uno nuevo (con sus valores normales).",
                "A mod could not read its settings file. If it is removed, the mod creates a new one (with its normal values)."
            ),
            evidence: Some(evidence(&text, at)),
            fixes: vec![option(t!("Mandar {relative} a la papelera", "Move {relative} to the Recycle Bin"), Fix::ResetConfig { path: relative.clone() }, true)],
        });
    }

    // 5b. Dos módulos de Java con el mismo paquete (Forge no arranca): el que no es Minecraft sobra.
    static SPLIT: OnceLock<Regex> = OnceLock::new();
    let split = SPLIT.get_or_init(|| regex(r"ResolutionException: Modules? (\S+) and (\S+) export package (\S+) to module (\S+)"));
    if let Some(capture) = split.captures(&text) {
        let base = |module: &str| matches!(module, "minecraft" | "forge" | "fml_loader" | "fmlcore" | "javafmllanguage" | "neoforge") || module.starts_with("_1.");
        let culprit = [&capture[1], &capture[2]].into_iter().find(|m| !base(m)).map(str::to_string);
        let line = evidence(&text, capture.get(0).unwrap().start());
        let disabled: Vec<(String, Vec<ModMeta>)> = all_mods(instance).into_iter().filter(|(f, _)| f.to_lowercase().ends_with(".disabled")).collect();
        let already_disabled = culprit.as_deref().and_then(|m| module_file(m, &files_by_id(&disabled), &disabled)).is_some();
        match culprit.as_deref().and_then(|m| module_file(m, &by_id, &mods)) {
            _ if already_disabled => {}
            Some(file) => {
                let name = names.get(&file).cloned().unwrap_or_else(|| file.clone());
                add(Finding {
                    severity: "error",
                    title: t!("{name} choca con otro mod al arrancar", "{name} clashes with another mod at startup"),
                    detail: t!(
                        "Java encontró el mismo paquete ({}) en dos módulos y Forge no puede arrancar así. Desactivar {name} lo evita.",
                        "Java found the same package ({}) in two modules and Forge cannot start like that. Disabling {name} avoids it.",
                        &capture[3]
                    ),
                    evidence: Some(line),
                    fixes: vec![option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file }, true)],
                });
            }
            None => add(Finding {
                severity: "error",
                title: t!("Dos módulos de Java chocan al arrancar", "Two Java modules clash at startup"),
                detail: t!(
                    "{} y {} traen el mismo paquete ({}). Si uno es un mod, desactívalo en Contenido.",
                    "{} and {} contain the same package ({}). If one is a mod, disable it in Content.",
                    &capture[1],
                    &capture[2],
                    &capture[3]
                ),
                evidence: Some(line),
                fixes: Vec::new(),
            }),
        }
    }

    // 5c. En un servidor: un mod hecho solo para el juego usa clases que el servidor no tiene.
    static CLIENT_CLASS: OnceLock<Regex> = OnceLock::new();
    let client_class = CLIENT_CLASS.get_or_init(|| {
        regex(r"Attempted to load class (net/minecraft/client/\S+) for invalid dist DEDICATED_SERVER|Cannot load class (net[./]minecraft[./]client\S*) in environment type SERVER|NoClassDefFoundError: (net/minecraft/client/\S+)")
    });
    // Mixin solo prueba si la clase existe ("@Mixin target … was not found"): eso no tumba nada.
    // Pueden fallar varios mods a la vez: se juntan todos para quitarlos de una.
    let mut culprits: Vec<String> = Vec::new();
    let mut unknown: Option<String> = None;
    for capture in client_class.captures_iter(&text) {
        let (start, end) = (capture.get(0).unwrap().start(), capture.get(0).unwrap().end());
        let after: String = text[end..].chars().take(700).collect();
        if after.contains("@Mixin target") && after.contains("was not found") {
            continue;
        }
        match client_only_culprit(&text, start, end, &by_id, &names) {
            Some(file) if !culprits.contains(&file) => culprits.push(file),
            Some(_) => {}
            None => unknown = unknown.or_else(|| Some(evidence(&text, start))),
        }
    }
    for file in &culprits {
        let name = names.get(file).cloned().unwrap_or_else(|| file.clone());
        let line = text.find(file.as_str()).map(|at| evidence(&text, at));
        add(Finding {
            severity: "error",
            title: t!("{name} es solo para el juego y tumba el servidor", "{name} is client-only and crashes the server"),
            detail: t!(
                "Usa partes de Minecraft que solo existen en el juego (gráficos, menús). Los jugadores pueden seguir teniéndolo; el servidor no lo necesita.",
                "It uses parts of Minecraft that only exist in the game (graphics, menus). Players can keep it; the server does not need it."
            ),
            evidence: line,
            fixes: vec![option(t!("Quitar {name} del servidor", "Remove {name} from the server"), Fix::DisableMod { file: file.clone() }, true)],
        });
    }
    if let (true, Some(line)) = (culprits.is_empty(), unknown) {
        add(Finding {
            severity: "error",
            title: t!("Un mod solo para el juego tumba el servidor", "A client-only mod crashes the server"),
            detail: t!(
                "Algún mod usa partes de Minecraft que solo existen en el juego. Revisa el registro para ver cuál y quítalo del servidor.",
                "Some mod uses parts of Minecraft that only exist in the game. Check the log to see which one and remove it from the server."
            ),
            evidence: Some(line),
            fixes: Vec::new(),
        });
    }

    // 5d. En un servidor: el mundo no se puede leer. No se arregla solo (el mundo es del jugador).
    if let Some(at) = text.find("World files may be corrupted") {
        add(Finding {
            severity: "error",
            title: t!("El mundo del servidor está dañado", "The server world is damaged"),
            detail: t!(
                "Minecraft no pudo leer level.dat ni su copia. Si tienes una copia del mundo, ponla en la carpeta del servidor; si no, puedes empezar un mundo nuevo (el dañado va a la papelera y se puede recuperar).",
                "Minecraft could not read level.dat or its backup. If you have a copy of the world, put it in the server folder; if not, you can start a new world (the damaged one goes to the Recycle Bin and can be restored)."
            ),
            evidence: Some(evidence(&text, at)),
            fixes: vec![option(t!("Empezar un mundo nuevo", "Start a new world"), Fix::ResetConfig { path: "world".into() }, false)],
        });
    }

    // 6. Mixins que fallan: el mod que los trae es sospechoso.
    static MIXIN: OnceLock<Regex> = OnceLock::new();
    let mixin = MIXIN.get_or_init(|| regex(r"(?:MixinApplyError|InvalidInjectionException|InvalidMixinException|Critical injection failure|MixinTransformerError)[^\n]*?from mod ([\w\-]+)"));
    for capture in mixin.captures_iter(&text) {
        suspect(&capture[1], evidence(&text, capture.get(0).unwrap().start()), &by_id, &names, &mut add);
    }

    // 7. Lo que el informe de fallo da como sospechoso (Forge: "Suspected Mods: Nombre (id)").
    static SUSPECTED: OnceLock<Regex> = OnceLock::new();
    let suspected = SUSPECTED.get_or_init(|| regex(r"Suspected Mods?:\s*([^\n]+)"));
    static IDS: OnceLock<Regex> = OnceLock::new();
    let ids = IDS.get_or_init(|| regex(r"\(([\w\-]+)\)"));
    for capture in suspected.captures_iter(&text) {
        if capture[1].trim().eq_ignore_ascii_case("none") {
            continue;
        }
        for id in ids.captures_iter(&capture[1]) {
            if !matches!(&id[1], "minecraft" | "forge" | "neoforge" | "fabricloader" | "java") {
                suspect(&id[1], evidence(&text, capture.get(0).unwrap().start()), &by_id, &names, &mut add);
            }
        }
    }

    // 8. Video: no se arregla desde aquí, pero se explica.
    static VIDEO: OnceLock<Regex> = OnceLock::new();
    let video = VIDEO.get_or_init(|| regex(r"GLFW error 65542|WGL: The driver does not appear to support OpenGL|Pixel format not accelerated|Couldn't set pixel format|OpenGL 3\.2 is not supported|GLFW error 65543"));
    if let Some(found) = video.find(&text) {
        add(Finding {
            severity: "info",
            title: t!("La tarjeta de video no abrió el juego", "The graphics card could not open the game"),
            detail: t!(
                "Windows no dio OpenGL a Minecraft. Suele arreglarse actualizando el driver de video (Intel, AMD o NVIDIA) desde la página del fabricante.",
                "Windows did not give OpenGL to Minecraft. It is usually fixed by updating the graphics driver (Intel, AMD or NVIDIA) from the manufacturer's page."
            ),
            evidence: Some(evidence(&text, found.start())),
            fixes: Vec::new(),
        });
    }

    // 9. Java se cerró de golpe (fallo nativo) sin informe.
    if matches!(context.exit_code, Some(-1073740791) | Some(-1073741819) | Some(-1073741571)) && !text.contains("---- Minecraft Crash Report ----") {
        add(Finding {
            severity: "info",
            title: t!("Java se cerró de golpe", "Java closed abruptly"),
            detail: t!(
                "Fue un fallo nativo (fuera de Minecraft). Suele ser el driver de video o un mod con partes nativas (shaders, física, audio). Actualiza el driver y, si sigue, desactiva esos mods.",
                "It was a native crash (outside Minecraft). It is usually the graphics driver or a mod with native parts (shaders, physics, audio). Update the driver and, if it continues, disable those mods."
            ),
            evidence: None,
            fixes: Vec::new(),
        });
    }

    // Lo que ya se arregló después de ese arranque (p. ej. el mod ya está desactivado) no se repite.
    findings.retain(|f| !f.fixes.iter().any(|o| already_done(instance, &o.fix)));
    // La revisión de mods explica los fallos al cargar (dependencias, versiones, repetidos).
    for finding in check(instance).findings {
        if !findings.iter().any(|f| f.title == finding.title) {
            findings.push(finding);
        }
    }
    Diagnosis { findings, checked_mods: mods.len() }
}

/// ¿Ese arreglo ya está aplicado? (para no volver a proponer lo que se arregló después del fallo)
fn already_done(instance: &Instance, fix: &Fix) -> bool {
    let mods = instance.game_dir.join("mods");
    match fix {
        Fix::AddJvmArg { arg } => instance.jvm_args.iter().any(|a| a == arg),
        Fix::SetLoader { requirements } => instance.loader_version.as_deref().is_some_and(|v| requirements.iter().all(|r| compat::matches(v, r))),
        Fix::DisableMod { file } => !mods.join(file).exists() && mods.join(format!("{file}.disabled")).exists(),
        Fix::EnableMod { file } => mods.join(file.strip_suffix(".disabled").unwrap_or(file)).exists(),
        Fix::ResetConfig { path } => !instance.game_dir.join(path).exists(),
        Fix::SetRam { gb } => instance.ram_gb == Some(*gb),
        Fix::UseJava { major } => instance.java_major == Some(*major),
        Fix::InstallMod { .. } | Fix::Repair => false,
    }
}

/// El mod culpable de un error de "clase de cliente en el servidor". Forge lo dice justo antes
/// ("Failed to create mod instance. ModID: oculus" en el registro; "Mod File: …/oculus.jar" en el
/// informe de fallo); si no, es el primer mod de la carpeta mods que aparece en la traza
/// ("~[archivo.jar%23123!/:?]").
fn client_only_culprit(text: &str, start: usize, end: usize, by_id: &HashMap<String, String>, names: &HashMap<String, String>) -> Option<String> {
    static BEFORE: OnceLock<Regex> = OnceLock::new();
    let before_pattern = BEFORE.get_or_init(|| regex(r"ModID: ([\w\-]+)|Mod File: [^\n]*?([^/\\\n]+\.jar)"));
    // Solo las 3 líneas anteriores: más arriba puede haber otro mod que no tiene que ver.
    let from = text[..start].match_indices('\n').rev().nth(3).map(|(i, _)| i).unwrap_or(0);
    let before = &text[from..start];
    let named = before_pattern.captures_iter(before).last().and_then(|c| match (c.get(1), c.get(2)) {
        (Some(id), _) => by_id.get(&id.as_str().to_lowercase()).cloned(),
        (_, Some(jar)) => Some(jar.as_str().to_string()).filter(|jar| names.contains_key(jar)),
        _ => None,
    });
    if named.is_some() {
        return named;
    }
    static FRAME: OnceLock<Regex> = OnceLock::new();
    let frame = FRAME.get_or_init(|| regex(r"([^\[\]/\\%\s]+\.jar)(?:%23\d+|!/)"));
    let window: String = text[end..].chars().take(8000).collect();
    frame.captures_iter(&window).map(|c| c[1].to_string()).find(|jar| names.contains_key(jar))
}

/// El archivo de un módulo de Java: su nombre suele ser el id del mod, o sale del nombre del archivo.
fn module_file(module: &str, by_id: &HashMap<String, String>, mods: &[(String, Vec<ModMeta>)]) -> Option<String> {
    let simple = |text: &str| text.to_lowercase().chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>();
    let wanted = simple(module);
    by_id.get(&module.to_lowercase()).cloned().or_else(|| {
        mods.iter().map(|(file, _)| file).find(|file| {
            let stem = simple(file.split(['-', '_', '+']).next().unwrap_or(file));
            !stem.is_empty() && (stem == wanted || simple(file).starts_with(&wanted))
        }).cloned()
    })
}

/// Un mod sospechoso: se puede desactivar para probar (no se recomienda solo: puede ser necesario).
fn suspect(id: &str, line: String, by_id: &HashMap<String, String>, names: &HashMap<String, String>, add: &mut impl FnMut(Finding)) {
    let Some(file) = by_id.get(&id.to_lowercase()) else { return };
    let name = names.get(file).cloned().unwrap_or_else(|| id.to_string());
    add(Finding {
        severity: "aviso",
        title: t!("Posible culpable: {name}", "Possible culprit: {name}"),
        detail: t!(
            "El error sale de {name}. Desactivarlo suele dejar abrir el juego; si otros mods lo necesitan, la revisión lo dirá.",
            "The error comes from {name}. Disabling it usually lets the game open; if other mods need it, the check will say so."
        ),
        evidence: Some(line),
        fixes: vec![option(t!("Desactivar {name}", "Disable {name}"), Fix::DisableMod { file: file.clone() }, false)],
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconoce_el_argumento_que_pide_customskinloader() {
        let text = "Caused by: java.lang.IllegalStateException: Patch 'x' matched protocol 0 but did not modify any bytecode. If you want to force CustomSkinLoader to ignore this patch failure, add -Dcustomskinloader.ignorePatchFailure=true to the JVM arguments.";
        let capture = Regex::new(r"(?i)add\s+(-D[\w.\-]+(?:=[^\s,;'`]+?)?)\.?\s+to\s+(?:the\s+|your\s+)?JVM\s+arg").unwrap().captures(text).unwrap();
        assert_eq!(&capture[1], "-Dcustomskinloader.ignorePatchFailure=true");
    }

    #[test]
    fn culpable_de_solo_cliente() {
        let names = HashMap::from([
            ("BetterF3-7.0.2-Forge-1.20.1.jar".to_string(), "BetterF3".to_string()),
            ("oculus-mc1.20.1-1.8.0.jar".to_string(), "Oculus".to_string()),
        ]);
        let by_id = HashMap::from([("oculus".to_string(), "oculus-mc1.20.1-1.8.0.jar".to_string())]);
        let error = "RuntimeException: Attempted to load class net/minecraft/client/gui/screens/Screen for invalid dist DEDICATED_SERVER";
        let culprit = |text: &str| {
            let start = text.find(error).unwrap();
            client_only_culprit(text, start, start + error.len(), &by_id, &names)
        };
        // En la traza, después del error.
        let trace = format!("{error}\n\tat net.minecraftforge.fml.loading.RuntimeDistCleaner.processClassWithFlags(RuntimeDistCleaner.java:57) ~[fmlloader-1.20.1-47.4.0.jar%23103!/:1.0]\n\tat tech.betterf3.BetterF3Forge.<init>(BetterF3Forge.java:20) ~[BetterF3-7.0.2-Forge-1.20.1.jar%23186!/:7.0.2]");
        assert_eq!(culprit(&trace).as_deref(), Some("BetterF3-7.0.2-Forge-1.20.1.jar"));
        // En el registro de Forge, antes del error.
        let log = format!("[modloading-worker-0/ERROR] [FMLModContainer/LOADING]: Failed to create mod instance. ModID: oculus, class net.irisshaders.iris.Iris\njava.lang.{error}\n\tat cpw.mods.Launcher.run(Launcher.java:1) ~[modlauncher-10.0.9.jar%2355!/:?]");
        assert_eq!(culprit(&log).as_deref(), Some("oculus-mc1.20.1-1.8.0.jar"));
        // En el informe de fallo, antes del error (la ruta tiene espacios).
        let report = format!("-- MOD oculus --\n\tMod File: /C:/Mis servidores/Prueba forge/mods/oculus-mc1.20.1-1.8.0.jar\n\tFailure message: Oculus (oculus) has failed to load correctly\n\t\tjava.lang.{error}");
        assert_eq!(culprit(&report).as_deref(), Some("oculus-mc1.20.1-1.8.0.jar"));
        assert!(culprit(&format!("{error}\n\tat net.minecraft.server.Main.main(Main.java:1)")).is_none());
    }

    #[test]
    fn rangos_legibles() {
        assert_eq!(readable("[47.4.23,)"), "47.4.23 o más nueva");
        assert_eq!(readable(">=0.16.10"), "0.16.10 o más nueva");
        assert_eq!(readable("[1.20,1.21)"), "[1.20,1.21)");
        assert_eq!(readable("[1.0.0-beta.18, )"), "1.0.0-beta.18 o más nueva");
    }
}
