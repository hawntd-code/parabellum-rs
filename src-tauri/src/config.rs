//! Configuración del launcher: la carpeta de Drive con los modpacks y las claves de Google y Firebase.
//! Viene integrada en el exe (se compila desde config-integrada.json, que no está en el repositorio),
//! así basta con repartir Parabellum.exe. Un lanzador-config.json junto al exe (el mismo archivo que
//! usaba la versión C#) sustituye los valores que traiga.

use serde::Deserialize;

use crate::paths::Paths;

const EMBEDDED: &str = include_str!(concat!(env!("OUT_DIR"), "/config-integrada.json"));
const LOCAL_FILE: &str = "lanzador-config.json";

#[derive(Deserialize, Default, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Carpeta pública de Google Drive con los paquetes.
    pub drive_folder_id: String,
    /// Clave de Google Cloud con la Drive API (solo para listar la carpeta).
    pub drive_api_key: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub firebase_api_key: String,
    pub firebase_database_url: String,
    /// Nombre de jugador de quien recibe los «Reportar problema» (sale elegido si es tu amigo).
    pub support_name: String,
}

impl Config {
    pub fn load(paths: &Paths) -> Self {
        let mut config: Config = serde_json::from_str(EMBEDDED).unwrap_or_default();
        let local = std::fs::read_to_string(paths.root.join(LOCAL_FILE)).ok().and_then(|t| serde_json::from_str::<Config>(&t).ok());
        if let Some(local) = local {
            let fields = [
                (&mut config.drive_folder_id, local.drive_folder_id),
                (&mut config.drive_api_key, local.drive_api_key),
                (&mut config.google_client_id, local.google_client_id),
                (&mut config.google_client_secret, local.google_client_secret),
                (&mut config.firebase_api_key, local.firebase_api_key),
                (&mut config.firebase_database_url, local.firebase_database_url),
                (&mut config.support_name, local.support_name),
            ];
            for (target, value) in fields {
                if !value.trim().is_empty() {
                    *target = value.trim().to_string();
                }
            }
        }
        // Pruebas: la base de datos del emulador de Firebase (docs/probar-reglas.py), p. ej.
        // «http://127.0.0.1:9399?ns=parabellum-prueba». Así se prueba con las reglas nuevas sin publicarlas.
        if let Ok(emulator) = std::env::var("PARABELLUM_FIREBASE_EMULADOR") {
            config.firebase_database_url = emulator;
        }
        config
    }

    pub fn has_drive(&self) -> bool {
        !self.drive_folder_id.is_empty() && !self.drive_api_key.is_empty()
    }

    pub fn drive_folder_url(&self) -> Option<String> {
        self.has_drive().then(|| format!("https://drive.google.com/drive/folders/{}", self.drive_folder_id))
    }
}
