//! Chat con amigos y grupos sobre la misma Realtime Database (reglas en firebase-reglas.json):
//!   chats/{chat}/members/{uid} = true       (directo: "d_{uid}_{uid}" ordenados; grupo: "g_…")
//!   chats/{chat}/meta = { kind, name, created, lastText, lastName, lastUid, lastAt }
//!   messages/{chat}/{mensaje} = { from, name, text | image, w, h, at }
//!   images/{chat}/{mensaje} = "data:image/jpeg;base64,…"   (aparte: la lista de mensajes es ligera)
//!   userChats/{uid}/{chat} = true            (las conversaciones de cada jugador)
//!   reads/{uid}/{chat} = hora de la última lectura
//! Las imágenes llegan ya reducidas desde la interfaz (máximo ~330 KB).

use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};

use crate::firebase::{self, Database, Listener};

/// Mensajes que se cargan de una conversación (los más nuevos).
const HISTORY: usize = 150;
pub const MAX_TEXT: usize = 2000;
pub const MAX_IMAGE: usize = 450_000;

#[derive(Clone, Debug, Serialize)]
pub struct Member {
    pub uid: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    /// "directo" o "grupo"
    pub kind: String,
    /// Nombre del grupo, o del amigo en un chat directo.
    pub name: String,
    pub members: Vec<Member>,
    pub last_text: String,
    pub last_name: String,
    pub last_at: i64,
    pub unread: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub from: String,
    pub name: String,
    pub text: Option<String>,
    pub image: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub at: i64,
    pub mine: bool,
}

#[derive(Default)]
struct Inner {
    chat_ids: HashSet<String>,
    chats: HashMap<String, Value>,
    chat_listeners: HashMap<String, Listener>,
    reads: HashMap<String, i64>,
    names: HashMap<String, String>,
    asked_names: HashSet<String>,
    /// La conversación abierta en la ventana y sus mensajes.
    open: Option<(String, Listener)>,
    messages: Value,
    /// Último mensaje conocido de cada chat (para avisar solo de lo nuevo).
    last_seen: HashMap<String, i64>,
}

type Notify = Box<dyn Fn() + Send + Sync>;
type Arrived = Box<dyn Fn(Summary) + Send + Sync>;
type Messages = Box<dyn Fn(String, Vec<Message>) + Send + Sync>;

pub struct Chat {
    db: Arc<Database>,
    uid: String,
    my_name: Mutex<String>,
    images: PathBuf,
    inner: Mutex<Inner>,
    tasks: Mutex<Vec<Listener>>,
    /// La lista de conversaciones cambió.
    changed: Notify,
    /// Llegó un mensaje nuevo de otro jugador (para avisar).
    arrived: Arrived,
    /// Cambiaron los mensajes de la conversación abierta.
    messages: Messages,
}

fn random_text(len: usize) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = vec![0u8; len];
    getrandom::getrandom(&mut bytes).expect("números aleatorios");
    bytes.iter().map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char).collect()
}

/// Id de mensaje ordenable por hora (los ids de Firebase se ordenan como texto).
fn message_id() -> String {
    format!("{:013}{}", chrono::Utc::now().timestamp_millis(), random_text(7))
}

pub fn direct_id(a: &str, b: &str) -> String {
    let (first, second) = if a < b { (a, b) } else { (b, a) };
    format!("d_{first}_{second}")
}

/// Ids que vienen de la base de datos y se usan en rutas: solo letras, números, guion y guion bajo.
fn safe(id: &str) -> Result<&str, String> {
    if !id.is_empty() && id.len() <= 200 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        Ok(id)
    } else {
        Err(t!("Conversación no válida.", "Invalid conversation."))
    }
}

fn preview(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 80 {
        format!("{}…", line.chars().take(79).collect::<String>())
    } else {
        line
    }
}

impl Chat {
    pub fn new(
        db: Arc<Database>,
        my_name: String,
        images: PathBuf,
        changed: impl Fn() + Send + Sync + 'static,
        arrived: impl Fn(Summary) + Send + Sync + 'static,
        messages: impl Fn(String, Vec<Message>) + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(Chat {
            uid: db.session.uid.clone(),
            db,
            my_name: Mutex::new(my_name),
            images,
            inner: Mutex::default(),
            tasks: Mutex::default(),
            changed: Box::new(changed),
            arrived: Box::new(arrived),
            messages: Box::new(messages),
        })
    }

    pub fn set_my_name(&self, name: &str) {
        *self.my_name.lock().unwrap() = name.to_string();
    }

    /// Empieza a escuchar las conversaciones del jugador y lo que ya leyó.
    pub fn start(self: &Arc<Self>) {
        let weak = Arc::downgrade(self);
        let chats = {
            let weak = weak.clone();
            self.db.listen(
                format!("userChats/{}", self.uid),
                move |data| {
                    if let Some(chat) = weak.upgrade() {
                        chat.update_chat_ids(&data);
                    }
                },
                |_| {},
            )
        };
        let reads = self.db.listen(
            format!("reads/{}", self.uid),
            move |data| {
                if let Some(chat) = weak.upgrade() {
                    let reads = data.as_object().map(|o| o.iter().filter_map(|(k, v)| Some((k.clone(), v.as_i64()?))).collect()).unwrap_or_default();
                    chat.inner.lock().unwrap().reads = reads;
                    (chat.changed)();
                }
            },
            |_| {},
        );
        let mut tasks = self.tasks.lock().unwrap();
        tasks.push(chats);
        tasks.push(reads);
    }

    pub fn stop(&self) {
        self.tasks.lock().unwrap().clear();
        let mut inner = self.inner.lock().unwrap();
        inner.chat_listeners.clear();
        inner.open = None;
    }

    fn update_chat_ids(self: &Arc<Self>, data: &Value) {
        let ids: HashSet<String> =
            data.as_object().map(|o| o.iter().filter(|(k, v)| v.as_bool() == Some(true) && safe(k).is_ok()).map(|(k, _)| k.clone()).collect()).unwrap_or_default();
        {
            let mut inner = self.inner.lock().unwrap();
            let removed: Vec<String> = inner.chat_ids.difference(&ids).cloned().collect();
            for id in removed {
                inner.chat_listeners.remove(&id);
                inner.chats.remove(&id);
            }
            let added: Vec<String> = ids.difference(&inner.chat_ids).cloned().collect();
            inner.chat_ids = ids;
            for id in added {
                let weak: Weak<Chat> = Arc::downgrade(self);
                let chat_id = id.clone();
                let listener = self.db.listen(
                    format!("chats/{id}"),
                    move |data| {
                        if let Some(chat) = weak.upgrade() {
                            chat.update_chat(&chat_id, data);
                        }
                    },
                    |_| {},
                );
                inner.chat_listeners.insert(id, listener);
            }
        }
        (self.changed)();
    }

    fn update_chat(self: &Arc<Self>, id: &str, data: Value) {
        let mut arrived = None;
        let unknown: Vec<String>;
        {
            let mut inner = self.inner.lock().unwrap();
            let last_at = data.pointer("/meta/lastAt").and_then(Value::as_i64).unwrap_or(0);
            let last_uid = data.pointer("/meta/lastUid").and_then(Value::as_str).unwrap_or_default().to_string();
            // Solo es nuevo si ya se conocía el chat (al conectar llega todo lo anterior).
            if let Some(previous) = inner.last_seen.insert(id.to_string(), last_at) {
                if last_at > previous && !last_uid.is_empty() && last_uid != self.uid {
                    arrived = Some(id.to_string());
                }
            }
            inner.chats.insert(id.to_string(), data.clone());
            unknown = data["members"]
                .as_object()
                .map(|m| m.keys().filter(|uid| !inner.names.contains_key(*uid) && !inner.asked_names.contains(*uid)).cloned().collect())
                .unwrap_or_default();
            for uid in &unknown {
                inner.asked_names.insert(uid.clone());
            }
        }
        // Nombres de los miembros (se leen una vez de su perfil).
        for uid in unknown {
            let chat = self.clone();
            tauri::async_runtime::spawn(async move {
                if let Ok(Value::String(name)) = chat.db.get(&format!("users/{uid}/name")).await {
                    chat.inner.lock().unwrap().names.insert(uid, name);
                    (chat.changed)();
                }
            });
        }
        (self.changed)();
        if let Some(summary) = arrived.and_then(|id| self.summary(&id)) {
            (self.arrived)(summary);
        }
    }

    fn summary_from(&self, inner: &Inner, id: &str, data: &Value) -> Summary {
        let meta = &data["meta"];
        let members: Vec<Member> = data["members"]
            .as_object()
            .map(|m| {
                m.iter()
                    .filter(|(_, v)| v.as_bool() == Some(true))
                    .map(|(uid, _)| Member { uid: uid.clone(), name: inner.names.get(uid).cloned().unwrap_or_else(|| "…".into()) })
                    .collect()
            })
            .unwrap_or_default();
        let kind = meta["kind"].as_str().unwrap_or(if id.starts_with("g_") { "grupo" } else { "directo" }).to_string();
        let name = if kind == "grupo" {
            meta["name"].as_str().filter(|n| !n.is_empty()).unwrap_or("Grupo").to_string()
        } else {
            members.iter().find(|m| m.uid != self.uid).map(|m| m.name.clone()).unwrap_or_else(|| "…".into())
        };
        let last_at = meta["lastAt"].as_i64().unwrap_or(0);
        let last_uid = meta["lastUid"].as_str().unwrap_or_default();
        let read = inner.reads.get(id).copied().unwrap_or(0);
        Summary {
            id: id.to_string(),
            kind,
            name,
            members,
            last_text: meta["lastText"].as_str().unwrap_or_default().to_string(),
            last_name: meta["lastName"].as_str().unwrap_or_default().to_string(),
            last_at,
            unread: !last_uid.is_empty() && last_uid != self.uid && last_at > read,
        }
    }

    pub fn summary(&self, id: &str) -> Option<Summary> {
        let inner = self.inner.lock().unwrap();
        let data = inner.chats.get(id)?.clone();
        Some(self.summary_from(&inner, id, &data))
    }

    /// Conversaciones, la más reciente primero.
    pub fn list(&self) -> Vec<Summary> {
        let inner = self.inner.lock().unwrap();
        let mut list: Vec<Summary> = inner.chats.iter().filter(|(_, d)| d.is_object()).map(|(id, d)| self.summary_from(&inner, id, d)).collect();
        list.sort_by_key(|s| std::cmp::Reverse(s.last_at));
        list
    }

    pub fn open_chat(&self) -> Option<String> {
        self.inner.lock().unwrap().open.as_ref().map(|(id, _)| id.clone())
    }

    // ================= Conversación abierta

    /// Escucha los últimos mensajes de una conversación (la que se ve en la ventana).
    pub fn open(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let id = safe(id)?.to_string();
        if self.open_chat().as_deref() == Some(id.as_str()) {
            self.emit_messages();
            return Ok(());
        }
        let weak = Arc::downgrade(self);
        let chat_id = id.clone();
        let listener = self.db.listen(
            format!("messages/{id}?orderBy=\"$key\"&limitToLast={HISTORY}"),
            move |data| {
                if let Some(chat) = weak.upgrade() {
                    {
                        let mut inner = chat.inner.lock().unwrap();
                        if inner.open.as_ref().is_some_and(|(open, _)| *open == chat_id) {
                            inner.messages = data;
                        }
                    }
                    chat.emit_messages();
                }
            },
            |_| {},
        );
        let mut inner = self.inner.lock().unwrap();
        inner.messages = Value::Null;
        inner.open = Some((id, listener));
        Ok(())
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.open = None;
        inner.messages = Value::Null;
    }

    fn emit_messages(&self) {
        let (id, list) = {
            let inner = self.inner.lock().unwrap();
            let Some((id, _)) = &inner.open else { return };
            let mut list: Vec<Message> = inner
                .messages
                .as_object()
                .map(|o| {
                    o.iter()
                        .filter(|(_, m)| m.is_object())
                        .map(|(key, m)| Message {
                            id: key.clone(),
                            from: m["from"].as_str().unwrap_or_default().to_string(),
                            name: m["name"].as_str().unwrap_or("?").to_string(),
                            text: m["text"].as_str().map(String::from),
                            image: m["image"].as_bool() == Some(true),
                            width: m["w"].as_u64().map(|v| v as u32),
                            height: m["h"].as_u64().map(|v| v as u32),
                            at: m["at"].as_i64().unwrap_or(0),
                            mine: m["from"].as_str() == Some(self.uid.as_str()),
                        })
                        .collect()
                })
                .unwrap_or_default();
            list.sort_by(|a, b| a.id.cmp(&b.id));
            (id.clone(), list)
        };
        (self.messages)(id, list);
    }

    /// Marca la conversación como leída (hasta ahora).
    pub async fn mark_read(&self, id: &str) -> Result<(), String> {
        let id = safe(id)?;
        self.db.put(&format!("reads/{}/{id}", self.uid), &firebase::server_timestamp()).await.map(|_| ()).map_err(|e| e.to_string())
    }

    // ================= Enviar

    fn members_of(&self, id: &str) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        inner.chats.get(id).and_then(|d| d["members"].as_object()).map(|m| m.iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect()).unwrap_or_default()
    }

    /// Escribe el mensaje y actualiza la conversación en una sola operación.
    async fn send(&self, id: &str, message_id: &str, message: Value, last_text: &str, mut update: Map<String, Value>) -> Result<(), String> {
        let id = safe(id)?;
        let name = self.my_name.lock().unwrap().clone();
        update.insert(format!("messages/{id}/{message_id}"), message);
        update.insert(format!("chats/{id}/meta/lastText"), json!(last_text));
        update.insert(format!("chats/{id}/meta/lastName"), json!(name));
        update.insert(format!("chats/{id}/meta/lastUid"), json!(self.uid));
        update.insert(format!("chats/{id}/meta/lastAt"), firebase::server_timestamp());
        // Si alguien había ocultado la conversación, vuelve a aparecerle.
        for member in self.members_of(id) {
            update.insert(format!("userChats/{member}/{id}"), json!(true));
        }
        self.db.patch("", &Value::Object(update)).await.map(|_| ()).map_err(|e| e.to_string())?;
        let _ = self.mark_read(id).await;
        Ok(())
    }

    pub async fn send_text(&self, id: &str, text: &str) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        if text.chars().count() > MAX_TEXT {
            return Err(t!("El mensaje es muy largo (máximo {MAX_TEXT} letras).", "The message is too long (maximum {MAX_TEXT} characters)."));
        }
        let name = self.my_name.lock().unwrap().clone();
        let message = json!({ "from": self.uid, "name": name, "text": text, "at": firebase::server_timestamp() });
        self.send(id, &message_id(), message, &preview(text), Map::new()).await
    }

    /// `data`: imagen ya reducida, como "data:image/jpeg;base64,…". Va aparte, con el id de su mensaje.
    pub async fn send_image(&self, id: &str, data: &str, width: u32, height: u32) -> Result<(), String> {
        let id = safe(id)?;
        if !data.starts_with("data:image/") || !data.contains(";base64,") {
            return Err(t!("La imagen no es válida.", "The image is not valid."));
        }
        if data.len() > MAX_IMAGE {
            return Err(t!("La imagen es muy pesada incluso reducida.", "The image is too large even after shrinking it."));
        }
        let message_id = message_id();
        let name = self.my_name.lock().unwrap().clone();
        let message = json!({ "from": self.uid, "name": name, "image": true, "w": width, "h": height, "at": firebase::server_timestamp() });
        let mut update = Map::new();
        update.insert(format!("images/{id}/{message_id}"), json!(data));
        self.send(id, &message_id, message, "📷 Imagen", update).await
    }

    /// La imagen de un mensaje (de la caché en disco si ya se bajó).
    pub async fn image(&self, id: &str, message: &str) -> Result<String, String> {
        let (id, message) = (safe(id)?, safe(message)?);
        let cached = self.images.join(format!("{id}-{message}.txt"));
        if let Ok(data) = std::fs::read_to_string(&cached) {
            return Ok(data);
        }
        let data = self.db.get(&format!("images/{id}/{message}")).await.map_err(|e| e.to_string())?;
        let data = data.as_str().filter(|d| d.starts_with("data:image/")).ok_or(t!("La imagen ya no está.", "The image is no longer available."))?.to_string();
        let _ = std::fs::create_dir_all(&self.images);
        let _ = std::fs::write(&cached, &data);
        Ok(data)
    }

    // ================= Conversaciones y grupos

    /// La conversación con un amigo (se crea la primera vez).
    pub async fn direct(&self, friend: &str) -> Result<String, String> {
        let friend = safe(friend)?;
        let id = direct_id(&self.uid, friend);
        let me = &self.uid;
        let update = json!({
            format!("chats/{id}/members/{me}"): true,
            format!("chats/{id}/members/{friend}"): true,
            format!("chats/{id}/meta/kind"): "directo",
            format!("userChats/{me}/{id}"): true,
            format!("userChats/{friend}/{id}"): true,
        });
        self.db.patch("", &update).await.map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub async fn create_group(&self, name: &str, friends: &[String]) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err(t!("El nombre del grupo debe tener de 1 a 40 letras.", "The group name must have 1 to 40 characters."));
        }
        if friends.is_empty() {
            return Err(t!("Elige al menos un amigo.", "Choose at least one friend."));
        }
        let id = format!("g_{}", random_text(16));
        let mut update = Map::new();
        let me = self.uid.clone();
        for uid in std::iter::once(&me).chain(friends.iter()) {
            let uid = safe(uid)?;
            update.insert(format!("chats/{id}/members/{uid}"), json!(true));
            update.insert(format!("userChats/{uid}/{id}"), json!(true));
        }
        update.insert(
            format!("chats/{id}/meta"),
            json!({ "kind": "grupo", "name": name, "created": firebase::server_timestamp(), "lastText": "", "lastName": "", "lastUid": "", "lastAt": firebase::server_timestamp() }),
        );
        self.db.patch("", &Value::Object(update)).await.map_err(|e| e.to_string())?;
        Ok(id)
    }

    pub async fn add_members(&self, id: &str, friends: &[String]) -> Result<(), String> {
        let id = safe(id)?;
        let mut update = Map::new();
        for uid in friends {
            let uid = safe(uid)?;
            update.insert(format!("chats/{id}/members/{uid}"), json!(true));
            update.insert(format!("userChats/{uid}/{id}"), json!(true));
        }
        self.db.patch("", &Value::Object(update)).await.map(|_| ()).map_err(|e| e.to_string())
    }

    pub async fn rename_group(&self, id: &str, name: &str) -> Result<(), String> {
        let id = safe(id)?;
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err(t!("El nombre del grupo debe tener de 1 a 40 letras.", "The group name must have 1 to 40 characters."));
        }
        self.db.put(&format!("chats/{id}/meta/name"), &json!(name)).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Salir de un grupo, u ocultar una conversación directa (vuelve si el amigo escribe).
    pub async fn leave(&self, id: &str) -> Result<(), String> {
        let id = safe(id)?;
        let me = &self.uid;
        let mut update = json!({ format!("userChats/{me}/{id}"): null });
        if id.starts_with("g_") {
            update[format!("chats/{id}/members/{me}")] = Value::Null;
        }
        self.db.patch("", &update).await.map(|_| ()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_de_conversaciones_y_mensajes() {
        assert_eq!(direct_id("b", "a"), direct_id("a", "b"));
        assert_eq!(direct_id("zz", "aa"), "d_aa_zz");
        let first = message_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        assert!(message_id() > first);
        assert!(safe("d_abc_DEF-1").is_ok());
        assert!(safe("../x").is_err() && safe("a/b").is_err() && safe("").is_err());
        assert_eq!(preview("  hola\n  mundo "), "hola mundo");
    }
}
