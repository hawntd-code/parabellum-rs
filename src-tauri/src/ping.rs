//! El estado de un servidor de Minecraft como lo ve la lista de Multijugador: su icono, su mensaje
//! (MOTD), cuántos están jugando y la versión. Es el «Server List Ping» del juego: se pregunta al
//! propio servidor, así que sirve para cualquier servidor (de un amigo por playit.gg o público).
//! Las direcciones sin puerto se buscan como registro SRV (_minecraft._tcp.<dirección>), igual que
//! el juego: así funcionan las de playit.gg. Lo último que se supo de cada uno queda en
//! datos/cache/servidores.json, para mostrar su icono aunque esté apagado.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const TIMEOUT: Duration = Duration::from_secs(5);
/// Versión de protocolo que se anuncia (para preguntar el estado, cualquiera sirve).
const PROTOCOL: i32 = 767;
const MAX_RESPONSE: usize = 512 * 1024;
const CACHE_FILE: &str = "servidores.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Status {
    /// Respondió ahora mismo.
    pub online: bool,
    /// El mensaje con códigos de color (§a, §l…).
    pub motd: String,
    pub players: u32,
    pub max_players: u32,
    /// Algunos nombres de los que están jugando (si el servidor los muestra).
    pub sample: Vec<String>,
    pub version: String,
    /// Icono como data URL (data:image/png;base64,…).
    pub favicon: Option<String>,
    pub latency_ms: Option<u32>,
    /// Cuándo se supo (milisegundos Unix).
    pub checked: i64,
}

/// "host", "host:puerto" o "[ipv6]:puerto" → (host, puerto si lo trae).
fn split_address(address: &str) -> Option<(String, Option<u16>)> {
    let address = address.trim();
    if address.is_empty() || address.len() > 255 || !address.chars().all(|c| c.is_ascii_alphanumeric() || ".-:[]_".contains(c)) {
        return None;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let (host, port) = rest.split_once(']')?;
        return Some((host.to_string(), port.strip_prefix(':').and_then(|p| p.parse().ok())));
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Some((host.to_string(), Some(port.parse().ok()?))),
        _ => Some((address.to_string(), None)),
    }
}

/// Registro SRV de Minecraft: (destino, puerto).
fn srv(host: &str) -> Option<(String, u16)> {
    use windows_sys::Win32::NetworkManagement::Dns::{DnsFree, DnsFreeRecordList, DnsQuery_W, DNS_QUERY_STANDARD, DNS_RECORDW, DNS_TYPE_SRV};
    if host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let name: Vec<u16> = format!("_minecraft._tcp.{host}").encode_utf16().chain(std::iter::once(0)).collect();
    let mut results: *mut DNS_RECORDW = std::ptr::null_mut();
    // SAFETY: nombre UTF-16 terminado en cero; la lista que devuelve Windows se recorre mientras vive
    // y se libera con DnsFree al final.
    unsafe {
        if DnsQuery_W(name.as_ptr(), DNS_TYPE_SRV, DNS_QUERY_STANDARD, std::ptr::null_mut(), (&mut results as *mut *mut DNS_RECORDW).cast(), std::ptr::null_mut()) != 0 {
            return None;
        }
        let mut found = None;
        let mut record = results;
        while !record.is_null() {
            if (*record).wType == DNS_TYPE_SRV {
                let data = (*record).Data.SRV;
                let target = data.pNameTarget;
                if !target.is_null() {
                    let length = (0..).take_while(|&i| *target.add(i) != 0).count();
                    let text = String::from_utf16_lossy(std::slice::from_raw_parts(target, length));
                    found = Some((text.trim_end_matches('.').to_string(), data.wPort));
                    break;
                }
            }
            record = (*record).pNext;
        }
        DnsFree(results.cast(), DnsFreeRecordList);
        found
    }
}

fn var_int(mut value: i32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value = ((value as u32) >> 7) as i32;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return;
        }
    }
}

async fn read_var_int(stream: &mut TcpStream) -> Result<i32, String> {
    let mut value: i32 = 0;
    for shift in 0..5 {
        let byte = stream.read_u8().await.map_err(|e| e.to_string())?;
        value |= ((byte & 0x7f) as i32) << (7 * shift);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("VarInt demasiado largo".into())
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = Vec::new();
    var_int(id, &mut inner);
    inner.extend(body);
    let mut out = Vec::new();
    var_int(inner.len() as i32, &mut out);
    out.extend(inner);
    out
}

/// Pregunta su estado a un servidor.
pub async fn status(address: &str) -> Result<Status, String> {
    let (host, port) = split_address(address).ok_or("dirección no válida")?;
    // Sin puerto: el registro SRV (playit.gg) o el 25565 de siempre.
    let (target, port) = match port {
        Some(port) => (host.clone(), port),
        None => {
            let lookup = host.clone();
            tauri::async_runtime::spawn_blocking(move || srv(&lookup)).await.ok().flatten().unwrap_or((host.clone(), 25565))
        }
    };
    let started = Instant::now();
    let work = async {
        let mut stream = TcpStream::connect((target.as_str(), port)).await.map_err(|e| e.to_string())?;
        let mut handshake = Vec::new();
        var_int(PROTOCOL, &mut handshake);
        var_int(host.len() as i32, &mut handshake);
        handshake.extend(host.as_bytes());
        handshake.extend(port.to_be_bytes());
        var_int(1, &mut handshake);
        stream.write_all(&packet(0, &handshake)).await.map_err(|e| e.to_string())?;
        stream.write_all(&packet(0, &[])).await.map_err(|e| e.to_string())?;
        let length = read_var_int(&mut stream).await? as usize;
        if length == 0 || length > MAX_RESPONSE {
            return Err("respuesta no válida".to_string());
        }
        let mut body = vec![0u8; length];
        stream.read_exact(&mut body).await.map_err(|e| e.to_string())?;
        Ok::<Vec<u8>, String>(body)
    };
    let body = tokio::time::timeout(TIMEOUT, work).await.map_err(|_| "no respondió".to_string())??;
    let latency = started.elapsed().as_millis() as u32;
    parse_response(&body).map(|mut status| {
        status.latency_ms = Some(latency);
        status
    })
}

/// Paquete de respuesta: id 0 y el JSON del estado.
fn parse_response(body: &[u8]) -> Result<Status, String> {
    let mut at = 0;
    let mut read = || -> Option<i32> {
        let mut value: i32 = 0;
        for shift in 0..5 {
            let byte = *body.get(at)?;
            at += 1;
            value |= ((byte & 0x7f) as i32) << (7 * shift);
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    };
    if read() != Some(0) {
        return Err("respuesta no válida".into());
    }
    let length = read().ok_or("respuesta no válida")? as usize;
    let text = body.get(at..at + length).ok_or("respuesta incompleta")?;
    let json: Value = serde_json::from_slice(text).map_err(|e| e.to_string())?;
    Ok(Status {
        online: true,
        motd: legacy_text(&json["description"], ""),
        players: json.pointer("/players/online").and_then(Value::as_u64).unwrap_or(0) as u32,
        max_players: json.pointer("/players/max").and_then(Value::as_u64).unwrap_or(0) as u32,
        sample: json.pointer("/players/sample").and_then(Value::as_array).into_iter().flatten().filter_map(|p| p["name"].as_str()).filter(|n| n.len() <= 32).take(12).map(String::from).collect(),
        version: legacy_text(&json.pointer("/version/name").cloned().unwrap_or_default(), "").chars().take(60).collect(),
        favicon: json["favicon"].as_str().filter(|f| f.starts_with("data:image/png;base64,") && f.len() <= 200_000).map(|f| f.replace(['\n', '\r'], "")),
        latency_ms: None,
        checked: chrono::Utc::now().timestamp_millis(),
    })
}

/// Un texto de Minecraft (JSON o con códigos §) como texto con códigos §. Cada parte lleva el estilo
/// heredado completo (como lo dibuja el juego).
fn legacy_text(component: &Value, inherited: &str) -> String {
    match component {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts.iter().map(|part| legacy_text(part, inherited)).collect(),
        Value::Object(object) => {
            let mut style = inherited.to_string();
            if let Some(code) = object.get("color").and_then(Value::as_str).and_then(color_code) {
                // Un color nuevo quita el formato anterior (como en el juego).
                style = format!("§{code}");
            }
            for (key, code) in [("bold", 'l'), ("italic", 'o'), ("underlined", 'n'), ("strikethrough", 'm'), ("obfuscated", 'k')] {
                match object.get(key) {
                    Some(Value::Bool(true)) if !style.contains(&format!("§{code}")) => style.push_str(&format!("§{code}")),
                    Some(Value::Bool(false)) => style = style.replace(&format!("§{code}"), ""),
                    _ => {}
                }
            }
            let mut out = String::new();
            if let Some(text) = object.get("text").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                out.push_str("§r");
                out.push_str(&style);
                out.push_str(text);
            }
            if let Some(extra) = object.get("extra").and_then(Value::as_array) {
                for part in extra {
                    let child = legacy_text(part, &style);
                    // Un texto suelto dentro de «extra» también hereda el estilo.
                    if part.is_string() && !child.is_empty() {
                        out.push_str("§r");
                        out.push_str(&style);
                    }
                    out.push_str(&child);
                }
            }
            out
        }
        _ => String::new(),
    }
}

/// Nombre de color (o #RRGGBB, al más parecido) → código §.
fn color_code(color: &str) -> Option<char> {
    const NAMES: [(&str, char, u32); 16] = [
        ("black", '0', 0x000000),
        ("dark_blue", '1', 0x0000AA),
        ("dark_green", '2', 0x00AA00),
        ("dark_aqua", '3', 0x00AAAA),
        ("dark_red", '4', 0xAA0000),
        ("dark_purple", '5', 0xAA00AA),
        ("gold", '6', 0xFFAA00),
        ("gray", '7', 0xAAAAAA),
        ("dark_gray", '8', 0x555555),
        ("blue", '9', 0x5555FF),
        ("green", 'a', 0x55FF55),
        ("aqua", 'b', 0x55FFFF),
        ("red", 'c', 0xFF5555),
        ("light_purple", 'd', 0xFF55FF),
        ("yellow", 'e', 0xFFFF55),
        ("white", 'f', 0xFFFFFF),
    ];
    if let Some((_, code, _)) = NAMES.iter().find(|(name, _, _)| *name == color) {
        return Some(*code);
    }
    let rgb = u32::from_str_radix(color.strip_prefix('#')?, 16).ok()?;
    let channel = |value: u32, shift: u32| ((value >> shift) & 0xff) as i64;
    NAMES
        .iter()
        .min_by_key(|(_, _, value)| (0..3).map(|i| (channel(rgb, i * 8) - channel(*value, i * 8)).pow(2)).sum::<i64>())
        .map(|(_, code, _)| *code)
}

// ================= Lo último que se supo (para el icono de los que están apagados)

pub fn load_cache(cache: &Path) -> HashMap<String, Status> {
    std::fs::read_to_string(cache.join(CACHE_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_cache(cache: &Path, entries: &HashMap<String, Status>) {
    let _ = std::fs::create_dir_all(cache);
    let _ = std::fs::write(cache.join(CACHE_FILE), serde_json::to_vec(entries).unwrap_or_default());
}

/// Estado de varios servidores a la vez. Los que no responden quedan con lo último que se supo.
pub async fn statuses(addresses: Vec<String>, cache_dir: &Path) -> HashMap<String, Status> {
    let mut cache = load_cache(cache_dir);
    let mut unique: Vec<String> = addresses.into_iter().map(|a| a.trim().to_lowercase()).filter(|a| split_address(a).is_some()).collect();
    unique.sort();
    unique.dedup();
    unique.truncate(40);
    let results = futures::future::join_all(unique.iter().map(|address| status(address))).await;
    let mut out = HashMap::new();
    for (address, result) in unique.into_iter().zip(results) {
        let entry = match result {
            Ok(status) => {
                cache.insert(address.clone(), Status { online: false, latency_ms: None, sample: Vec::new(), ..status.clone() });
                status
            }
            Err(_) => Status { online: false, players: 0, sample: Vec::new(), latency_ms: None, ..cache.get(&address).cloned().unwrap_or_default() },
        };
        out.insert(address, entry);
    }
    // Lo más viejo se olvida (no crece sin fin).
    if cache.len() > 200 {
        let mut entries: Vec<(String, Status)> = cache.into_iter().collect();
        entries.sort_by_key(|(_, s)| std::cmp::Reverse(s.checked));
        entries.truncate(200);
        cache = entries.into_iter().collect();
    }
    save_cache(cache_dir, &cache);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direcciones() {
        assert_eq!(split_address("mc.universocraft.com"), Some(("mc.universocraft.com".into(), None)));
        assert_eq!(split_address("192.168.1.5:25570"), Some(("192.168.1.5".into(), Some(25570))));
        assert_eq!(split_address("[::1]:25565"), Some(("::1".into(), Some(25565))));
        assert_eq!(split_address("algo malo/../"), None);
        assert_eq!(split_address("host:noesnumero"), None);
    }

    #[test]
    fn mensaje_con_colores() {
        let json: Value = serde_json::from_str(r#"{"text":"","extra":[{"text":"Servidor ","color":"gold"},{"text":"de Juan","color":"light_purple","bold":true},"!"]}"#).unwrap();
        assert_eq!(legacy_text(&json, ""), "§r§6Servidor §r§d§lde Juan§r!");
        assert_eq!(legacy_text(&Value::String("§aHola".into()), ""), "§aHola");
        assert_eq!(color_code("#FF5556"), Some('c'));
        let mut packet_body = Vec::new();
        var_int(0, &mut packet_body);
        let json = r#"{"version":{"name":"1.21.1"},"players":{"online":2,"max":10,"sample":[{"name":"Hawntd","id":"x"}]},"description":"§bHola","favicon":"data:image/png;base64,AAAA"}"#;
        var_int(json.len() as i32, &mut packet_body);
        packet_body.extend(json.as_bytes());
        let status = parse_response(&packet_body).unwrap();
        assert_eq!((status.players, status.max_players, status.sample.clone(), status.version.as_str()), (2, 10, vec!["Hawntd".to_string()], "1.21.1"));
        assert_eq!(status.favicon.as_deref(), Some("data:image/png;base64,AAAA"));
        assert_eq!(status.motd, "§bHola");
    }
}
