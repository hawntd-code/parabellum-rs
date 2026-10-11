//! Opciones de un servidor, como en Aternos: cada ajuste de server.properties y las reglas del juego
//! (gamerules) con un nombre claro, su control y debajo el nombre real («max-players=20»,
//! «gamerule keep_inventory true»), para no depender de comandos. Se guardan una por una.
//!
//! Las reglas del juego viven en el mundo: hasta 1.21.10 en level.dat › Data › GameRules (texto, con
//! nombres como keepInventory); desde 26.1 en data/minecraft/game_rules.dat (minecraft:keep_inventory,
//! con 0/1 y números). Desde 1.21.11 PvP, el Nether y los monstruos dejaron server.properties y son
//! reglas. Con el servidor encendido una regla se cambia al momento con /gamerule; apagado se escribe
//! en el mundo; si todavía no hay mundo queda pendiente y se aplica en cuanto el servidor está listo.

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::loaders::compare_versions;
use crate::nbt::{self, Tag};
use crate::servidor::{self, ServerConfig, DIFFICULTIES, GAMEMODES};

#[derive(Clone, Copy)]
enum Kind {
    Bool,
    Number(i64, i64),
    Choice(&'static [&'static str]),
    Text(usize),
}

#[derive(Clone, Copy)]
enum Source {
    /// Una línea de server.properties.
    Property(&'static str),
    /// Una regla del juego: nombre hasta 1.21.10 (si existía) y desde 1.21.11.
    /// `invert_old`: la vieja decía lo contrario (disableRaids). `radius`: la nueva es un radio
    /// (el fuego: 0 no se propaga, 128 lo normal) y la vieja un sí/no (doFireTick).
    Rule { old: Option<&'static str>, new: &'static str, invert_old: bool, radius: bool },
    /// server.properties hasta 1.21.10 y regla del juego desde 1.21.11.
    Moved { property: &'static str, rule: &'static str },
}

struct Def {
    id: &'static str,
    group: &'static str,
    kind: Kind,
    source: Source,
    default: &'static str,
    /// Versiones en las que existe (solo cuenta mientras el servidor no haya escrito sus archivos).
    since: Option<&'static str>,
    until: Option<&'static str>,
}

impl Def {
    const fn since(mut self, version: &'static str) -> Self {
        self.since = Some(version);
        self
    }

    const fn until(mut self, version: &'static str) -> Self {
        self.until = Some(version);
        self
    }
}

const fn property(id: &'static str, group: &'static str, kind: Kind, default: &'static str) -> Def {
    Def { id, group, kind, source: Source::Property(id), default, since: None, until: None }
}

const fn rule(id: &'static str, old: &'static str, new: &'static str, kind: Kind, default: &'static str) -> Def {
    Def { id, group: "rules", kind, source: Source::Rule { old: Some(old), new, invert_old: false, radius: false }, default, since: None, until: None }
}

const fn moved(id: &'static str, rule: &'static str, default: &'static str) -> Def {
    Def { id, group: "game", kind: Kind::Bool, source: Source::Moved { property: id, rule }, default, since: None, until: None }
}

const BOOL: Kind = Kind::Bool;

const DEFS: &[Def] = &[
    // Jugadores
    property("max-players", "players", Kind::Number(1, 100), "20"),
    property("white-list", "players", BOOL, "true"),
    property("player-idle-timeout", "players", Kind::Number(0, 240), "0"),
    property("hide-online-players", "players", BOOL, "false").since("1.18"),
    property("spawn-protection", "players", Kind::Number(0, 64), "16"),
    property("allow-flight", "players", BOOL, "false"),
    // Partida
    property("gamemode", "game", Kind::Choice(&GAMEMODES), "survival"),
    property("force-gamemode", "game", BOOL, "false"),
    property("difficulty", "game", Kind::Choice(&DIFFICULTIES), "easy"),
    property("hardcore", "game", BOOL, "false"),
    moved("pvp", "pvp", "true"),
    moved("allow-nether", "allow_entering_nether_using_portals", "true"),
    moved("spawn-monsters", "spawn_monsters", "true"),
    property("spawn-animals", "game", BOOL, "true").until("1.21.2"),
    property("spawn-npcs", "game", BOOL, "true").until("1.21.2"),
    property("enable-command-block", "game", BOOL, "false"),
    // Reglas del juego
    rule("keep_inventory", "keepInventory", "keep_inventory", BOOL, "false"),
    rule("mob_griefing", "mobGriefing", "mob_griefing", BOOL, "true"),
    rule("advance_time", "doDaylightCycle", "advance_time", BOOL, "true"),
    rule("advance_weather", "doWeatherCycle", "advance_weather", BOOL, "true").since("1.11"),
    Def {
        id: "fire_spread",
        group: "rules",
        kind: BOOL,
        source: Source::Rule { old: Some("doFireTick"), new: "fire_spread_radius_around_player", invert_old: false, radius: true },
        default: "true",
        since: None,
        until: None,
    },
    rule("spawn_phantoms", "doInsomnia", "spawn_phantoms", BOOL, "true").since("1.15"),
    rule("players_sleeping_percentage", "playersSleepingPercentage", "players_sleeping_percentage", Kind::Number(0, 100), "100").since("1.17"),
    rule("natural_health_regeneration", "naturalRegeneration", "natural_health_regeneration", BOOL, "true"),
    rule("immediate_respawn", "doImmediateRespawn", "immediate_respawn", BOOL, "false").since("1.15"),
    rule("show_death_messages", "showDeathMessages", "show_death_messages", BOOL, "true"),
    rule("show_advancement_messages", "announceAdvancements", "show_advancement_messages", BOOL, "true").since("1.12"),
    rule("fall_damage", "fallDamage", "fall_damage", BOOL, "true").since("1.15"),
    rule("spawn_mobs", "doMobSpawning", "spawn_mobs", BOOL, "true"),
    rule("mob_drops", "doMobLoot", "mob_drops", BOOL, "true"),
    Def {
        id: "raids",
        group: "rules",
        kind: BOOL,
        source: Source::Rule { old: Some("disableRaids"), new: "raids", invert_old: true, radius: false },
        default: "true",
        since: Some("1.14.3"),
        until: None,
    },
    rule("reduced_debug_info", "reducedDebugInfo", "reduced_debug_info", BOOL, "false"),
    rule("locator_bar", "locatorBar", "locator_bar", BOOL, "true").since("1.21.6"),
    rule("tnt_explodes", "tntExplodes", "tnt_explodes", BOOL, "true").since("1.21.5"),
    rule("random_tick_speed", "randomTickSpeed", "random_tick_speed", Kind::Number(0, 100), "3"),
    // Mundo
    property("view-distance", "world", Kind::Number(3, 32), "10"),
    property("simulation-distance", "world", Kind::Number(3, 32), "10").since("1.18"),
    property("pause-when-empty-seconds", "world", Kind::Number(0, 3600), "60").since("1.21.2"),
    property("generate-structures", "world", BOOL, "true"),
    // Paquete de recursos
    property("resource-pack", "pack", Kind::Text(500), ""),
    property("require-resource-pack", "pack", BOOL, "false").since("1.17"),
    property("resource-pack-prompt", "pack", Kind::Text(200), "").since("1.17"),
];

/// Desde esta versión las reglas tienen los nombres nuevos y PvP/Nether/monstruos son reglas.
const NEW_RULES: &str = "1.21.11";
/// Las opciones propias de Parabellum (no son de Minecraft).
const OWN: [&str; 4] = ["verify", "ram", "jvm", "eula"];
const MOTD_MAX: usize = 150;

/// Nombre claro y ayuda de cada opción.
fn describe(id: &str) -> (String, String) {
    match id {
        "max-players" => (t!("Espacios", "Slots"), t!("Cuántos jugadores pueden estar conectados a la vez.", "How many players can be online at the same time.")),
        "white-list" => (
            t!("Lista blanca", "Whitelist"),
            t!("Solo entran tus invitados (pestaña Jugadores). Apagada, entra cualquiera que tenga la dirección.", "Only your guests can join (Players tab). Off, anyone with the address can join."),
        ),
        "verify" => (
            t!("Solo desde Parabellum", "Only from Parabellum"),
            t!(
                "Quien entra con el nombre de un invitado tiene que estar jugando desde Parabellum con esa cuenta: nadie puede usar el nombre de otro.",
                "Whoever joins with a guest's name must be playing from Parabellum with that account: nobody can use someone else's name."
            ),
        ),
        "player-idle-timeout" => (t!("Expulsar inactivos (minutos)", "Kick idle players (minutes)"), t!("Saca a quien no se mueve en ese tiempo. 0: nunca.", "Kicks whoever does not move for that long. 0: never.")),
        "hide-online-players" => (t!("Ocultar quién está conectado", "Hide online players"), t!("En la lista de servidores no se ve quién está jugando.", "The server list does not show who is playing.")),
        "spawn-protection" => (
            t!("Protección del spawn", "Spawn protection"),
            t!("Bloques alrededor del punto de aparición donde solo los administradores construyen. 0: sin protección.", "Blocks around the spawn point where only admins can build. 0: no protection."),
        ),
        "allow-flight" => (t!("Volar", "Flight"), t!("Que el servidor no eche a quien vuela. Hace falta con mods de alas, jetpacks o mochilas cohete.", "The server does not kick players who fly. Needed with wing, jetpack or rocket pack mods.")),
        "gamemode" => (t!("Modo de juego", "Game mode"), t!("Con el que entran los jugadores nuevos.", "The one new players join with.")),
        "force-gamemode" => (t!("Forzar modo de juego", "Force game mode"), t!("Todos entran siempre en el modo de juego del servidor.", "Everyone always joins in the server's game mode.")),
        "difficulty" => (t!("Dificultad", "Difficulty"), String::new()),
        "hardcore" => (t!("Extremo (hardcore)", "Hardcore"), t!("Una sola vida: al morir, el jugador se queda de espectador.", "One life only: after dying, the player becomes a spectator.")),
        "pvp" => (t!("PvP", "PvP"), t!("Los jugadores pueden lastimarse entre ellos.", "Players can hurt each other.")),
        "allow-nether" => (t!("Nether", "Nether"), t!("Los portales al Nether funcionan.", "Nether portals work.")),
        "spawn-monsters" => (t!("Monstruos", "Monsters"), t!("Aparecen zombis, esqueletos, creepers y demás.", "Zombies, skeletons, creepers and the rest spawn.")),
        "spawn-animals" => (t!("Animales", "Animals"), t!("Aparecen vacas, cerdos, ovejas…", "Cows, pigs, sheep… spawn.")),
        "spawn-npcs" => (t!("Aldeanos", "Villagers"), t!("Aparecen aldeanos en las aldeas.", "Villagers spawn in villages.")),
        "enable-command-block" => (t!("Bloques de comandos", "Command blocks"), t!("Los bloques de comandos funcionan.", "Command blocks work.")),
        "keep_inventory" => (t!("Conservar el inventario", "Keep inventory"), t!("Al morir no se pierde nada.", "Nothing is lost on death.")),
        "mob_griefing" => (
            t!("Los mobs rompen bloques", "Mobs break blocks"),
            t!("Creepers que destruyen, endermans que se llevan bloques, aldeanos que cosechan.", "Creepers that destroy, endermen that take blocks, villagers that harvest."),
        ),
        "advance_time" => (t!("Ciclo de día y noche", "Day and night cycle"), t!("Apagado, la hora se queda fija.", "Off, the time stays fixed.")),
        "advance_weather" => (t!("Cambios de clima", "Weather changes"), t!("Apagado, el clima se queda como está.", "Off, the weather stays as it is.")),
        "fire_spread" => (t!("El fuego se propaga", "Fire spreads"), t!("El fuego quema y se extiende a los bloques de alrededor.", "Fire burns and spreads to nearby blocks.")),
        "spawn_phantoms" => (t!("Phantoms", "Phantoms"), t!("Aparecen si alguien lleva varias noches sin dormir.", "They spawn if someone has not slept for several nights.")),
        "players_sleeping_percentage" => (
            t!("Jugadores que deben dormir (%)", "Players that must sleep (%)"),
            t!("Qué parte de los jugadores tiene que dormir para pasar la noche. Con 0 basta uno.", "Which share of the players must sleep to skip the night. With 0, one is enough."),
        ),
        "natural_health_regeneration" => (t!("Regenerar vida", "Health regeneration"), t!("La vida se recupera sola con la barra de comida llena.", "Health comes back by itself with a full hunger bar.")),
        "immediate_respawn" => (t!("Reaparecer al instante", "Instant respawn"), t!("Sin la pantalla de «Has muerto».", "Without the «You died» screen.")),
        "show_death_messages" => (t!("Mensajes de muerte", "Death messages"), t!("El chat avisa cuando alguien muere.", "The chat says when someone dies.")),
        "show_advancement_messages" => (t!("Anunciar logros", "Announce advancements"), t!("El chat avisa cuando alguien consigue un logro.", "The chat says when someone gets an advancement.")),
        "fall_damage" => (t!("Daño por caída", "Fall damage"), String::new()),
        "spawn_mobs" => (t!("Aparición de criaturas", "Mob spawning"), t!("Apagado, no aparece ninguna criatura por sí sola.", "Off, no creature spawns by itself.")),
        "mob_drops" => (t!("Las criaturas sueltan objetos", "Mobs drop items"), String::new()),
        "raids" => (t!("Asaltos", "Raids"), t!("Los saqueadores atacan la aldea cuando alguien llega con Mal presagio.", "Pillagers attack the village when someone arrives with Bad Omen.")),
        "reduced_debug_info" => (t!("Ocultar coordenadas (F3)", "Hide coordinates (F3)"), t!("La pantalla de F3 sale reducida, sin coordenadas.", "The F3 screen is reduced, without coordinates.")),
        "locator_bar" => (t!("Barra de localización", "Locator bar"), t!("Muestra hacia dónde están los demás jugadores.", "Shows where the other players are.")),
        "tnt_explodes" => (t!("La TNT explota", "TNT explodes"), String::new()),
        "random_tick_speed" => (t!("Velocidad de crecimiento", "Random tick speed"), t!("Qué tan rápido crecen plantas y cultivos. Normal: 3.", "How fast plants and crops grow. Normal: 3.")),
        "view-distance" => (t!("Distancia de visión", "View distance"), t!("Hasta dónde ven los jugadores, en chunks. Menos usa menos memoria.", "How far players see, in chunks. Less uses less memory.")),
        "simulation-distance" => (
            t!("Distancia de simulación", "Simulation distance"),
            t!("Hasta dónde se mueven las criaturas y crecen las plantas, en chunks.", "How far creatures move and plants grow, in chunks."),
        ),
        "pause-when-empty-seconds" => (
            t!("Pausar sin jugadores (segundos)", "Pause when empty (seconds)"),
            t!("El mundo se detiene cuando no hay nadie conectado. 0: nunca.", "The world stops when nobody is online. 0: never."),
        ),
        "generate-structures" => (t!("Estructuras", "Structures"), t!("Aldeas, templos y fortalezas en lo que se genere desde ahora.", "Villages, temples and strongholds in what is generated from now on.")),
        "resource-pack" => (t!("Paquete de recursos", "Resource pack"), t!("Enlace directo a un .zip que se descarga al entrar.", "Direct link to a .zip that is downloaded on joining.")),
        "require-resource-pack" => (t!("Paquete de recursos obligatorio", "Required resource pack"), t!("Quien no lo acepte no puede entrar.", "Whoever does not accept it cannot join.")),
        "resource-pack-prompt" => (t!("Mensaje del paquete de recursos", "Resource pack message"), t!("Lo que ven al pedirles que lo descarguen.", "What they see when asked to download it.")),
        "ram" => (t!("Memoria del servidor", "Server memory"), String::new()),
        "jvm" => (t!("Argumentos de Java", "Java arguments"), t!("Opcional, p. ej. -XX:+UseZGC", "Optional, e.g. -XX:+UseZGC")),
        "eula" => (t!("EULA de Minecraft", "Minecraft EULA"), t!("Hay que aceptarlo para encender el servidor (aka.ms/MinecraftEULA).", "It must be accepted to start the server (aka.ms/MinecraftEULA).")),
        _ => (id.to_string(), String::new()),
    }
}

fn group_title(group: &str) -> String {
    match group {
        "players" => t!("Jugadores", "Players"),
        "game" => t!("Partida", "Game"),
        "rules" => t!("Reglas del juego", "Game rules"),
        "world" => t!("Mundo", "World"),
        "pack" => t!("Paquete de recursos", "Resource pack"),
        _ => t!("Servidor", "Server"),
    }
}

fn choice_label(value: &str) -> String {
    match value {
        "survival" => t!("Supervivencia", "Survival"),
        "creative" => t!("Creativo", "Creative"),
        "adventure" => t!("Aventura", "Adventure"),
        "spectator" => t!("Espectador", "Spectator"),
        "peaceful" => t!("Pacífico", "Peaceful"),
        "easy" => t!("Fácil", "Easy"),
        "normal" => t!("Normal", "Normal"),
        "hard" => t!("Difícil", "Hard"),
        other => other.to_string(),
    }
}

// ================= Vista para la interfaz

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptionView {
    pub id: String,
    pub label: String,
    pub help: String,
    /// "bool", "number", "choice", "text" o "ram".
    pub kind: &'static str,
    pub value: Value,
    pub min: Option<i64>,
    pub max: Option<i64>,
    /// (valor, nombre) de las opciones de una lista.
    pub choices: Vec<(String, String)>,
    /// Cómo se ve de verdad: «max-players=20» o «gamerule keep_inventory true».
    pub source: String,
    /// Con el servidor encendido se aplica al momento (si no, al reiniciarlo).
    pub live: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct GroupView {
    pub id: &'static str,
    pub title: String,
    pub options: Vec<OptionView>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptionsView {
    pub name: String,
    /// El mensaje de la lista de servidores, con sus códigos de color (§a, §l…).
    pub motd: String,
    /// server-icon.png como data URL.
    pub icon: Option<String>,
    pub groups: Vec<GroupView>,
}

// ================= Reglas guardadas en el mundo

/// Dónde guarda las reglas el mundo del servidor.
enum RulesFile {
    /// data/minecraft/game_rules.dat (26.1+): raíz › data, nombres «minecraft:…», valores 0/1 y números.
    Split(PathBuf),
    /// level.dat: Data › GameRules, nombres camelCase (o los nuevos, según la versión), valores como texto.
    Level(PathBuf),
}

fn world_folder(config: &ServerConfig) -> PathBuf {
    let level = servidor::property(&config.folder, "level-name").filter(|l| !l.trim().is_empty()).unwrap_or_else(|| "world".into());
    config.folder.join(level)
}

fn rules_file(config: &ServerConfig) -> Option<RulesFile> {
    let world = world_folder(config);
    let split = world.join("data").join("minecraft").join("game_rules.dat");
    if split.is_file() {
        return Some(RulesFile::Split(split));
    }
    let level = world.join("level.dat");
    level.is_file().then_some(RulesFile::Level(level))
}

/// La parte del archivo con las reglas.
fn rules_compound<'a>(root: &'a mut Tag, file: &RulesFile) -> Option<&'a mut Vec<(Vec<u8>, Tag)>> {
    fn child<'a>(tag: &'a mut Tag, name: &str) -> Option<&'a mut Tag> {
        match tag {
            Tag::Compound(entries) => entries.iter_mut().find(|(key, _)| key == name.as_bytes()).map(|(_, tag)| tag),
            _ => None,
        }
    }
    let compound = match file {
        RulesFile::Split(_) => child(root, "data")?,
        RulesFile::Level(_) => {
            let data = child(root, "Data")?;
            // El nombre exacto cambió con las versiones; se acepta cualquiera de los dos.
            let name = if data.get("GameRules").is_some() { "GameRules" } else { "game_rules" };
            child(data, name)?
        }
    };
    match compound {
        Tag::Compound(entries) => Some(entries),
        _ => None,
    }
}

fn plain_name(key: &[u8]) -> String {
    let key = String::from_utf8_lossy(key);
    key.strip_prefix("minecraft:").unwrap_or(&key).to_string()
}

fn tag_text(tag: &Tag) -> Option<String> {
    match tag {
        Tag::String(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        Tag::Byte(value) => Some((*value != 0).to_string()),
        Tag::Int(value) => Some(value.to_string()),
        Tag::Short(value) => Some(value.to_string()),
        Tag::Long(value) => Some(value.to_string()),
        _ => None,
    }
}

/// Las reglas del mundo (nombre sin «minecraft:» → valor como texto) y si usan los nombres nuevos.
struct WorldRules {
    values: HashMap<String, String>,
    new_names: bool,
}

fn read_world_rules(config: &ServerConfig) -> Option<WorldRules> {
    let file = rules_file(config)?;
    let path = match &file {
        RulesFile::Split(p) | RulesFile::Level(p) => p,
    };
    let mut root = nbt::read_gzip(&std::fs::read(path).ok()?)?;
    let entries = rules_compound(&mut root, &file)?;
    let values: HashMap<String, String> = entries.iter().filter_map(|(key, tag)| Some((plain_name(key), tag_text(tag)?))).collect();
    let new_names = matches!(file, RulesFile::Split(_)) || values.keys().any(|k| k.contains('_'));
    Some(WorldRules { values, new_names })
}

/// Escribe reglas en el mundo (con el servidor apagado), con el tipo y el nombre que ya usa el archivo.
/// `changes`: (nombre viejo, nombre nuevo, valor como texto para cada estilo).
fn write_world_rules(config: &ServerConfig, changes: &[RuleWrite]) -> Result<(), String> {
    let Some(file) = rules_file(config) else { return Ok(()) };
    let path = match &file {
        RulesFile::Split(p) | RulesFile::Level(p) => p.clone(),
    };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut root = nbt::read_gzip(&bytes).ok_or(t!("No se pudo leer el mundo del servidor.", "Could not read the server world."))?;
    let entries = rules_compound(&mut root, &file).ok_or(t!("El mundo no tiene reglas del juego guardadas.", "The world has no saved game rules."))?;
    let split = matches!(file, RulesFile::Split(_));
    let new_names = split || entries.iter().any(|(k, _)| plain_name(k).contains('_'));
    for change in changes {
        let (name, text) = if new_names { (Some(change.new), &change.new_value) } else { (change.old, &change.old_value) };
        let Some(name) = name else { continue };
        let existing = entries.iter_mut().find(|(key, _)| plain_name(key) == name);
        let typed = |text: &str, like: Option<&Tag>| -> Tag {
            match like {
                Some(Tag::Byte(_)) => Tag::Byte(i8::from(text == "true")),
                Some(Tag::Int(_)) => Tag::Int(text.parse().unwrap_or(0)),
                Some(Tag::String(_)) => Tag::String(text.as_bytes().to_vec()),
                _ if split => match text {
                    "true" | "false" => Tag::Byte(i8::from(text == "true")),
                    number => Tag::Int(number.parse().unwrap_or(0)),
                },
                _ => Tag::String(text.as_bytes().to_vec()),
            }
        };
        match existing {
            Some((_, tag)) => *tag = typed(text, Some(tag)),
            None => {
                let key = if split { format!("minecraft:{name}") } else { name.to_string() };
                entries.push((key.into_bytes(), typed(text, None)));
            }
        }
    }
    let written = nbt::write_gzip(&root);
    // Antes de reemplazar el archivo se comprueba que se lee igual.
    if nbt::read_gzip(&written).as_ref() != Some(&root) {
        return Err(t!("No se pudo guardar la regla en el mundo.", "Could not save the rule in the world."));
    }
    let temporary = path.with_extension("parabellum-tmp");
    std::fs::write(&temporary, written).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, &path).map_err(|e| e.to_string())
}

struct RuleWrite {
    old: Option<&'static str>,
    new: &'static str,
    old_value: String,
    new_value: String,
}

// ================= Leer las opciones

/// Desde 1.21.11: nombres nuevos de las reglas y PvP/Nether/monstruos como reglas.
fn new_era(config: &ServerConfig) -> bool {
    compare_versions(&config.minecraft, NEW_RULES) != Ordering::Less
}

fn in_version(def: &Def, minecraft: &str) -> bool {
    def.since.is_none_or(|v| compare_versions(minecraft, v) != Ordering::Less) && def.until.is_none_or(|v| compare_versions(minecraft, v) == Ordering::Less)
}

/// Lo necesario para leer y escribir: server.properties, las reglas del mundo y lo pendiente.
struct Context {
    properties: Vec<(String, String)>,
    /// server.properties lo escribió el servidor (tiene todas las opciones de su versión).
    full_properties: bool,
    world: Option<WorldRules>,
    new_era: bool,
}

impl Context {
    fn load(config: &ServerConfig) -> Self {
        let properties = servidor::properties(&config.folder);
        // Parabellum nunca escribe max-tick-time: si está, el archivo lo completó el servidor.
        let full_properties = properties.iter().any(|(k, _)| k == "max-tick-time");
        Context { properties, full_properties, world: read_world_rules(config), new_era: new_era(config) }
    }

    fn property(&self, key: &str) -> Option<&str> {
        self.properties.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// La regla tal como la ve el juego (nombre según la versión) y cómo pasar su valor.
struct RuleName {
    name: &'static str,
    invert: bool,
    radius: bool,
}

fn rule_name(def: &Def, new_names: bool) -> Option<RuleName> {
    match def.source {
        Source::Rule { old, new, invert_old, radius } => {
            if new_names {
                Some(RuleName { name: new, invert: false, radius })
            } else {
                old.map(|name| RuleName { name, invert: invert_old, radius: false })
            }
        }
        Source::Moved { rule, .. } => Some(RuleName { name: rule, invert: false, radius: false }),
        Source::Property(_) => None,
    }
}

/// Valor guardado → valor de la interfaz ("true"/"false" o número como texto).
fn from_rule(text: &str, name: &RuleName) -> String {
    if name.radius {
        return (text.parse::<i64>().unwrap_or(1) > 0).to_string();
    }
    if name.invert {
        return (text != "true").to_string();
    }
    text.to_string()
}

/// Valor de la interfaz → valor para /gamerule o el archivo.
fn to_rule(value: &str, name: &RuleName) -> String {
    if name.radius {
        return if value == "true" { "128" } else { "0" }.to_string();
    }
    if name.invert {
        return (value != "true").to_string();
    }
    value.to_string()
}

/// Si la opción es una regla en este servidor (y no una línea de server.properties).
fn is_rule(def: &Def, era: bool) -> bool {
    matches!(def.source, Source::Rule { .. }) || (matches!(def.source, Source::Moved { .. }) && era)
}

fn property_key(def: &Def) -> Option<&'static str> {
    match def.source {
        Source::Property(key) | Source::Moved { property: key, .. } => Some(key),
        Source::Rule { .. } => None,
    }
}

/// El valor actual como texto de la interfaz, o None si la opción no existe en este servidor.
fn current(def: &Def, config: &ServerConfig, context: &Context) -> Option<String> {
    if is_rule(def, context.new_era) {
        let new_names = context.world.as_ref().map(|w| w.new_names).unwrap_or(context.new_era);
        let name = rule_name(def, new_names)?;
        if let Some(pending) = config.pending_rules.get(def.id) {
            return Some(pending.clone());
        }
        return match &context.world {
            // Con mundo, existe si el mundo la tiene.
            Some(world) => world.values.get(name.name).map(|v| from_rule(v, &name)),
            None => in_version(def, &config.minecraft).then(|| def.default.to_string()),
        };
    }
    let key = property_key(def)?;
    match context.property(key) {
        Some(value) => Some(match key {
            "gamemode" => servidor::named(value, &GAMEMODES, "survival"),
            "difficulty" => servidor::named(value, &DIFFICULTIES, "easy"),
            "resource-pack-prompt" => prompt_text(value),
            _ => value.to_string(),
        }),
        None if context.full_properties => None,
        None => in_version(def, &config.minecraft).then(|| def.default.to_string()),
    }
}

/// resource-pack-prompt es un texto JSON de Minecraft; en la interfaz se ve como texto normal.
fn prompt_text(value: &str) -> String {
    match serde_json::from_str::<Value>(value) {
        Ok(Value::String(text)) => text,
        Ok(Value::Object(object)) => object.get("text").and_then(Value::as_str).unwrap_or(value).to_string(),
        _ => value.to_string(),
    }
}

fn view(def: &Def, value: String, config: &ServerConfig, context: &Context) -> OptionView {
    let (label, help) = describe(def.id);
    let (kind, json_value, min, max, choices) = match def.kind {
        Kind::Bool => ("bool", json!(value == "true"), None, None, Vec::new()),
        Kind::Number(min, max) => ("number", json!(value.parse::<i64>().unwrap_or(0)), Some(min), Some(max), Vec::new()),
        Kind::Choice(list) => ("choice", json!(value), None, None, list.iter().map(|v| (v.to_string(), choice_label(v))).collect()),
        Kind::Text(max) => ("text", json!(value), None, Some(max as i64), Vec::new()),
    };
    let rule = is_rule(def, context.new_era);
    let source = if rule {
        let new_names = context.world.as_ref().map(|w| w.new_names).unwrap_or(context.new_era);
        match rule_name(def, new_names) {
            Some(name) => format!("gamerule {} {}", name.name, to_rule(&value, &name)),
            None => String::new(),
        }
    } else {
        let key = property_key(def).unwrap_or(def.id);
        let stored = match key {
            "gamemode" => servidor::stored(&value, &GAMEMODES, &config.minecraft),
            "difficulty" => servidor::stored(&value, &DIFFICULTIES, &config.minecraft),
            _ => value.clone(),
        };
        format!("{key}={stored}")
    };
    let live = rule || matches!(def.id, "difficulty" | "gamemode" | "white-list");
    OptionView { id: def.id.to_string(), label, help, kind, value: json_value, min, max, choices, source, live }
}

fn own_view(id: &str, config: &ServerConfig, total_gb: u32) -> OptionView {
    let (label, help) = describe(id);
    let base = OptionView { id: id.to_string(), label, help, kind: "bool", value: Value::Null, min: None, max: None, choices: Vec::new(), source: String::new(), live: false };
    match id {
        "verify" => OptionView { value: json!(config.verify_players), source: "Parabellum".into(), live: true, ..base },
        "ram" => OptionView {
            kind: "ram",
            value: json!(config.ram_gb),
            min: Some(1),
            max: Some(total_gb.saturating_sub(2).clamp(2, 32) as i64),
            source: format!("-Xmx{}G", config.ram_gb),
            ..base
        },
        "jvm" => OptionView { kind: "text", value: json!(config.jvm_args.join(" ")), max: Some(1000), source: if config.jvm_args.is_empty() { "java".into() } else { format!("java {}", config.jvm_args.join(" ")) }, ..base },
        _ => OptionView { value: json!(config.eula), source: format!("eula.txt: eula={}", config.eula), ..base },
    }
}

pub fn read(config: &ServerConfig, total_gb: u32) -> OptionsView {
    let context = Context::load(config);
    let mut groups: Vec<GroupView> = Vec::new();
    for group in ["players", "game", "rules", "world", "pack", "server"] {
        let mut options: Vec<OptionView> = DEFS
            .iter()
            .filter(|d| d.group == group)
            .filter_map(|def| current(def, config, &context).map(|value| view(def, value, config, &context)))
            .collect();
        match group {
            // Junto a la lista blanca.
            "players" => options.insert(options.iter().position(|o| o.id == "white-list").map_or(0, |i| i + 1), own_view("verify", config, total_gb)),
            "server" => options.extend(["ram", "jvm", "eula"].map(|id| own_view(id, config, total_gb))),
            _ => {}
        }
        if !options.is_empty() {
            groups.push(GroupView { id: group, title: group_title(group), options });
        }
    }
    let icon = std::fs::read(config.folder.join("server-icon.png")).ok().map(|png| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)));
    OptionsView { name: config.name.clone(), motd: context.property("motd").unwrap_or_default().to_string(), icon, groups }
}

// ================= Cambiar una opción

/// Lo que pasó al cambiar una opción.
pub struct Change {
    pub option: Option<OptionView>,
    /// Comandos para aplicarla ya en el servidor encendido.
    pub commands: Vec<String>,
    /// Con el servidor encendido se aplica al reiniciarlo.
    pub needs_restart: bool,
}

fn as_text(value: &Value, kind: Kind) -> Result<String, String> {
    let invalid = || t!("Ese valor no es válido.", "That value is not valid.");
    match kind {
        Kind::Bool => value.as_bool().map(|b| b.to_string()).ok_or_else(invalid),
        Kind::Number(min, max) => {
            let number = value.as_i64().or_else(|| value.as_f64().map(|f| f.round() as i64)).or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())).ok_or_else(invalid)?;
            Ok(number.clamp(min, max).to_string())
        }
        Kind::Choice(list) => value.as_str().filter(|v| list.contains(v)).map(String::from).ok_or_else(invalid),
        Kind::Text(max) => {
            let text = value.as_str().ok_or_else(invalid)?;
            Ok(text.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string())
        }
    }
}

/// Cambia una opción. `running`: el servidor está encendido (las reglas van por /gamerule).
pub fn change(config: &mut ServerConfig, id: &str, value: &Value, running: bool, total_gb: u32) -> Result<Change, String> {
    let invalid = || t!("Ese valor no es válido.", "That value is not valid.");
    // Las de Parabellum.
    if OWN.contains(&id) || id == "name" || id == "motd" {
        let mut needs_restart = running;
        match id {
            "name" => {
                let name = value.as_str().map(str::trim).filter(|n| !n.is_empty()).ok_or(t!("Escribe un nombre para el servidor.", "Type a name for the server."))?;
                config.name = name.chars().take(40).collect();
                needs_restart = false;
            }
            "motd" => {
                let text = value.as_str().ok_or_else(invalid)?;
                // Dos renglones como mucho (así se ve en la lista de servidores).
                let lines: Vec<String> = text.replace('\r', "").split('\n').take(2).map(|l| l.chars().filter(|c| !c.is_control()).collect()).collect();
                let motd: String = lines.join("\n").chars().take(MOTD_MAX).collect();
                servidor::set_properties(&config.folder, &[("motd", motd)])?;
            }
            "verify" => {
                config.verify_players = value.as_bool().ok_or_else(invalid)?;
                needs_restart = false;
            }
            "ram" => config.ram_gb = (value.as_u64().ok_or_else(invalid)? as u32).clamp(1, 64),
            "jvm" => config.jvm_args = value.as_str().ok_or_else(invalid)?.split_whitespace().map(String::from).collect(),
            _ => {
                config.eula = value.as_bool().ok_or_else(invalid)?;
                servidor::write_eula(&config.folder, config.eula)?;
                needs_restart = false;
            }
        }
        servidor::save(config)?;
        let option = OWN.contains(&id).then(|| own_view(id, config, total_gb));
        return Ok(Change { option, commands: Vec::new(), needs_restart });
    }

    let def = DEFS.iter().find(|d| d.id == id).ok_or(t!("Esa opción no existe.", "That option does not exist."))?;
    let text = as_text(value, def.kind)?;
    let context = Context::load(config);
    let mut commands = Vec::new();
    let mut needs_restart = false;
    if is_rule(def, context.new_era) {
        if running {
            // Al momento, con el nombre de la versión del servidor; queda pendiente por si se cae antes
            // de guardar el mundo (al apagarse se escribe en el mundo).
            if let Some(name) = rule_name(def, context.new_era) {
                commands.push(format!("gamerule {} {}", name.name, to_rule(&text, &name)));
            }
            config.pending_rules.insert(def.id.to_string(), text.clone());
        } else if context.world.is_some() {
            write_world_rules(config, &[rule_write(def, &text)])?;
            config.pending_rules.remove(def.id);
        } else {
            // Sin mundo todavía: se aplica en cuanto el servidor esté listo.
            config.pending_rules.insert(def.id.to_string(), text.clone());
        }
        servidor::save(config)?;
    } else {
        let key = property_key(def).ok_or_else(invalid)?;
        let stored = match key {
            "gamemode" => servidor::stored(&text, &GAMEMODES, &config.minecraft),
            "difficulty" => servidor::stored(&text, &DIFFICULTIES, &config.minecraft),
            "resource-pack-prompt" if !text.is_empty() => serde_json::to_string(&text).unwrap_or_default(),
            _ => text.clone(),
        };
        let mut values = vec![(key, stored)];
        if key == "white-list" {
            // Sin esto, los que ya estaban conectados no salen al activarla.
            values.push(("enforce-whitelist", text.clone()));
        }
        servidor::set_properties(&config.folder, &values)?;
        if running {
            match key {
                "difficulty" => commands.push(format!("difficulty {text}")),
                "gamemode" => commands.push(format!("defaultgamemode {text}")),
                "white-list" => commands.push(format!("whitelist {}", if text == "true" { "on" } else { "off" })),
                _ => needs_restart = true,
            }
        }
    }
    let context = Context::load(config);
    let option = current(def, config, &context).map(|value| view(def, value, config, &context));
    Ok(Change { option, commands, needs_restart })
}

fn rule_write(def: &Def, text: &str) -> RuleWrite {
    let old = rule_name(def, false);
    let new = rule_name(def, true);
    RuleWrite {
        old: old.as_ref().map(|n| n.name),
        new: new.as_ref().map(|n| n.name).unwrap_or(def.id),
        old_value: old.as_ref().map(|n| to_rule(text, n)).unwrap_or_else(|| text.to_string()),
        new_value: new.as_ref().map(|n| to_rule(text, n)).unwrap_or_else(|| text.to_string()),
    }
}

/// Las reglas pendientes como comandos (al quedar listo el servidor).
pub fn pending_commands(config: &ServerConfig) -> Vec<String> {
    let era = new_era(config);
    config
        .pending_rules
        .iter()
        .filter_map(|(id, value)| {
            let def = DEFS.iter().find(|d| d.id == id)?;
            let name = rule_name(def, era)?;
            Some(format!("gamerule {} {}", name.name, to_rule(value, &name)))
        })
        .collect()
}

/// Al apagarse el servidor: lo pendiente se escribe en el mundo (por si se cerró sin guardar) y se olvida.
pub fn settle_pending(config: &mut ServerConfig) {
    if config.pending_rules.is_empty() || rules_file(config).is_none() {
        return;
    }
    let writes: Vec<RuleWrite> = config.pending_rules.iter().filter_map(|(id, value)| DEFS.iter().find(|d| d.id == id).map(|def| rule_write(def, value))).collect();
    if write_world_rules(config, &writes).is_ok() {
        config.pending_rules.clear();
        let _ = servidor::save(config);
    }
}

/// Las opciones del servidor nuevo que en su versión son reglas del juego (PvP desde 1.21.11).
pub fn initial_rules(config: &mut ServerConfig, pvp: bool) {
    if new_era(config) {
        config.pending_rules.insert("pvp".into(), pvp.to_string());
    }
}

/// Guarda el icono del servidor (PNG de 64×64 como data URL) o lo quita.
pub fn set_icon(config: &ServerConfig, data: Option<&str>) -> Result<(), String> {
    let path = config.folder.join("server-icon.png");
    let Some(data) = data else {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        };
    };
    let invalid = || t!("El icono tiene que ser una imagen PNG de 64×64.", "The icon must be a 64×64 PNG image.");
    let png = data.strip_prefix("data:image/png;base64,").and_then(|b| base64::engine::general_purpose::STANDARD.decode(b).ok()).ok_or_else(invalid)?;
    // Firma PNG y el tamaño de la cabecera IHDR.
    let size = |at: usize| png.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    if !png.starts_with(b"\x89PNG\r\n\x1a\n") || size(16) != Some(64) || size(20) != Some(64) {
        return Err(invalid());
    }
    std::fs::write(path, png).map_err(|e| e.to_string())
}


#[cfg(test)]
mod tests {
    use super::*;

    /// Carpeta temporal que se borra al terminar la prueba.
    struct Temporary(PathBuf);

    impl Drop for Temporary {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn server(minecraft: &str) -> (Temporary, ServerConfig) {
        static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let number = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let folder = std::env::temp_dir().join(format!("parabellum-opciones-{}-{number}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let config = ServerConfig { minecraft: minecraft.into(), folder: folder.clone(), ..Default::default() };
        (Temporary(folder), config)
    }

    fn compound(entries: Vec<(&str, Tag)>) -> Tag {
        Tag::Compound(entries.into_iter().map(|(k, v)| (k.as_bytes().to_vec(), v)).collect())
    }

    fn find<'a>(view: &'a OptionsView, id: &str) -> Option<&'a OptionView> {
        view.groups.iter().flat_map(|g| g.options.iter()).find(|o| o.id == id)
    }

    #[test]
    fn opciones_de_un_servidor_viejo_con_mundo() {
        let (_dir, mut config) = server("1.20.1");
        std::fs::write(config.folder.join("server.properties"), "max-tick-time=60000\nmax-players=10\npvp=false\ngamemode=creative\nwhite-list=true\nallow-nether=true\nspawn-monsters=true\nspawn-animals=true\nmotd=\\u00a7aHola\n").unwrap();
        let world = config.folder.join("world");
        std::fs::create_dir_all(&world).unwrap();
        let level = compound(vec![("Data", compound(vec![("GameRules", compound(vec![("keepInventory", Tag::String(b"false".to_vec())), ("disableRaids", Tag::String(b"true".to_vec())), ("doFireTick", Tag::String(b"true".to_vec()))]))]))]);
        std::fs::write(world.join("level.dat"), nbt::write_gzip(&level)).unwrap();

        let view = read(&config, 16);
        assert_eq!(view.motd, "§aHola");
        assert_eq!(find(&view, "max-players").unwrap().value, json!(10));
        assert_eq!(find(&view, "pvp").unwrap().source, "pvp=false");
        assert_eq!(find(&view, "gamemode").unwrap().value, json!("creative"));
        assert_eq!(find(&view, "raids").unwrap().value, json!(false), "disableRaids=true es sin asaltos");
        assert_eq!(find(&view, "keep_inventory").unwrap().source, "gamerule keepInventory false");
        // El archivo completo de la versión no la tiene: no se muestra.
        assert!(find(&view, "hide-online-players").is_none());
        // El mundo no la tiene: no se muestra.
        assert!(find(&view, "locator_bar").is_none());

        // Apagado: la regla se escribe en el mundo, con su tipo.
        let change = change(&mut config, "keep_inventory", &json!(true), false, 16).unwrap();
        assert!(change.commands.is_empty());
        assert_eq!(change.option.unwrap().source, "gamerule keepInventory true");
        let rules = read_world_rules(&config).unwrap();
        assert_eq!(rules.values["keepInventory"], "true");
        let change = super::change(&mut config, "raids", &json!(true), false, 16).unwrap();
        assert_eq!(change.option.unwrap().value, json!(true));
        assert_eq!(read_world_rules(&config).unwrap().values["disableRaids"], "false");

        // Encendido: /gamerule al momento; una propiedad pide reiniciar.
        let change = super::change(&mut config, "mob_griefing", &json!(false), true, 16).unwrap();
        assert_eq!(change.commands, vec!["gamerule mobGriefing false"]);
        assert_eq!(config.pending_rules.get("mob_griefing").map(String::as_str), Some("false"));
        let change = super::change(&mut config, "pvp", &json!(true), true, 16).unwrap();
        assert!(change.needs_restart && change.commands.is_empty());
        assert_eq!(servidor::property(&config.folder, "pvp").as_deref(), Some("true"));
        let change = super::change(&mut config, "difficulty", &json!("hard"), true, 16).unwrap();
        assert_eq!(change.commands, vec!["difficulty hard"]);
        // Al apagarse, lo pendiente queda en el mundo.
        settle_pending(&mut config);
        assert!(config.pending_rules.is_empty());
        assert_eq!(read_world_rules(&config).unwrap().values["mobGriefing"], "false");
    }

    #[test]
    fn opciones_de_un_servidor_nuevo() {
        let (_dir, mut config) = server("26.2");
        std::fs::write(config.folder.join("server.properties"), "motd=Prueba\npvp=true\nmax-players=10\n").unwrap();
        // Sin mundo: las reglas salen con su valor normal y quedan pendientes.
        let view = read(&config, 16);
        assert_eq!(find(&view, "pvp").unwrap().source, "gamerule pvp true");
        assert_eq!(find(&view, "fire_spread").unwrap().source, "gamerule fire_spread_radius_around_player 128");
        assert!(find(&view, "spawn-animals").is_none());
        assert!(find(&view, "pause-when-empty-seconds").is_some());
        super::change(&mut config, "pvp", &json!(false), false, 16).unwrap();
        super::change(&mut config, "fire_spread", &json!(false), false, 16).unwrap();
        let mut commands = pending_commands(&config);
        commands.sort();
        assert_eq!(commands, vec!["gamerule fire_spread_radius_around_player 0", "gamerule pvp false"]);

        // Con mundo (game_rules.dat de 26.x): se escribe con «minecraft:» y 0/1.
        let rules = config.folder.join("world").join("data").join("minecraft");
        std::fs::create_dir_all(&rules).unwrap();
        let file = compound(vec![("DataVersion", Tag::Int(4903)), ("data", compound(vec![("minecraft:pvp", Tag::Byte(1)), ("minecraft:fire_spread_radius_around_player", Tag::Int(128)), ("minecraft:keep_inventory", Tag::Byte(0))]))]);
        std::fs::write(rules.join("game_rules.dat"), nbt::write_gzip(&file)).unwrap();
        settle_pending(&mut config);
        let world = read_world_rules(&config).unwrap();
        assert_eq!((world.values["pvp"].as_str(), world.values["fire_spread_radius_around_player"].as_str()), ("false", "0"));
        super::change(&mut config, "keep_inventory", &json!(true), false, 16).unwrap();
        let root = nbt::read_gzip(&std::fs::read(rules.join("game_rules.dat")).unwrap()).unwrap();
        assert_eq!(root.get("data").and_then(|d| d.get("minecraft:keep_inventory")), Some(&Tag::Byte(1)));
        let view = read(&config, 16);
        assert_eq!(find(&view, "keep_inventory").unwrap().value, json!(true));
        assert!(find(&view, "locator_bar").is_none(), "el mundo no la tiene");
    }

    #[test]
    fn valores_que_llegan_de_la_interfaz() {
        let (_dir, mut config) = server("1.21.1");
        std::fs::write(config.folder.join("server.properties"), "max-players=10\n").unwrap();
        assert!(super::change(&mut config, "max-players", &json!("abc"), false, 16).is_err());
        super::change(&mut config, "max-players", &json!(500), false, 16).unwrap();
        assert_eq!(servidor::property(&config.folder, "max-players").as_deref(), Some("100"));
        assert!(super::change(&mut config, "gamemode", &json!("dios"), false, 16).is_err());
        assert!(super::change(&mut config, "online-mode", &json!(true), false, 16).is_err(), "no se puede volver premium");
        super::change(&mut config, "resource-pack-prompt", &json!("Descárgalo"), false, 16).unwrap();
        assert_eq!(servidor::property(&config.folder, "resource-pack-prompt").as_deref(), Some("\"Descárgalo\""));
        assert_eq!(find(&read(&config, 16), "resource-pack-prompt").unwrap().value, json!("Descárgalo"));
        super::change(&mut config, "white-list", &json!(false), false, 16).unwrap();
        assert_eq!(servidor::property(&config.folder, "enforce-whitelist").as_deref(), Some("false"));
        super::change(&mut config, "motd", &json!("§aUno\n§bDos\nTres"), false, 16).unwrap();
        assert_eq!(read(&config, 16).motd, "§aUno\n§bDos");
    }

    #[test]
    fn icono_del_servidor() {
        let (_dir, config) = server("1.21.1");
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        png.extend(64u32.to_be_bytes());
        png.extend(64u32.to_be_bytes());
        let data = format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png));
        set_icon(&config, Some(&data)).unwrap();
        assert!(read(&config, 16).icon.is_some());
        let mut big = png.clone();
        big[16..20].copy_from_slice(&128u32.to_be_bytes());
        assert!(set_icon(&config, Some(&format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&big)))).is_err());
        set_icon(&config, None).unwrap();
        assert!(read(&config, 16).icon.is_none());
    }
}
