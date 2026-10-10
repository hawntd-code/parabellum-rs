//! Ajustes del jugador: nombre, UUID, memoria y tema.

use md5::{Digest, Md5};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::paths::Paths;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub username: String,
    /// Vacío: se usa el UUID offline calculado del nombre (el mismo que usan los servidores no premium).
    pub uuid: String,
    pub ram_gb: u32,
    /// "oscuro", "claro" o "sistema".
    pub theme: String,
    pub window_width: u32,
    pub window_height: u32,
    /// Minimizar Parabellum cuando se abre el juego.
    pub minimize_on_launch: bool,
    /// Servidores guardados en la página Servidores.
    pub servers: Vec<SavedServer>,
    /// Otras cuentas guardadas para cambiar con un clic (la que se usa es username/uuid).
    pub accounts: Vec<Account>,
    /// Con la sesión de Google iniciada, la X deja Parabellum junto al reloj.
    pub close_to_tray: bool,
    /// Avisos de Windows cuando un amigo te manda solicitud o abre su mundo.
    pub notifications: bool,
    /// Tamaño de la interfaz (1.0 = 100 %).
    pub ui_scale: f64,
    /// "es" o "en"; vacío hasta el primer arranque, que toma el de Windows.
    pub language: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    pub username: String,
    /// Vacío: el UUID offline de su nombre.
    pub uuid: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedServer {
    pub name: String,
    pub address: String,
    /// Instancia con la que se entra (id = nombre de su carpeta).
    pub instance: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            username: "Jugador".into(),
            uuid: String::new(),
            ram_gb: 4,
            theme: "oscuro".into(),
            window_width: 1280,
            window_height: 720,
            minimize_on_launch: true,
            servers: Vec::new(),
            accounts: Vec::new(),
            close_to_tray: true,
            notifications: true,
            ui_scale: 1.0,
            language: String::new(),
        }
    }
}

/// datos/config.json de la versión C# (retirada): al abrir Parabellum en una de sus carpetas se
/// conservan el nombre, el UUID fijo (con el que sus mundos reconocen al jugador) y la memoria.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OldSettings {
    username: Option<String>,
    uuid: Option<String>,
    #[serde(rename = "RamGB")]
    ram_gb: Option<u32>,
}

fn from_old(text: &str) -> Option<Settings> {
    let old: OldSettings = serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()?;
    let mut settings = Settings::default();
    if let Some(name) = old.username.filter(|name| validate_username(name).is_none()) {
        settings.username = name;
    }
    if let Some(uuid) = old.uuid.as_deref().and_then(parse_uuid) {
        settings.uuid = dashed(&uuid);
    }
    if let Some(ram) = old.ram_gb {
        settings.ram_gb = ram.clamp(1, 64);
    }
    Some(settings)
}

impl Settings {
    pub fn load(paths: &Paths) -> Self {
        match std::fs::read_to_string(paths.settings_file()) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
            Err(_) => std::fs::read_to_string(paths.data.join("config.json"))
                .ok()
                .and_then(|text| from_old(&text))
                .unwrap_or_default(),
        }
    }

    pub fn save(&self, paths: &Paths) -> std::io::Result<()> {
        let file = paths.settings_file();
        std::fs::create_dir_all(file.parent().unwrap())?;
        let temp = file.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self).unwrap())?;
        std::fs::rename(temp, file)
    }

    /// Deja los UUID con guiones y la lista de cuentas sin repetidas, sin nombres no válidos y sin la
    /// que se está usando. Error si el UUID de la cuenta en uso no es válido.
    pub fn normalize(&mut self) -> Result<(), &'static str> {
        self.username = self.username.trim().to_string();
        if let Some(problem) = validate_username(&self.username) {
            return Err(problem);
        }
        self.uuid = match self.uuid.trim() {
            "" => String::new(),
            text => dashed(&parse_uuid(text).ok_or("El UUID no es válido. Déjalo vacío para usar el de tu nombre.")?),
        };
        let mut seen = vec![self.username.to_lowercase()];
        self.accounts.retain_mut(|account| {
            account.username = account.username.trim().to_string();
            account.uuid = parse_uuid(&account.uuid).map(|u| dashed(&u)).unwrap_or_default();
            let key = account.username.to_lowercase();
            let keep = validate_username(&account.username).is_none() && !seen.contains(&key);
            seen.push(key);
            keep
        });
        Ok(())
    }

    /// UUID que recibe el juego, sin guiones.
    pub fn effective_uuid(&self) -> String {
        parse_uuid(&self.uuid).unwrap_or_else(|| offline_uuid(&self.username))
    }
}

/// Mensaje de error si el nombre no sirve para Minecraft (1 a 16 letras, números o guion bajo).
pub fn validate_username(name: &str) -> Option<&'static str> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| Regex::new("^[A-Za-z0-9_]{1,16}$").unwrap());
    if pattern.is_match(name) {
        None
    } else {
        Some("El nombre debe tener de 1 a 16 letras, números o guiones bajos (sin espacios ni acentos).")
    }
}

/// El mismo algoritmo que Minecraft usa para los jugadores sin cuenta premium:
/// MD5("OfflinePlayer:" + nombre) con los bits de versión 3.
pub fn offline_uuid(name: &str) -> String {
    let mut bytes: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    hex::encode(bytes)
}

/// Acepta un UUID con o sin guiones y lo devuelve sin guiones.
pub fn parse_uuid(text: &str) -> Option<String> {
    let clean: String = text.trim().chars().filter(|c| *c != '-').collect();
    (clean.len() == 32 && clean.chars().all(|c| c.is_ascii_hexdigit())).then(|| clean.to_lowercase())
}

/// Formato con guiones (8-4-4-4-12) para mostrarlo.
pub fn dashed(uuid: &str) -> String {
    if uuid.len() != 32 {
        return uuid.to_string();
    }
    format!("{}-{}-{}-{}-{}", &uuid[..8], &uuid[8..12], &uuid[12..16], &uuid[16..20], &uuid[20..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_minecraft() {
        // Valor conocido: el UUID offline de "GxxseS" que vimos en el servidor de prueba.
        assert_eq!(dashed(&offline_uuid("GxxseS")), "8b67db88-c6fb-3d4d-bbee-71f84a0e752d");
    }

    #[test]
    fn ajustes_de_la_version_csharp() {
        let old = r#"{ "Username": "Steve_2", "Uuid": "0123456789ABCDEF0123456789abcdef", "RamGB": 6, "WindowWidth": 1280, "CloseToTray": true }"#;
        let settings = from_old(old).unwrap();
        assert_eq!(settings.username, "Steve_2");
        assert_eq!(settings.uuid, "01234567-89ab-cdef-0123-456789abcdef");
        assert_eq!(settings.ram_gb, 6);
        assert_eq!(settings.theme, "oscuro");
        // Un nombre que Minecraft no acepta no se copia.
        assert_eq!(from_old(r#"{ "Username": "con espacio" }"#).unwrap().username, "Jugador");
    }

    #[test]
    fn ordena_las_cuentas() {
        let mut settings = Settings {
            username: " GxxseS ".into(),
            uuid: "1525C21EE760484CA393E32CCB3611D2".into(),
            accounts: vec![
                Account { username: "Hawntd".into(), uuid: String::new() },
                Account { username: "gxxses".into(), uuid: String::new() },
                Account { username: "con espacio".into(), uuid: String::new() },
                Account { username: "hawntd".into(), uuid: "x".into() },
            ],
            ..Settings::default()
        };
        settings.normalize().unwrap();
        assert_eq!(settings.username, "GxxseS");
        assert_eq!(settings.uuid, "1525c21e-e760-484c-a393-e32ccb3611d2");
        assert_eq!(settings.accounts, vec![Account { username: "Hawntd".into(), uuid: String::new() }]);
        settings.uuid = "no".into();
        assert!(settings.normalize().is_err());
    }

    #[test]
    fn username_rules() {
        assert!(validate_username("GxxseS").is_none());
        assert!(validate_username("con espacio").is_some());
        assert!(validate_username("").is_some());
        assert!(validate_username("abcdefghijklmnopq").is_some());
    }
}
