//! Inicio de sesión con Google para apps de escritorio: OAuth 2.0 con PKCE y redirección a
//! 127.0.0.1 (el navegador vuelve a un puerto local de Parabellum). Solo pide el correo y la carpeta
//! oculta de la app en el Drive del jugador (drive.appdata), donde se guarda su perfil.
//! La sesión queda cifrada para el usuario de Windows (DPAPI), en el mismo archivo y formato que la
//! versión C#: quien llegó desde ella sigue con su sesión iniciada.

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::config::Config;
use crate::paths::Paths;
use crate::settings::Account;

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USER_INFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const SCOPES: &str = "openid email profile https://www.googleapis.com/auth/drive.appdata";
const PROFILE_FILE: &str = "perfil-lanzador.json";
const SESSION_FILE: &str = "sesion-google.bin";

/// Perfil del jugador guardado en su cuenta (mismo formato que la versión C#, más las otras cuentas).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Profile {
    pub username: String,
    pub uuid: String,
    #[serde(rename = "RamGB")]
    pub ram_gb: u32,
    pub accounts: Vec<Account>,
}

struct Token {
    access: String,
    id: Option<String>,
    expires: Instant,
}

pub struct GoogleAccount {
    client_id: String,
    client_secret: String,
    pub refresh_token: String,
    pub email: String,
    pub name: String,
    token: tokio::sync::Mutex<Option<Token>>,
}

fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("Parabellum/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("cliente HTTP")
    })
}

fn unavailable() -> String {
    t!("No se pudo conectar con Google. Revisa tu conexión a internet.", "Could not connect to Google. Check your internet connection.")
}

fn random_text(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    getrandom::getrandom(&mut buffer).expect("números aleatorios");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buffer)
}

/// Abre la página en el navegador predeterminado.
pub fn open_browser(url: &str) -> Result<(), String> {
    std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn().map(|_| ()).map_err(|e| e.to_string())
}

impl GoogleAccount {
    /// Abre el navegador para elegir la cuenta y espera la respuesta en un puerto local (hasta 5 minutos).
    pub async fn sign_in(config: &Config) -> Result<Self, String> {
        let verifier = random_text(32);
        let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let state = random_text(16);
        // Un mini servidor en 127.0.0.1 recibe la respuesta de Google.
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| e.to_string())?;
        let redirect = format!("http://127.0.0.1:{}/", listener.local_addr().map_err(|e| e.to_string())?.port());
        let url = reqwest::Url::parse_with_params(
            AUTH_URL,
            &[
                ("client_id", config.google_client_id.as_str()),
                ("redirect_uri", &redirect),
                ("response_type", "code"),
                ("scope", SCOPES),
                ("code_challenge", &challenge),
                ("code_challenge_method", "S256"),
                ("state", &state),
                ("prompt", "select_account"),
            ],
        )
        .map_err(|e| e.to_string())?;
        open_browser(url.as_str())?;

        let wait = async {
            loop {
                let (mut stream, _) = listener.accept().await.map_err(|e| e.to_string())?;
                let target = read_request_target(&mut stream).await.unwrap_or_default();
                let query: Vec<(String, String)> = reqwest::Url::parse(&format!("http://local{target}"))
                    .map(|u| u.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect())
                    .unwrap_or_default();
                let get = |key: &str| query.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
                if get("code").is_none() && get("error").is_none() {
                    respond(&mut stream, 404, "").await; // p. ej. /favicon.ico
                    continue;
                }
                let ok = get("state").as_deref() == Some(state.as_str()) && get("code").is_some();
                respond(
                    &mut stream,
                    200,
                    &if ok {
                        t!("Sesión iniciada. Ya puedes volver a Parabellum y cerrar esta pestaña.", "Signed in. You can go back to Parabellum and close this tab.")
                    } else {
                        t!("No se completó el inicio de sesión. Vuelve a Parabellum e inténtalo de nuevo.", "The sign-in was not completed. Go back to Parabellum and try again.")
                    },
                )
                .await;
                if !ok {
                    return Err(if get("error").as_deref() == Some("access_denied") {
                        t!("Se canceló el inicio de sesión.", "The sign-in was cancelled.")
                    } else {
                        t!("Google no devolvió un código de acceso válido.", "Google did not return a valid access code.")
                    });
                }
                return Ok(get("code").unwrap_or_default());
            }
        };
        let code = tokio::time::timeout(Duration::from_secs(300), wait)
            .await
            .map_err(|_| t!("No se completó el inicio de sesión a tiempo. Vuelve a intentarlo.", "The sign-in was not completed in time. Try again."))??;

        let token = post_form(&[
            ("code", code.as_str()),
            ("client_id", &config.google_client_id),
            ("client_secret", &config.google_client_secret),
            ("redirect_uri", &redirect),
            ("grant_type", "authorization_code"),
            ("code_verifier", &verifier),
        ])
        .await?;
        let refresh = token["refresh_token"].as_str().ok_or(t!("Google no devolvió un token de actualización.", "Google did not return a refresh token."))?.to_string();
        let mut account = Self::new(config, refresh);
        account.accept(&token).await?;
        account.load_user_info().await?;
        Ok(account)
    }

    /// Recupera una sesión guardada en este PC.
    pub async fn resume(config: &Config, refresh_token: String) -> Result<Self, String> {
        let mut account = Self::new(config, refresh_token);
        account.load_user_info().await?;
        Ok(account)
    }

    fn new(config: &Config, refresh_token: String) -> Self {
        Self {
            client_id: config.google_client_id.clone(),
            client_secret: config.google_client_secret.clone(),
            refresh_token,
            email: String::new(),
            name: String::new(),
            token: tokio::sync::Mutex::new(None),
        }
    }

    async fn accept(&self, token: &Value) -> Result<(), String> {
        let access = token["access_token"].as_str().ok_or(t!("Google no devolvió un token de acceso.", "Google did not return an access token."))?.to_string();
        let seconds = token["expires_in"].as_u64().unwrap_or(3600).saturating_sub(60);
        let mut current = self.token.lock().await;
        let id = token["id_token"].as_str().map(String::from).or_else(|| current.as_ref().and_then(|t| t.id.clone()));
        *current = Some(Token { access, id, expires: Instant::now() + Duration::from_secs(seconds) });
        Ok(())
    }

    async fn refresh(&self) -> Result<(), String> {
        let token = post_form(&[
            ("client_id", self.client_id.as_str()),
            ("client_secret", &self.client_secret),
            ("refresh_token", &self.refresh_token),
            ("grant_type", "refresh_token"),
        ])
        .await?;
        self.accept(&token).await
    }

    async fn access_token(&self) -> Result<String, String> {
        if let Some(token) = self.token.lock().await.as_ref().filter(|t| Instant::now() < t.expires) {
            return Ok(token.access.clone());
        }
        self.refresh().await?;
        Ok(self.token.lock().await.as_ref().map(|t| t.access.clone()).unwrap_or_default())
    }

    /// Token de identidad (OpenID) vigente: Firebase lo acepta para entrar con la misma cuenta.
    pub async fn id_token(&self) -> Result<String, String> {
        let fresh = self.token.lock().await.as_ref().filter(|t| Instant::now() < t.expires).and_then(|t| t.id.clone());
        if let Some(id) = fresh {
            return Ok(id);
        }
        self.refresh().await?;
        self.token.lock().await.as_ref().and_then(|t| t.id.clone()).ok_or(t!("Google no devolvió un token de identidad.", "Google did not return an identity token."))
    }

    async fn load_user_info(&mut self) -> Result<(), String> {
        let token = self.access_token().await?;
        let info: Value = http()
            .get(USER_INFO_URL)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| unavailable())?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        self.email = info["email"].as_str().unwrap_or_default().to_string();
        self.name = info["name"].as_str().map(String::from).unwrap_or_else(|| self.email.clone());
        Ok(())
    }

    async fn profile_file_id(&self) -> Result<Option<String>, String> {
        let token = self.access_token().await?;
        let list: Value = http()
            .get(FILES_URL)
            .bearer_auth(token)
            .query(&[("spaces", "appDataFolder"), ("fields", "files(id)"), ("q", &format!("name = '{PROFILE_FILE}'"))])
            .send()
            .await
            .map_err(|_| unavailable())?
            .error_for_status()
            .map_err(|e| t!("Google Drive respondió {}", "Google Drive answered {}", e.status().map(|s| s.as_u16()).unwrap_or(0)))?
            .json()
            .await
            .map_err(|e| e.to_string())?;
        Ok(list["files"].get(0).and_then(|f| f["id"].as_str()).map(String::from))
    }

    /// Perfil guardado en la cuenta (None si todavía no hay).
    pub async fn load_profile(&self) -> Result<Option<Profile>, String> {
        let Some(id) = self.profile_file_id().await? else { return Ok(None) };
        let token = self.access_token().await?;
        let text = http()
            .get(format!("{FILES_URL}/{id}"))
            .bearer_auth(token)
            .query(&[("alt", "media")])
            .send()
            .await
            .map_err(|_| unavailable())?
            .text()
            .await
            .map_err(|e| e.to_string())?;
        Ok(serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
    }

    pub async fn save_profile(&self, profile: &Profile) -> Result<(), String> {
        let content = serde_json::to_string(profile).unwrap();
        let id = self.profile_file_id().await?;
        let token = self.access_token().await?;
        let request = match id {
            Some(id) => http()
                .patch(format!("{UPLOAD_URL}/{id}"))
                .query(&[("uploadType", "media")])
                .header("Content-Type", "application/json")
                .body(content),
            None => {
                // Archivo nuevo en la carpeta oculta de la app: metadatos y contenido en una sola petición.
                let boundary = format!("parabellum{}", random_text(9).replace(['-', '_'], "x"));
                let metadata = serde_json::json!({ "name": PROFILE_FILE, "parents": ["appDataFolder"] });
                let body = format!(
                    "--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{metadata}\r\n--{boundary}\r\nContent-Type: application/json\r\n\r\n{content}\r\n--{boundary}--\r\n"
                );
                http()
                    .post(UPLOAD_URL)
                    .query(&[("uploadType", "multipart")])
                    .header("Content-Type", format!("multipart/related; boundary={boundary}"))
                    .body(body)
            }
        };
        request
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| unavailable())?
            .error_for_status()
            .map_err(|e| t!("Google Drive respondió {}", "Google Drive answered {}", e.status().map(|s| s.as_u16()).unwrap_or(0)))?;
        Ok(())
    }
}

async fn post_form(form: &[(&str, &str)]) -> Result<Value, String> {
    let response = http().post(TOKEN_URL).form(form).send().await.map_err(|_| unavailable())?;
    let ok = response.status().is_success();
    let body: Value = response.json().await.unwrap_or_default();
    if !ok {
        let error = body["error"].as_str().unwrap_or("desconocido");
        return Err(if error == "invalid_grant" {
            t!("La sesión de Google caducó o se revocó. Vuelve a iniciar sesión.", "The Google session expired or was revoked. Sign in again.")
        } else {
            t!("Google rechazó el inicio de sesión ({error}).", "Google rejected the sign-in ({error}).")
        });
    }
    Ok(body)
}

async fn read_request_target(stream: &mut tokio::net::TcpStream) -> std::io::Result<String> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 4096];
    while data.len() < 16 * 1024 && !data.windows(4).any(|w| w == b"\r\n\r\n") {
        let read = tokio::time::timeout(Duration::from_secs(10), stream.read(&mut chunk)).await.map_err(std::io::Error::other)??;
        if read == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..read]);
    }
    let text = String::from_utf8_lossy(&data);
    Ok(text.lines().next().and_then(|l| l.split(' ').nth(1)).unwrap_or("/").to_string())
}

async fn respond(stream: &mut tokio::net::TcpStream, status: u16, message: &str) {
    let html = if status == 200 {
        format!(
            "<!doctype html><meta charset=utf-8><title>Parabellum</title><body style=\"font-family:Segoe UI,sans-serif;background:#0e0c13;color:#ece8f4;display:grid;place-items:center;height:100vh;margin:0\"><div style=\"max-width:420px;text-align:center;font-size:18px\">{message}</div></body>"
        )
    } else {
        String::new()
    };
    let head = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        if status == 200 { "OK" } else { "Not Found" },
        html.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(html.as_bytes()).await;
}

// ================= Sesión guardada (cifrada con DPAPI para el usuario de Windows)

pub fn save_session(paths: &Paths, refresh_token: &str) -> Result<(), String> {
    let data = dpapi(refresh_token.as_bytes(), true).ok_or(t!("No se pudo cifrar la sesión.", "Could not encrypt the session."))?;
    std::fs::write(paths.data.join(SESSION_FILE), data).map_err(|e| e.to_string())
}

pub fn load_session(paths: &Paths) -> Option<String> {
    let data = std::fs::read(paths.data.join(SESSION_FILE)).ok()?;
    String::from_utf8(dpapi(&data, false)?).ok().filter(|t| !t.is_empty())
}

pub fn clear_session(paths: &Paths) {
    let _ = std::fs::remove_file(paths.data.join(SESSION_FILE));
}

pub fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: los punteros apuntan a buffers válidos durante la llamada; el resultado lo reserva
    // Windows y se libera con LocalFree después de copiarlo.
    unsafe {
        let ok = if protect {
            CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        };
        if ok == 0 || output.pbData.is_null() {
            return None;
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sesion_cifrada_de_ida_y_vuelta() {
        let paths = Paths::new(std::env::temp_dir().join(format!("parabellum-google-{}", std::process::id())));
        std::fs::create_dir_all(&paths.data).unwrap();
        save_session(&paths, "1//token-de-prueba").unwrap();
        assert_ne!(std::fs::read(paths.data.join(SESSION_FILE)).unwrap(), b"1//token-de-prueba");
        assert_eq!(load_session(&paths).as_deref(), Some("1//token-de-prueba"));
        clear_session(&paths);
        assert!(load_session(&paths).is_none());
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    #[test]
    fn perfil_de_la_version_csharp() {
        let profile: Profile = serde_json::from_str(r#"{"Username":"GxxseS","Uuid":"1525c21e-e760-484c-a393-e32ccb3611d2","RamGB":6}"#).unwrap();
        assert_eq!((profile.username.as_str(), profile.ram_gb, profile.accounts.len()), ("GxxseS", 6, 0));
    }
}
