//! «Jugando con Parabellum» en Discord (Rich Presence): si Discord está abierto en este PC y el jugador no
//! lo apagó (Ajustes › «Mostrar en Discord a qué juegas»), sus amigos de Discord ven qué instancia juega.
//! Se habla con Discord por su conexión local (`plataforma::discord_ipc`): un saludo con el id de la
//! aplicación de Parabellum en Discord (`discordClientId` de config-integrada.json) y después la
//! actividad. Sin Discord abierto, o sin ese id, no hace nada.
//!
//! Cada mensaje: tipo (u32) y largo (u32), en little endian, y el JSON.

use serde_json::{json, Value};
use std::io::Write;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};

const HANDSHAKE: u32 = 0;
const FRAME: u32 = 1;
const LOGO: &str = "https://raw.githubusercontent.com/hawntd-code/parabellum-rs/main/ui/logo.png";
const WEBSITE: &str = "https://parabellum-roan.vercel.app";

/// Lo que se muestra mientras se juega.
#[derive(Clone, Debug, PartialEq)]
pub struct Activity {
    /// La instancia («Prominence II»).
    pub details: String,
    /// «Minecraft 1.20.1 · Fabric».
    pub state: String,
    /// Desde cuándo (segundos Unix).
    pub start: i64,
}

static SENDER: OnceLock<Mutex<Sender<Option<Activity>>>> = OnceLock::new();

/// Empieza a escuchar cambios de actividad (una vez). Sin id de aplicación no hace nada.
pub fn init(client_id: &str) {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() || !client_id.bytes().all(|b| b.is_ascii_digit()) {
        return;
    }
    let (sender, receiver) = channel::<Option<Activity>>();
    if SENDER.set(Mutex::new(sender)).is_err() {
        return;
    }
    std::thread::spawn(move || {
        let mut pipe: Option<Box<dyn crate::plataforma::DiscordPipe>> = None;
        while let Ok(mut wanted) = receiver.recv() {
            // Solo importa la última.
            while let Ok(next) = receiver.try_recv() {
                wanted = next;
            }
            for _ in 0..2 {
                if pipe.is_none() {
                    pipe = connect(&client_id);
                }
                let Some(open) = pipe.as_mut() else { break };
                if set_activity(open.as_mut(), wanted.as_ref()).is_ok() {
                    break;
                }
                // Discord se cerró o se reinició: se vuelve a conectar una vez.
                pipe = None;
            }
        }
    });
}

/// Muestra (o quita, con None) la actividad, si está encendido en Ajustes.
pub fn show(activity: Option<Activity>) {
    if let Some(sender) = SENDER.get() {
        let _ = sender.lock().unwrap().send(activity);
    }
}

fn frame(op: u32, body: &Value) -> Vec<u8> {
    let text = body.to_string();
    let mut out = Vec::with_capacity(text.len() + 8);
    out.extend(op.to_le_bytes());
    out.extend((text.len() as u32).to_le_bytes());
    out.extend(text.as_bytes());
    out
}

fn read_frame(pipe: &mut dyn crate::plataforma::DiscordPipe) -> std::io::Result<(u32, Value)> {
    let mut header = [0u8; 8];
    pipe.read_exact(&mut header)?;
    let op = u32::from_le_bytes(header[..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
    if len > 64 * 1024 {
        return Err(std::io::Error::other("mensaje de Discord demasiado grande"));
    }
    let mut body = vec![0u8; len];
    pipe.read_exact(&mut body)?;
    Ok((op, serde_json::from_slice(&body).unwrap_or(Value::Null)))
}

fn connect(client_id: &str) -> Option<Box<dyn crate::plataforma::DiscordPipe>> {
    (0..10).find_map(|n| {
        let mut pipe = crate::plataforma::discord_ipc(n)?;
        pipe.write_all(&frame(HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))).ok()?;
        let (_, ready) = read_frame(pipe.as_mut()).ok()?;
        (ready["evt"] == "READY").then_some(pipe)
    })
}

/// La actividad como la pide Discord.
fn payload(activity: Option<&Activity>, nonce: u64) -> Value {
    let activity = activity.map(|a| {
        json!({
            "details": cut(&a.details),
            "state": cut(&a.state),
            "timestamps": { "start": a.start },
            "assets": { "large_image": LOGO, "large_text": "Parabellum" },
            "buttons": [{ "label": t!("Descargar Parabellum", "Download Parabellum"), "url": WEBSITE }],
        })
    });
    json!({ "cmd": "SET_ACTIVITY", "args": { "pid": std::process::id(), "activity": activity }, "nonce": nonce.to_string() })
}

/// Discord acepta de 2 a 128 caracteres.
fn cut(text: &str) -> String {
    let text: String = text.chars().take(128).collect();
    if text.chars().count() < 2 {
        format!("{text}  ")
    } else {
        text
    }
}

fn set_activity(pipe: &mut dyn crate::plataforma::DiscordPipe, activity: Option<&Activity>) -> std::io::Result<()> {
    static NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let nonce = NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    pipe.write_all(&frame(FRAME, &payload(activity, nonce)))?;
    // Discord contesta cada orden; se lee para no llenar la conexión.
    read_frame(pipe).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mensajes_como_los_pide_discord() {
        let bytes = frame(HANDSHAKE, &json!({ "v": 1 }));
        assert_eq!(&bytes[..4], &0u32.to_le_bytes());
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize, bytes.len() - 8);
        let activity = Activity { details: "P".into(), state: "Minecraft 1.20.1 · Fabric".into(), start: 10 };
        let body = payload(Some(&activity), 3);
        assert_eq!(body["cmd"], "SET_ACTIVITY");
        assert_eq!(body["args"]["activity"]["details"].as_str().unwrap().trim(), "P", "rellena lo muy corto");
        assert!(body["args"]["activity"]["details"].as_str().unwrap().chars().count() >= 2);
        assert_eq!(body["args"]["activity"]["timestamps"]["start"], 10);
        assert!(payload(None, 4)["args"]["activity"].is_null(), "sin actividad se quita");
        assert_eq!(cut(&"x".repeat(300)).chars().count(), 128);
    }
}
