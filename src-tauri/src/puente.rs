//! Puente con el mod de Parabellum dentro del juego (carpeta `mod/` del repositorio). El mod solo dibuja
//! la interfaz (amigos, chats, capturas, invitaciones); la cuenta, el chat y el P2P los hace Parabellum.
//!
//! TCP en 127.0.0.1, una línea JSON por mensaje. El juego recibe el puerto y una clave con
//! -Dparabellum.puente / -Dparabellum.clave (ver jvm_arguments); la clave se guarda en
//! datos/puente.txt para que un juego abierto siga conectando si Parabellum se reinicia.
//!
//!   Del mod:        {"op": "enviar", "id": 7, "chat": "…", "texto": "…", "respuesta": "…"}
//!   Respuesta:      {"re": 7, "ok": true, "datos": …}   o   {"re": 7, "error": "…"}
//!   De Parabellum:  {"ev": "estado" | "mensajes" | "aviso", …}   (cuando cambia algo)

use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::amigos::Presence;
use crate::{cuenta, hosting, ping, servidor, AppState};

/// Siempre el mismo puerto si está libre (como el servidor de sesión local).
const PREFERRED_PORT: u16 = 47816;
/// Las imágenes del chat llegan en una línea (hasta ~450 KB en base64).
const MAX_LINE: usize = 2 * 1024 * 1024;
/// Las invitaciones son mensajes de texto con este prefijo y un JSON pequeño.
pub const INVITE_PREFIX: &str = "parabellum:invitacion:";

pub struct Puente {
    port: u16,
    key: String,
    clients: Mutex<Vec<mpsc::UnboundedSender<String>>>,
    /// Hay un estado por mandar (los cambios seguidos se juntan en uno).
    pending: AtomicBool,
}

fn puente(app: &AppHandle) -> Option<Arc<Puente>> {
    app.state::<AppState>().puente.lock().unwrap().clone()
}

/// Propiedades de Java que le dicen al mod dónde está Parabellum.
pub fn jvm_arguments(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let fonts = state.paths.data.join("fuentes");
    let language = if state.settings.lock().unwrap().language == "en" { "en" } else { "es" };
    let mut arguments = vec![format!("-Dparabellum.fuentes={}", fonts.display()), format!("-Dparabellum.idioma={language}")];
    if let Some(bridge) = puente(app) {
        arguments.push(format!("-Dparabellum.puente={}", bridge.port));
        arguments.push(format!("-Dparabellum.clave={}", bridge.key));
    }
    arguments
}

fn load_key(path: &std::path::Path) -> String {
    if let Ok(key) = std::fs::read_to_string(path) {
        let key = key.trim().to_string();
        if key.len() >= 24 && key.chars().all(|c| c.is_ascii_alphanumeric()) {
            return key;
        }
    }
    let mut bytes = [0u8; 24];
    getrandom::getrandom(&mut bytes).expect("números aleatorios");
    let key = hex::encode(bytes);
    let _ = std::fs::write(path, &key);
    key
}

/// Abre el puente (en segundo plano).
pub async fn start(app: &AppHandle) -> std::io::Result<()> {
    let listener = match TcpListener::bind(("127.0.0.1", PREFERRED_PORT)).await {
        Ok(listener) => listener,
        Err(_) => TcpListener::bind(("127.0.0.1", 0)).await?,
    };
    let key = load_key(&app.state::<AppState>().paths.data.join("puente.txt"));
    let bridge = Arc::new(Puente { port: listener.local_addr()?.port(), key, clients: Mutex::default(), pending: AtomicBool::new(false) });
    *app.state::<AppState>().puente.lock().unwrap() = Some(bridge.clone());
    let app = app.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let (app, bridge) = (app.clone(), bridge.clone());
            tokio::spawn(async move { serve(app, bridge, stream).await });
        }
    });
    Ok(())
}

fn line(value: &Value) -> String {
    let mut text = value.to_string();
    text.push('\n');
    text
}

/// A todos los juegos conectados.
fn broadcast(bridge: &Puente, value: &Value) {
    let text = line(value);
    bridge.clients.lock().unwrap().retain(|client| client.send(text.clone()).is_ok());
}

/// Hay un juego conectado (con el mod): los avisos se ven dentro del juego.
pub fn game_connected(app: &AppHandle) -> bool {
    puente(app).is_some_and(|bridge| {
        let mut clients = bridge.clients.lock().unwrap();
        clients.retain(|client| !client.is_closed());
        !clients.is_empty()
    })
}

/// Algo cambió (amigos, presencia, conversaciones): el estado nuevo llega al juego enseguida, juntando
/// los cambios de un cuarto de segundo.
pub fn changed(app: &AppHandle) {
    let Some(bridge) = puente(app) else { return };
    if bridge.clients.lock().unwrap().is_empty() || bridge.pending.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        bridge.pending.store(false, Ordering::SeqCst);
        broadcast(&bridge, &snapshot(&app));
    });
}

/// Un aviso dentro del juego (mensaje nuevo, solicitud, invitación, amigo que abrió su mundo).
pub fn notice(app: &AppHandle, kind: &str, title: &str, text: &str, extra: Value) {
    let Some(bridge) = puente(app) else { return };
    let mut value = json!({ "ev": "aviso", "tipo": kind, "titulo": title, "texto": text });
    if let (Some(target), Some(fields)) = (value.as_object_mut(), extra.as_object()) {
        target.extend(fields.clone());
    }
    broadcast(&bridge, &value);
}

// ================= Conexión con un juego

async fn serve(app: AppHandle, bridge: Arc<Puente>, stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    // Primero la clave: sin ella no se contesta nada.
    let Ok(Ok(Some(first))) = tokio::time::timeout(Duration::from_secs(10), lines.next_line()).await else { return };
    let hello: Value = serde_json::from_str(&first).unwrap_or(Value::Null);
    if hello["op"] != "hola" || hello["clave"].as_str() != Some(bridge.key.as_str()) {
        return;
    }
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        while let Some(text) = rx.recv().await {
            if write.write_all(text.as_bytes()).await.is_err() {
                break;
            }
        }
    });
    let me = cuenta::social_handle(&app).and_then(|s| s.name()).unwrap_or_else(|| app.state::<AppState>().settings.lock().unwrap().username.clone());
    let _ = tx.send(line(&json!({ "re": hello["id"], "ok": true, "datos": { "nombre": me, "version": env!("CARGO_PKG_VERSION") } })));
    let _ = tx.send(line(&snapshot(&app)));
    bridge.clients.lock().unwrap().push(tx.clone());
    // La conversación que el juego tiene abierta (su propia escucha: la ventana puede ver otra).
    let mut watching: Option<crate::chat::Watch> = None;
    loop {
        let next = match lines.next_line().await {
            Ok(Some(text)) => text,
            _ => break,
        };
        if next.len() > MAX_LINE {
            break;
        }
        let Ok(request) = serde_json::from_str::<Value>(&next) else { continue };
        let id = request["id"].clone();
        let op = request["op"].as_str().unwrap_or_default().to_string();
        let result = match op.as_str() {
            "abrir_chat" => watch_chat(&app, &request, &tx).map(|listener| {
                watching = Some(listener);
                Value::Null
            }),
            "cerrar_chat" => {
                watching = None;
                Ok(Value::Null)
            }
            _ => handle(&app, &op, &request).await,
        };
        let reply = match result {
            Ok(data) => json!({ "re": id, "ok": true, "datos": data }),
            Err(error) => json!({ "re": id, "error": error }),
        };
        if tx.send(line(&reply)).is_err() {
            break;
        }
    }
    drop(watching);
    writer.abort();
}

/// Una lista de ids de amigos (crear un grupo, agregar a uno).
fn uids(request: &Value, field: &str) -> Vec<String> {
    request[field].as_array().map(|list| list.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default()
}

/// Para que el juego sepa si la foto de un grupo cambió sin mandarla en cada estado (se pide aparte).
fn photo_key(photo: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    photo.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn text<'a>(request: &'a Value, field: &str) -> Result<&'a str, String> {
    request[field].as_str().filter(|s| !s.is_empty()).ok_or_else(|| format!("falta «{field}»"))
}

fn chat_service(app: &AppHandle) -> Result<Arc<crate::chat::Chat>, String> {
    cuenta::chat_handle(app).ok_or(t!("Inicia sesión con Google en Parabellum.", "Sign in with Google in Parabellum."))
}

fn social(app: &AppHandle) -> Result<Arc<crate::amigos::Social>, String> {
    cuenta::social_handle(app).ok_or(t!("Inicia sesión con Google en Parabellum.", "Sign in with Google in Parabellum."))
}

fn watch_chat(app: &AppHandle, request: &Value, tx: &mpsc::UnboundedSender<String>) -> Result<crate::chat::Watch, String> {
    let chat = text(request, "chat")?.to_string();
    let service = chat_service(app)?;
    let (tx, id) = (tx.clone(), chat.clone());
    let listener = service.watch(&chat, move |messages| {
        let _ = tx.send(line(&json!({ "ev": "mensajes", "chat": id, "mensajes": messages })));
    })?;
    tauri::async_runtime::spawn(async move {
        let _ = service.mark_read(&chat).await;
    });
    Ok(listener)
}

async fn handle(app: &AppHandle, op: &str, request: &Value) -> Result<Value, String> {
    match op {
        "estado" => Ok(snapshot(app)),
        "chat_amigo" => Ok(json!({ "chat": chat_service(app)?.direct(text(request, "uid")?).await? })),
        "enviar" => chat_service(app)?.send_text(text(request, "chat")?, text(request, "texto")?, request["respuesta"].as_str().filter(|r| !r.is_empty())).await.map(|_| Value::Null),
        "reaccionar" => {
            let emoji = request["emoji"].as_str().filter(|e| !e.is_empty());
            chat_service(app)?.react(text(request, "chat")?, text(request, "mensaje")?, emoji).await.map(|_| Value::Null)
        }
        "favorito_chat" => crate::set_favorite_chat(app, text(request, "chat")?, request["activo"].as_bool().unwrap_or(true)).map(|_| Value::Null),
        "favorito_amigo" => {
            let chat = chat_service(app)?.direct(text(request, "uid")?).await?;
            crate::set_favorite_chat(app, &chat, request["activo"].as_bool().unwrap_or(true)).map(|_| Value::Null)
        }
        "crear_grupo" => Ok(json!({ "chat": chat_service(app)?.create_group(text(request, "nombre")?, &uids(request, "amigos"), request["foto"].as_str()).await? })),
        "agregar_al_grupo" => chat_service(app)?.add_members(text(request, "chat")?, &uids(request, "amigos")).await.map(|_| Value::Null),
        "editar_grupo" => {
            let name = request["nombre"].as_str().filter(|n| !n.trim().is_empty());
            let photo = request["foto"].as_str().map(Some);
            chat_service(app)?.edit_group(text(request, "chat")?, name, photo).await.map(|_| Value::Null)
        }
        "sacar_del_grupo" => chat_service(app)?.remove_member(text(request, "chat")?, text(request, "uid")?).await.map(|_| Value::Null),
        "pasar_grupo" => chat_service(app)?.transfer_group(text(request, "chat")?, text(request, "uid")?).await.map(|_| Value::Null),
        "salir_chat" => chat_service(app)?.leave(text(request, "chat")?).await.map(|_| Value::Null),
        "foto_grupo" => {
            let photo = chat_service(app)?.summary(text(request, "chat")?).and_then(|s| s.photo).ok_or(t!("Ese grupo no tiene foto.", "That group has no photo."))?;
            Ok(json!({ "datos": photo }))
        }
        "abrir_carpeta" => {
            // Solo la carpeta de capturas de un juego (la ruta la manda el mod).
            let folder = std::path::PathBuf::from(text(request, "ruta")?);
            if !folder.is_dir() || folder.file_name().is_none_or(|n| n != "screenshots") {
                return Err(t!("Esa carpeta no existe.", "That folder does not exist."));
            }
            crate::plataforma::open_path(&folder).map(|_| Value::Null)
        }
        "enviar_imagen" => {
            let (width, height) = (request["ancho"].as_u64().unwrap_or(0) as u32, request["alto"].as_u64().unwrap_or(0) as u32);
            chat_service(app)?.send_image(text(request, "chat")?, text(request, "datos")?, width, height).await.map(|_| Value::Null)
        }
        "imagen" => Ok(json!({ "datos": chat_service(app)?.image(text(request, "chat")?, text(request, "mensaje")?).await? })),
        "editar" => chat_service(app)?.edit(text(request, "chat")?, text(request, "mensaje")?, text(request, "texto")?).await.map(|_| Value::Null),
        "borrar" => chat_service(app)?.delete(text(request, "chat")?, text(request, "mensaje")?).await.map(|_| Value::Null),
        "leido" => chat_service(app)?.mark_read(text(request, "chat")?).await.map(|_| Value::Null),
        "solicitud" => {
            let name = text(request, "nombre")?.trim().to_string();
            if let Some(problem) = crate::settings::validate_username(&name) {
                return Err(problem.into());
            }
            Ok(json!({ "texto": social(app)?.send_request(&name).await? }))
        }
        "aceptar" => social(app)?.accept(text(request, "uid")?).await.map(|_| Value::Null),
        "rechazar" => social(app)?.decline(text(request, "uid")?).await.map(|_| Value::Null),
        "quitar_amigo" => social(app)?.remove_friend(text(request, "uid")?).await.map(|_| Value::Null),
        "invitar" => invite(app, text(request, "uid")?).await.map(|message| json!({ "texto": message })),
        "unirse" => join(app, request).await,
        "abrir_parabellum" => {
            cuenta::show_window(app);
            Ok(Value::Null)
        }
        _ => Err(format!("operación desconocida: {op}")),
    }
}

// ================= Lo que ve el juego

fn snapshot(app: &AppHandle) -> Value {
    let social = cuenta::social_handle(app);
    let chats = cuenta::chat_handle(app).map(|c| c.list()).unwrap_or_default();
    let view = social.as_ref().map(|s| s.view());
    let favorites: std::collections::HashSet<String> = app.state::<AppState>().settings.lock().unwrap().favorite_chats.iter().cloned().collect();
    let my_uid = social.as_ref().map(|s| s.uid.clone()).unwrap_or_default();
    let friends: Vec<Value> = view
        .as_ref()
        .map(|v| {
            v.friends
                .iter()
                .map(|f| {
                    let p = f.presence.clone().unwrap_or_default();
                    let hosting = p.hosting.clone().filter(|_| f.online);
                    // A dónde se puede entrar con él: su mundo, el servidor público donde juega o su
                    // servidor (si te invitó).
                    let invited = v.servers.iter().any(|s| s.host == f.uid && s.online);
                    let joinable = f.online && (f.world_open || (p.state == "playing" && p.server.is_some()) || invited);
                    json!({
                        "uid": f.uid,
                        "nombre": f.name,
                        "enLinea": f.online,
                        "jugando": f.online && p.state == "playing",
                        "pack": p.pack,
                        "mundo": f.world_open,
                        "servidor": p.server,
                        "hostea": hosting,
                        "unirse": joinable,
                        "visto": f.last_seen,
                        "favorito": favorites.contains(&crate::chat::direct_id(&my_uid, &f.uid)),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let requests: Vec<Value> = view.as_ref().map(|v| v.requests.iter().map(|r| json!({ "uid": r.uid, "nombre": r.name })).collect()).unwrap_or_default();
    let servers: Vec<Value> = view
        .as_ref()
        .map(|v| {
            v.servers
                .iter()
                .map(|s| json!({ "anfitrion": s.host, "nombreAnfitrion": s.host_name, "clave": s.key, "nombre": s.name, "encendido": s.online, "jugadores": s.players, "max": s.max_players, "pack": s.pack }))
                .collect()
        })
        .unwrap_or_default();
    let chat_list: Vec<Value> = chats
        .iter()
        .map(|c| {
            let friend = (c.kind != "grupo").then(|| c.members.iter().find(|m| !m.you).map(|m| m.uid.clone())).flatten();
            let members: Vec<Value> = c.members.iter().map(|m| json!({ "uid": m.uid, "nombre": m.name, "tu": m.you })).collect();
            json!({
                "id": c.id, "nombre": c.name, "tipo": c.kind, "ultimo": c.last_text, "ultimoNombre": c.last_name, "hora": c.last_at, "sinLeer": c.unread,
                "favorito": favorites.contains(&c.id),
                "amigo": friend,
                "dueno": c.owner,
                "puedoEditar": c.can_edit,
                "miembros": members,
                "foto": c.photo.as_deref().map(photo_key),
            })
        })
        .collect();
    let me = social.as_ref().map(|s| s.my_presence()).unwrap_or_default();
    // Sin sesión de Google, el nombre con el que juegas.
    let name = social.as_ref().and_then(|s| s.name()).unwrap_or_else(|| app.state::<AppState>().settings.lock().unwrap().username.clone());
    json!({
        "ev": "estado",
        "sesion": social.is_some(),
        "yo": { "nombre": name, "uid": social.as_ref().map(|s| s.uid.clone()) },
        // Puedes invitar si tienes un servidor encendido o tu mundo abierto.
        "puedoInvitar": hosting::ready_server(app).is_some() || world_open(&me),
        "amigos": friends,
        "solicitudes": requests,
        "servidores": servers,
        "chats": chat_list,
    })
}

fn world_open(me: &Presence) -> bool {
    me.state == "playing" && me.lan.is_some()
}

// ================= Invitar y unirse

/// Invita a un amigo a lo que tienes: tu servidor encendido (entra a la lista blanca) o tu mundo abierto.
pub async fn invite(app: &AppHandle, uid: &str) -> Result<String, String> {
    let social = social(app)?;
    let friend = social.view().friends.into_iter().find(|f| f.uid == uid).ok_or(t!("Ese jugador ya no está en tus amigos.", "That player is no longer your friend."))?;
    let (invite, label) = if let Some(config) = hosting::ready_server(app) {
        if !config.guests.iter().any(|g| g.uid.as_deref() == Some(uid)) {
            let mut guests = config.guests.clone();
            guests.push(servidor::Guest { name: friend.name.clone(), uid: Some(uid.to_string()) });
            hosting::set_guests(app, &config.id, guests).await?;
        }
        (json!({ "t": "servidor", "n": config.name, "p": config.instance_name, "k": config.key }), config.name.clone())
    } else {
        let me = social.my_presence();
        if !world_open(&me) {
            return Err(t!(
                "Abre tu mundo (Pausa › Abrir en LAN) o enciende un servidor para invitar.",
                "Open your world (Pause › Open to LAN) or start a server to invite."
            ));
        }
        let pack = me.pack.clone().unwrap_or_default();
        (json!({ "t": "mundo", "n": pack, "p": pack }), pack)
    };
    let service = chat_service(app)?;
    let chat = service.direct(uid).await?;
    service.send_invite(&chat, &format!("{INVITE_PREFIX}{invite}"), &format!("🎮 {}", t!("Invitación: {label}", "Invite: {label}"))).await?;
    Ok(t!("Invitaste a {}.", "You invited {}.", friend.name))
}

/// La dirección por la que entrar a la partida de un amigo (la ruta más rápida: red local, P2P o
/// playit.gg). Con `clave`, un servidor suyo; si no, su mundo o el servidor donde juega.
async fn join(app: &AppHandle, request: &Value) -> Result<Value, String> {
    let social = social(app)?;
    let view = social.view();
    let host = request["anfitrion"].as_str().or(request["uid"].as_str()).ok_or("falta «uid»")?;
    let key = request["clave"].as_str();
    let friend = view.friends.iter().find(|f| f.uid == host);
    let name = friend.map(|f| f.name.clone()).unwrap_or_else(|| "…".into());
    let presence = friend.filter(|f| f.online).and_then(|f| f.presence.clone());
    let server = view.servers.iter().find(|s| s.host == host && s.online && key.is_none_or(|k| s.key == k));
    let route = |address: &Option<String>, kind: &str| address.clone().filter(|a| !a.is_empty()).map(|address| ping::Route { address, kind: kind.into(), latency_ms: None, via: None });
    let p2p = |p2p: &Option<String>, target: &str| p2p.clone().filter(|a| !a.is_empty()).map(|a| ping::Route { address: format!("p2p:{a}#{target}"), kind: "p2p".into(), latency_ms: None, via: None });
    let (routes, pack, max) = match (&presence, server) {
        (Some(p), _) if key.is_none() && p.state == "playing" && p.lan.is_some() => {
            ([route(&p.lan, "lan"), p2p(&p.p2p, "world"), route(&p.address, "playit")].into_iter().flatten().collect::<Vec<_>>(), p.pack_folder.clone().or(p.pack.clone()), None)
        }
        (_, Some(s)) => ([route(&s.lan, "lan"), p2p(&s.p2p, &s.key), route(&s.address, "playit")].into_iter().flatten().collect(), s.pack_folder.clone().or(s.pack.clone()), Some(s.max_players as u32)),
        (Some(p), None) if p.state == "playing" && p.server.is_some() => (vec![ping::Route { address: p.server.clone().unwrap_or_default(), kind: "direct".into(), latency_ms: None, via: None }], None, None),
        _ => return Err(t!("{name} no está en una partida a la que puedas entrar.", "{name} is not in a game you can join.")),
    };
    // Con otro modpack no se puede entrar (habría que abrir el suyo desde Parabellum).
    if let Some(pack) = pack.filter(|p| !p.is_empty()) {
        let state = app.state::<AppState>();
        let running = state.games.lock().unwrap().keys().next().cloned();
        if let Some(running) = running.and_then(|id| crate::instances::find(&state.paths, &id)) {
            if !running.id.eq_ignore_ascii_case(&pack) && !running.name.eq_ignore_ascii_case(&pack) {
                return Err(t!(
                    "{name} juega con «{pack}» y tú con «{}». Ábrelo desde Parabellum para unirte.",
                    "{name} plays with «{pack}» and you with «{}». Open it from Parabellum to join.",
                    running.name
                ));
            }
        }
    }
    let fallback = routes.iter().find(|r| r.kind == "playit" || r.kind == "direct").map(|r| r.address.clone());
    let p2p_service = app.state::<AppState>().p2p.lock().unwrap().clone();
    let measured = if routes.len() > 1 || routes.iter().any(|r| r.kind == "p2p") { ping::best_routes(routes, max, p2p_service).await } else { routes };
    let best = measured.iter().find(|r| r.latency_ms.is_some()).cloned();
    let (address, kind, via, ms) = match best {
        Some(r) => (r.address, r.kind, r.via, r.latency_ms),
        None => (fallback.ok_or(t!("No se pudo llegar a la partida de {name}.", "Could not reach {name}'s game."))?, "playit".to_string(), None, None),
    };
    Ok(json!({ "direccion": address, "ruta": kind, "via": via, "ms": ms, "nombre": name }))
}
