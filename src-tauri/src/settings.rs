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
        }
    }
}

impl Settings {
    pub fn load(paths: &Paths) -> Self {
        std::fs::read_to_string(paths.settings_file())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, paths: &Paths) -> std::io::Result<()> {
        let file = paths.settings_file();
        std::fs::create_dir_all(file.parent().unwrap())?;
        let temp = file.with_extension("tmp");
        std::fs::write(&temp, serde_json::to_vec_pretty(self).unwrap())?;
        std::fs::rename(temp, file)
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
    fn username_rules() {
        assert!(validate_username("GxxseS").is_none());
        assert!(validate_username("con espacio").is_some());
        assert!(validate_username("").is_some());
        assert!(validate_username("abcdefghijklmnopq").is_some());
    }
}
