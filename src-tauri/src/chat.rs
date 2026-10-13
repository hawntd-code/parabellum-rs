//! Chat con amigos y grupos sobre la misma Realtime Database (reglas en firebase-reglas.json):
//!   chats/{chat}/members/{uid} = true       (directo: "d_{uid}_{uid}" ordenados; grupo: "g_…")
//!   chats/{chat}/meta = { kind, name, created, lastText, lastName, lastUid, lastAt, owner, photo }
//!                                            (grupos: solo el dueño cambia nombre, foto y miembros;
//!                                            los de antes de la 0.27 no tienen dueño hasta que alguien
//!                                            los edita)
//!   messages/{chat}/{mensaje} = { from, name, text | image, w, h, at, edited, reply }
//!                                            (quien lo mandó puede editar el texto o borrarlo; reply es
//!                                            una copia corta del mensaje al que responde)
//!   reactions/{chat}/{mensaje}/{uid} = "👍"  (una por persona y mensaje)
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
/// La foto de un grupo (ya reducida por la interfaz a 128 px).
pub const MAX_PHOTO: usize = 40_000;
/// Las reacciones que se pueden poner, en el orden en que se muestran.
pub const REACTIONS: [&str; 6] = ["👍", "❤️", "😂", "😮", "😢", "🔥"];

#[derive(Clone, Debug, Serialize)]
pub struct Member {
    pub uid: String,
    pub name: String,
    /// Eres tú.
    pub you: bool,
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
    /// Grupos: quien lo creó (o a quien se lo pasaron). None en los de antes de la 0.27.
    pub owner: Option<String>,
    /// Grupos: su foto, como "data:image/jpeg;base64,…".
    pub photo: Option<String>,
    /// Puedes cambiar nombre, foto y miembros (eres el dueño, o el grupo todavía no tiene).
    pub can_edit: bool,
}

/// El mensaje al que responde otro: una copia corta (el original puede borrarse).
#[derive(Clone, Debug, Serialize)]
pub struct Reply {
    pub id: String,
    pub name: String,
    pub text: String,
}

/// Las reacciones de un mensaje con el mismo emoji.
#[derive(Clone, Debug, Serialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: usize,
    /// Una es tuya.
    pub mine: bool,
    pub names: Vec<String>,
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
    /// Se cambió el texto después de mandarlo.
    pub edited: bool,
    pub reply: Option<Reply>,
    pub reactions: Vec<Reaction>,
}

#[derive(Default)]
struct Inner {
    chat_ids: HashSet<String>,
    chats: HashMap<String, Value>,
    chat_listeners: HashMap<String, Listener>,
    reads: HashMap<String, i64>,
    names: HashMap<String, String>,
    asked_names: HashSet<String>,
    /// La conversación abierta en la ventana, sus mensajes y sus reacciones.
    open: Option<Open>,
    messages: Value,
    reactions: Value,
    /// Último mensaje conocido de cada chat (para avisar solo de lo nuevo).
    last_seen: HashMap<String, i64>,
}

struct Open {
    id: String,
    _messages: Listener,
    _reactions: Listener,
}

/// Lo que escucha el juego de una conversación (ver puente.rs): sus mensajes y sus reacciones.
pub struct Watch {
    _messages: Listener,
    _reactions: Listener,
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

/// Id de mensaje ordenable por hora (los ids de Firebase se ordenan como texto). `now`: la hora del
/// servidor, no la del PC (si el reloj de alguien va atrasado, sus mensajes quedarían arriba).
fn message_id(now: i64) -> String {
    format!("{:013}{}", now, random_text(7))
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

/// La foto de un grupo: una imagen ya reducida, como data URL.
fn check_photo(photo: &str) -> Result<(), String> {
    if !photo.starts_with("data:image/") || !photo.contains(";base64,") {
        return Err(t!("La foto no es válida.", "The photo is not valid."));
    }
    if photo.len() > MAX_PHOTO {
        return Err(t!("La foto pesa demasiado incluso reducida.", "The photo is too large even after shrinking it."));
    }
    Ok(())
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
                    .map(|(uid, _)| Member { uid: uid.clone(), name: inner.names.get(uid).cloned().unwrap_or_else(|| "…".into()), you: *uid == self.uid })
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
        // Si el dueño ya no está en el grupo, es como si no tuviera (cualquiera puede tomarlo).
        let owner = meta["owner"].as_str().filter(|o| members.iter().any(|m| m.uid == *o)).map(String::from);
        let can_edit = kind == "grupo" && owner.as_deref().is_none_or(|o| o == self.uid);
        let photo = meta["photo"].as_str().filter(|p| p.starts_with("data:image/")).map(String::from);
        Summary {
            id: id.to_string(),
            kind,
            name,
            members,
            last_text: meta["lastText"].as_str().unwrap_or_default().to_string(),
            last_name: meta["lastName"].as_str().unwrap_or_default().to_string(),
            last_at,
            unread: !last_uid.is_empty() && last_uid != self.uid && last_at > read,
            owner,
            photo,
            can_edit,
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
        self.inner.lock().unwrap().open.as_ref().map(|o| o.id.clone())
    }

    // ================= Conversación abierta

    /// Escucha los últimos mensajes de una conversación (la que se ve en la ventana).
    pub fn open(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let id = safe(id)?.to_string();
        if self.open_chat().as_deref() == Some(id.as_str()) {
            self.emit_messages();
            return Ok(());
        }
        let listen = |path: String, reactions: bool| {
            let weak = Arc::downgrade(self);
            let chat_id = id.clone();
            self.db.listen(
                format!("{path}/{id}?orderBy=\"$key\"&limitToLast={HISTORY}"),
                move |data| {
                    if let Some(chat) = weak.upgrade() {
                        {
                            let mut inner = chat.inner.lock().unwrap();
                            if inner.open.as_ref().is_some_and(|o| o.id == chat_id) {
                                if reactions {
                                    inner.reactions = data;
                                } else {
                                    inner.messages = data;
                                }
                            }
                        }
                        chat.emit_messages();
                    }
                },
                |_| {},
            )
        };
        let open = Open { _messages: listen("messages".into(), false), _reactions: listen("reactions".into(), true), id };
        let mut inner = self.inner.lock().unwrap();
        inner.messages = Value::Null;
        inner.reactions = Value::Null;
        inner.open = Some(open);
        Ok(())
    }

    pub fn close(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.open = None;
        inner.messages = Value::Null;
        inner.reactions = Value::Null;
    }

    /// Las reacciones de un mensaje ({uid: emoji}), juntas por emoji y en el orden de REACTIONS.
    fn parse_reactions(&self, data: &Value, names: &HashMap<String, String>) -> Vec<Reaction> {
        let mut list: Vec<Reaction> = Vec::new();
        for (uid, emoji) in data.as_object().into_iter().flatten() {
            let Some(emoji) = emoji.as_str().filter(|e| !e.is_empty()) else { continue };
            let name = names.get(uid).cloned().unwrap_or_else(|| "…".into());
            match list.iter_mut().find(|r| r.emoji == emoji) {
                Some(reaction) => {
                    reaction.count += 1;
                    reaction.mine |= *uid == self.uid;
                    reaction.names.push(name);
                }
                None => list.push(Reaction { emoji: emoji.to_string(), count: 1, mine: *uid == self.uid, names: vec![name] }),
            }
        }
        list.sort_by_key(|r| REACTIONS.iter().position(|e| *e == r.emoji).unwrap_or(REACTIONS.len()));
        list
    }

    /// Los mensajes de una conversación tal como llegan de Firebase, en orden, con sus reacciones.
    fn parse_messages(&self, data: &Value, reactions: &Value, names: &HashMap<String, String>) -> Vec<Message> {
        let mut list: Vec<Message> = data
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
                            edited: m["edited"].is_number(),
                            reply: m["reply"]["id"].as_str().map(|id| Reply {
                                id: id.to_string(),
                                name: m["reply"]["name"].as_str().unwrap_or("?").to_string(),
                                text: m["reply"]["text"].as_str().unwrap_or_default().to_string(),
                            }),
                            reactions: self.parse_reactions(&reactions[key.as_str()], names),
                        })
                        .collect()
                })
                .unwrap_or_default();
        // En el orden en que llegaron al servidor (su hora, la misma para todos); los ids llevan la
        // hora del PC que los escribió y antes de la 0.15 no se corregía.
        list.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.id.cmp(&b.id)));
        list
    }

    fn emit_messages(&self) {
        let (id, list) = {
            let inner = self.inner.lock().unwrap();
            let Some(open) = &inner.open else { return };
            (open.id.clone(), self.parse_messages(&inner.messages, &inner.reactions, &inner.names))
        };
        (self.messages)(id, list);
    }

    /// Escucha una conversación aparte de la que se ve en la ventana (la del juego: ver puente.rs), con sus
    /// reacciones. Avisa cada vez que cambia una de las dos, en cuanto llegaron los mensajes.
    pub fn watch(self: &Arc<Self>, id: &str, on_messages: impl Fn(Vec<Message>) + Send + Sync + 'static) -> Result<Watch, String> {
        let id = safe(id)?.to_string();
        // (mensajes, reacciones, ya llegaron los mensajes)
        let data: Arc<Mutex<(Value, Value, bool)>> = Arc::default();
        let on_messages: Arc<dyn Fn(Vec<Message>) + Send + Sync> = Arc::new(on_messages);
        let listen = |path: &str, reactions: bool| {
            let (weak, data, on_messages) = (Arc::downgrade(self), data.clone(), on_messages.clone());
            self.db.listen(
                format!("{path}/{id}?orderBy=\"$key\"&limitToLast={HISTORY}"),
                move |value| {
                    let Some(chat) = weak.upgrade() else { return };
                    let (messages, reacted) = {
                        let mut data = data.lock().unwrap();
                        if reactions {
                            data.1 = value;
                        } else {
                            data.0 = value;
                            data.2 = true;
                        }
                        if !data.2 {
                            return;
                        }
                        (data.0.clone(), data.1.clone())
                    };
                    let names = chat.inner.lock().unwrap().names.clone();
                    on_messages(chat.parse_messages(&messages, &reacted, &names));
                },
                |_| {},
            )
        };
        Ok(Watch { _messages: listen("messages", false), _reactions: listen("reactions", true) })
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

    /// `reply_to`: el mensaje al que responde (de la misma conversación).
    pub async fn send_text(&self, id: &str, text: &str, reply_to: Option<&str>) -> Result<(), String> {
        let text = text.trim();
        if text.is_empty() {
            return Ok(());
        }
        if text.chars().count() > MAX_TEXT {
            return Err(t!("El mensaje es muy largo (máximo {MAX_TEXT} letras).", "The message is too long (maximum {MAX_TEXT} characters)."));
        }
        let name = self.my_name.lock().unwrap().clone();
        let mut message = json!({ "from": self.uid, "name": name, "text": text, "at": firebase::server_timestamp() });
        if let Some(original) = reply_to {
            message["reply"] = self.quote(safe(id)?, safe(original)?).await?;
        }
        self.send(id, &message_id(self.db.server_now()), message, &preview(text), Map::new()).await
    }

    /// La copia corta de un mensaje para responderle: quién lo mandó y su vista previa.
    async fn quote(&self, id: &str, message: &str) -> Result<Value, String> {
        let original = self.recent(id).await.get(message).filter(|m| m.is_object()).cloned().ok_or(t!("El mensaje al que respondes ya no está.", "The message you are replying to is no longer there."))?;
        let name: String = original["name"].as_str().unwrap_or("?").chars().take(16).collect();
        Ok(json!({ "id": message, "name": name, "text": Self::preview_of(&original) }))
    }

    /// Pone (o quita, con None) tu reacción a un mensaje. Una por persona: otra la reemplaza.
    pub async fn react(&self, id: &str, message: &str, emoji: Option<&str>) -> Result<(), String> {
        let (id, message) = (safe(id)?, safe(message)?);
        let path = format!("reactions/{id}/{message}/{}", self.uid);
        match emoji {
            Some(emoji) if REACTIONS.contains(&emoji) => self.db.put(&path, &json!(emoji)).await.map(|_| ()),
            Some(_) => return Err(t!("Esa reacción no está disponible.", "That reaction is not available.")),
            None => self.db.delete(&path).await,
        }
        .map_err(|e| e.to_string())
    }

    /// Una instancia compartida (su receta, ver compartir.rs). En la lista de conversaciones se ve
    /// «📦 nombre».
    pub async fn send_share(&self, id: &str, text: &str, title: &str) -> Result<(), String> {
        if text.chars().count() > MAX_TEXT {
            return Err(t!("La instancia no cabe en un mensaje.", "The instance does not fit in a message."));
        }
        let name = self.my_name.lock().unwrap().clone();
        let message = json!({ "from": self.uid, "name": name, "text": text, "at": firebase::server_timestamp() });
        self.send(id, &message_id(self.db.server_now()), message, &preview(&format!("📦 {title}")), Map::new()).await
    }

    /// Una invitación a tu mundo o servidor (ver puente.rs). En la lista se ve «🎮 …».
    pub async fn send_invite(&self, id: &str, text: &str, preview_text: &str) -> Result<(), String> {
        let name = self.my_name.lock().unwrap().clone();
        let message = json!({ "from": self.uid, "name": name, "text": text, "at": firebase::server_timestamp() });
        self.send(id, &message_id(self.db.server_now()), message, &preview(preview_text), Map::new()).await
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
        let message_id = message_id(self.db.server_now());
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

    // ================= Editar y borrar (solo los mensajes propios)

    /// Los últimos mensajes de una conversación: los de la ventana si es la abierta; si no, de Firebase.
    async fn recent(&self, id: &str) -> Value {
        {
            let inner = self.inner.lock().unwrap();
            if inner.open.as_ref().is_some_and(|o| o.id == id) {
                return inner.messages.clone();
            }
        }
        self.db.get(&format!("messages/{id}?orderBy=\"$key\"&limitToLast={HISTORY}")).await.unwrap_or(Value::Null)
    }

    /// Un mensaje propio de una conversación.
    async fn own_message(&self, id: &str, message: &str) -> Result<Value, String> {
        let data = self.recent(id).await.get(message).filter(|m| m.is_object()).cloned().ok_or(t!("Ese mensaje ya no está.", "That message is no longer there."))?;
        if data["from"].as_str() != Some(self.uid.as_str()) {
            return Err(t!("Solo puedes cambiar tus propios mensajes.", "You can only change your own messages."));
        }
        Ok(data)
    }

    /// Si es el último de la conversación, la vista previa de la lista cambia también (sin mover la
    /// hora: no cuenta como mensaje nuevo para nadie).
    async fn is_last(&self, id: &str, message: &str) -> Option<Value> {
        let recent = self.recent(id).await;
        let messages = recent.as_object()?;
        let mut sorted: Vec<(&String, &Value)> = messages.iter().filter(|(_, m)| m.is_object()).collect();
        sorted.sort_by(|a, b| a.1["at"].as_i64().unwrap_or(0).cmp(&b.1["at"].as_i64().unwrap_or(0)).then_with(|| a.0.cmp(b.0)));
        let last = sorted.last()?;
        (last.0 == message).then(|| sorted.iter().rev().nth(1).map(|(_, m)| (*m).clone()).unwrap_or(Value::Null))
    }

    fn preview_of(message: &Value) -> String {
        if message["image"].as_bool() == Some(true) {
            "📷 Imagen".into()
        } else {
            let text = message["text"].as_str().unwrap_or_default();
            match crate::compartir::decode(text) {
                Some(recipe) => preview(&format!("📦 {}", recipe.name)),
                None if text.starts_with(crate::puente::INVITE_PREFIX) => "🎮 Invitación".into(),
                None => preview(text),
            }
        }
    }

    /// Cambia el texto de un mensaje propio (las imágenes y las instancias compartidas no se editan).
    pub async fn edit(&self, id: &str, message: &str, text: &str) -> Result<(), String> {
        let (id, message) = (safe(id)?, safe(message)?);
        let text = text.trim();
        if text.is_empty() {
            return Err(t!("El mensaje no puede quedar vacío (para quitarlo, bórralo).", "The message cannot be empty (to remove it, delete it)."));
        }
        if text.chars().count() > MAX_TEXT {
            return Err(t!("El mensaje es muy largo (máximo {MAX_TEXT} letras).", "The message is too long (maximum {MAX_TEXT} characters)."));
        }
        let mut data = self.own_message(id, message).await?;
        let old = data["text"].as_str().ok_or(t!("Las imágenes no se editan.", "Images cannot be edited."))?;
        if crate::compartir::decode(old).is_some() || old.starts_with(crate::puente::INVITE_PREFIX) {
            return Err(t!("Las instancias compartidas y las invitaciones no se editan.", "Shared instances and invites cannot be edited."));
        }
        if old == text {
            return Ok(());
        }
        data["text"] = json!(text);
        data["edited"] = firebase::server_timestamp();
        let mut update = Map::new();
        update.insert(format!("messages/{id}/{message}"), data);
        if self.is_last(id, message).await.is_some() {
            update.insert(format!("chats/{id}/meta/lastText"), json!(preview(text)));
        }
        self.db.patch("", &Value::Object(update)).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Borra un mensaje propio para todos (con su imagen, si tenía).
    pub async fn delete(&self, id: &str, message: &str) -> Result<(), String> {
        let (id, message) = (safe(id)?, safe(message)?);
        let data = self.own_message(id, message).await?;
        let mut update = Map::new();
        update.insert(format!("messages/{id}/{message}"), Value::Null);
        if data["image"].as_bool() == Some(true) {
            update.insert(format!("images/{id}/{message}"), Value::Null);
        }
        let reacted = {
            let inner = self.inner.lock().unwrap();
            inner.open.as_ref().is_none_or(|o| o.id != id) || inner.reactions.get(message).is_some_and(|r| r.is_object())
        };
        if reacted {
            update.insert(format!("reactions/{id}/{message}"), Value::Null);
        }
        if let Some(previous) = self.is_last(id, message).await {
            let (text, name, uid) = if previous.is_object() {
                (Self::preview_of(&previous), previous["name"].as_str().unwrap_or_default().to_string(), previous["from"].as_str().unwrap_or_default().to_string())
            } else {
                (String::new(), String::new(), String::new())
            };
            update.insert(format!("chats/{id}/meta/lastText"), json!(text));
            update.insert(format!("chats/{id}/meta/lastName"), json!(name));
            update.insert(format!("chats/{id}/meta/lastUid"), json!(uid));
        }
        self.db.patch("", &Value::Object(update)).await.map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(self.images.join(format!("{id}-{message}.txt")));
        Ok(())
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

    pub async fn create_group(&self, name: &str, friends: &[String], photo: Option<&str>) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            return Err(t!("El nombre del grupo debe tener de 1 a 40 letras.", "The group name must have 1 to 40 characters."));
        }
        if friends.is_empty() {
            return Err(t!("Elige al menos un amigo.", "Choose at least one friend."));
        }
        if let Some(photo) = photo {
            check_photo(photo)?;
        }
        let id = format!("g_{}", random_text(16));
        let mut update = Map::new();
        let me = self.uid.clone();
        for uid in std::iter::once(&me).chain(friends.iter()) {
            let uid = safe(uid)?;
            update.insert(format!("chats/{id}/members/{uid}"), json!(true));
            update.insert(format!("userChats/{uid}/{id}"), json!(true));
        }
        let mut meta = json!({ "kind": "grupo", "name": name, "owner": me, "created": firebase::server_timestamp(), "lastText": "", "lastName": "", "lastUid": "", "lastAt": firebase::server_timestamp() });
        if let Some(photo) = photo {
            meta["photo"] = json!(photo);
        }
        update.insert(format!("chats/{id}/meta"), meta);
        self.db.patch("", &Value::Object(update)).await.map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// Un grupo que puedes editar. Si todavía no tiene dueño (de antes de la 0.27), quien lo edita
    /// queda como dueño: se agrega a `update`.
    fn editable(&self, id: &str, update: &mut Map<String, Value>) -> Result<Summary, String> {
        let group = self.summary(id).filter(|s| s.kind == "grupo").ok_or(t!("Ese grupo ya no está.", "That group is no longer there."))?;
        if !group.can_edit {
            let owner = group.members.iter().find(|m| Some(&m.uid) == group.owner.as_ref()).map(|m| m.name.clone()).unwrap_or_default();
            return Err(t!("Solo {owner}, quien creó el grupo, puede cambiarlo.", "Only {owner}, who created the group, can change it."));
        }
        if group.owner.is_none() {
            update.insert(format!("chats/{id}/meta/owner"), json!(self.uid));
        }
        Ok(group)
    }

    pub async fn add_members(&self, id: &str, friends: &[String]) -> Result<(), String> {
        let id = safe(id)?;
        let mut update = Map::new();
        self.editable(id, &mut update)?;
        for uid in friends {
            let uid = safe(uid)?;
            update.insert(format!("chats/{id}/members/{uid}"), json!(true));
            update.insert(format!("userChats/{uid}/{id}"), json!(true));
        }
        self.db.patch("", &Value::Object(update)).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Cambia el nombre y la foto de un grupo. `photo`: None la deja igual; Some(None) la quita.
    pub async fn edit_group(&self, id: &str, name: Option<&str>, photo: Option<Option<&str>>) -> Result<(), String> {
        let id = safe(id)?;
        let mut update = Map::new();
        self.editable(id, &mut update)?;
        if let Some(name) = name {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 40 {
                return Err(t!("El nombre del grupo debe tener de 1 a 40 letras.", "The group name must have 1 to 40 characters."));
            }
            update.insert(format!("chats/{id}/meta/name"), json!(name));
        }
        match photo {
            Some(Some(photo)) => {
                check_photo(photo)?;
                update.insert(format!("chats/{id}/meta/photo"), json!(photo));
            }
            Some(None) => {
                update.insert(format!("chats/{id}/meta/photo"), Value::Null);
            }
            None => {}
        }
        self.db.patch("", &Value::Object(update)).await.map(|_| ()).map_err(|e| e.to_string())
    }

    pub async fn rename_group(&self, id: &str, name: &str) -> Result<(), String> {
        self.edit_group(id, Some(name), None).await
    }

    /// Saca a alguien del grupo (solo el dueño).
    pub async fn remove_member(&self, id: &str, uid: &str) -> Result<(), String> {
        let (id, uid) = (safe(id)?, safe(uid)?);
        let mut claim = Map::new();
        self.editable(id, &mut claim)?;
        if uid == self.uid {
            return self.leave(id).await;
        }
        // Las reglas miran quién era el dueño antes de escribir: si el grupo no tenía, primero se toma.
        if !claim.is_empty() {
            self.db.patch("", &Value::Object(claim)).await.map_err(|e| e.to_string())?;
        }
        let update = json!({ format!("chats/{id}/members/{uid}"): null, format!("userChats/{uid}/{id}"): null });
        self.db.patch("", &update).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Le pasa el grupo a otro miembro (deja de ser tuyo).
    pub async fn transfer_group(&self, id: &str, uid: &str) -> Result<(), String> {
        let (id, uid) = (safe(id)?, safe(uid)?);
        let mut claim = Map::new();
        let group = self.editable(id, &mut claim)?;
        if !group.members.iter().any(|m| m.uid == uid) {
            return Err(t!("Esa persona ya no está en el grupo.", "That person is no longer in the group."));
        }
        if !claim.is_empty() {
            self.db.patch("", &Value::Object(claim)).await.map_err(|e| e.to_string())?;
        }
        self.db.put(&format!("chats/{id}/meta/owner"), &json!(uid)).await.map(|_| ()).map_err(|e| e.to_string())
    }

    /// Salir de un grupo, u ocultar una conversación directa (vuelve si el amigo escribe). Si eras el
    /// dueño, el grupo pasa a otro miembro.
    pub async fn leave(&self, id: &str) -> Result<(), String> {
        let id = safe(id)?;
        let me = &self.uid;
        let mut update = json!({ format!("userChats/{me}/{id}"): null });
        if id.starts_with("g_") {
            update[format!("chats/{id}/members/{me}")] = Value::Null;
            if let Some(group) = self.summary(id).filter(|s| s.owner.as_deref() == Some(me.as_str())) {
                let mut others: Vec<&Member> = group.members.iter().filter(|m| m.uid != *me).collect();
                others.sort_by_key(|m| m.name.to_lowercase());
                if let Some(next) = others.first() {
                    update[format!("chats/{id}/meta/owner")] = json!(next.uid);
                }
            }
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
        assert!(message_id(1_700_000_000_001) > message_id(1_700_000_000_000));
        assert_eq!(message_id(1_700_000_000_000).len(), 20);
        assert!(safe("d_abc_DEF-1").is_ok());
        assert!(safe("../x").is_err() && safe("a/b").is_err() && safe("").is_err());
        assert_eq!(preview("  hola\n  mundo "), "hola mundo");
    }
}
