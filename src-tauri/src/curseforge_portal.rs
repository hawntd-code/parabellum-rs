//! Proxy CONNECT de una sola ventana. Conserva el TLS entre WebView y CurseForge; el servicio
//! remoto transporta bytes cifrados y solo permite los hosts del portal, nunca la API ni otras webs.
use std::{path::PathBuf, time::Duration};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{TcpListener, TcpStream}, sync::{watch, Semaphore}};
use std::sync::Arc;

pub const ENDPOINT: &str = "https://curseforge-portal-86460886589.us-central1.run.app";
const HOSTS: &[&str] = &["curseforge.com", "www.curseforge.com", "static.curseforge.com", "static-beta.curseforge.com", "media.forgecdn.net",
    "mediafilez.forgecdn.net", "edge.forgecdn.net", "static.forgecdn.net", "static-assets.overwolf.com", "challenges.cloudflare.com"];

pub struct PortalProxy {
    pub url: reqwest::Url,
    stop: watch::Sender<bool>,
}
impl PortalProxy { pub fn close(&self) { let _ = self.stop.send(true); } }
impl Drop for PortalProxy { fn drop(&mut self) { self.close(); } }

fn control(client: &reqwest::Client, action: &str, key: &str) -> reqwest::RequestBuilder {
    // El frontal de Cloud Run responde 411 a POST vacíos sin longitud, aunque HTTP/2 la omita.
    client.post(format!("{ENDPOINT}/{action}")).header("X-Portal-Session", key)
        .header(reqwest::header::CONTENT_LENGTH, "0").body(Vec::<u8>::new())
}

struct RemoteSession { client: reqwest::Client, key: String, closed: bool }
impl RemoteSession {
    async fn close(&mut self) {
        self.closed = control(&self.client, "close", &self.key).timeout(Duration::from_secs(5)).send().await
            .is_ok_and(|r| r.status().is_success() || r.status() == reqwest::StatusCode::GONE);
    }
}
impl Drop for RemoteSession {
    fn drop(&mut self) {
        if self.closed { return; }
        let (client, key) = (self.client.clone(), self.key.clone());
        tauri::async_runtime::spawn(async move {
            let _ = control(&client, "close", &key).send().await;
        });
    }
}

fn host(request: &str) -> Option<&str> {
    let mut parts = request.lines().next()?.split_whitespace();
    if parts.next()? != "CONNECT" { return None; }
    let target = parts.next()?.strip_suffix(":443")?;
    if !HOSTS.contains(&target) || !matches!(parts.next()?, "HTTP/1.1" | "HTTP/1.0") || parts.next().is_some() { return None; }
    Some(target)
}

fn log(path: &std::path::Path, host: &str, event: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    if path.metadata().is_ok_and(|m| m.len() > 1_000_000) { let _ = std::fs::remove_file(path); }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} host={host} estado={event}", chrono::Utc::now().to_rfc3339());
    }
}

pub async fn start(log_path: PathBuf) -> Result<PortalProxy, String> {
    if ENDPOINT.is_empty() { return Err(t!("La conexión alternativa todavía no está disponible.", "The alternative connection is not available yet.")); }
    let client = reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(40)).build().map_err(|e| e.to_string())?;
    let health = client.get(format!("{ENDPOINT}/health")).send().await.map_err(|_| t!("No se pudo conectar con el portal alternativo. Conservamos tus archivos pendientes.", "Could not connect to the alternative portal. Your pending files have been kept."))?;
    let info: serde_json::Value = health.error_for_status().map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    if info["protocol"] != 1 || info["service"] != "portal-curseforge" { return Err(t!("Respuesta inesperada del portal alternativo.", "Unexpected response from the alternative portal.")); }
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.map_err(|e| e.to_string())?;
    let url = reqwest::Url::parse(&format!("http://{}", listener.local_addr().map_err(|e| e.to_string())?)).map_err(|e| e.to_string())?;
    let (stop, mut closed) = watch::channel(false);
    tauri::async_runtime::spawn(async move {
        let limit = Arc::new(Semaphore::new(6));
        loop {
            tokio::select! {
                _ = closed.changed() => break,
                accepted = listener.accept() => {
                    let Ok((mut stream, _)) = accepted else { break };
                    let Ok(permit) = limit.clone().try_acquire_owned() else {
                        let _ = stream.write_all(b"HTTP/1.1 503 Busy\r\nConnection: close\r\n\r\n").await;
                        continue;
                    };
                    let (client, mut closed, path) = (client.clone(), closed.clone(), log_path.clone());
                    tauri::async_runtime::spawn(async move {
                        let _permit = permit;
                        tokio::select! {
                            _ = closed.changed() => (),
                            _ = tokio::time::timeout(Duration::from_secs(900), tunnel(stream, client, path)) => (),
                        }
                    });
                }
            }
        }
    });
    Ok(PortalProxy { url, stop })
}

async fn tunnel(mut stream: TcpStream, client: reqwest::Client, path: PathBuf) -> Result<(), String> {
    let request = tokio::time::timeout(Duration::from_secs(10), async {
        let mut header = Vec::new();
        while header.len() < 8192 {
            header.push(stream.read_u8().await.map_err(|e| e.to_string())?);
            if header.ends_with(b"\r\n\r\n") { return String::from_utf8(header).map_err(|e| e.to_string()); }
        }
        Err("Cabecera demasiado grande".into())
    }).await.map_err(|e| e.to_string())??;
    let Some(host) = host(&request) else {
        let _ = stream.write_all(b"HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n").await;
        return Err("Destino no permitido".into());
    };
    let opened = client.post(format!("{ENDPOINT}/open")).json(&serde_json::json!({"host": host})).send().await;
    let opened = match opened.and_then(reqwest::Response::error_for_status) {
        Ok(response) => response,
        Err(_) => { log(&path, host, "conexion-fallida"); let _ = stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nConnection: close\r\n\r\n").await; return Err("Conexión fallida".into()); }
    };
    let session: serde_json::Value = opened.json().await.map_err(|e| e.to_string())?;
    let key = session["session"].as_str().filter(|s| s.len() < 100).ok_or("Respuesta inválida")?;
    let mut remote = RemoteSession { client: client.clone(), key: key.into(), closed: false };
    stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n").await.map_err(|e| e.to_string())?;
    log(&path, host, "conectado");
    let (mut reader, mut writer) = stream.into_split();
    let send = async {
        let mut bytes = vec![0; 65536];
        loop {
            let size = reader.read(&mut bytes).await.map_err(|e| e.to_string())?;
            if size == 0 { return Ok::<_, String>(()); }
            client.post(format!("{ENDPOINT}/send")).header("X-Portal-Session", key).body(bytes[..size].to_vec())
                .send().await.and_then(reqwest::Response::error_for_status).map_err(|e| e.to_string())?;
        }
    };
    let receive = async {
        loop {
            let response = control(&client, "receive", key).send().await.map_err(|e| e.to_string())?;
            if response.status() == reqwest::StatusCode::GONE { return Ok::<_, String>(()); }
            let bytes = response.error_for_status().map_err(|e| e.to_string())?.bytes().await.map_err(|e| e.to_string())?;
            if bytes.len() > 65536 { return Err("Respuesta demasiado grande".into()); }
            writer.write_all(&bytes).await.map_err(|e| e.to_string())?;
        }
    };
    let result = tokio::select! { result = send => result, result = receive => result };
    // Espera el cierre antes de devolver el diagnóstico: al terminar un CLI, las tareas en segundo
    // plano se cancelan. El Drop sigue cubriendo el cierre de ventana y la cancelación del túnel.
    remote.close().await;
    log(&path, host, if result.is_ok() { "cerrado" } else { "interrumpido" });
    result
}

/// Diagnóstico independiente del WebView. Registra host, tipo y estado, nunca cookies, rutas
/// completas ni parámetros. Sirve para comparar HTML y recursos cuando una red entrega bloqueos.
pub async fn diagnose(page: &str, log_path: PathBuf, alternative: bool) -> Result<serde_json::Value, String> {
    let page = reqwest::Url::parse(page).map_err(|e| e.to_string())?;
    if page.scheme() != "https" || !matches!(page.host_str(), Some("www.curseforge.com" | "curseforge.com")) || page.port_or_known_default() != Some(443) {
        return Err("Solo se diagnostica el portal oficial".into());
    }
    let proxy = if alternative { Some(start(log_path.clone()).await?) } else { None };
    let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(60))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/142.0.0.0 Safari/537.36")
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 5 && attempt.url().scheme() == "https" && attempt.url().host_str().is_some_and(|h| HOSTS.contains(&h)) { attempt.follow() } else { attempt.stop() }
        }));
    if let Some(proxy) = &proxy { builder = builder.proxy(reqwest::Proxy::all(proxy.url.as_str()).map_err(|e| e.to_string())?); }
    let client = builder.build().map_err(|e| e.to_string())?;
    let mut response = client.get(page.clone()).send().await.map_err(|e| e.to_string())?;
    let mut results = vec![serde_json::json!({"host":page.host_str(),"kind":"html","status":response.status().as_u16(),"type":response.headers().get("content-type").and_then(|h|h.to_str().ok())})];
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if body.len() + chunk.len() > 4_000_000 { return Err("HTML demasiado grande".into()); }
        body.extend_from_slice(&chunk);
    }
    let html = String::from_utf8_lossy(&body);
    let expression = regex::Regex::new(r#"(?:href|src)=["']([^"']+)["']"#).unwrap();
    let mut seen = std::collections::HashSet::new();
    let resources: Vec<_> = expression.captures_iter(&html).filter_map(|c| page.join(&c[1]).ok())
        .filter(|u| u.scheme() == "https" && u.host_str().is_some_and(|h| HOSTS.contains(&h)) && (u.path().ends_with(".css") || u.path().ends_with(".js")))
        .filter(|u| seen.insert(u.to_string())).take(8).collect();
    for url in resources {
        let kind = if url.path().ends_with(".css") { "css" } else { "javascript" };
        let item = match client.get(url.clone()).send().await {
            Ok(response) => serde_json::json!({"host":url.host_str(),"kind":kind,"status":response.status().as_u16(),"type":response.headers().get("content-type").and_then(|h|h.to_str().ok())}),
            Err(_) => serde_json::json!({"host":url.host_str(),"kind":kind,"error":"transporte"}),
        };
        results.push(item);
    }
    for result in &results { log(&log_path, result["host"].as_str().unwrap_or_default(), &result.to_string()); }
    Ok(serde_json::json!({"alternative":alternative,"resources":results}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controles_vacios_llevan_longitud_explicita_para_cloud_run() {
        for action in ["receive", "close"] {
            let request = control(&reqwest::Client::new(), action, "sesion-de-prueba").build().unwrap();
            assert_eq!(request.headers()[reqwest::header::CONTENT_LENGTH], "0");
            assert_eq!(request.method(), reqwest::Method::POST);
        }
    }
    #[test]
    fn solo_connect_a_hosts_del_portal_y_puerto_443() {
        assert_eq!(host("CONNECT www.curseforge.com:443 HTTP/1.1\r\nHost: www.curseforge.com\r\n\r\n"), Some("www.curseforge.com"));
        assert_eq!(host("CONNECT static-beta.curseforge.com:443 HTTP/1.1\r\n\r\n"), Some("static-beta.curseforge.com"));
        for target in ["127.0.0.1:443", "api.curseforge.com:443", "www.curseforge.com.ejemplo.com:443", "www.curseforge.com:80", "user@www.curseforge.com:443"] {
            assert_eq!(host(&format!("CONNECT {target} HTTP/1.1\r\n\r\n")), None);
        }
        assert!(host("GET https://www.curseforge.com HTTP/1.1\r\n\r\n").is_none());
    }
}
