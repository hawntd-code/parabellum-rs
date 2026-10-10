//! Amigos y presencia sobre Firebase Realtime Database. Estructura (reglas en firebase-reglas.json):
//!   names/{nombre en minúsculas} = uid            (cada nombre es de una sola cuenta de Google)
//!   users/{uid} = { name, uuid }
//!   requests/{destino}/{origen} = { name, at }    (solicitudes pendientes)
//!   friends/{uid}/{amigo} = true                  (en las dos direcciones)
//!   presence/{uid} = { name, state, pack, …, updated }   (solo la leen los amigos)
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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub friends: Vec<Friend>,
    pub requests: Vec<Request>,
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
        }))
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
        View { friends, requests: inner.requests.clone(), name: inner.name.clone() }
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
        };
        let json = presence.to_json("GxxseS");
        assert_eq!(json["mc"], "1.20.1");
        assert!(json.get("server").is_none());
        assert_eq!(Presence::from_json(&json), presence);
    }
}
