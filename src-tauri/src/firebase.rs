//! Firebase: la sesión sale del token de Google que ya tiene Parabellum (no hay que iniciar sesión dos
//! veces) y la Realtime Database se usa por su API REST, con escucha en tiempo real (Server-Sent
//! Events) que se vuelve a conectar sola si se corta o caduca el token.

use serde_json::{Map, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Cuánto se usa Firebase, para medirlo antes de cambiar cómo se conecta (mejora 5 de
/// docs/plan-diciembre.md). Ver `metrics` y `cuenta::startup`, que lo anota cada minuto.
pub mod metrics {
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Escuchas activas (una por rama: amigos, cada amigo, cada conversación…).
    pub static LISTENERS: AtomicU64 = AtomicU64::new(0);
    /// Conexiones abiertas ahora.
    pub static OPEN: AtomicU64 = AtomicU64::new(0);
    /// Conexiones abiertas desde que arrancó (las de más son reconexiones).
    pub static CONNECTS: AtomicU64 = AtomicU64::new(0);
    /// Cambios que llegaron (put/patch) y bytes de las escuchas.
    pub static EVENTS: AtomicU64 = AtomicU64::new(0);
    pub static STREAM_BYTES: AtomicU64 = AtomicU64::new(0);
    /// Lecturas y escrituras sueltas y sus bytes.
    pub static REQUESTS: AtomicU64 = AtomicU64::new(0);
    pub static REQUEST_BYTES: AtomicU64 = AtomicU64::new(0);

    pub fn add(counter: &AtomicU64, amount: u64) {
        counter.fetch_add(amount, Ordering::Relaxed);
    }

    pub fn sub(counter: &AtomicU64) {
        counter.fetch_sub(1, Ordering::Relaxed);
    }

    /// Una línea del CSV: escuchas, abiertas, conexiones, eventos, bytes de escuchas, peticiones, bytes.
    pub fn line() -> String {
        let get = |c: &AtomicU64| c.load(Ordering::Relaxed);
        format!(
            "{},{},{},{},{},{},{}",
            get(&LISTENERS),
            get(&OPEN),
            get(&CONNECTS),
            get(&EVENTS),
            get(&STREAM_BYTES),
            get(&REQUESTS),
            get(&REQUEST_BYTES)
        )
    }
}

const SIGN_IN_URL: &str = "https://identitytoolkit.googleapis.com/v1/accounts:signInWithIdp?key=";
const REFRESH_URL: &str = "https://securetoken.googleapis.com/v1/token?key=";

fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(30)).build().expect("cliente HTTP"))
}

/// Las escuchas mantienen la conexión abierta indefinidamente (sin tiempo total).
fn stream_http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| reqwest::Client::builder().connect_timeout(Duration::from_secs(20)).build().expect("cliente HTTP"))
}

fn explain(error: Option<&str>) -> String {
    match error {
        None => t!("Firebase no respondió como se esperaba.", "Firebase did not answer as expected."),
        Some(e) if e.starts_with("OPERATION_NOT_ALLOWED") => t!("En Firebase falta activar el acceso con Google.", "Google sign-in is not enabled in Firebase."),
        Some(e) if e.starts_with("INVALID_IDP_RESPONSE") || e.to_lowercase().contains("audience") => {
            t!("Firebase no acepta el inicio de sesión de Parabellum (falta su ID de cliente en la lista de permitidos). Detalle: {e}", "Firebase does not accept Parabellum's sign-in (its client ID is missing from the allowed list). Detail: {e}")
        }
        Some(e) if e.starts_with("API_KEY") => t!("La clave de API de Firebase no es válida.", "The Firebase API key is not valid."),
        Some(e) if e.starts_with("USER_DISABLED") => t!("Esta cuenta está desactivada en Parabellum.", "This account is disabled in Parabellum."),
        Some(e) => t!("Firebase rechazó el inicio de sesión ({e}).", "Firebase rejected the sign-in ({e})."),
    }
}

struct Token {
    id: String,
    refresh: String,
    expires: Instant,
}

pub struct Session {
    api_key: String,
    /// Identificador estable de la cuenta (uno por cuenta de Google).
    pub uid: String,
    token: tokio::sync::Mutex<Token>,
}

impl Session {
    pub async fn sign_in_with_google(api_key: &str, google_id_token: &str) -> Result<Self, String> {
        let body = serde_json::json!({
            "postBody": format!("id_token={google_id_token}&providerId=google.com"),
            "requestUri": "http://localhost",
            "returnSecureToken": true,
            "returnIdpCredential": true,
        });
        let response = http()
            .post(format!("{SIGN_IN_URL}{api_key}"))
            .json(&body)
            .send()
            .await
            .map_err(|_| t!("No se pudo conectar con el servidor de amigos.", "Could not connect to the friends server."))?;
        let ok = response.status().is_success();
        let json: Value = response.json().await.unwrap_or_default();
        if !ok {
            return Err(explain(json.pointer("/error/message").and_then(Value::as_str)));
        }
        let text = |key: &str| json[key].as_str().map(String::from).ok_or_else(|| explain(None));
        let seconds = json["expiresIn"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(3600);
        Ok(Self {
            api_key: api_key.to_string(),
            uid: text("localId")?,
            token: tokio::sync::Mutex::new(Token {
                id: text("idToken")?,
                refresh: text("refreshToken")?,
                expires: Instant::now() + Duration::from_secs(seconds.saturating_sub(120)),
            }),
        })
    }

    /// Hasta cuándo sirve el token que se tiene ahora (después, `token()` pide otro).
    pub async fn valid_until(&self) -> Instant {
        self.token.lock().await.expires
    }

    /// Token vigente para la base de datos; se renueva solo antes de caducar (dura una hora).
    pub async fn token(&self) -> Result<String, String> {
        let mut token = self.token.lock().await;
        if Instant::now() < token.expires {
            return Ok(token.id.clone());
        }
        let response = http()
            .post(format!("{REFRESH_URL}{}", self.api_key))
            .form(&[("grant_type", "refresh_token"), ("refresh_token", token.refresh.as_str())])
            .send()
            .await
            .map_err(|_| t!("No se pudo conectar con el servidor de amigos.", "Could not connect to the friends server."))?;
        let ok = response.status().is_success();
        let json: Value = response.json().await.unwrap_or_default();
        if !ok {
            return Err(explain(json.pointer("/error/message").and_then(Value::as_str)));
        }
        token.id = json["id_token"].as_str().ok_or_else(|| explain(None))?.to_string();
        if let Some(refresh) = json["refresh_token"].as_str() {
            token.refresh = refresh.to_string();
        }
        let seconds = json["expires_in"].as_str().and_then(|s| s.parse::<u64>().ok()).unwrap_or(3600);
        token.expires = Instant::now() + Duration::from_secs(seconds.saturating_sub(120));
        Ok(token.id.clone())
    }
}

#[derive(Debug)]
pub enum DbError {
    /// Las reglas de la base de datos no lo permiten (p. ej. un nombre que ya es de otro).
    Permission(String),
    Other(String),
}

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbError::Permission(path) => f.write_str(&t!(
                "Sin permiso para \"{path}\" (revisa las reglas de la base de datos).",
                "No permission for \"{path}\" (check the database rules)."
            )),
            DbError::Other(message) => f.write_str(message),
        }
    }
}

impl From<String> for DbError {
    fn from(message: String) -> Self {
        DbError::Other(message)
    }
}

/// Marcador que el servidor sustituye por su hora (milisegundos Unix).
pub fn server_timestamp() -> Value {
    serde_json::json!({ ".sv": "timestamp" })
}

pub struct Database {
    url: String,
    pub session: Session,
    /// Diferencia entre el reloj del servidor y el de este PC (ms). Los relojes de los PC pueden ir
    /// minutos adelantados o atrasados: lo que se ordena por hora usa la del servidor.
    server_offset_ms: std::sync::atomic::AtomicI64,
}

impl Database {
    pub fn new(url: &str, session: Session) -> Self {
        Self { url: url.trim_end_matches('/').to_string(), session, server_offset_ms: Default::default() }
    }

    pub fn set_server_offset(&self, offset_ms: i64) {
        self.server_offset_ms.store(offset_ms, std::sync::atomic::Ordering::Relaxed);
    }

    /// La hora del servidor (milisegundos Unix), calculada con el reloj de este PC corregido.
    pub fn server_now(&self) -> i64 {
        chrono::Utc::now().timestamp_millis() + self.server_offset_ms.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// `path` puede llevar una consulta: "messages/x?orderBy=\"$key\"&limitToLast=100".
    async fn url(&self, path: &str) -> Result<String, String> {
        let (path, query) = path.split_once('?').map(|(p, q)| (p, format!("{q}&"))).unwrap_or((path, String::new()));
        // La del emulador lleva su espacio («?ns=…»); la de verdad, nada.
        let (base, fixed) = self.url.split_once('?').map(|(b, q)| (b, format!("{q}&"))).unwrap_or((self.url.as_str(), String::new()));
        Ok(format!("{}/{}.json?{fixed}{query}auth={}", base, path.trim_matches('/'), self.session.token().await?))
    }

    async fn send(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Result<Value, DbError> {
        let mut request = http().request(method, self.url(path).await?);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|_| DbError::Other(t!("Sin conexión con el servidor de amigos.", "No connection with the friends server.")))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        metrics::add(&metrics::REQUESTS, 1);
        metrics::add(&metrics::REQUEST_BYTES, text.len() as u64);
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DbError::Permission(path.to_string()));
        }
        if matches!(status.as_u16(), 402 | 429 | 500..=599) {
            return Err(DbError::Other(status_message(status)));
        }
        if !status.is_success() {
            let detail = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or(text);
            return Err(DbError::Other(t!("La base de datos respondió {}: {detail}", "The database answered {}: {detail}", status.as_u16())));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    pub async fn get(&self, path: &str) -> Result<Value, DbError> {
        self.send(reqwest::Method::GET, path, None).await
    }

    pub async fn put(&self, path: &str, value: &Value) -> Result<Value, DbError> {
        self.send(reqwest::Method::PUT, path, Some(value)).await
    }

    /// Escritura de varias rutas a la vez (claves tipo "a/b/c"; null borra). Es atómica.
    pub async fn patch(&self, path: &str, values: &Value) -> Result<Value, DbError> {
        self.send(reqwest::Method::PATCH, path, Some(values)).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), DbError> {
        self.send(reqwest::Method::DELETE, path, None).await.map(|_| ())
    }

    /// Escucha una rama: `on_change` recibe su contenido completo cada vez que cambia (Null si está
    /// vacía) y `on_error` los problemas de conexión. Se detiene al soltar el Listener.
    pub fn listen(
        self: &Arc<Self>,
        path: String,
        on_change: impl Fn(Value) + Send + Sync + 'static,
        on_error: impl Fn(String) + Send + Sync + 'static,
    ) -> Listener {
        let db = self.clone();
        metrics::add(&metrics::LISTENERS, 1);
        let task = tauri::async_runtime::spawn(async move {
            let mut delay = Duration::from_secs(1);
            loop {
                let mut data = Value::Null;
                match db.stream(&path, &mut data, &on_change).await {
                    // Terminó bien (token por caducar, conexión muda o el servidor la cerró): se
                    // vuelve a conectar enseguida, sin esperas que dejen la lista desactualizada.
                    Ok(()) => {
                        delay = Duration::from_secs(1);
                        tokio::time::sleep(Duration::from_millis(200)).await;
                    }
                    Err(error) => {
                        on_error(error);
                        tokio::time::sleep(delay).await;
                        delay = (delay * 2).min(Duration::from_secs(20));
                    }
                }
            }
        });
        Listener { task, counted: true }
    }

    async fn stream(&self, path: &str, data: &mut Value, on_change: &(impl Fn(Value) + Send + Sync)) -> Result<(), String> {
        // El servidor manda un keep-alive cada 30 s: sin nada en 45 s, la conexión se dio por perdida
        // (p. ej. tras suspender el PC o cambiar de red).
        const IDLE: Duration = Duration::from_secs(45);
        let url = self.url(path).await?;
        // Antes de que caduque el token con el que se abrió, se vuelve a abrir con uno nuevo (si se
        // espera a que Firebase lo corte, hay un rato en que no llega nada).
        let rotate_at = self.session.valid_until().await + Duration::from_secs(1);
        let response = stream_http()
            .get(url)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(|_| t!("Sin conexión con el servidor de amigos. Se reintentará solo.", "No connection with the friends server. It will retry on its own."))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DbError::Permission(path.to_string()).to_string());
        }
        // Sin la dirección en el mensaje: lleva la clave de la sesión (se veía en pantalla).
        if !response.status().is_success() {
            return Err(status_message(response.status()));
        }
        metrics::add(&metrics::CONNECTS, 1);
        metrics::add(&metrics::OPEN, 1);
        // Al salir (por donde sea) deja de contar como abierta.
        struct Open;
        impl Drop for Open {
            fn drop(&mut self) {
                metrics::sub(&metrics::OPEN);
            }
        }
        let _open = Open;
        let mut response = response;
        // Bytes y no texto: una letra con tilde o un emoji puede quedar partida entre dos trozos.
        let mut buffer: Vec<u8> = Vec::new();
        let mut event = String::new();
        loop {
            let left = rotate_at.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(());
            }
            let chunk = match tokio::time::timeout(IDLE.min(left), response.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) | Err(_) => return Ok(()),
                Ok(Err(e)) => return Err(e.without_url().to_string()),
            };
            metrics::add(&metrics::STREAM_BYTES, chunk.len() as u64);
            buffer.extend_from_slice(&chunk);
            while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
                let bytes: Vec<u8> = buffer.drain(..=end).collect();
                let line = String::from_utf8_lossy(&bytes);
                let line = line.trim_end();
                if let Some(name) = line.strip_prefix("event:") {
                    event = name.trim().to_string();
                } else if let Some(payload) = line.strip_prefix("data:") {
                    if event == "put" || event == "patch" {
                        if let Ok(message) = serde_json::from_str::<Value>(payload.trim()) {
                            metrics::add(&metrics::EVENTS, 1);
                            apply(data, event == "patch", &message);
                            on_change(data.clone());
                        }
                    }
                }
                // auth_revoked: el token caducó; cancel: se perdió el permiso. Se vuelve a conectar.
                if event == "auth_revoked" || event == "cancel" {
                    return Ok(());
                }
            }
        }
    }
}

/// Lo que se le dice al jugador cuando la base responde con un error (nunca la dirección: lleva la
/// clave de la sesión). 402: el plan de Firebase llegó a su límite (p. ej. de conexiones a la vez).
fn status_message(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        402 | 429 => t!("El servidor de amigos está lleno o llegó a su límite. Se reintentará solo.", "The friends server is full or reached its limit. It will retry on its own."),
        code @ 500..=599 => t!("El servidor de amigos tiene problemas ({code}). Se reintentará solo.", "The friends server is having problems ({code}). It will retry on its own."),
        code => t!("El servidor de amigos respondió {code}.", "The friends server answered {code}."),
    }
}

/// Una escucha activa; se detiene al soltarla.
pub struct Listener {
    task: tauri::async_runtime::JoinHandle<()>,
    /// Es una escucha de Firebase (cuenta en metrics::LISTENERS); si no, otra tarea que se para igual.
    counted: bool,
}

impl Listener {
    /// Una tarea cualquiera que se detiene al soltarla (no es una escucha de Firebase).
    pub fn task(task: tauri::async_runtime::JoinHandle<()>) -> Self {
        Self { task, counted: false }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.task.abort();
        if self.counted {
            metrics::sub(&metrics::LISTENERS);
        }
    }
}

/// Aplica un evento {path, data} de la escucha a la copia local.
fn apply(root: &mut Value, merge: bool, message: &Value) {
    let segments: Vec<&str> = message["path"].as_str().unwrap_or("/").split('/').filter(|s| !s.is_empty()).collect();
    let value = &message["data"];
    match (merge, value) {
        // Las claves de un patch pueden ser rutas: al escribir varias rutas a la vez («meta/lastText»,
        // «meta/lastAt»…) Firebase manda {"meta/lastText": …} y no {"meta": {…}}.
        (true, Value::Object(children)) => {
            for (key, child) in children {
                let mut path = segments.clone();
                path.extend(key.split('/').filter(|s| !s.is_empty()));
                set_at(root, &path, child.clone());
            }
        }
        _ => set_at(root, &segments, value.clone()),
    }
}

fn set_at(root: &mut Value, segments: &[&str], value: Value) {
    let Some((last, parents)) = segments.split_last() else {
        *root = value;
        return;
    };
    let mut current = root;
    for segment in parents {
        if !current.is_object() {
            *current = Value::Object(Map::new());
        }
        current = current.as_object_mut().unwrap().entry(segment.to_string()).or_insert(Value::Object(Map::new()));
    }
    if !current.is_object() {
        *current = Value::Object(Map::new());
    }
    let object = current.as_object_mut().unwrap();
    if value.is_null() {
        object.remove(*last);
    } else {
        object.insert(last.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aplica_los_eventos_de_la_escucha() {
        let mut data = Value::Null;
        apply(&mut data, false, &serde_json::json!({ "path": "/", "data": { "a": true, "b": true } }));
        apply(&mut data, false, &serde_json::json!({ "path": "/c", "data": true }));
        apply(&mut data, false, &serde_json::json!({ "path": "/a", "data": null }));
        apply(&mut data, true, &serde_json::json!({ "path": "/", "data": { "d": { "name": "x" }, "b": null } }));
        assert_eq!(data, serde_json::json!({ "c": true, "d": { "name": "x" } }));
        apply(&mut data, true, &serde_json::json!({ "path": "/", "data": { "d/name": "y", "e/f/g": 1, "c": null } }));
        assert_eq!(data, serde_json::json!({ "d": { "name": "y" }, "e": { "f": { "g": 1 } } }));
        apply(&mut data, false, &serde_json::json!({ "path": "/", "data": null }));
        assert_eq!(data, Value::Null);
    }
}
