//! Firebase: la sesión sale del token de Google que ya tiene Parabellum (no hay que iniciar sesión dos
//! veces) y la Realtime Database se usa por su API REST, con escucha en tiempo real (Server-Sent
//! Events) que se vuelve a conectar sola si se corta o caduca el token.

use serde_json::{Map, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};

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
        None => "Firebase no respondió como se esperaba.".into(),
        Some(e) if e.starts_with("OPERATION_NOT_ALLOWED") => "En Firebase falta activar el acceso con Google.".into(),
        Some(e) if e.starts_with("INVALID_IDP_RESPONSE") || e.to_lowercase().contains("audience") => {
            format!("Firebase no acepta el inicio de sesión de Parabellum (falta su ID de cliente en la lista de permitidos). Detalle: {e}")
        }
        Some(e) if e.starts_with("API_KEY") => "La clave de API de Firebase no es válida.".into(),
        Some(e) if e.starts_with("USER_DISABLED") => "Esta cuenta está desactivada en Parabellum.".into(),
        Some(e) => format!("Firebase rechazó el inicio de sesión ({e})."),
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
            .map_err(|_| "No se pudo conectar con el servidor de amigos.".to_string())?;
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
            .map_err(|_| "No se pudo conectar con el servidor de amigos.".to_string())?;
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
            DbError::Permission(path) => write!(f, "Sin permiso para \"{path}\" (revisa las reglas de la base de datos)."),
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
}

impl Database {
    pub fn new(url: &str, session: Session) -> Self {
        Self { url: url.trim_end_matches('/').to_string(), session }
    }

    /// `path` puede llevar una consulta: "messages/x?orderBy=\"$key\"&limitToLast=100".
    async fn url(&self, path: &str) -> Result<String, String> {
        let (path, query) = path.split_once('?').map(|(p, q)| (p, format!("{q}&"))).unwrap_or((path, String::new()));
        Ok(format!("{}/{}.json?{query}auth={}", self.url, path.trim_matches('/'), self.session.token().await?))
    }

    async fn send(&self, method: reqwest::Method, path: &str, body: Option<&Value>) -> Result<Value, DbError> {
        let mut request = http().request(method, self.url(path).await?);
        if let Some(body) = body {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|_| DbError::Other("Sin conexión con el servidor de amigos.".into()))?;
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DbError::Permission(path.to_string()));
        }
        if !status.is_success() {
            let detail = serde_json::from_str::<Value>(&text).ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or(text);
            return Err(DbError::Other(format!("La base de datos respondió {}: {detail}", status.as_u16())));
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
        let task = tauri::async_runtime::spawn(async move {
            let mut delay = Duration::from_secs(2);
            loop {
                let mut data = Value::Null;
                if let Err(error) = db.stream(&path, &mut data, &on_change, &mut delay).await {
                    on_error(error);
                }
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(60));
            }
        });
        Listener(task)
    }

    async fn stream(&self, path: &str, data: &mut Value, on_change: &(impl Fn(Value) + Send + Sync), delay: &mut Duration) -> Result<(), String> {
        // El servidor manda un keep-alive cada 30 s: sin nada en 75 s, la conexión se dio por perdida.
        const IDLE: Duration = Duration::from_secs(75);
        let response = stream_http()
            .get(self.url(path).await?)
            .header("Accept", "text/event-stream")
            .send()
            .await
            .map_err(|_| "Sin conexión con el servidor de amigos. Se reintentará solo.".to_string())?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(DbError::Permission(path.to_string()).to_string());
        }
        let mut response = response.error_for_status().map_err(|e| e.to_string())?;
        let mut buffer = String::new();
        let mut event = String::new();
        loop {
            let chunk = match tokio::time::timeout(IDLE, response.chunk()).await {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) | Err(_) => return Ok(()),
                Ok(Err(e)) => return Err(e.to_string()),
            };
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find('\n') {
                let line: String = buffer.drain(..=end).collect();
                let line = line.trim_end();
                if let Some(name) = line.strip_prefix("event:") {
                    event = name.trim().to_string();
                } else if let Some(payload) = line.strip_prefix("data:") {
                    if event == "put" || event == "patch" {
                        if let Ok(message) = serde_json::from_str::<Value>(payload.trim()) {
                            apply(data, event == "patch", &message);
                            on_change(data.clone());
                        }
                    }
                    *delay = Duration::from_secs(2);
                }
                // auth_revoked: el token caducó; cancel: se perdió el permiso. Se vuelve a conectar.
                if event == "auth_revoked" || event == "cancel" {
                    return Ok(());
                }
            }
        }
    }
}

/// Una escucha activa; se detiene al soltarla.
pub struct Listener(pub tauri::async_runtime::JoinHandle<()>);

impl Drop for Listener {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// Aplica un evento {path, data} de la escucha a la copia local.
fn apply(root: &mut Value, merge: bool, message: &Value) {
    let segments: Vec<&str> = message["path"].as_str().unwrap_or("/").split('/').filter(|s| !s.is_empty()).collect();
    let value = &message["data"];
    match (merge, value) {
        (true, Value::Object(children)) => {
            for (key, child) in children {
                let mut path = segments.clone();
                path.push(key);
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
        apply(&mut data, false, &serde_json::json!({ "path": "/", "data": null }));
        assert_eq!(data, Value::Null);
    }
}
