//! Qué hace el jugador dentro del juego, sin mods propios: se lee el registro (logs/latest.log)
//! para saber si está en su mundo, en un servidor o si abrió su mundo a otros (LAN y e4mc).
//! También ajusta e4mc para que siempre use su relé.

use regex::Regex;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Activity {
    /// "single" (mundo propio) o "server" (conectado a un servidor).
    pub world: Option<String>,
    pub server: Option<String>,
    /// Dirección pública de e4mc del mundo abierto.
    pub address: Option<String>,
    /// Puerto del mundo abierto en LAN.
    pub lan_port: Option<u16>,
}

/// Sigue el registro de una partida, leyendo solo lo nuevo cada vez.
#[derive(Default)]
pub struct Tracker {
    log: Option<PathBuf>,
    started_ms: i64,
    offset: u64,
    partial: String,
    pub activity: Activity,
}

impl Tracker {
    /// Lee lo nuevo del registro de la partida en marcha (None si no hay juego). Devuelve true si cambió algo.
    pub fn poll(&mut self, game_dir: Option<&Path>, started_ms: i64) -> bool {
        let before = self.activity.clone();
        let log = game_dir.map(|dir| dir.join("logs").join("latest.log"));
        if log != self.log || started_ms != self.started_ms {
            *self = Tracker { log, started_ms, ..Tracker::default() };
        }
        if self.log.is_some() {
            self.read_new_lines();
        }
        before != self.activity
    }

    fn read_new_lines(&mut self) {
        let Some(path) = &self.log else { return };
        let Ok(meta) = std::fs::metadata(path) else { return };
        // Hasta que el juego rota el registro, latest.log es el de la partida anterior.
        let modified = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64);
        if modified.is_none_or(|m| m + 5000 < self.started_ms) {
            return;
        }
        if meta.len() < self.offset {
            self.offset = 0;
            self.partial.clear();
            self.activity = Activity::default();
        }
        if meta.len() == self.offset {
            return;
        }
        let Ok(mut file) = std::fs::File::open(path) else { return };
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).is_err() {
            return;
        }
        self.offset += bytes.len() as u64;
        self.partial.push_str(&String::from_utf8_lossy(&bytes));
        let Some(last_break) = self.partial.rfind('\n') else { return };
        let complete: String = self.partial.drain(..=last_break).collect();
        for line in complete.lines() {
            self.activity.handle(line.trim_end_matches('\r'));
        }
    }
}

fn patterns() -> &'static (Regex, Regex, Regex) {
    static PATTERNS: OnceLock<(Regex, Regex, Regex)> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        (
            Regex::new(r"Connecting to ([^,\s]+), (\d+)").unwrap(),
            Regex::new(r"Started serving on (\d+)").unwrap(),
            Regex::new(r"(?i)(?:^|[^a-z0-9.-])((?:[a-z0-9-]+\.)+e4mc\.link)(?:$|[^a-z0-9-])").unwrap(),
        )
    })
}

impl Activity {
    fn reset(&mut self) {
        *self = Activity::default();
    }

    pub fn handle(&mut self, line: &str) {
        let (connecting, serving, domain) = patterns();
        if line.contains("Starting integrated minecraft server") {
            self.world = Some("single".into());
            self.server = None;
        } else if line.contains("Stopping singleplayer server") || line.contains("]: Stopping server") {
            self.reset();
        } else if line.contains("Server Connector") && connecting.is_match(line) {
            // "Server Connector" evita confundirlo con otras conexiones de los mods (p. ej. los relés de e4mc).
            let captures = connecting.captures(line).unwrap();
            let (host, port) = (&captures[1], &captures[2]);
            self.reset();
            self.world = Some("server".into());
            self.server = Some(if port == "25565" { host.to_string() } else { format!("{host}:{port}") });
        } else if line.contains("Disconnecting from server") || line.contains("Disconnected from server") {
            if self.world.as_deref() == Some("server") {
                self.reset();
            }
        } else if let Some(captures) = serving.captures(line) {
            self.lan_port = captures[1].parse().ok();
        } else if line.contains("[CHAT]")
            && (line.to_lowercase().contains("no longer publicly hosted") || line.to_lowercase().contains("dejado de compartir"))
        {
            self.address = None;
        } else if line.contains("[CHAT]") && line.to_lowercase().contains("e4mc.link") {
            for captures in domain.captures_iter(line) {
                let host = captures[1].to_lowercase();
                if !matches!(host.as_str(), "e4mc.link" | "broker.e4mc.link" | "natives.e4mc.link" | "test.e4mc.link") {
                    self.address = Some(host);
                }
            }
        }
    }
}

/// IP de este PC en la red local (la de la interfaz que sale a internet), para jugar por LAN.
pub fn local_address() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

/// Desde e4mc 6, si el anfitrión y el amigo tienen el mod se intenta una conexión directa ("Dialtone")
/// que en muchas redes falla con "No addressing information available" y no prueba el relé, que
/// funciona desde cualquier red. Desactivándola en los dos lados la conexión siempre va por el relé.
pub fn prefer_e4mc_relay(game_dir: &Path) {
    let has_e4mc = std::fs::read_dir(game_dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("e4mc") && e.file_name().to_string_lossy().ends_with(".jar"));
    if !has_e4mc {
        return;
    }
    let file = game_dir.join("config").join("e4mc").join("e4mc.toml");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut updated = text.clone();
    for key in ["dialtoneHostEnabled", "dialtonePlayerEnabled"] {
        let pattern = Regex::new(&format!(r"(?m)^(\s*{key}\s*=\s*)\S+")).unwrap();
        updated = if pattern.is_match(&updated) {
            pattern.replace_all(&updated, "${1}false").to_string()
        } else {
            let separator = if updated.trim_end().is_empty() { "" } else { "\n" };
            format!("{}{separator}{key} = false\n", updated.trim_end())
        };
    }
    if updated != text {
        let _ = std::fs::create_dir_all(file.parent().unwrap());
        let _ = std::fs::write(&file, updated);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigue_el_registro_del_juego() {
        let mut a = Activity::default();
        a.handle("[12:00:00] [Server thread/INFO]: Starting integrated minecraft server version 1.21.1");
        assert_eq!(a.world.as_deref(), Some("single"));
        a.handle("[12:00:05] [Server thread/INFO]: Started serving on 51234");
        assert_eq!(a.lan_port, Some(51234));
        a.handle("[12:00:06] [Render thread/INFO]: [System] [CHAT] Local game hosted on domain [silly-cat.sa.e4mc.link] (click to copy)");
        assert_eq!(a.address.as_deref(), Some("silly-cat.sa.e4mc.link"));
        a.handle("[12:00:07] [e4mc/INFO]: connecting to broker.e4mc.link");
        assert_eq!(a.address.as_deref(), Some("silly-cat.sa.e4mc.link"));
        a.handle("[12:10:00] [Server thread/INFO]: Stopping singleplayer server as player logged out");
        assert_eq!(a, Activity::default());
        a.handle("[12:11:00] [Server Connector #1/INFO]: Connecting to mc.universocraft.com, 25565");
        assert_eq!((a.world.as_deref(), a.server.as_deref()), (Some("server"), Some("mc.universocraft.com")));
        a.handle("[12:12:00] [Render thread/INFO]: Disconnecting from server");
        assert_eq!(a, Activity::default());
    }

    #[test]
    fn e4mc_por_el_rele() {
        let dir = std::env::temp_dir().join(format!("parabellum-e4mc-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("mods")).unwrap();
        std::fs::write(dir.join("mods/e4mc-6.0.1-fabric.jar"), b"x").unwrap();
        std::fs::create_dir_all(dir.join("config/e4mc")).unwrap();
        std::fs::write(dir.join("config/e4mc/e4mc.toml"), "useBroker = true\ndialtoneHostEnabled = true\n").unwrap();
        prefer_e4mc_relay(&dir);
        let text = std::fs::read_to_string(dir.join("config/e4mc/e4mc.toml")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(text, "useBroker = true\ndialtoneHostEnabled = false\ndialtonePlayerEnabled = false\n");
    }
}
