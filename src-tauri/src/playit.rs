//! playit.gg: el túnel que deja entrar a tus amigos sin abrir puertos del router ni pagar nada.
//! Parabellum usa el agente oficial (playit.exe, firmado por playit.gg; se baja la primera vez) y su API:
//!   1. Vincular: se abre playit.gg/claim/<código> en el navegador y, al aceptar, la API entrega la
//!      clave del agente, que se guarda cifrada para el usuario de Windows (DPAPI) en datos/playit.dat.
//!   2. Un único túnel "minecraft-java" llamado "parabellum" con dirección fija (p. ej.
//!      algo.gl.joinmc.link): solo cambia el puerto local al que apunta (el servidor o el mundo
//!      abierto en LAN), así que los amigos usan siempre la misma dirección.
//!   3. El agente corre oculto mientras haya algo compartido y se cierra con Parabellum.

use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::io::BufRead;
use crate::plataforma::CommandExt as _;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::download::{self, FileDownload};
use crate::paths::Paths;
use crate::proceso;

pub const AGENT_VERSION: &str = "0.17.1";
const API: &str = "https://api.playit.gg";
const SECRET_FILE: &str = "playit.dat";
pub const TUNNEL_NAME: &str = "parabellum";

fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(20)).build().expect("cliente HTTP"))
}

fn unavailable() -> String {
    t!("No se pudo conectar con playit.gg. Revisa tu conexión a internet.", "Could not connect to playit.gg. Check your internet connection.")
}

// ================= Agente (playit.exe)

/// El agente para este sistema (plataforma::PLAYIT_AGENT), o por qué no hay.
fn agent() -> Result<&'static crate::plataforma::AgentDownload, String> {
    crate::plataforma::PLAYIT_AGENT.as_ref().ok_or_else(|| {
        t!("playit.gg todavía no tiene su agente para {}.", "playit.gg does not have its agent for {} yet.", crate::plataforma::NAME)
    })
}

pub fn agent_path(paths: &Paths) -> PathBuf {
    let name = agent().map(|a| a.file_name).unwrap_or("playit");
    paths.base.join("playit").join(name)
}

/// Está bajado y es exactamente el agente esperado: su SHA-256, no solo el tamaño. Lo que haya en esa
/// ruta se ejecuta (en Mac, además, se marca como programa), y en base/ también escriben los paquetes
/// que se importan. Si no coincide, se vuelve a bajar.
pub fn agent_installed(paths: &Paths) -> bool {
    agent().is_ok_and(|a| std::fs::read(agent_path(paths)).is_ok_and(|bytes| bytes.len() as u64 == a.size && hex::encode(Sha256::digest(&bytes)) == a.sha256))
}

/// Baja el agente oficial y comprueba que sea exactamente el esperado.
pub async fn download_agent(paths: &Paths, progress: impl Fn(String, f64) + Send + Sync + 'static) -> Result<(), String> {
    if agent_installed(paths) {
        return Ok(());
    }
    let agent = agent()?;
    let target = agent_path(paths);
    let partial = target.with_extension("descarga");
    let _ = std::fs::remove_file(&partial);
    progress(t!("Descargando el agente de playit.gg…", "Downloading the playit.gg agent…"), 0.0);
    download::download_all(vec![FileDownload { url: agent.url.into(), path: partial.clone(), sha1: None, size: Some(agent.size) }], move |_, _, bytes, total| {
        progress(t!("Descargando el agente de playit.gg…", "Downloading the playit.gg agent…"), bytes as f64 / total.max(1) as f64)
    })
    .await
    .map_err(|_| t!("No se pudo descargar el agente de playit.gg (GitHub). Revisa tu conexión.", "Could not download the playit.gg agent (GitHub). Check your connection."))?;
    let bytes = std::fs::read(&partial).map_err(|e| e.to_string())?;
    if hex::encode(Sha256::digest(&bytes)) != agent.sha256 {
        let _ = std::fs::remove_file(&partial);
        return Err(t!("El agente de playit.gg llegó dañado o no es el oficial. Vuelve a intentarlo.", "The playit.gg agent arrived damaged or is not the official one. Try again."));
    }
    std::fs::rename(&partial, &target).map_err(|e| e.to_string())
}

/// El agente en marcha y lo último que escribió (para explicar errores).
pub struct Agent {
    child: Child,
    log: Arc<Mutex<VecDeque<String>>>,
}

impl Agent {
    pub fn start(paths: &Paths, secret: &str) -> Result<Self, String> {
        let exe = agent_path(paths);
        if !agent_installed(paths) {
            return Err(t!("Falta el agente de playit.gg.", "The playit.gg agent is missing."));
        }
        // En Mac lo descargado llega sin permiso de ejecución (en Windows no hace falta).
        crate::plataforma::make_executable(&exe);
        // Carpeta propia: el agente busca un playit.toml en la carpeta actual y no debe encontrar otro.
        let folder = paths.data.join("playit");
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        // La clave va por variable de entorno (no aparece en la línea de comandos del proceso).
        let mut child = Command::new(&exe)
            .args(["--stdout", "start"])
            .env("PLAYIT_SECRET", secret)
            .env("PLAYIT_LOG", "info")
            .current_dir(&folder)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .no_console()
            .spawn()
            .map_err(|e| t!("No se pudo abrir el agente de playit.gg: {e}", "Could not open the playit.gg agent: {e}"))?;
        proceso::tie_to_launcher(&child);
        let log = Arc::new(Mutex::new(VecDeque::new()));
        let file = paths.logs.join("playit.log");
        let _ = std::fs::write(&file, "");
        for stream in [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>)]
            .into_iter()
            .flatten()
        {
            let (log, file) = (log.clone(), file.clone());
            std::thread::spawn(move || {
                let mut reader = std::io::BufReader::new(stream);
                let mut buffer = Vec::new();
                while reader.read_until(b'\n', &mut buffer).is_ok_and(|n| n > 0) {
                    let line = strip_ansi(String::from_utf8_lossy(&buffer).trim_end());
                    buffer.clear();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(mut out) = std::fs::OpenOptions::new().append(true).open(&file) {
                        use std::io::Write;
                        let _ = writeln!(out, "{line}");
                    }
                    let mut log = log.lock().unwrap();
                    log.push_back(line);
                    while log.len() > 200 {
                        log.pop_front();
                    }
                }
            });
        }
        Ok(Agent { child, log })
    }

    /// Ya se conectó a playit.gg (lo que espera la página de playit.gg tras aceptar).
    pub fn connected(&self) -> bool {
        self.log.lock().unwrap().iter().any(|l| l.contains("agent registered") || l.contains("tunnel running"))
    }

    /// Lo último que escribió (para explicar por qué se cerró).
    pub fn last_line(&self) -> Option<String> {
        // Un solo candado: si se tomaba otra vez dentro de la misma expresión, esperaba para siempre.
        let log = self.log.lock().unwrap();
        log.iter().rev().find(|l| l.contains("ERROR") || l.contains("WARN")).or(log.back()).cloned()
    }

    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Quita los colores de la terminal (el agente los usa en algunos mensajes).
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for c in chars.by_ref() {
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

// ================= Clave del agente (cifrada)

pub fn save_secret(paths: &Paths, secret: &str) -> Result<(), String> {
    let data = crate::google::dpapi(secret.as_bytes(), true).ok_or(t!("No se pudo cifrar la clave de playit.gg.", "Could not encrypt the playit.gg key."))?;
    std::fs::write(paths.data.join(SECRET_FILE), data).map_err(|e| e.to_string())
}

pub fn load_secret(paths: &Paths) -> Option<String> {
    let data = std::fs::read(paths.data.join(SECRET_FILE)).ok()?;
    String::from_utf8(crate::google::dpapi(&data, false)?).ok().filter(|s| !s.trim().is_empty())
}

pub fn clear_secret(paths: &Paths) {
    let _ = std::fs::remove_file(paths.data.join(SECRET_FILE));
}

// ================= API

#[derive(Debug)]
pub enum ApiError {
    /// La API dice que no (p. ej. "RequiresVerifiedAccount").
    Fail(String),
    /// Error de la API: autenticación, validación, ruta inexistente…
    Error { kind: String, message: String },
    Network,
}

impl ApiError {
    /// La clave ya no sirve (se borró el agente en playit.gg o se revocó).
    pub fn invalid_key(&self) -> bool {
        matches!(self, ApiError::Error { kind, message } if kind == "auth" && matches!(message.as_str(), "InvalidAgentKey" | "InvalidSignature" | "NoLongerValid" | "AccountDoesNotExist"))
    }

    /// La cuenta todavía no puede tener dirección: falta verificar el correo (cuentas nuevas).
    pub fn needs_verification(&self) -> bool {
        matches!(self, ApiError::Fail(reason) | ApiError::Error { message: reason, .. } if matches!(reason.as_str(), "RequiresVerifiedAccount" | "EmailMustBeVerified"))
    }

    fn missing_path(&self) -> bool {
        matches!(self, ApiError::Error { kind, .. } if kind == "path-not-found" || kind == "validation")
    }

    pub fn explain(&self) -> String {
        match self {
            ApiError::Network => unavailable(),
            e if e.invalid_key() => t!(
                "La vinculación con playit.gg ya no es válida (quizá se borró el agente en su página). Vuelve a conectar playit.gg.",
                "The playit.gg link is no longer valid (maybe the agent was deleted on their site). Connect playit.gg again."
            ),
            ApiError::Fail(reason) | ApiError::Error { message: reason, .. } => match reason.as_str() {
                "RequiresVerifiedAccount" | "EmailMustBeVerified" => t!(
                    "playit.gg pide verificar tu correo antes de crear el túnel. Entra a playit.gg, verifica tu cuenta y vuelve a intentarlo.",
                    "playit.gg asks you to verify your email before creating the tunnel. Go to playit.gg, verify your account and try again."
                ),
                "GuestAccountNotAllowed" => t!(
                    "playit.gg no permite esto con una cuenta de invitado. Crea una cuenta gratis en playit.gg y vuelve a conectar.",
                    "playit.gg does not allow this with a guest account. Create a free account on playit.gg and connect again."
                ),
                "RequiresPlayitPremium" | "PublicPortRequiresPlayitPremium" | "RegionRequiresPlayitPremium" => t!(
                    "playit.gg pide su plan de pago para esto. Borra túneles que no uses en playit.gg y vuelve a intentarlo.",
                    "playit.gg requires its paid plan for this. Delete tunnels you do not use on playit.gg and try again."
                ),
                "AgentVersionTooOld" => t!("playit.gg dice que su agente está desactualizado. Actualiza Parabellum.", "playit.gg says its agent is outdated. Update Parabellum."),
                "AgentNotFound" | "InvalidAgentId" => t!("playit.gg no encuentra este agente. Vuelve a conectar playit.gg.", "playit.gg cannot find this agent. Connect playit.gg again."),
                other => t!("playit.gg respondió: {other}", "playit.gg answered: {other}"),
            },
        }
    }
}

async fn call(path: &str, body: Value, secret: Option<&str>) -> Result<Value, ApiError> {
    let mut request = http().post(format!("{API}{path}")).json(&body);
    if let Some(secret) = secret {
        request = request.header("Authorization", format!("Agent-Key {}", secret.trim()));
    }
    let response = request.send().await.map_err(|_| ApiError::Network)?;
    let json: Value = response.json().await.map_err(|_| ApiError::Network)?;
    let text = |v: &Value| v.as_str().map(String::from).unwrap_or_else(|| v.to_string());
    match json["status"].as_str() {
        Some("success") => Ok(json["data"].clone()),
        Some("fail") => Err(ApiError::Fail(match &json["data"] {
            // Algunos fallos vienen como {"error": "…", "details": …}.
            Value::Object(o) => o.get("error").map(text).unwrap_or_else(|| json["data"].to_string()),
            other => text(other),
        })),
        Some("error") => Err(ApiError::Error { kind: text(&json["data"]["type"]), message: text(&json["data"]["message"]) }),
        _ => Err(ApiError::Network),
    }
}

// ================= Vincular (claim)

/// Código de vinculación: 5 bytes al azar en hexadecimal, como el agente oficial.
pub fn claim_code() -> String {
    let mut buffer = [0u8; 5];
    getrandom::getrandom(&mut buffer).expect("números aleatorios");
    hex::encode(buffer)
}

pub fn claim_url(code: &str) -> String {
    format!("https://playit.gg/claim/{code}")
}

/// En qué va la vinculación.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClaimStep {
    /// Todavía no se abrió la página de vincular (p. ej. se quedó en iniciar sesión).
    Visit,
    /// La página está abierta: falta aceptar.
    Accept,
    Accepted,
}

impl ClaimStep {
    pub fn code(self) -> &'static str {
        match self {
            ClaimStep::Visit => "visit",
            ClaimStep::Accept => "accept",
            ClaimStep::Accepted => "finishing",
        }
    }
}

/// Paso de la vinculación. Ok(None): sin respuesta clara (se vuelve a preguntar).
pub async fn claim_poll(code: &str) -> Result<Option<ClaimStep>, String> {
    let body = json!({ "code": code, "agent_type": "self-managed", "version": format!("playit-cli {AGENT_VERSION}") });
    match call("/claim/setup", body, None).await {
        Ok(Value::String(status)) => match status.as_str() {
            "UserAccepted" => Ok(Some(ClaimStep::Accepted)),
            "UserRejected" => Err(t!("Se rechazó la vinculación en playit.gg.", "The link was rejected on playit.gg.")),
            "WaitingForUser" => Ok(Some(ClaimStep::Accept)),
            _ => Ok(Some(ClaimStep::Visit)),
        },
        Ok(_) => Ok(None),
        Err(ApiError::Fail(reason)) if reason == "CodeExpired" => {
            Err(t!("El código de vinculación caducó. Vuelve a pulsar «Conectar».", "The link code expired. Press «Connect» again."))
        }
        Err(ApiError::Network) => Ok(None),
        Err(error) => Err(error.explain()),
    }
}

/// Tras aceptar: la clave del agente.
pub async fn claim_exchange(code: &str) -> Result<Option<String>, String> {
    match call("/claim/exchange", json!({ "code": code }), None).await {
        Ok(data) => Ok(data["secret_key"].as_str().map(String::from)),
        Err(ApiError::Fail(reason)) if reason == "UserRejected" => Err(t!("Se rechazó la vinculación en playit.gg.", "The link was rejected on playit.gg.")),
        Err(ApiError::Fail(_)) | Err(ApiError::Network) => Ok(None),
        Err(error) => Err(error.explain()),
    }
}

// ================= Túnel

/// Lo que la API dice del agente y de su túnel.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunData {
    pub agent_id: String,
    /// "guest", "email-not-verified" o "verified".
    pub account: String,
    pub tunnel: Option<TunnelInfo>,
    /// El túnel se está creando (aún sin dirección).
    pub pending: bool,
    pub notices: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInfo {
    pub id: String,
    pub address: String,
    pub local_port: Option<u16>,
    pub disabled: Option<String>,
}

pub async fn run_data(secret: &str) -> Result<RunData, ApiError> {
    let data = call("/v1/agents/rundata", json!({}), Some(secret)).await?;
    let tunnel = data["tunnels"]
        .as_array()
        .into_iter()
        .flatten()
        // Solo por el nombre: el tipo puede venir escrito de otra forma y se crearían túneles repetidos.
        .find(|t| t["name"].as_str() == Some(TUNNEL_NAME))
        .map(|t| TunnelInfo {
            id: t["id"].as_str().unwrap_or_default().to_string(),
            address: t["display_address"].as_str().unwrap_or_default().to_string(),
            local_port: t["agent_config"]["fields"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|f| f["name"].as_str() == Some("local_port"))
                .and_then(|f| f["value"].as_str()?.parse().ok()),
            disabled: t["disabled_reason"].as_str().map(String::from),
        });
    Ok(RunData {
        agent_id: data["agent_id"].as_str().unwrap_or_default().to_string(),
        account: data["permissions"]["account_status"].as_str().unwrap_or("verified").to_string(),
        pending: tunnel.is_none() && data["pending"].as_array().is_some_and(|p| !p.is_empty()),
        tunnel,
        notices: data["notices"].as_array().into_iter().flatten().filter_map(|n| n["message"].as_str().map(String::from)).collect(),
    })
}

fn local_fields(port: u16) -> Value {
    json!({ "fields": [ { "name": "local_ip", "value": "127.0.0.1" }, { "name": "local_port", "value": port.to_string() } ] })
}

async fn create_tunnel(secret: &str, agent_id: &str, port: u16) -> Result<(), ApiError> {
    let body = json!({
        "name": TUNNEL_NAME,
        "ports": { "type": "tunnel-type", "details": "minecraft-java" },
        "origin": { "type": "agent", "data": { "agent_id": agent_id, "config": local_fields(port) } },
        "enabled": true,
        "alloc": null,
        "firewall_id": null,
    });
    match call("/v1/tunnels/create", body, Some(secret)).await {
        Err(error) if error.missing_path() => {
            // API anterior.
            let body = json!({
                "name": TUNNEL_NAME,
                "tunnel_type": "minecraft-java",
                "port_type": "tcp",
                "port_count": 1,
                "origin": { "type": "agent", "data": { "agent_id": agent_id, "local_ip": "127.0.0.1", "local_port": port } },
                "enabled": true,
                "alloc": null,
                "firewall_id": null,
                "proxy_protocol": null,
            });
            call("/tunnels/create", body, Some(secret)).await.map(|_| ())
        }
        other => other.map(|_| ()),
    }
}

async fn point_tunnel(secret: &str, agent_id: &str, tunnel_id: &str, port: u16) -> Result<(), ApiError> {
    let body = json!({ "tunnel_id": tunnel_id, "new_agent_id": null, "new_config": local_fields(port) });
    match call("/v1/tunnels/config", body, Some(secret)).await {
        Err(error) if error.missing_path() => {
            let body = json!({ "tunnel_id": tunnel_id, "local_ip": "127.0.0.1", "local_port": port, "agent_id": agent_id, "enabled": true });
            call("/tunnels/update", body, Some(secret)).await.map(|_| ())
        }
        other => other.map(|_| ()),
    }
}

/// Deja el túnel "parabellum" apuntando a 127.0.0.1:<puerto> (lo crea la primera vez) y devuelve
/// su dirección pública cuando ya está lista.
pub async fn ensure_tunnel(secret: &str, port: u16, progress: impl Fn(String)) -> Result<(String, RunData), ApiError> {
    let mut data = run_data(secret).await?;
    match &data.tunnel {
        None if !data.pending => {
            progress(t!("Creando tu dirección en playit.gg…", "Creating your address on playit.gg…"));
            create_tunnel(secret, &data.agent_id, port).await?;
        }
        Some(tunnel) if tunnel.local_port != Some(port) => {
            point_tunnel(secret, &data.agent_id, &tunnel.id, port).await?;
        }
        _ => {}
    }
    // Un túnel nuevo tarda unos segundos en tener dirección.
    for _ in 0..40 {
        if let Some(tunnel) = &data.tunnel {
            if let Some(reason) = &tunnel.disabled {
                return Err(ApiError::Fail(reason.clone()));
            }
            if !tunnel.address.is_empty() && tunnel.local_port == Some(port) {
                return Ok((tunnel.address.clone(), data));
            }
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
        data = run_data(secret).await?;
    }
    Err(ApiError::Fail(t!("el túnel no tuvo dirección a tiempo", "the tunnel did not get an address in time")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colores_fuera() {
        assert_eq!(strip_ansi("\u{1b}[32m INFO\u{1b}[0m tunnel running"), " INFO tunnel running");
    }

    #[test]
    fn otro_archivo_del_mismo_tamano_no_es_el_agente() {
        let Ok(agent) = agent() else { return };
        let paths = Paths::new(std::env::temp_dir().join(format!("parabellum-agente-{}", std::process::id())));
        std::fs::create_dir_all(agent_path(&paths).parent().unwrap()).unwrap();
        std::fs::write(agent_path(&paths), vec![0u8; agent.size as usize]).unwrap();
        let installed = agent_installed(&paths);
        let _ = std::fs::remove_dir_all(&paths.root);
        assert!(!installed);
    }

    #[test]
    fn ultima_linea_sin_trabarse() {
        let last_line = |lines: &[&str]| {
            // Cualquier proceso sirve de agente: aquí, esta misma prueba con --list (termina al momento).
            let child = Command::new(std::env::current_exe().unwrap()).arg("--list").stdout(Stdio::null()).spawn().unwrap();
            let agent = Agent { child, log: Arc::new(Mutex::new(lines.iter().map(|l| l.to_string()).collect())) };
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let _ = sender.send(agent.last_line());
                agent.stop();
            });
            receiver.recv_timeout(Duration::from_secs(5)).expect("last_line se trabó")
        };
        assert_eq!(last_line(&["INFO checking if secret key is valid", "WARN algo raro", "INFO otra"]).as_deref(), Some("WARN algo raro"));
        assert_eq!(last_line(&["INFO checking if secret key is valid", "Error: InvalidSecret"]).as_deref(), Some("Error: InvalidSecret"));
        assert_eq!(last_line(&[]), None);
    }

    #[test]
    fn codigo_de_vinculacion() {
        let code = claim_code();
        assert_eq!(code.len(), 10);
        assert!(code.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(claim_url("00aa11bb22"), "https://playit.gg/claim/00aa11bb22");
    }
}
