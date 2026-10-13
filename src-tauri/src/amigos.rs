//! Amigos y presencia sobre Firebase Realtime Database. Estructura (reglas en firebase-reglas.json):
//!   names/{nombre en minúsculas} = uid            (cada nombre es de una sola cuenta de Google)
//!   users/{uid} = { name, uuid }
//!   requests/{destino}/{origen} = { name, at }    (solicitudes pendientes)
//!   friends/{uid}/{amigo} = true                  (en las dos direcciones)
//!   presence/{uid} = { name, state, pack, …, updated }   (solo la leen los amigos)
//!   servers/{uid}/{clave} = { name, pack, address, online, players, updated, guests: {uid: true} }
//!                                                 (servidores de «Hostear partida»: solo los leen los invitados)
//!   invites/{invitado}/{anfitrión}/{clave} = true (a qué servidores te invitaron)
//! Una cuenta de Google puede reservar varios nombres: los de cada cuenta de Parabellum que use.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use crate::firebase::{self, Database, DbError, Listener, Session};

/// Cada minuto se publica la presencia; sin noticias en 3 minutos, el amigo está desconectado.
const HEARTBEAT: Duration = Duration::from_secs(60);
const OFFLINE_AFTER_MS: i64 = 3 * 60 * 1000;

/// Lo que Parabellum publica del jugador: si está jugando, a qué y si tiene un mundo abierto.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Presence {
    /// "online" o "playing"
    pub state: String,
    pub pack: Option<String>,
    pub pack_folder: Option<String>,
    pub minecraft: Option<String>,
    /// "single" (mundo propio) o "server"
    pub world: Option<String>,
    pub server: Option<String>,
    /// Dirección pública de e4mc del mundo abierto.
    pub address: Option<String>,
    /// IP:puerto en la red local.
    pub lan: Option<String>,
    /// Nombre del servidor que tiene encendido en «Hostear partida» (lo ven todos sus amigos;
    /// la dirección solo los invitados).
    pub hosting: Option<String>,
    /// Con qué nombre está jugando (los servidores de los amigos lo comprueban al entrar).
    pub player: Option<String>,
    /// Dirección P2P de su Parabellum ("id|relevo"): con ella sus amigos entran directo a sus
    /// servidores y mundos abiertos, y él sabe quién se le conecta.
    pub p2p: Option<String>,
}

impl Presence {
    pub fn online() -> Self {
        Presence { state: "online".into(), ..Default::default() }
    }

    fn to_json(&self, name: &str) -> Value {
        let mut json = json!({ "name": name, "state": self.state, "updated": firebase::server_timestamp() });
        let fields = [
            ("pack", &self.pack),
            ("packFolder", &self.pack_folder),
            ("mc", &self.minecraft),
            ("world", &self.world),
            ("server", &self.server),
            ("address", &self.address),
            ("lan", &self.lan),
            ("hosting", &self.hosting),
            ("player", &self.player),
            ("p2p", &self.p2p),
        ];
        for (key, value) in fields {
            if let Some(value) = value.as_deref().filter(|v| !v.is_empty()) {
                json[key] = json!(value.chars().take(200).collect::<String>());
            }
        }
        json
    }

    fn from_json(node: &Value) -> Self {
        let text = |key: &str| node[key].as_str().map(String::from);
        Presence {
            state: text("state").unwrap_or_else(|| "online".into()),
            pack: text("pack"),
            pack_folder: text("packFolder"),
            minecraft: text("mc"),
            world: text("world"),
            server: text("server"),
            address: text("address"),
            lan: text("lan"),
            hosting: text("hosting"),
            player: text("player"),
            p2p: text("p2p"),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Friend {
    pub uid: String,
    pub name: String,
    pub online: bool,
    pub presence: Option<Presence>,
    /// Última vez que se supo de él (milisegundos Unix).
    pub last_seen: Option<i64>,
    pub world_open: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Request {
    pub uid: String,
    pub name: String,
}

/// Un servidor de un amigo al que te invitó (con su dirección actual).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendServer {
    pub host: String,
    pub host_name: String,
    pub key: String,
    pub name: String,
    pub pack: Option<String>,
    pub pack_folder: Option<String>,
    pub modpack: Option<String>,
    pub project: Option<String>,
    pub minecraft: Option<String>,
    pub loader: Option<String>,
    pub address: Option<String>,
    /// Misma red (al unirse se prueba junto con P2P y playit.gg).
    pub lan: Option<String>,
    /// Dirección P2P del anfitrión (conexión directa entre las dos PCs).
    pub p2p: Option<String>,
    pub online: bool,
    pub players: u64,
    pub max_players: u64,
    /// Ticks por segundo del servidor (20 = sin atrasos).
    pub tps: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub friends: Vec<Friend>,
    pub requests: Vec<Request>,
    /// Servidores de amigos a los que te invitaron.
    pub servers: Vec<FriendServer>,
    /// Nombre con el que te ven (None si aún no se reservó).
    pub name: Option<String>,
}

pub enum ClaimError {
    /// El nombre ya es de otro jugador de Parabellum.
    Taken,
    Other(String),
}

#[derive(Default)]
struct Inner {
    friend_ids: HashSet<String>,
    names: HashMap<String, String>,
    presence: HashMap<String, Value>,
    presence_listeners: HashMap<String, Listener>,
    requests: Vec<Request>,
    mine: Presence,
    dirty: bool,
    server_offset_ms: i64,
    name: Option<String>,
    /// "anfitrión/clave" -> escucha de ese servidor.
    invite_listeners: HashMap<String, Listener>,
    /// "anfitrión/clave" -> datos del servidor.
    servers: HashMap<String, Value>,
}

type Callback = Box<dyn Fn() + Send + Sync>;
type ErrorCallback = Box<dyn Fn(String) + Send + Sync>;

pub struct Social {
    db: Arc<Database>,
    pub uid: String,
    inner: Mutex<Inner>,
    tasks: Mutex<Vec<Listener>>,
    publishing: tokio::sync::Mutex<()>,
    changed: Callback,
    failed: ErrorCallback,
    /// La última vez que se releyó la presencia de todos (refresh_presence).
    presence_refreshed: Mutex<Option<std::time::Instant>>,
}

impl Social {
    pub async fn connect(
        api_key: &str,
        database_url: &str,
        google_id_token: &str,
        changed: impl Fn() + Send + Sync + 'static,
        failed: impl Fn(String) + Send + Sync + 'static,
    ) -> Result<Arc<Self>, String> {
        let session = Session::sign_in_with_google(api_key, google_id_token).await?;
        let db = Arc::new(Database::new(database_url, session));
        Ok(Arc::new(Social {
            uid: db.session.uid.clone(),
            db,
            inner: Mutex::new(Inner { mine: Presence::online(), dirty: true, ..Default::default() }),
            tasks: Mutex::default(),
            publishing: tokio::sync::Mutex::new(()),
            changed: Box::new(changed),
            failed: Box::new(failed),
            presence_refreshed: Mutex::default(),
        }))
    }

    /// Vuelve a leer la presencia de todos los amigos (por si una escucha se quedó atrás: p. ej. entra
    /// por P2P alguien que no se reconoce). Como mucho una vez cada 10 s.
    pub fn refresh_presence(self: &Arc<Self>) {
        {
            let mut last = self.presence_refreshed.lock().unwrap();
            if last.is_some_and(|at| at.elapsed() < Duration::from_secs(10)) {
                return;
            }
            *last = Some(std::time::Instant::now());
        }
        let social = self.clone();
        tauri::async_runtime::spawn(async move {
            let ids: Vec<String> = social.inner.lock().unwrap().friend_ids.iter().cloned().collect();
            // En p2p.log, lo que impide reconocer a un amigo: su presencia no se deja leer (¿te quitó
            // de amigos?) o no tiene dirección P2P (su Parabellum es viejo o no llega al relevo).
            let mut missing = Vec::new();
            for uid in &ids {
                match social.db.get(&format!("presence/{uid}")).await {
                    Ok(presence) if !presence.is_null() => {
                        if presence["p2p"].as_str().is_none_or(str::is_empty) {
                            missing.push(format!("{} sin dirección P2P", presence["name"].as_str().unwrap_or(uid)));
                        }
                        social.inner.lock().unwrap().presence.insert(uid.clone(), presence);
                    }
                    Ok(_) => missing.push(format!("{uid} sin presencia")),
                    Err(error) => missing.push(format!("{uid}: {error}")),
                }
            }
            crate::p2p::note(format!("presencia releída ({} amigos){}", ids.len(), if missing.is_empty() { String::new() } else { format!(": {}", missing.join("; ")) }));
            (social.changed)();
        });
    }

    /// Empieza a escuchar amigos y solicitudes y a publicar la presencia cada minuto.
    pub fn start(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let on_error = |weak: Weak<Social>| move |error: String| {
            if let Some(social) = weak.upgrade() {
                (social.failed)(error);
            }
        };
        let friends = {
            let (weak, failed) = (weak.clone(), on_error(weak.clone()));
            self.db.listen(
                format!("friends/{}", self.uid),
                move |data| {
                    if let Some(social) = weak.upgrade() {
                        social.update_friends(&data);
                    }
                },
                failed,
            )
        };
        let requests = {
            let (weak, failed) = (weak.clone(), on_error(weak.clone()));
            self.db.listen(
                format!("requests/{}", self.uid),
                move |data| {
                    if let Some(social) = weak.upgrade() {
                        social.update_requests(&data);
                    }
                },
                failed,
            )
        };
        let invites = {
            let (weak, failed) = (weak.clone(), on_error(weak.clone()));
            self.db.listen(
                format!("invites/{}", self.uid),
                move |data| {
                    if let Some(social) = weak.upgrade() {
                        social.update_invites(&data);
                    }
                },
                failed,
            )
        };
        let heartbeat = tauri::async_runtime::spawn(async move {
            loop {
                match weak.upgrade() {
                    Some(social) => {
                        social.publish(true).await;
                        // Recalcula quién lleva demasiado tiempo sin noticias.
                        (social.changed)();
                    }
                    None => return,
                }
                tokio::time::sleep(HEARTBEAT).await;
            }
        });
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(friends);
        tasks.push(requests);
        tasks.push(invites);
        tasks.push(Listener(heartbeat));
    }

    /// La base de datos y la sesión, para el chat.
    pub fn database(&self) -> Arc<Database> {
        self.db.clone()
    }

    pub fn name(&self) -> Option<String> {
        self.inner.lock().unwrap().name.clone()
    }

    // ================= Nombre

    /// Reserva el nombre para esta cuenta de Google. Falla si ya es de otro jugador.
    pub async fn claim_name(&self, name: &str, uuid: &str) -> Result<(), ClaimError> {
        let key = name.to_lowercase();
        let owner = self.db.get(&format!("names/{key}")).await.map_err(|e| ClaimError::Other(e.to_string()))?;
        if owner.as_str().is_some_and(|owner| owner != self.uid) {
            return Err(ClaimError::Taken);
        }
        let update = json!({
            format!("names/{key}"): self.uid,
            format!("users/{}", self.uid): { "name": name, "uuid": uuid },
        });
        match self.db.patch("", &update).await {
            Ok(_) => {}
            // Otro jugador lo reservó justo antes.
            Err(DbError::Permission(_)) => return Err(ClaimError::Taken),
            Err(error) => return Err(ClaimError::Other(error.to_string())),
        }
        let mut inner = self.inner.lock().unwrap();
        inner.name = Some(name.to_string());
        inner.dirty = true;
        Ok(())
    }

    // ================= Amigos

    pub fn view(&self) -> View {
        let inner = self.inner.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis() + inner.server_offset_ms;
        let mut friends: Vec<Friend> = inner
            .friend_ids
            .iter()
            .map(|uid| {
                let node = inner.presence.get(uid).filter(|n| n.is_object());
                let last_seen = node.and_then(|n| n["updated"].as_i64());
                let online = node.is_some_and(|n| matches!(n["state"].as_str(), Some("online" | "playing")))
                    && last_seen.is_some_and(|seen| now - seen < OFFLINE_AFTER_MS);
                let presence = node.filter(|_| online).map(Presence::from_json);
                let world_open = presence.as_ref().is_some_and(|p| p.address.is_some() || p.lan.is_some());
                Friend {
                    uid: uid.clone(),
                    name: node
                        .and_then(|n| n["name"].as_str().filter(|s| !s.is_empty()).map(String::from))
                        .or_else(|| inner.names.get(uid).cloned())
                        .unwrap_or_else(|| "…".into()),
                    online,
                    presence,
                    last_seen,
                    world_open,
                }
            })
            .collect();
        // Primero quien tiene el mundo abierto, luego quien juega, quien está en línea y el resto.
        friends.sort_by_key(|f| (!f.world_open, !(f.online && f.presence.as_ref().is_some_and(|p| p.state == "playing")), !f.online, f.name.to_lowercase()));
        let mut servers: Vec<FriendServer> = inner
            .servers
            .iter()
            .filter(|(_, data)| data.is_object())
            .filter_map(|(path, data)| {
                let (host, key) = path.split_once('/')?;
                let text = |field: &str| data[field].as_str().filter(|s| !s.is_empty()).map(String::from);
                let fresh = data["updated"].as_i64().is_some_and(|updated| now - updated < OFFLINE_AFTER_MS);
                Some(FriendServer {
                    host: host.to_string(),
                    host_name: text("hostName").or_else(|| inner.names.get(host).cloned()).unwrap_or_else(|| "…".into()),
                    key: key.to_string(),
                    name: text("name").unwrap_or_else(|| "…".into()),
                    pack: text("pack"),
                    pack_folder: text("packFolder"),
                    modpack: text("modpack"),
                    project: text("project"),
                    minecraft: text("mc"),
                    loader: text("loader"),
                    address: text("address"),
                    lan: text("lan"),
                    p2p: text("p2p"),
                    online: fresh && data["online"].as_bool() == Some(true),
                    players: data["players"].as_u64().unwrap_or(0),
                    max_players: data["max"].as_u64().unwrap_or(0),
                    tps: data["tps"].as_f64(),
                })
            })
            .collect();
        servers.sort_by_key(|s| (!s.online, s.name.to_lowercase()));
        View { friends, requests: inner.requests.clone(), servers, name: inner.name.clone() }
    }

    /// El amigo está jugando ahora mismo desde Parabellum, con ese nombre (lo publica su Parabellum;
    /// las versiones anteriores a la 0.15 solo publican el nombre de su cuenta).
    pub fn playing_as(&self, uid: &str, name: &str) -> bool {
        let inner = self.inner.lock().unwrap();
        let now = chrono::Utc::now().timestamp_millis() + inner.server_offset_ms;
        let Some(node) = inner.presence.get(uid).filter(|n| n.is_object()) else { return false };
        let fresh = node["updated"].as_i64().is_some_and(|seen| now - seen < OFFLINE_AFTER_MS);
        let player = node["player"].as_str().or(node["name"].as_str()).unwrap_or_default();
        fresh && node["state"].as_str() == Some("playing") && player.eq_ignore_ascii_case(name)
    }

    /// El amigo que tiene esa identidad P2P (la publica su Parabellum en la presencia): (uid, nombre).
    pub fn friend_by_p2p(&self, id: &str) -> Option<(String, String)> {
        let inner = self.inner.lock().unwrap();
        inner.friend_ids.iter().find_map(|uid| {
            let node = inner.presence.get(uid)?;
            let published = node["p2p"].as_str()?;
            (published.split('|').next() == Some(id)).then(|| (uid.clone(), node["name"].as_str().unwrap_or("?").to_string()))
        })
    }

    /// Nombre de cada amigo (uid -> nombre), para la lista blanca de los servidores.
    pub fn friend_names(&self) -> HashMap<String, String> {
        self.view().friends.into_iter().filter(|f| f.name != "…").map(|f| (f.uid, f.name)).collect()
    }

    // ================= Servidores de «Hostear partida»

    /// Publica (o actualiza) un servidor propio para sus invitados.
    pub async fn publish_server(&self, key: &str, mut data: Value) -> Result<(), String> {
        data["hostName"] = json!(self.name().unwrap_or_default());
        data["updated"] = firebase::server_timestamp();
        self.db.put(&format!("servers/{}/{key}", self.uid), &data).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Invita (o deja de invitar) a un amigo a un servidor propio.
    pub async fn set_invite(&self, key: &str, guest: &str, invited: bool) -> Result<(), String> {
        let path = format!("invites/{guest}/{}/{key}", self.uid);
        let result = if invited { self.db.put(&path, &json!(true)).await.map(|_| ()) } else { self.db.delete(&path).await };
        result.map_err(|e| e.to_string())
    }

    /// Borra un servidor propio y sus invitaciones.
    pub async fn remove_server(&self, key: &str, guests: &[String]) -> Result<(), String> {
        for guest in guests {
            let _ = self.set_invite(key, guest, false).await;
        }
        self.db.delete(&format!("servers/{}/{key}", self.uid)).await.map_err(|e| e.to_string())
    }

    /// El invitado quita de su lista un servidor de un amigo.
    pub async fn leave_server(&self, host: &str, key: &str) -> Result<(), String> {
        self.db.delete(&format!("invites/{}/{host}/{key}", self.uid)).await.map_err(|e| e.to_string())
    }

    fn update_invites(self: &Arc<Self>, data: &Value) {
        let wanted: HashSet<String> = data
            .as_object()
            .into_iter()
            .flatten()
            .flat_map(|(host, keys)| keys.as_object().into_iter().flatten().filter(|(_, v)| v.as_bool() == Some(true)).map(move |(key, _)| format!("{host}/{key}")))
            .filter(|path| !path.contains(['.', '#', '$', '[', ']']))
            .collect();
        {
            let mut inner = self.inner.lock().unwrap();
            inner.invite_listeners.retain(|path, _| wanted.contains(path));
            inner.servers.retain(|path, _| wanted.contains(path));
            let added: Vec<String> = wanted.iter().filter(|p| !inner.invite_listeners.contains_key(*p)).cloned().collect();
            for path in added {
                let weak = Arc::downgrade(self);
                let key = path.clone();
                let listener = self.db.listen(
                    format!("servers/{path}"),
                    move |server| {
                        if let Some(social) = weak.upgrade() {
                            social.inner.lock().unwrap().servers.insert(key.clone(), server);
                            (social.changed)();
                        }
                    },
                    |_| {},
                );
                inner.invite_listeners.insert(path, listener);
            }
        }
        (self.changed)();
    }

    /// Envía una solicitud al jugador con ese nombre. Si él ya te la había enviado, se hacen amigos.
    pub async fn send_request(&self, name: &str) -> Result<String, String> {
        let my_name = self.name().ok_or(t!("Primero necesitas tu nombre reservado (revisa el aviso de Amigos).", "First you need your name reserved (check the notice in Friends)."))?;
        let target = self.db.get(&format!("names/{}", name.to_lowercase())).await.map_err(|e| e.to_string())?;
        let target = target.as_str().ok_or(t!("No hay ningún jugador de Parabellum llamado \"{name}\".", "There is no Parabellum player named \"{name}\"."))?.to_string();
        if target == self.uid {
            return Err(t!("Ese nombre es tuyo.", "That name is yours."));
        }
        let (already, they_asked) = {
            let inner = self.inner.lock().unwrap();
            (inner.friend_ids.contains(&target), inner.requests.iter().any(|r| r.uid == target))
        };
        if already {
            return Ok(t!("{name} ya es tu amigo.", "{name} is already your friend."));
        }
        if they_asked {
            self.accept(&target).await?;
            return Ok(t!("{name} ya te había enviado una solicitud: ahora son amigos.", "{name} had already sent you a request: you are now friends."));
        }
        self.db
            .put(&format!("requests/{target}/{}", self.uid), &json!({ "name": my_name, "at": firebase::server_timestamp() }))
            .await
            .map_err(|e| e.to_string())?;
        Ok(t!("Solicitud enviada a {name}. Aparecerá en tu lista cuando la acepte.", "Request sent to {name}. They will appear in your list when they accept it."))
    }

    pub async fn accept(&self, uid: &str) -> Result<(), String> {
        let me = &self.uid;
        let update = json!({
            format!("friends/{me}/{uid}"): true,
            format!("friends/{uid}/{me}"): true,
            format!("requests/{me}/{uid}"): null,
        });
        self.db.patch("", &update).await.map(|_| ()).map_err(|e| e.to_string())
    }

    pub async fn decline(&self, uid: &str) -> Result<(), String> {
        self.db.delete(&format!("requests/{}/{uid}", self.uid)).await.map_err(|e| e.to_string())
    }

    pub async fn remove_friend(&self, uid: &str) -> Result<(), String> {
        let me = &self.uid;
        let update = json!({ format!("friends/{me}/{uid}"): null, format!("friends/{uid}/{me}"): null });
        self.db.patch("", &update).await.map(|_| ()).map_err(|e| e.to_string())
    }

    fn update_friends(self: &Arc<Self>, data: &Value) {
        let ids: HashSet<String> = data.as_object().map(|o| o.iter().filter(|(_, v)| !v.is_null()).map(|(k, _)| k.clone()).collect()).unwrap_or_default();
        let added: Vec<String> = {
            let mut inner = self.inner.lock().unwrap();
            let removed: Vec<String> = inner.friend_ids.difference(&ids).cloned().collect();
            for uid in removed {
                inner.presence_listeners.remove(&uid);
                inner.presence.remove(&uid);
            }
            let added: Vec<String> = ids.difference(&inner.friend_ids).cloned().collect();
            inner.friend_ids = ids;
            for uid in &added {
                let weak = Arc::downgrade(self);
                let friend = uid.clone();
                let listener = self.db.listen(
                    format!("presence/{uid}"),
                    move |presence| {
                        if let Some(social) = weak.upgrade() {
                            social.inner.lock().unwrap().presence.insert(friend.clone(), presence);
                            (social.changed)();
                        }
                    },
                    |_| {},
                );
                inner.presence_listeners.insert(uid.clone(), listener);
            }
            added
        };
        // El nombre sale de la presencia, pero un amigo que nunca se conectó no tiene: se lee de su perfil.
        for uid in added {
            let social = self.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(Value::String(name)) = social.db.get(&format!("users/{uid}/name")).await {
                    social.inner.lock().unwrap().names.insert(uid, name);
                    (social.changed)();
                }
            });
        }
        (self.changed)();
    }

    fn update_requests(&self, data: &Value) {
        let requests = data
            .as_object()
            .map(|o| {
                o.iter()
                    .filter(|(_, v)| v.is_object())
                    .map(|(uid, v)| Request { uid: uid.clone(), name: v["name"].as_str().unwrap_or("?").to_string() })
                    .collect()
            })
            .unwrap_or_default();
        self.inner.lock().unwrap().requests = requests;
        (self.changed)();
    }

    // ================= Presencia

    pub fn set_presence(self: &Arc<Self>, presence: Presence) {
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.mine == presence {
                return;
            }
            inner.mine = presence;
            inner.dirty = true;
        }
        let social = self.clone();
        tauri::async_runtime::spawn(async move { social.publish(false).await });
    }

    pub fn my_presence(&self) -> Presence {
        self.inner.lock().unwrap().mine.clone()
    }

    async fn publish(&self, force: bool) {
        let _guard = self.publishing.lock().await;
        let (presence, name) = {
            let mut inner = self.inner.lock().unwrap();
            if !force && !inner.dirty {
                return;
            }
            inner.dirty = false;
            (inner.mine.clone(), inner.name.clone().unwrap_or_default())
        };
        let sent = chrono::Utc::now().timestamp_millis();
        match self.db.put(&format!("presence/{}", self.uid), &presence.to_json(&name)).await {
            Ok(result) => {
                if let Some(server) = result["updated"].as_i64() {
                    let local = (sent + chrono::Utc::now().timestamp_millis()) / 2;
                    self.inner.lock().unwrap().server_offset_ms = server - local;
                    self.db.set_server_offset(server - local);
                }
            }
            Err(error) => {
                self.inner.lock().unwrap().dirty = true;
                (self.failed)(error.to_string());
            }
        }
    }

    /// Al salir de Parabellum: los amigos te ven desconectado al momento (y no tras 3 minutos).
    pub async fn go_offline(&self) {
        self.tasks.lock().unwrap().clear();
        self.inner.lock().unwrap().presence_listeners.clear();
        self.inner.lock().unwrap().invite_listeners.clear();
        let name = self.name().unwrap_or_default();
        let offline = json!({ "name": name, "state": "offline", "updated": firebase::server_timestamp() });
        let _ = tokio::time::timeout(Duration::from_secs(3), self.db.put(&format!("presence/{}", self.uid), &offline)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presencia_de_ida_y_vuelta() {
        let presence = Presence {
            state: "playing".into(),
            pack: Some("Better MC BMC4".into()),
            pack_folder: Some("Better MC BMC4".into()),
            minecraft: Some("1.20.1".into()),
            world: Some("single".into()),
            address: Some("silly-cat.sa.e4mc.link".into()),
            lan: Some("192.168.1.20:51234".into()),
            server: None,
            hosting: Some("LosPanas".into()),
            player: Some("Juan_07".into()),
            p2p: Some("0a1b2c|https://use1-1.relay.n0.iroh.link./".into()),
        };
        let json = presence.to_json("GxxseS");
        assert_eq!(json["mc"], "1.20.1");
        assert!(json.get("server").is_none());
        assert_eq!(Presence::from_json(&json), presence);
    }
}
