//! Servidores dedicados hechos a partir de una instancia («Hostear partida»). Cada uno vive en
//! servidores/<nombre>/ con su propio software (vanilla, Fabric, Quilt, Forge o NeoForge), los mods de
//! la instancia que sirven en un servidor, su configuración y su mundo.
//! Es un servidor no premium (online-mode=false) con lista blanca: solo entran los invitados.

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::BufRead;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use crate::download::{self, FileDownload};
use crate::instances::{self, Instance};
use crate::java;
use crate::loaders::{compare_versions, Loader};
use crate::paths::Paths;
use crate::{diagnostico, forge, modrinth, mojang, nbt, proceso, settings};

const CONFIG_FILE: &str = "parabellum-servidor.json";
/// Carpetas de la instancia que también usa el servidor (además de mods).
const CONTENT_FOLDERS: [&str; 8] = ["config", "defaultconfigs", "kubejs", "scripts", "global_packs", "globalpacks", "openloader", "paxi"];
/// Mods que solo tienen sentido en el cliente (gráficos, cámara, menús) y que a menudo tumban un servidor.
const CLIENT_ONLY: &[&str] = &[
    "oculus", "iris", "sodium", "embeddium", "rubidium", "magnesium", "entityculling", "entity_texture_features", "entity_model_features",
    "notenoughanimations", "lambdynlights", "dynamiclightsreforged", "betterf3", "citresewn", "continuity", "zoomify", "okzoomer", "modmenu",
    "presencefootsteps", "skinlayers3d", "firstperson", "customskinloader", "shouldersurfing", "betterthirdperson", "legendarytooltips",
    "fancymenu", "drippyloadingscreen", "chat_heads", "fpsreducer", "ambientsounds", "controlling", "mousetweaks",
];
/// Mods que abren su propio túnel a internet (e4mc, World Host): el servidor ya tiene la dirección de
/// playit.gg, así que sobran y confunden (otra dirección en la consola). Además e4mc 6.2 tumba los
/// servidores de Minecraft 26.2 en cuanto entra alguien.
const TUNNEL_MODS: &[&str] = &["e4mc", "e4mc_minecraft", "world-host"];

/// Si el mod es de túnel, su id.
fn tunnel_mod(file: &str, metas: &[crate::compat::ModMeta]) -> Option<String> {
    metas
        .first()
        .map(|m| m.id.as_str())
        .filter(|id| TUNNEL_MODS.contains(id))
        .map(String::from)
        .or_else(|| file.to_lowercase().starts_with("e4mc").then(|| "e4mc".to_string()))
}

/// Banderas de Java para servidores (las de Aikar, sin reservar toda la memoria al arrancar: en un PC
/// de 8 GB eso dejaría sin memoria al juego).
const SERVER_FLAGS: [&str; 14] = [
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-Dfile.encoding=UTF-8",
    "-Dstdout.encoding=UTF-8",
    "-Dstderr.encoding=UTF-8",
    "-Dlog4j2.formatMsgNoLookups=true",
];

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Guest {
    pub name: String,
    /// Cuenta de Parabellum del amigo (si es amigo): recibe la dirección aunque cambie.
    pub uid: Option<String>,
}

/// Un mod de la instancia que no se puso en el servidor, y por qué (la interfaz lo explica en su idioma).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct LeftOut {
    pub file: String,
    /// "modrinth" (Modrinth dice que es de cliente), "client" (Parabellum lo sabe), "dependency"
    /// (necesita uno de cliente) o "doctor" (tumbaba el servidor).
    pub kind: String,
    /// El id del mod ("client") o el de la dependencia ("dependency").
    pub detail: Option<String>,
}

impl LeftOut {
    pub fn new(file: &str, kind: &str, detail: Option<&str>) -> Self {
        LeftOut { file: file.to_string(), kind: kind.to_string(), detail: detail.map(String::from) }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ServerConfig {
    /// Nombre de la carpeta (no cambia al renombrar).
    #[serde(skip_deserializing)]
    pub id: String,
    pub name: String,
    /// Identificador para Firebase (letras y números al azar).
    pub key: String,
    /// Instancia de la que salió (y que deben tener los invitados).
    pub instance: String,
    pub instance_name: String,
    pub modpack: Option<String>,
    pub modpack_project: Option<String>,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub ram_gb: u32,
    pub jvm_args: Vec<String>,
    pub java_major: u32,
    /// El jugador aceptó el EULA de Minecraft (sin eso el servidor no arranca).
    pub eula: bool,
    pub port: u16,
    /// Lo que va después de los argumentos de Java: ["-jar", "server.jar", "nogui"] o ["@libraries/…/win_args.txt", "nogui"].
    pub launch: Vec<String>,
    pub guests: Vec<Guest>,
    pub created: i64,
    pub last_started: Option<i64>,
    pub left_out: Vec<LeftOut>,
    /// Segundos que tardó en estar listo la última vez.
    pub startup_seconds: Option<u64>,
    /// El nombre con el que el anfitrión jugó aquí la última vez: si se lo cambia, sus cosas
    /// (inventario, logros) y el permiso de administrador pasan al nombre nuevo.
    pub host_name: Option<String>,
    /// Nombres anteriores del anfitrión: dejan de ser administradores (con /op el servidor los
    /// guarda a su manera y ya no se distinguen de los que dio el jugador).
    pub former_host_names: Vec<String>,
    /// Quien entra con el nombre de un amigo invitado (o el del anfitrión) tiene que estar jugando desde
    /// Parabellum con esa cuenta; si no, se le saca. Así nadie entra poniéndose el nombre de otro.
    #[serde(default = "verify_by_default")]
    pub verify_players: bool,
    /// Cambios de nombre (anterior, nuevo) cuyas cosas falta pasar: el jugador estaba conectado.
    pub pending_moves: Vec<(String, String)>,
    /// Reglas del juego elegidas que todavía no están en el mundo (id de la opción → valor): se
    /// aplican con /gamerule en cuanto el servidor está listo (ver opciones.rs).
    pub pending_rules: std::collections::BTreeMap<String, String>,
    #[serde(skip)]
    pub folder: PathBuf,
}

fn verify_by_default() -> bool {
    true
}

impl ServerConfig {
    /// La instancia "vista desde el servidor": su carpeta de juego es la del servidor (para el doctor).
    pub fn as_instance(&self) -> Instance {
        Instance {
            id: self.id.clone(),
            name: self.name.clone(),
            version_id: self.minecraft.clone(),
            minecraft_version: self.minecraft.clone(),
            loader: self.loader.clone(),
            loader_version: self.loader_version.clone(),
            folder: self.folder.clone(),
            game_dir: self.folder.clone(),
            ram_gb: Some(self.ram_gb),
            jvm_args: self.jvm_args.clone(),
            last_played: self.last_started,
            created: Some(self.created),
            icon: None,
            modpack: self.modpack.clone(),
            modpack_project: self.modpack_project.clone(),
            java_major: Some(self.java_major),
        }
    }
}

fn load(folder: &Path) -> Option<ServerConfig> {
    let text = std::fs::read_to_string(folder.join(CONFIG_FILE)).ok()?;
    let mut config: ServerConfig = serde_json::from_str(&text).ok()?;
    config.id = folder.file_name()?.to_string_lossy().to_string();
    config.folder = folder.to_path_buf();
    Some(config)
}

pub fn list(paths: &Paths) -> Vec<ServerConfig> {
    let Ok(entries) = std::fs::read_dir(&paths.servers) else { return Vec::new() };
    let mut servers: Vec<ServerConfig> = entries.flatten().filter_map(|e| load(&e.path())).collect();
    servers.sort_by_key(|s| std::cmp::Reverse(s.last_started.unwrap_or(s.created)));
    servers
}

pub fn find(paths: &Paths, id: &str) -> Option<ServerConfig> {
    if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
        return None;
    }
    load(&paths.servers.join(id))
}

pub fn save(config: &ServerConfig) -> Result<(), String> {
    std::fs::write(config.folder.join(CONFIG_FILE), serde_json::to_vec_pretty(config).unwrap()).map_err(|e| e.to_string())
}

/// A la papelera de reciclaje (con su mundo: se puede recuperar).
pub fn delete(config: &ServerConfig) -> Result<(), String> {
    trash::delete(&config.folder).map_err(|e| t!("No se pudo mandar a la papelera: {e}", "Could not move it to the Recycle Bin: {e}"))
}

fn random_key() -> String {
    let mut buffer = [0u8; 6];
    getrandom::getrandom(&mut buffer).expect("números aleatorios");
    format!("s{}", hex::encode(buffer))
}

/// UUID con guiones (como lo escribe Minecraft en whitelist.json).
pub fn dashed(uuid: &str) -> String {
    if uuid.len() != 32 {
        return uuid.to_string();
    }
    format!("{}-{}-{}-{}-{}", &uuid[..8], &uuid[8..12], &uuid[12..16], &uuid[16..20], &uuid[20..])
}

pub fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ================= Memoria recomendada

/// Memoria para el servidor según cuántos mods lleva.
pub fn recommended_ram(mods: usize) -> u32 {
    match mods {
        0 => 2,
        1..=40 => 3,
        41..=120 => 4,
        121..=250 => 6,
        _ => 8,
    }
}

/// Un puerto libre: el de siempre si se puede (25565), si no, el que dé Windows.
pub fn free_port(preferred: u16) -> u16 {
    if preferred != 0 && std::net::TcpListener::bind(("0.0.0.0", preferred)).is_ok() {
        return preferred;
    }
    if std::net::TcpListener::bind(("0.0.0.0", 25565)).is_ok() {
        return 25565;
    }
    std::net::TcpListener::bind(("0.0.0.0", 0)).and_then(|l| l.local_addr()).map(|a| a.port()).unwrap_or(25565)
}

// ================= server.properties, EULA y listas

/// Escapa como un .properties de Java (los textos con ñ o tildes no se rompen en ninguna versión).
fn escape(value: &str) -> String {
    value
        .chars()
        .map(|c| match c {
            '\\' => "\\\\".to_string(),
            c if c.is_ascii() && !c.is_control() => c.to_string(),
            c => {
                let mut units = [0u16; 2];
                c.encode_utf16(&mut units).iter().map(|u| format!("\\u{u:04x}")).collect()
            }
        })
        .collect()
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut units: Vec<u16> = Vec::new();
    let mut chars = value.chars().peekable();
    let flush = |units: &mut Vec<u16>, out: &mut String| {
        out.push_str(&String::from_utf16_lossy(units));
        units.clear();
    };
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('u') => {
                    let code: String = chars.by_ref().take(4).collect();
                    if let Ok(unit) = u16::from_str_radix(&code, 16) {
                        units.push(unit);
                        continue;
                    }
                }
                Some(other) => {
                    flush(&mut units, &mut out);
                    out.push(match other {
                        'n' => '\n',
                        't' => '\t',
                        o => o,
                    });
                    continue;
                }
                None => {}
            }
        }
        flush(&mut units, &mut out);
        out.push(c);
    }
    flush(&mut units, &mut out);
    out
}

pub fn properties(folder: &Path) -> Vec<(String, String)> {
    let Ok(text) = std::fs::read_to_string(folder.join("server.properties")) else { return Vec::new() };
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.trim().to_string(), unescape(v.trim())))
        .collect()
}

pub fn property(folder: &Path, key: &str) -> Option<String> {
    properties(folder).into_iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// Cambia o agrega propiedades sin tocar las demás (ni los comentarios).
pub fn set_properties(folder: &Path, values: &[(&str, String)]) -> Result<(), String> {
    let file = folder.join("server.properties");
    let text = std::fs::read_to_string(&file).unwrap_or_else(|_| "#Minecraft server properties\n".into());
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    for (key, value) in values {
        let line = format!("{key}={}", escape(value));
        match lines.iter_mut().find(|l| l.split_once('=').is_some_and(|(k, _)| k.trim() == *key)) {
            Some(existing) => *existing = line,
            None => lines.push(line),
        }
    }
    std::fs::write(file, lines.join("\n") + "\n").map_err(|e| e.to_string())
}

pub fn write_eula(folder: &Path, accepted: bool) -> Result<(), String> {
    let text = format!(
        "#By changing the setting below to TRUE you are indicating your agreement to our EULA (https://aka.ms/MinecraftEULA).\n#{}\neula={accepted}\n",
        chrono::Local::now().format("%a %b %d %H:%M:%S %Y")
    );
    std::fs::write(folder.join("eula.txt"), text).map_err(|e| e.to_string())
}

/// whitelist.json con los invitados y el anfitrión; ops.json con el anfitrión (puede usar comandos).
/// Los nombres son los de ahora: el UUID de un servidor no premium sale del nombre exacto.
pub fn write_access_lists(config: &ServerConfig, host: &str) -> Result<(), String> {
    let mut names: Vec<&str> = Vec::new();
    for name in std::iter::once(host).chain(config.guests.iter().map(|g| g.name.as_str())) {
        if valid_name(name) && !names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            names.push(name);
        }
    }
    let entry = |name: &str| json!({ "uuid": dashed(&settings::offline_uuid(name)), "name": name });
    let whitelist: Vec<Value> = names.iter().map(|n| entry(n)).collect();
    std::fs::write(config.folder.join("whitelist.json"), serde_json::to_vec_pretty(&whitelist).unwrap()).map_err(|e| e.to_string())?;
    if valid_name(host) {
        let original: Vec<Value> = std::fs::read_to_string(config.folder.join("ops.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        // El administrador que pone Parabellum es el anfitrión de ahora, no sus nombres anteriores
        // (los que dio el jugador con /op se quedan).
        let former = |o: &Value| o["name"].as_str().is_some_and(|n| config.former_host_names.iter().any(|f| f == n));
        let mut ops: Vec<Value> = original.iter().filter(|o| o["name"].as_str() == Some(host) || !(parabellum_op(o) || former(o))).cloned().collect();
        if !ops.iter().any(|o| o["name"].as_str() == Some(host)) {
            let mut op = entry(host);
            op["level"] = json!(4);
            op["bypassesPlayerLimit"] = json!(true);
            ops.push(op);
        }
        if ops != original {
            std::fs::write(config.folder.join("ops.json"), serde_json::to_vec_pretty(&ops).unwrap()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Los administradores que pone Parabellum (el anfitrión) van con nivel 4 y sin límite de jugadores;
/// los de /op, no.
fn parabellum_op(op: &Value) -> bool {
    op["level"].as_u64() == Some(4) && op["bypassesPlayerLimit"].as_bool() == Some(true)
}

// ================= Los nombres siguen a la cuenta de cada jugador

/// Un cambio de nombre en un servidor.
#[derive(Clone, Debug, PartialEq)]
pub struct Rename {
    pub old: String,
    pub new: String,
    /// Es el anfitrión (pasa también el permiso de administrador).
    pub host: bool,
}

/// Pone los nombres de ahora: el del anfitrión y el de cada amigo invitado (que se sigue por su
/// cuenta de Google, no por el nombre). `friends`: uid -> nombre, si hay sesión.
pub fn follow_names(config: &mut ServerConfig, host: &str, friends: Option<&std::collections::HashMap<String, String>>) -> Vec<Rename> {
    let mut renames = Vec::new();
    if valid_name(host) {
        let previous = match &config.host_name {
            Some(name) => Some(name.clone()),
            // Servidores de antes de la 0.15: el administrador que puso Parabellum era el anfitrión.
            None => {
                let ops: Vec<Value> = std::fs::read_to_string(config.folder.join("ops.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
                let others: Vec<String> = ops.iter().filter(|o| parabellum_op(o)).filter_map(|o| o["name"].as_str()).filter(|n| *n != host).map(String::from).collect();
                (others.len() == 1).then(|| others[0].clone())
            }
        };
        if let Some(old) = previous.filter(|old| old != host && valid_name(old)) {
            config.former_host_names.retain(|n| n != host && n != &old);
            config.former_host_names.push(old.clone());
            renames.push(Rename { old, new: host.to_string(), host: true });
        }
        config.host_name = Some(host.to_string());
    }
    if let Some(friends) = friends {
        for i in 0..config.guests.len() {
            let Some(new) = config.guests[i].uid.as_ref().and_then(|uid| friends.get(uid)).filter(|n| valid_name(n)).cloned() else { continue };
            let old = config.guests[i].name.clone();
            // Otro invitado (o el anfitrión) ya usa ese nombre: se deja como estaba.
            let taken = new.eq_ignore_ascii_case(host) || config.guests.iter().enumerate().any(|(j, g)| j != i && g.name.eq_ignore_ascii_case(&new));
            if old == new || taken {
                continue;
            }
            config.guests[i].name = new.clone();
            renames.push(Rename { old, new, host: false });
        }
    }
    renames
}

/// Pasa las cosas de un jugador de su nombre anterior al nuevo: inventario, posición, logros,
/// estadísticas y lo que los mods guardan con su UUID en esas carpetas (en un servidor no premium el
/// UUID sale del nombre). Si el nombre nuevo ya tiene cosas propias no se toca nada. Devuelve cuántos
/// archivos pasó.
pub fn move_player_data(folder: &Path, old: &str, new: &str) -> usize {
    let level = property(folder, "level-name").filter(|l| !l.trim().is_empty()).unwrap_or_else(|| "world".into());
    let world = folder.join(level);
    let (from, to) = (dashed(&settings::offline_uuid(old)), dashed(&settings::offline_uuid(new)));
    if from == to {
        return 0;
    }
    let files: Vec<(PathBuf, PathBuf)> = ["playerdata", "advancements", "stats"]
        .iter()
        .flat_map(|dir| {
            let dir = world.join(dir);
            std::fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let rest = name.strip_prefix(&from)?;
                    Some((e.path(), dir.join(format!("{to}{rest}"))))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    if files.iter().any(|(_, target)| target.exists()) {
        return 0;
    }
    files.iter().filter(|(source, target)| std::fs::rename(source, target).is_ok()).count()
}

// ================= Ajustes que se cambian desde Parabellum

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub name: String,
    pub ram_gb: u32,
    pub jvm_args: Vec<String>,
    pub eula: bool,
    pub motd: String,
    pub max_players: u32,
    pub gamemode: String,
    pub difficulty: String,
    pub pvp: bool,
    pub view_distance: u32,
    pub simulation_distance: u32,
    pub allow_flight: bool,
    #[serde(default = "verify_by_default")]
    pub verify_players: bool,
}

pub const GAMEMODES: [&str; 4] = ["survival", "creative", "adventure", "spectator"];
pub const DIFFICULTIES: [&str; 4] = ["peaceful", "easy", "normal", "hard"];

/// Las versiones antiguas (antes de 1.13) guardan el modo y la dificultad como número.
fn legacy(minecraft: &str) -> bool {
    compare_versions(minecraft, "1.13") == std::cmp::Ordering::Less
}

pub fn named(value: &str, names: &[&str; 4], default: &str) -> String {
    match value.parse::<usize>() {
        Ok(number) => names.get(number).copied().unwrap_or(default).to_string(),
        Err(_) if names.contains(&value) => value.to_string(),
        Err(_) => default.to_string(),
    }
}

pub fn stored(value: &str, names: &[&str; 4], minecraft: &str) -> String {
    let index = names.iter().position(|n| *n == value).unwrap_or(0);
    if legacy(minecraft) { index.to_string() } else { names[index].to_string() }
}

pub fn read_settings(config: &ServerConfig) -> Settings {
    let properties = properties(&config.folder);
    let get = |key: &str| properties.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default();
    let number = |key: &str, default: u32| get(key).parse().unwrap_or(default);
    Settings {
        name: config.name.clone(),
        ram_gb: config.ram_gb,
        jvm_args: config.jvm_args.clone(),
        eula: config.eula,
        motd: get("motd"),
        max_players: number("max-players", 20),
        gamemode: named(&get("gamemode"), &GAMEMODES, "survival"),
        difficulty: named(&get("difficulty"), &DIFFICULTIES, "normal"),
        pvp: get("pvp") != "false",
        view_distance: number("view-distance", 10),
        simulation_distance: number("simulation-distance", 10),
        allow_flight: get("allow-flight") == "true",
        verify_players: config.verify_players,
    }
}

pub fn apply_settings(config: &mut ServerConfig, settings: &Settings) -> Result<(), String> {
    let name = settings.name.trim();
    if !name.is_empty() {
        config.name = name.chars().take(40).collect();
    }
    config.ram_gb = settings.ram_gb.clamp(1, 64);
    config.jvm_args = settings.jvm_args.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect();
    config.eula = settings.eula;
    config.verify_players = settings.verify_players;
    set_properties(
        &config.folder,
        &[
            ("motd", settings.motd.chars().take(59).collect()),
            ("max-players", settings.max_players.clamp(1, 100).to_string()),
            ("gamemode", stored(&settings.gamemode, &GAMEMODES, &config.minecraft)),
            ("difficulty", stored(&settings.difficulty, &DIFFICULTIES, &config.minecraft)),
            ("pvp", settings.pvp.to_string()),
            ("view-distance", settings.view_distance.clamp(3, 32).to_string()),
            ("simulation-distance", settings.simulation_distance.clamp(3, 32).to_string()),
            ("allow-flight", settings.allow_flight.to_string()),
        ],
    )?;
    write_eula(&config.folder, config.eula)?;
    save(config)
}

// ================= Crear

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NewServer {
    pub instance: String,
    pub name: String,
    /// Mundo de la instancia que se copia (None: mundo nuevo).
    pub world: Option<String>,
    #[serde(default)]
    pub seed: String,
    pub ram_gb: u32,
    #[serde(default)]
    pub jvm_args: Vec<String>,
    pub eula: bool,
    pub gamemode: String,
    pub difficulty: String,
    pub max_players: u32,
    pub pvp: bool,
}

/// Mundos guardados de una instancia (carpetas de saves con level.dat).
pub fn worlds(instance: &Instance) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(instance.game_dir.join("saves")) else { return Vec::new() };
    let mut worlds: Vec<(i64, String)> = entries
        .flatten()
        .filter(|e| e.path().join("level.dat").exists())
        .map(|e| {
            let modified = e.path().join("level.dat").metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64).unwrap_or(0);
            (modified, e.file_name().to_string_lossy().to_string())
        })
        .collect();
    worlds.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    worlds.into_iter().map(|(_, name)| name).collect()
}

fn folder_for(name: &str, paths: &Paths) -> Result<PathBuf, String> {
    let clean: String = name.trim().chars().map(|c| if r#"<>:"/\|?*"#.contains(c) || c.is_control() { '_' } else { c }).collect();
    let clean = clean.trim_matches(['.', ' ']).to_string();
    if clean.is_empty() {
        return Err(t!("Escribe un nombre para el servidor.", "Type a name for the server."));
    }
    let mut candidate = paths.servers.join(&clean);
    let mut number = 2;
    while candidate.exists() {
        candidate = paths.servers.join(format!("{clean} ({number})"));
        number += 1;
    }
    Ok(candidate)
}

/// JSON de la versión de Minecraft (el instalado, o el de Mojang si no está).
async fn vanilla_json(minecraft: &str, paths: &Paths) -> Result<Value, String> {
    if let Some(json) = std::fs::read_to_string(paths.version_json(minecraft)).ok().and_then(|t| serde_json::from_str(&t).ok()) {
        return Ok(json);
    }
    let release = mojang::list().await?.into_iter().find(|r| r.id == minecraft).ok_or(t!("Mojang no tiene la versión {minecraft}.", "Mojang does not have version {minecraft}."))?;
    serde_json::from_str(&download::get_text(&release.url).await.map_err(|_| mojang::unavailable())?).map_err(|e| e.to_string())
}

/// Java del servidor: el que eligió la instancia o el que pide la versión de Minecraft.
fn java_major(instance: &Instance, version: &Value) -> u32 {
    instance.java_major.unwrap_or_else(|| version.pointer("/javaVersion/majorVersion").and_then(Value::as_u64).unwrap_or(8) as u32)
}

pub fn java_executable(major: u32, paths: &Paths) -> Option<PathBuf> {
    let runtimes = java::discover(paths);
    let runtime = runtimes.iter().filter(|r| r.major == major).max_by(|a, b| a.version.cmp(&b.version)).or_else(|| java::select(&runtimes, None, major))?;
    Some(runtime.home.join("bin").join("java.exe"))
}

pub async fn create(
    new: NewServer,
    instance: Instance,
    paths: Paths,
    host: String,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<ServerConfig, String> {
    let name = new.name.trim().to_string();
    let folder = folder_for(if name.is_empty() { &instance.name } else { &name }, &paths)?;
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let result = build(&new, &instance, &paths, &host, &folder, progress).await;
    if result.is_err() {
        // Lo que se alcanzó a crear va a la papelera (no queda un servidor a medias).
        let _ = trash::delete(&folder);
    }
    result
}

async fn build(
    new: &NewServer,
    instance: &Instance,
    paths: &Paths,
    host: &str,
    folder: &Path,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<ServerConfig, String> {
    progress(t!("Preparando el servidor…", "Preparing the server…"), 0.0);
    let version = vanilla_json(&instance.minecraft_version, paths).await?;
    let major = java_major(instance, &version);
    {
        let progress = progress.clone();
        mojang::ensure_java(major, paths, move |stage, fraction| progress(stage, 0.02 + 0.1 * fraction)).await?;
    }
    let java = java_executable(major, paths).ok_or(t!("Falta Java {major}.", "Java {major} is missing."))?;
    let mut config = ServerConfig {
        id: folder.file_name().unwrap().to_string_lossy().to_string(),
        name: if new.name.trim().is_empty() { instance.name.clone() } else { new.name.trim().to_string() },
        key: random_key(),
        instance: instance.id.clone(),
        instance_name: instance.name.clone(),
        modpack: instance.modpack.clone(),
        modpack_project: instance.modpack_project.clone(),
        minecraft: instance.minecraft_version.clone(),
        loader: instance.loader.clone(),
        loader_version: instance.loader_version.clone(),
        ram_gb: new.ram_gb.clamp(1, 64),
        jvm_args: new.jvm_args.iter().map(|a| a.trim().to_string()).filter(|a| !a.is_empty()).collect(),
        java_major: major,
        eula: new.eula,
        port: free_port(25565),
        created: chrono::Utc::now().timestamp_millis(),
        host_name: valid_name(host).then(|| host.to_string()),
        verify_players: true,
        folder: folder.to_path_buf(),
        ..Default::default()
    };
    config.launch = install_software(&config, &version, &java, paths, progress.clone()).await?;

    // Mods: los que sirven en un servidor.
    progress(t!("Revisando qué mods van en el servidor…", "Checking which mods go on the server…"), 0.75);
    config.left_out = client_only(instance).await;
    {
        let (instance, folder, left) = (instance.clone(), folder.to_path_buf(), config.left_out.clone());
        progress(t!("Copiando mods y configuración…", "Copying mods and configuration…"), 0.8);
        tauri::async_runtime::spawn_blocking(move || copy_content(&instance, &folder, &left)).await.map_err(|e| e.to_string())??;
    }
    if let Some(world) = new.world.as_deref().filter(|w| !w.is_empty()) {
        progress(t!("Copiando el mundo «{world}»…", "Copying the world «{world}»…"), 0.9);
        let (instance, folder, world, host) = (instance.clone(), folder.to_path_buf(), world.to_string(), host.to_string());
        tauri::async_runtime::spawn_blocking(move || copy_world(&instance, &world, &folder, &host)).await.map_err(|e| e.to_string())??;
    }

    // Configuración del servidor.
    let small_pc = proceso::system_memory().0 <= 9 * 1024 * 1024 * 1024;
    set_properties(
        folder,
        &[
            ("motd", config.name.clone()),
            ("server-port", config.port.to_string()),
            ("online-mode", "false".into()),
            ("white-list", "true".into()),
            ("enforce-whitelist", "true".into()),
            ("enforce-secure-profile", "false".into()),
            ("max-players", new.max_players.clamp(1, 100).to_string()),
            ("gamemode", stored(&new.gamemode, &GAMEMODES, &config.minecraft)),
            ("difficulty", stored(if new.difficulty.is_empty() { "normal" } else { &new.difficulty }, &DIFFICULTIES, &config.minecraft)),
            ("pvp", new.pvp.to_string()),
            ("view-distance", if small_pc { "8" } else { "10" }.into()),
            ("simulation-distance", if small_pc { "6" } else { "8" }.into()),
            // Los modpacks con mochilas cohete, alas, etc. echarían al jugador por "volar".
            ("allow-flight", "true".into()),
            ("spawn-protection", "0".into()),
            ("level-name", "world".into()),
            ("level-seed", new.seed.trim().to_string()),
        ],
    )?;
    // Desde 1.21.11 el PvP es una regla del juego (en server.properties ya no cuenta).
    crate::opciones::initial_rules(&mut config, new.pvp);
    write_eula(folder, config.eula)?;
    write_access_lists(&config, host)?;
    save(&config)?;
    progress(t!("Servidor listo", "Server ready"), 1.0);
    Ok(config)
}

// ================= Software del servidor

/// Instala el servidor del loader y devuelve cómo arrancarlo.
async fn install_software(
    config: &ServerConfig,
    version: &Value,
    java: &Path,
    paths: &Paths,
    progress: impl Fn(String, f64) + Send + Sync + Clone + 'static,
) -> Result<Vec<String>, String> {
    let folder = &config.folder;
    let minecraft = &config.minecraft;
    let loader = Loader::parse(&config.loader);
    let loader_version = config.loader_version.clone().unwrap_or_default();
    let jar = |name: &str| vec!["-jar".to_string(), name.to_string(), "nogui".to_string()];
    // El servidor vanilla (Fabric y Quilt lo usan por dentro).
    let vanilla = || async {
        let server = version.pointer("/downloads/server").ok_or(t!("Minecraft {minecraft} no tiene servidor oficial.", "Minecraft {minecraft} has no official server."))?;
        let file = FileDownload::from_json(server, folder.join("server.jar")).ok_or(t!("Minecraft {minecraft} no tiene servidor oficial.", "Minecraft {minecraft} has no official server."))?;
        let (report, minecraft) = (progress.clone(), minecraft.clone());
        download::download_all(vec![file], move |_, _, bytes, total| {
            report(t!("Descargando el servidor de Minecraft {minecraft}…", "Downloading the Minecraft {minecraft} server…"), 0.15 + 0.3 * bytes as f64 / total.max(1) as f64)
        })
        .await
        .map_err(|_| mojang::unavailable())
    };
    match loader {
        None => {
            vanilla().await?;
            Ok(jar("server.jar"))
        }
        Some(Loader::Fabric) => {
            vanilla().await?;
            progress(t!("Descargando Fabric para servidor…", "Downloading Fabric for servers…"), 0.5);
            let installers: Value = serde_json::from_str(&download::get_text("https://meta.fabricmc.net/v2/versions/installer").await.map_err(|_| fabric_unavailable())?).map_err(|e| e.to_string())?;
            let installer = installers.as_array().into_iter().flatten().find(|i| i["stable"].as_bool() == Some(true)).or_else(|| installers.get(0)).and_then(|i| i["version"].as_str()).ok_or_else(fabric_unavailable)?;
            let url = format!("https://meta.fabricmc.net/v2/versions/loader/{minecraft}/{loader_version}/{installer}/server/jar");
            download::download_all(vec![FileDownload { url, path: folder.join("fabric-server-launch.jar"), sha1: None, size: None }], |_, _, _, _| {})
                .await
                .map_err(|_| fabric_unavailable())?;
            Ok(jar("fabric-server-launch.jar"))
        }
        Some(Loader::Quilt) => {
            progress(t!("Descargando Quilt para servidor…", "Downloading Quilt for servers…"), 0.2);
            let base = "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer";
            let metadata = download::get_text(&format!("{base}/maven-metadata.xml")).await.map_err(|_| quilt_unavailable())?;
            let latest = metadata.split("<release>").nth(1).and_then(|r| r.split("</release>").next()).ok_or_else(quilt_unavailable)?.trim().to_string();
            let installer = paths.data.join("descargas").join(format!("quilt-installer-{latest}.jar"));
            download::download_all(
                vec![FileDownload { url: format!("{base}/{latest}/quilt-installer-{latest}.jar"), path: installer.clone(), sha1: None, size: None }],
                |_, _, _, _| {},
            )
            .await
            .map_err(|_| quilt_unavailable())?;
            let args = vec![
                "-jar".to_string(),
                installer.display().to_string(),
                "install".into(),
                "server".into(),
                minecraft.clone(),
                loader_version.clone(),
                "--download-server".into(),
                // Relativa a la carpeta del servidor: el instalador de Quilt parte las rutas con espacios.
                "--install-dir=.".into(),
            ];
            run_installer(java, &args, folder, "Quilt", progress.clone()).await?;
            let _ = std::fs::remove_file(&installer);
            if !folder.join("quilt-server-launch.jar").exists() {
                return Err(t!("El instalador de Quilt no dejó el servidor listo.", "The Quilt installer did not leave the server ready."));
            }
            Ok(jar("quilt-server-launch.jar"))
        }
        Some(loader @ (Loader::Forge | Loader::NeoForge)) => {
            let name = loader.name();
            progress(t!("Descargando el instalador de {name}…", "Downloading the {name} installer…"), 0.15);
            let url = forge::installer_url(loader, minecraft, &loader_version);
            let sha1 = download::get_text(&format!("{url}.sha1")).await.ok().map(|t| t.trim().to_string()).filter(|s| s.len() == 40);
            let installer = folder.join(format!("{}-installer.jar", name.to_lowercase()));
            download::download_all(vec![FileDownload { url, path: installer.clone(), sha1, size: None }], |_, _, _, _| {})
                .await
                .map_err(|_| t!("No se pudo descargar el instalador de {name}.", "Could not download the {name} installer."))?;
            let args = vec!["-jar".to_string(), installer.display().to_string(), "--installServer".into(), folder.display().to_string()];
            run_installer(java, &args, folder, name, progress.clone()).await?;
            for leftover in [installer.clone(), folder.join(format!("{}.log", installer.file_name().unwrap().to_string_lossy())), folder.join("installer.log")] {
                let _ = std::fs::remove_file(leftover);
            }
            forge_launch(folder, &loader_version).ok_or(t!("El instalador de {name} terminó, pero no dejó cómo abrir el servidor.", "The {name} installer finished but left no way to start the server."))
        }
    }
}

fn fabric_unavailable() -> String {
    t!("No se puede conectar con los servidores de Fabric.", "Cannot connect to the Fabric servers.")
}

fn quilt_unavailable() -> String {
    t!("No se puede conectar con los servidores de Quilt.", "Cannot connect to the Quilt servers.")
}

/// Ejecuta un instalador de Java (oculto) mostrando lo que va haciendo.
async fn run_installer(java: &Path, args: &[String], folder: &Path, name: &str, progress: impl Fn(String, f64) + Send + Sync + 'static) -> Result<(), String> {
    let mut child = Command::new(java)
        .args(args)
        .current_dir(folder)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(proceso::CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| t!("No se pudo abrir el instalador de {name}: {e}", "Could not open the {name} installer: {e}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let name = name.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let errors = std::thread::spawn(move || stderr.map(|s| std::io::BufReader::new(s).lines().map_while(Result::ok).collect::<Vec<_>>()).unwrap_or_default());
        let mut last = Vec::new();
        let mut count = 0usize;
        if let Some(stdout) = stdout {
            for line in std::io::BufReader::new(stdout).lines().map_while(Result::ok) {
                count += 1;
                let shown: String = line.trim().chars().take(90).collect();
                if !shown.is_empty() {
                    progress(t!("Instalando {name} para servidor… {shown}", "Installing {name} for servers… {shown}"), (0.45 + count as f64 / 4000.0).min(0.74));
                }
                last.push(line);
                if last.len() > 15 {
                    last.remove(0);
                }
            }
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        let errors = errors.join().unwrap_or_default();
        if status.success() {
            return Ok(());
        }
        let detail: Vec<String> = last.into_iter().chain(errors).filter(|l| !l.trim().is_empty()).collect();
        let detail = detail[detail.len().saturating_sub(4)..].join(" · ");
        Err(t!("El instalador de {name} falló: {detail}", "The {name} installer failed: {detail}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Cómo arrancar un servidor de Forge o NeoForge recién instalado.
fn forge_launch(folder: &Path, loader_version: &str) -> Option<Vec<String>> {
    // 1.17 en adelante: un archivo con los argumentos (lo que usa run.bat).
    for base in ["libraries/net/minecraftforge/forge", "libraries/net/neoforged/neoforge", "libraries/net/neoforged/forge"] {
        let Ok(entries) = std::fs::read_dir(folder.join(base)) else { continue };
        let mut candidates: Vec<String> = entries.flatten().filter(|e| e.path().join("win_args.txt").exists()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
        candidates.sort_by_key(|c| !c.contains(loader_version));
        if let Some(version) = candidates.first() {
            return Some(vec![format!("@{base}/{version}/win_args.txt"), "nogui".into()]);
        }
    }
    // Hasta 1.16: forge-<versión>.jar en la carpeta.
    let jar = std::fs::read_dir(folder).ok()?.flatten().map(|e| e.file_name().to_string_lossy().to_string()).find(|n| {
        let lower = n.to_lowercase();
        lower.starts_with("forge-") && lower.ends_with(".jar") && !lower.contains("installer")
    })?;
    Some(vec!["-jar".into(), jar, "nogui".into()])
}

// ================= Mods y archivos

fn is_mod_file(name: &str) -> bool {
    name.to_lowercase().ends_with(".jar")
}

/// Mods de la instancia que no deben ir al servidor: los que Modrinth marca como solo de cliente y
/// los que Parabellum sabe que lo son.
pub async fn client_only(instance: &Instance) -> Vec<LeftOut> {
    let files: Vec<modrinth::InstalledFile> = modrinth::identify(instance).await.into_iter().filter(|f| f.kind == "mods" && is_mod_file(&f.file)).collect();
    let ids: Vec<String> = files.iter().filter_map(|f| f.project_id.clone()).collect();
    let sides = modrinth::server_sides(&ids).await;
    let mut left = Vec::new();
    for file in &files {
        if let Some(id) = tunnel_mod(&file.file, &file.mods) {
            left.push(LeftOut::new(&file.file, "tunnel", Some(&id)));
        } else if file.project_id.as_ref().and_then(|id| sides.get(id)).is_some_and(|side| side == "unsupported") {
            left.push(LeftOut::new(&file.file, "modrinth", None));
        } else if let Some(id) = file.mods.first().map(|m| m.id.as_str()).filter(|id| CLIENT_ONLY.contains(id)) {
            left.push(LeftOut::new(&file.file, "client", Some(id)));
        }
    }
    let mods = diagnostico::instance_mods(instance);
    // Sin conexión, identify no trae los mods leídos: se usan los del doctor.
    if files.is_empty() {
        for (file, metas) in &mods {
            if let Some(id) = tunnel_mod(file, metas) {
                left.push(LeftOut::new(file, "tunnel", Some(&id)));
                continue;
            }
            // Solo el mod principal: llevar dentro uno de cliente (p. ej. Mod Menu) no lo hace de cliente.
            if let Some(id) = metas.first().map(|m| m.id.as_str()).filter(|id| CLIENT_ONLY.contains(id)) {
                left.push(LeftOut::new(file, "client", Some(id)));
            }
        }
    }
    leave_out_dependents(&mods, &mut left);
    left
}

/// Un mod que necesita otro que se quedó fuera (p. ej. un complemento de Oculus) también se queda
/// fuera: si no, el servidor no arrancaría por la dependencia que falta.
fn leave_out_dependents(mods: &[(String, Vec<crate::compat::ModMeta>)], left: &mut Vec<LeftOut>) {
    let provided = |metas: &[crate::compat::ModMeta]| -> Vec<String> { metas.iter().flat_map(|m| std::iter::once(m.id.clone()).chain(m.provides.iter().cloned())).collect() };
    loop {
        let is_left = |file: &str| left.iter().any(|l| l.file.eq_ignore_ascii_case(file));
        let kept: std::collections::HashSet<String> = mods.iter().filter(|(f, _)| !is_left(f)).flat_map(|(_, m)| provided(m)).collect();
        let gone: std::collections::HashSet<String> = mods.iter().filter(|(f, _)| is_left(f)).flat_map(|(_, m)| provided(m)).filter(|id| !kept.contains(id)).collect();
        let more: Vec<LeftOut> = mods
            .iter()
            .filter(|(f, _)| !is_left(f))
            .filter_map(|(file, metas)| {
                let needed = metas.first()?.depends.iter().map(|(id, _)| id).find(|id| gone.contains(*id))?;
                Some(LeftOut::new(file, "dependency", Some(needed)))
            })
            .collect();
        if more.is_empty() {
            return;
        }
        left.extend(more);
    }
}

/// Copia (o actualiza) los mods y la configuración de la instancia en el servidor.
pub fn copy_content(instance: &Instance, folder: &Path, left_out: &[LeftOut]) -> Result<usize, String> {
    let error = |e: std::io::Error| t!("No se pudieron copiar los archivos de la instancia: {e}", "Could not copy the instance files: {e}");
    let source = instance.game_dir.join("mods");
    let target = folder.join("mods");
    std::fs::create_dir_all(&target).map_err(error)?;
    let skip = |name: &str| left_out.iter().any(|l| l.file.eq_ignore_ascii_case(name));
    let mut copied = 0;
    let mut wanted = std::collections::HashSet::new();
    for entry in std::fs::read_dir(&source).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_mod_file(&name) || skip(&name) || !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        wanted.insert(name.to_lowercase());
        let destination = target.join(&name);
        let same = destination.metadata().ok().zip(entry.metadata().ok()).is_some_and(|(a, b)| a.len() == b.len());
        if !same {
            std::fs::copy(entry.path(), &destination).map_err(error)?;
        }
        copied += 1;
    }
    // Los .jar que ya no están en la instancia (actualizados o quitados) salen del servidor.
    for entry in std::fs::read_dir(&target).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_mod_file(&name) && !wanted.contains(&name.to_lowercase()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    for folder_name in CONTENT_FOLDERS {
        let from = instance.game_dir.join(folder_name);
        if from.is_dir() {
            instances::copy_dir(&from, &folder.join(folder_name)).map_err(error)?;
        }
    }
    Ok(copied)
}

/// Copia un mundo de la instancia como el mundo del servidor. El jugador del mundo individual (su
/// inventario y posición, que van en level.dat) pasa a ser el del anfitrión en el servidor.
pub fn copy_world(instance: &Instance, world: &str, folder: &Path, host: &str) -> Result<(), String> {
    if world.contains(['/', '\\']) || world.contains("..") {
        return Err(t!("Ese mundo no existe.", "That world does not exist."));
    }
    let source = instance.game_dir.join("saves").join(world);
    if !source.join("level.dat").exists() {
        return Err(t!("Ese mundo no existe.", "That world does not exist."));
    }
    let target = folder.join("world");
    if target.exists() {
        trash::delete(&target).map_err(|e| t!("No se pudo mandar el mundo anterior a la papelera: {e}", "Could not move the previous world to the Recycle Bin: {e}"))?;
    }
    copy_without_lock(&source, &target).map_err(|e| t!("No se pudo copiar el mundo: {e}", "Could not copy the world: {e}"))?;
    if valid_name(host) {
        let player = std::fs::read(target.join("level.dat")).ok().and_then(|bytes| nbt::read_gzip(&bytes)).and_then(|root| root.get("Data")?.get("Player").cloned());
        if let Some(player) = player {
            let dir = target.join("playerdata");
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::write(dir.join(format!("{}.dat", dashed(&settings::offline_uuid(host)))), nbt::write_gzip(&player)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn copy_without_lock(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name == "session.lock" {
            continue;
        }
        if entry.file_type()?.is_dir() {
            copy_without_lock(&entry.path(), &to.join(&name))?;
        } else {
            std::fs::copy(entry.path(), to.join(&name))?;
        }
    }
    Ok(())
}

/// Quita del servidor los mods de túnel que tenga (de servidores creados antes de excluirlos).
/// Devuelve los archivos quitados.
pub fn remove_tunnel_mods(config: &mut ServerConfig) -> Vec<String> {
    let mods = config.folder.join("mods");
    let mut removed = Vec::new();
    for entry in std::fs::read_dir(&mods).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_mod_file(&name) {
            continue;
        }
        let metas = std::fs::File::open(entry.path()).map(crate::compat::read_jar).unwrap_or_default();
        let Some(id) = tunnel_mod(&name, &metas) else { continue };
        if std::fs::rename(entry.path(), mods.join(format!("{name}.disabled"))).is_ok() {
            if !config.left_out.iter().any(|l| l.file.eq_ignore_ascii_case(&name)) {
                config.left_out.push(LeftOut::new(&name, "tunnel", Some(&id)));
            }
            removed.push(name);
        }
    }
    removed
}

/// Instancias con un mod de túnel activado (e4mc): con playit.gg ya no hace falta.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TunnelModInstance {
    pub id: String,
    pub name: String,
    pub file: String,
}

pub fn instances_with_tunnel_mods(paths: &Paths) -> Vec<TunnelModInstance> {
    let mut found = Vec::new();
    for instance in instances::list(paths) {
        for entry in std::fs::read_dir(instance.game_dir.join("mods")).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if is_mod_file(&name) && (lower.starts_with("e4mc") || lower.starts_with("world-host") || lower.starts_with("worldhost")) {
                found.push(TunnelModInstance { id: instance.id.clone(), name: instance.name.clone(), file: name });
            }
        }
    }
    found
}

pub fn mod_count(folder: &Path) -> usize {
    std::fs::read_dir(folder.join("mods")).into_iter().flatten().flatten().filter(|e| is_mod_file(&e.file_name().to_string_lossy())).count()
}

// ================= Arrancar

pub fn command(config: &ServerConfig, paths: &Paths) -> Result<Command, String> {
    let java = java_executable(config.java_major, paths).ok_or(t!("Falta Java {}.", "Java {} is missing.", config.java_major))?;
    if config.launch.is_empty() {
        return Err(t!("El servidor no está instalado del todo. Bórralo y créalo otra vez.", "The server is not fully installed. Delete it and create it again."));
    }
    let mut command = Command::new(java);
    command
        .current_dir(&config.folder)
        .arg(format!("-Xms{}M", (config.ram_gb * 1024).min(1024)))
        .arg(format!("-Xmx{}G", config.ram_gb))
        .args(SERVER_FLAGS)
        .args(&config.jvm_args)
        .args(&config.launch)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(proceso::CREATE_NO_WINDOW);
    Ok(command)
}

/// Lo que importa de una línea de la consola.
#[derive(Debug, PartialEq, Eq)]
pub enum LineEvent {
    Ready,
    Joined(String),
    Left(String),
    BindFailed,
    NeedsEula,
}

fn line_patterns() -> &'static (Regex, Regex) {
    static PATTERNS: OnceLock<(Regex, Regex)> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        (Regex::new(r"\]: ([A-Za-z0-9_]{1,16}) joined the game\s*$").unwrap(), Regex::new(r"\]: ([A-Za-z0-9_]{1,16}) left the game\s*$").unwrap())
    })
}

pub fn parse_line(line: &str) -> Option<LineEvent> {
    let (joined, left) = line_patterns();
    if line.contains("]: Done (") && line.contains("For help") {
        Some(LineEvent::Ready)
    } else if let Some(c) = joined.captures(line) {
        Some(LineEvent::Joined(c[1].to_string()))
    } else if let Some(c) = left.captures(line) {
        Some(LineEvent::Left(c[1].to_string()))
    } else if line.contains("FAILED TO BIND TO PORT") {
        Some(LineEvent::BindFailed)
    } else if line.contains("You need to agree to the EULA") {
        Some(LineEvent::NeedsEula)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propiedades_con_tildes_y_sin_perder_lo_demas() {
        let folder = std::env::temp_dir().join(format!("parabellum-servidor-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("server.properties"), "#comentario\nmotd=A Minecraft Server\nmax-players=20\n").unwrap();
        set_properties(&folder, &[("motd", "Mundo de Juan · ñandú".into()), ("online-mode", "false".into())]).unwrap();
        let text = std::fs::read_to_string(folder.join("server.properties")).unwrap();
        assert!(text.starts_with("#comentario\nmotd=Mundo de Juan \\u00b7 \\u00f1and\\u00fa\nmax-players=20\nonline-mode=false"));
        assert_eq!(property(&folder, "motd").as_deref(), Some("Mundo de Juan · ñandú"));
        assert_eq!(property(&folder, "max-players").as_deref(), Some("20"));
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn lineas_de_la_consola() {
        assert_eq!(parse_line("[12:00:01] [Server thread/INFO]: Done (5.123s)! For help, type \"help\""), Some(LineEvent::Ready));
        assert_eq!(parse_line("[12:00:09] [Server thread/INFO] [minecraft/MinecraftServer]: Hawntd joined the game"), Some(LineEvent::Joined("Hawntd".into())));
        assert_eq!(parse_line("[12:01:00] [Server thread/INFO]: Hawntd left the game"), Some(LineEvent::Left("Hawntd".into())));
        // Un mensaje de chat no cuenta como entrar.
        assert_eq!(parse_line("[12:01:00] [Server thread/INFO]: <Bob> Alice joined the game"), None);
        assert_eq!(parse_line("[12:00:00] [Server thread/WARN]: **** FAILED TO BIND TO PORT!"), Some(LineEvent::BindFailed));
    }

    #[test]
    fn complementos_de_mods_de_cliente_tambien_fuera() {
        use crate::compat::ModMeta;
        let meta = |id: &str, depends: &[&str]| ModMeta { id: id.into(), depends: depends.iter().map(|d| (d.to_string(), Vec::new())).collect(), ..Default::default() };
        let mods = vec![
            ("oculus.jar".to_string(), vec![meta("oculus", &[])]),
            ("oculus-flywheel-compat.jar".to_string(), vec![meta("irisflw", &["oculus", "flywheel"])]),
            ("flywheel.jar".to_string(), vec![meta("flywheel", &[])]),
            ("create.jar".to_string(), vec![meta("create", &["flywheel"])]),
        ];
        let mut left = vec![LeftOut::new("oculus.jar", "modrinth", None)];
        leave_out_dependents(&mods, &mut left);
        let files: Vec<&str> = left.iter().map(|l| l.file.as_str()).collect();
        assert_eq!(files, vec!["oculus.jar", "oculus-flywheel-compat.jar"]);
    }

    #[test]
    fn modo_y_dificultad_en_versiones_antiguas() {
        assert_eq!(stored("creative", &GAMEMODES, "1.12.2"), "1");
        assert_eq!(stored("creative", &GAMEMODES, "1.20.1"), "creative");
        assert_eq!(named("2", &DIFFICULTIES, "normal"), "normal");
        assert_eq!(named("hard", &DIFFICULTIES, "normal"), "hard");
        assert_eq!(named("", &GAMEMODES, "survival"), "survival");
    }

    #[test]
    fn e4mc_no_va_en_el_servidor() {
        use crate::compat::ModMeta;
        let e4mc = vec![ModMeta { id: "e4mc".into(), ..Default::default() }];
        assert_eq!(tunnel_mod("e4mc-fabric-6.2.1-modern.jar", &e4mc).as_deref(), Some("e4mc"));
        assert_eq!(tunnel_mod("e4mc-forge-5.0.jar", &[]).as_deref(), Some("e4mc"));
        assert!(tunnel_mod("lithium-fabric-0.25.3.jar", &[ModMeta { id: "lithium".into(), ..Default::default() }]).is_none());
        let folder = std::env::temp_dir().join(format!("parabellum-tunel-{}", std::process::id()));
        std::fs::create_dir_all(folder.join("mods")).unwrap();
        std::fs::write(folder.join("mods").join("e4mc-fabric-6.2.1-modern.jar"), b"x").unwrap();
        std::fs::write(folder.join("mods").join("lithium.jar"), b"x").unwrap();
        let mut config = ServerConfig { folder: folder.clone(), ..Default::default() };
        assert_eq!(remove_tunnel_mods(&mut config), vec!["e4mc-fabric-6.2.1-modern.jar".to_string()]);
        assert!(folder.join("mods").join("e4mc-fabric-6.2.1-modern.jar.disabled").exists() && folder.join("mods").join("lithium.jar").exists());
        assert_eq!(config.left_out[0].kind, "tunnel");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn los_nombres_siguen_a_la_cuenta() {
        let folder = std::env::temp_dir().join(format!("parabellum-nombres-{}", std::process::id()));
        let world = folder.join("world");
        for dir in ["playerdata", "advancements", "stats"] {
            std::fs::create_dir_all(world.join(dir)).unwrap();
        }
        let old = dashed(&settings::offline_uuid("Pepito28"));
        std::fs::write(world.join("playerdata").join(format!("{old}.dat")), b"inventario").unwrap();
        std::fs::write(world.join("playerdata").join(format!("{old}.dat_old")), b"respaldo").unwrap();
        std::fs::write(world.join("stats").join(format!("{old}.json")), b"{}").unwrap();
        // Servidor de antes de la 0.15: el anfitrión se reconoce por el administrador que puso Parabellum.
        std::fs::write(
            folder.join("ops.json"),
            serde_json::to_vec(&json!([
                { "uuid": old, "name": "Pepito28", "level": 4, "bypassesPlayerLimit": true },
                { "uuid": "x", "name": "Amigo", "level": 4, "bypassesPlayerLimit": false }
            ]))
            .unwrap(),
        )
        .unwrap();
        let mut config = ServerConfig {
            folder: folder.clone(),
            guests: vec![Guest { name: "Hawntd".into(), uid: Some("u1".into()) }, Guest { name: "Manual".into(), uid: None }],
            ..Default::default()
        };
        let friends = std::collections::HashMap::from([("u1".to_string(), "Hawntd_mx".to_string())]);
        let renames = follow_names(&mut config, "Juanito28", Some(&friends));
        assert_eq!(
            renames,
            vec![
                Rename { old: "Pepito28".into(), new: "Juanito28".into(), host: true },
                Rename { old: "Hawntd".into(), new: "Hawntd_mx".into(), host: false }
            ]
        );
        assert_eq!(config.host_name.as_deref(), Some("Juanito28"));
        assert_eq!(config.guests[0].name, "Hawntd_mx");
        assert_eq!(move_player_data(&folder, "Pepito28", "Juanito28"), 3);
        let new = dashed(&settings::offline_uuid("Juanito28"));
        assert_eq!(std::fs::read(world.join("playerdata").join(format!("{new}.dat"))).unwrap(), b"inventario");
        // Si el nombre nuevo ya tiene cosas, no se pisa nada.
        std::fs::write(world.join("playerdata").join(format!("{old}.dat")), b"otro").unwrap();
        assert_eq!(move_player_data(&folder, "Pepito28", "Juanito28"), 0);
        write_access_lists(&config, "Juanito28").unwrap();
        let ops: Vec<Value> = serde_json::from_slice(&std::fs::read(folder.join("ops.json")).unwrap()).unwrap();
        let names: Vec<&str> = ops.iter().filter_map(|o| o["name"].as_str()).collect();
        assert_eq!(names, vec!["Amigo", "Juanito28"]);
        // El servidor encendido guarda el /op del nombre nuevo a su manera; al volver al nombre
        // anterior, igual deja de ser administrador.
        std::fs::write(folder.join("ops.json"), serde_json::to_vec(&json!([{ "uuid": "y", "name": "Juanito28", "level": 4, "bypassesPlayerLimit": false }])).unwrap()).unwrap();
        assert_eq!(follow_names(&mut config, "Pepito28", None).len(), 1);
        write_access_lists(&config, "Pepito28").unwrap();
        let ops: Vec<Value> = serde_json::from_slice(&std::fs::read(folder.join("ops.json")).unwrap()).unwrap();
        assert_eq!(ops.iter().filter_map(|o| o["name"].as_str()).collect::<Vec<_>>(), vec!["Pepito28"]);
        assert_eq!(config.former_host_names, vec!["Juanito28".to_string()]);
        follow_names(&mut config, "Juanito28", None);
        write_access_lists(&config, "Juanito28").unwrap();
        let whitelist: Vec<Value> = serde_json::from_slice(&std::fs::read(folder.join("whitelist.json")).unwrap()).unwrap();
        assert_eq!(whitelist.iter().filter_map(|o| o["name"].as_str()).collect::<Vec<_>>(), vec!["Juanito28", "Hawntd_mx", "Manual"]);
        // Sin cambios, nada que hacer.
        assert!(follow_names(&mut config, "Juanito28", Some(&friends)).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn uuid_con_guiones() {
        assert_eq!(dashed("1525c21ee760484ca393e32ccb3611d2"), "1525c21e-e760-484c-a393-e32ccb3611d2");
        assert!(valid_name("Juan_07") && !valid_name("Juan Rojas") && !valid_name(""));
    }

    #[test]
    fn arranque_de_forge() {
        let folder = std::env::temp_dir().join(format!("parabellum-forge-{}", std::process::id()));
        let args = folder.join("libraries/net/minecraftforge/forge/1.20.1-47.4.0");
        std::fs::create_dir_all(&args).unwrap();
        std::fs::write(args.join("win_args.txt"), "").unwrap();
        assert_eq!(forge_launch(&folder, "47.4.0").unwrap(), vec!["@libraries/net/minecraftforge/forge/1.20.1-47.4.0/win_args.txt".to_string(), "nogui".into()]);
        let _ = std::fs::remove_dir_all(&folder);
        let old = std::env::temp_dir().join(format!("parabellum-forge-viejo-{}", std::process::id()));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::write(old.join("forge-1.12.2-14.23.5.2860.jar"), "").unwrap();
        std::fs::write(old.join("forge-1.12.2-14.23.5.2860-installer.jar"), "").unwrap();
        assert_eq!(forge_launch(&old, "14.23.5.2860").unwrap()[1], "forge-1.12.2-14.23.5.2860.jar");
        let _ = std::fs::remove_dir_all(&old);
    }
}
