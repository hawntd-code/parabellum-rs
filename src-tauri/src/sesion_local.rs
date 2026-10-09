//! Servidor de sesión local: deja jugar con cuentas no premium en mundos abiertos (LAN o e4mc) sin
//! desactivar el "online mode" y sin el error "Invalid session".
//!
//! Minecraft (authlib, desde 1.16) deja cambiar los servidores de Mojang con propiedades de Java
//! (minecraft.api.session.host…). Parabellum los apunta a este servidor en 127.0.0.1, que:
//!   - responde "sí" a join y hasJoined, justo la comprobación que falla con "Invalid session";
//!   - reenvía todo lo demás (skins, claves, perfiles) a los servidores reales de Mojang.
//! Equivale a un servidor en modo offline: cada jugador entra con el UUID que sale de su nombre.
//! Lo mismo que hacía la versión C# (probado con usuarios reales).

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::settings;

/// Siempre el mismo puerto si está libre: un juego abierto sigue funcionando si Parabellum se reinicia.
const PREFERRED_PORT: u16 = 47815;
const UPSTREAMS: [(&str, &str); 5] = [
    ("session", "https://sessionserver.mojang.com"),
    ("services", "https://api.minecraftservices.com"),
    ("profiles", "https://api.mojang.com"),
    ("account", "https://api.mojang.com"),
    ("auth", "https://authserver.mojang.com"),
];

/// Propiedades de Java que hacen que el juego use este servidor.
pub fn jvm_arguments(port: u16) -> Vec<String> {
    std::iter::once("-Dminecraft.api.env=custom".to_string())
        .chain(UPSTREAMS.iter().map(|(kind, _)| format!("-Dminecraft.api.{kind}.host=http://127.0.0.1:{port}/{kind}")))
        .collect()
}

/// Abre el servidor (en segundo plano) y devuelve su puerto.
pub async fn start(log: PathBuf) -> std::io::Result<u16> {
    let listener = match TcpListener::bind(("127.0.0.1", PREFERRED_PORT)).await {
        Ok(listener) => listener,
        Err(_) => TcpListener::bind(("127.0.0.1", 0)).await?,
    };
    let port = listener.local_addr()?.port();
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(std::io::Error::other)?;
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (client, log) = (client.clone(), log.clone());
            tokio::spawn(async move {
                let line = match tokio::time::timeout(Duration::from_secs(30), handle(stream, &client)).await {
                    Ok(Ok(line)) => line,
                    Ok(Err(error)) => format!("error: {error}"),
                    Err(_) => "error: tiempo agotado".into(),
                };
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log) {
                    let _ = writeln!(file, "[{}] {line}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"));
                }
            });
        }
    });
    Ok(port)
}

struct Request {
    method: String,
    target: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn handle(mut stream: TcpStream, client: &reqwest::Client) -> std::io::Result<String> {
    let Some(request) = read_request(&mut stream).await? else { return Ok("conexión vacía".into()) };
    let (status, content_type, body) = route(&request, client).await;
    let summary = format!("{} {} -> {status}", request.method, request.target);
    let mut head = format!("HTTP/1.1 {status} {}\r\n", if status < 300 { "OK" } else { "Error" });
    if let Some(kind) = content_type {
        head.push_str(&format!("Content-Type: {kind}\r\n"));
    }
    head.push_str(&format!("Content-Length: {}\r\nConnection: close\r\n\r\n", body.len()));
    stream.write_all(head.as_bytes()).await?;
    stream.write_all(&body).await?;
    Ok(summary)
}

async fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<Request>> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            return Ok(None);
        }
        data.extend_from_slice(&chunk[..read]);
        if let Some(position) = data.windows(4).position(|w| w == b"\r\n\r\n") {
            break position;
        }
        if data.len() > 64 * 1024 {
            return Ok(None);
        }
    };
    let head = String::from_utf8_lossy(&data[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next().unwrap_or_default().split(' ');
    let (Some(method), Some(target)) = (first.next(), first.next()) else { return Ok(None) };
    let headers: HashMap<String, String> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let length = headers.get("content-length").and_then(|v| v.parse::<usize>().ok()).unwrap_or(0).min(1 << 20);
    let mut body = data[header_end + 4..].to_vec();
    while body.len() < length {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(Some(Request { method: method.to_string(), target: target.to_string(), headers, body }))
}

/// (estado, tipo de contenido, cuerpo)
async fn route(request: &Request, client: &reqwest::Client) -> (u16, Option<String>, Vec<u8>) {
    let path = request.target.split('?').next().unwrap_or_default();
    let (kind, rest) = match path[1.min(path.len())..].find('/') {
        Some(slash) => (&path[1..slash + 1], &request.target[slash + 1..]),
        None => (path.trim_matches('/'), "/"),
    };
    let Some((_, upstream)) = UPSTREAMS.iter().find(|(k, _)| *k == kind) else { return (404, None, Vec::new()) };

    // El jugador "se registra" para entrar a un mundo: siempre aceptado.
    if kind == "session" && path.ends_with("/session/minecraft/join") && request.method == "POST" {
        return (204, None, Vec::new());
    }
    // El anfitrión pregunta si el jugador se registró: sí, con el UUID de su nombre (como un servidor offline).
    if kind == "session" && path.ends_with("/session/minecraft/hasJoined") {
        return match query(&request.target, "username").filter(|name| settings::validate_username(name).is_none()) {
            Some(name) => {
                let profile = serde_json::json!({ "id": settings::offline_uuid(&name), "name": name, "properties": [] });
                (200, Some("application/json".into()), profile.to_string().into_bytes())
            }
            None => (204, None, Vec::new()),
        };
    }

    // Todo lo demás va a Mojang tal cual (skins, claves públicas, atributos del jugador…).
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut forward = client.request(method, format!("{upstream}{rest}"));
    for header in ["content-type", "authorization", "accept"] {
        if let Some(value) = request.headers.get(header) {
            forward = forward.header(header, value);
        }
    }
    if !request.body.is_empty() {
        forward = forward.body(request.body.clone());
    }
    match forward.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);
            (status, content_type, response.bytes().await.map(|b| b.to_vec()).unwrap_or_default())
        }
        // Sin internet (o en la escuela): el juego lo trata igual que si Mojang no respondiera.
        Err(_) => (503, None, Vec::new()),
    }
}

fn query(target: &str, key: &str) -> Option<String> {
    let url = reqwest::Url::parse(&format!("http://local{target}")).ok()?;
    url.query_pairs().find(|(k, _)| k == key).map(|(_, v)| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responde_como_servidor_offline() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let log = std::env::temp_dir().join(format!("parabellum-sesion-{}.log", std::process::id()));
            let port = start(log.clone()).await.unwrap();
            let base = format!("http://127.0.0.1:{port}");
            let client = reqwest::Client::new();
            let joined = client.post(format!("{base}/session/session/minecraft/join")).body("{}").send().await.unwrap();
            assert_eq!(joined.status(), 204);
            let has = client.get(format!("{base}/session/session/minecraft/hasJoined?username=GxxseS&serverId=abc")).send().await.unwrap();
            assert_eq!(has.status(), 200);
            let profile: serde_json::Value = has.json().await.unwrap();
            assert_eq!(profile["id"], settings::offline_uuid("GxxseS"));
            let bad = client.get(format!("{base}/session/session/minecraft/hasJoined?username=con%20espacio")).send().await.unwrap();
            assert_eq!(bad.status(), 204);
            let unknown = client.get(format!("{base}/otra/cosa")).send().await.unwrap();
            assert_eq!(unknown.status(), 404);
            let _ = std::fs::remove_file(log);
        });
        assert_eq!(jvm_arguments(1)[1], "-Dminecraft.api.session.host=http://127.0.0.1:1/session");
    }
}
