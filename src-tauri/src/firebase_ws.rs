//! Una conexión de tiempo real por Database. El formato de transporte se comprueba contra el SDK
//! oficial y el emulador; las peticiones REST conservan sus respuestas y escrituras atómicas.
use super::{metrics, Database, DbError, OpenConnection};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Weak,
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
const MAX_PACKET: usize = 16 * 1024 * 1024;

#[derive(Clone)]
pub(super) struct Timing {
    pub connect: Duration,
    pub retry: Duration,
    pub idle: Duration,
    pub ping: Duration,
    pub response: Duration,
    /// Cuánto se aguanta sin reconectar antes de abrir el respaldo SSE, si el WebSocket ya
    /// había funcionado. Evita abrir y cerrar una conexión por escucha con cada corte breve.
    pub fallback_after: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(4),
            retry: Duration::from_secs(30),
            idle: Duration::from_secs(60),
            ping: Duration::from_secs(20),
            response: Duration::from_secs(15),
            fallback_after: Duration::from_secs(10),
        }
    }
}

pub(super) enum Notice {
    Mode { generation: u64, sse: bool },
    Data { generation: u64, value: Value },
    Error { generation: u64, error: String },
}
enum Command {
    Add(u64, Spec, mpsc::UnboundedSender<Notice>),
    Remove(u64),
}
pub(super) struct Hub {
    tx: mpsc::UnboundedSender<Command>,
    next: AtomicU64,
    active: Arc<AtomicU64>,
    empty: Arc<tokio::sync::Notify>,
}
pub(super) struct Registration {
    tx: mpsc::UnboundedSender<Command>,
    id: u64,
    active: Arc<AtomicU64>,
    empty: Arc<tokio::sync::Notify>,
}
impl Drop for Registration {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Remove(self.id));
        if self.active.fetch_sub(1, Ordering::SeqCst) == 1 {
            self.empty.notify_one();
        }
    }
}

impl Hub {
    pub fn new(db: Weak<Database>, url: &str, timing: Timing) -> Arc<Self> {
        let (tx, rx) = mpsc::unbounded_channel();
        let endpoint = Endpoint::parse(url);
        let manager = Manager::new(db, endpoint, timing);
        let (active, empty) = (manager.active.clone(), manager.empty.clone());
        tauri::async_runtime::spawn(async move {
            manager.run(rx).await;
        });
        Arc::new(Self {
            tx,
            next: AtomicU64::new(1),
            active,
            empty,
        })
    }
    pub fn register(&self, spec: Spec, tx: mpsc::UnboundedSender<Notice>) -> Registration {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.active.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(Command::Add(id, spec, tx));
        Registration {
            tx: self.tx.clone(),
            id,
            active: self.active.clone(),
            empty: self.empty.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Spec {
    path: String,
    query: Map<String, Value>,
}
impl Spec {
    pub fn parse(path: &str) -> Option<Self> {
        let (path, params) = path.split_once('?').unwrap_or((path, ""));
        let mut query = Map::new();
        let url = reqwest::Url::parse(&format!("http://localhost/?{params}")).ok()?;
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "orderBy" if value == "\"$key\"" => {
                    query.insert("i".into(), json!(".key"));
                }
                "limitToLast" | "limitToFirst" => {
                    let limit = value.parse::<u32>().ok()?;
                    if limit == 0 || limit > 10000 {
                        return None;
                    }
                    query.insert("l".into(), json!(limit));
                    query.insert(
                        "vf".into(),
                        json!(if key == "limitToLast" { "r" } else { "l" }),
                    );
                }
                "startAt" | "endAt" => {
                    query.insert(
                        if key == "startAt" { "sp" } else { "ep" }.into(),
                        serde_json::from_str(&value).ok()?,
                    );
                }
                _ => return None,
            }
        }
        if !query.is_empty() && query.get("i") != Some(&json!(".key")) {
            return None;
        }
        Some(Self {
            path: format!("/{}", path.trim_matches('/')),
            query,
        })
    }
    fn key(&self) -> String {
        format!("{}?{}", self.path, Value::Object(self.query.clone()))
    }
    fn body(&self, tag: u64, listen: bool) -> Value {
        let mut body = json!({"p":self.path});
        if listen {
            body["h"] = json!("");
        }
        if !self.query.is_empty() {
            body["q"] = Value::Object(self.query.clone());
            body["t"] = json!(tag);
        }
        body
    }
    fn cancelled(&self, query: &Value) -> bool {
        if query.is_null() {
            return self.query.is_empty();
        }
        let expected = Value::Object(self.query.clone());
        query == &expected
            || query
                .as_array()
                .is_some_and(|queries| queries.contains(&expected))
    }
    pub(super) fn trim(&self, value: &mut Value) {
        let Some(object) = value.as_object_mut() else {
            return;
        };
        let mut keys: Vec<String> = object.keys().cloned().collect();
        keys.sort();
        if let Some(start) = self.query.get("sp").and_then(Value::as_str) {
            keys.retain(|key| key.as_str() >= start);
        }
        if let Some(end) = self.query.get("ep").and_then(Value::as_str) {
            keys.retain(|key| key.as_str() <= end);
        }
        if let Some(limit) = self.query.get("l").and_then(Value::as_u64) {
            let limit = limit as usize;
            if keys.len() > limit {
                if self.query.get("vf") == Some(&json!("r")) {
                    keys.drain(..keys.len() - limit);
                } else {
                    keys.truncate(limit);
                }
            }
        }
        object.retain(|key, _| keys.binary_search(key).is_ok());
        if object.is_empty() {
            *value = Value::Null;
        }
    }
}

#[derive(Clone)]
struct Endpoint {
    url: reqwest::Url,
    local: bool,
}
impl Endpoint {
    fn parse(url: &str) -> Option<Self> {
        let mut url = reqwest::Url::parse(url).ok()?;
        if !url.username().is_empty() || url.password().is_some() {
            return None;
        }
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        let secure = url.scheme() == "https";
        if !secure && !(local && url.scheme() == "http") {
            return None;
        }
        let namespace = url
            .query_pairs()
            .find(|(k, _)| k == "ns")
            .map(|(_, v)| v.into_owned())
            .or_else(|| url.host_str()?.split('.').next().map(String::from))?;
        url.set_scheme(if secure { "wss" } else { "ws" }).ok()?;
        url.set_path("/.ws");
        url.set_fragment(None);
        url.set_query(None);
        url.query_pairs_mut()
            .append_pair("v", "5")
            .append_pair("ns", &namespace);
        Some(Self { url, local })
    }
    fn redirect(&self, host: &str) -> Option<Self> {
        let target = reqwest::Url::parse(&format!("{}://{host}/", self.url.scheme())).ok()?;
        if !target.username().is_empty()
            || target.password().is_some()
            || target.path() != "/"
            || target.query().is_some()
            || target.fragment().is_some()
        {
            return None;
        }
        let name = target.host_str()?;
        let allowed = if self.local {
            target.host_str() == self.url.host_str()
                && target.port_or_known_default() == self.url.port_or_known_default()
        } else {
            target.port_or_known_default() == Some(443)
                && (name == self.url.host_str()?
                    || name.ends_with(".firebaseio.com")
                    || name.ends_with(".firebasedatabase.app"))
        };
        if !allowed {
            return None;
        }
        let mut next = self.clone();
        next.url.set_host(Some(name)).ok()?;
        next.url.set_port(target.port()).ok()?;
        Some(next)
    }
}

/// Firebase puede dividir un JSON entre varios mensajes WebSocket (distinto de la fragmentación RFC).
#[derive(Default)]
struct Frames {
    left: usize,
    bytes: String,
}
impl Frames {
    fn push(&mut self, text: &str) -> Result<Option<Value>, ()> {
        if self.left == 0 {
            if text == "0" {
                return Ok(None);
            }
            if text.len() <= 6 && text.bytes().all(|b| b.is_ascii_digit()) {
                let count = text.parse::<usize>().map_err(|_| ())?;
                if count == 0 || count > 1024 {
                    return Err(());
                }
                self.left = count;
                self.bytes.clear();
                return Ok(None);
            }
            self.left = 1;
        }
        if self.bytes.len() + text.len() > MAX_PACKET {
            return Err(());
        }
        self.bytes.push_str(text);
        self.left -= 1;
        if self.left != 0 {
            return Ok(None);
        }
        let parsed = serde_json::from_str(&self.bytes).map_err(|_| ());
        self.bytes.clear();
        parsed.map(Some)
    }
}

async fn send(socket: &mut Socket, value: Value) -> Result<(), ()> {
    let text = value.to_string();
    // Las peticiones de escucha son pequeñas; este límite también protege la cola del transporte.
    if text.len() > 16384 {
        return Err(());
    }
    tokio::time::timeout(
        Duration::from_secs(5),
        socket.send(Message::Text(text.into())),
    )
    .await
    .map_err(|_| ())?
    .map_err(|_| ())
}
async fn packet(socket: &mut Socket, frames: &mut Frames) -> Result<Value, ()> {
    loop {
        match socket.next().await.ok_or(())?.map_err(|_| ())? {
            Message::Text(text) => {
                if let Some(value) = frames.push(text.as_str())? {
                    return Ok(value);
                }
            }
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await.map_err(|_| ())?,
            Message::Pong(_) => {}
            _ => return Err(()),
        }
    }
}
struct Live {
    socket: Socket,
    frames: Frames,
    auth_at: Instant,
    auth_pending: Option<Instant>,
    received: Instant,
    ping: Instant,
    started: Instant,
    _open: OpenConnection,
}
async fn open(db: &Weak<Database>, mut endpoint: Endpoint) -> Result<(Live, Endpoint), ()> {
    let (token, auth_at) = {
        let db = db.upgrade().ok_or(())?;
        (
            db.session.token().await.map_err(|_| ())?,
            db.session.valid_until().await,
        )
    };
    for _ in 0..3 {
        let sent = chrono::Utc::now().timestamp_millis();
        let (mut socket, _) = tokio_tungstenite::connect_async(endpoint.url.as_str())
            .await
            .map_err(|_| ())?;
        let opened = OpenConnection::new(true);
        let mut frames = Frames::default();
        let hello = packet(&mut socket, &mut frames).await?;
        if hello["t"] != "c" {
            return Err(());
        }
        if hello["d"]["t"] == "r" {
            endpoint = endpoint
                .redirect(hello["d"]["d"].as_str().ok_or(())?)
                .ok_or(())?;
            continue;
        }
        if hello["d"]["t"] != "h" || hello["d"]["d"]["v"].as_str() != Some("5") {
            return Err(());
        }
        if let Some(timestamp) = hello["d"]["d"]["ts"].as_i64() {
            if let Some(db) = db.upgrade() {
                db.set_server_offset(
                    timestamp - (sent + chrono::Utc::now().timestamp_millis()) / 2,
                );
            }
        }
        if !endpoint.local {
            if let Some(next) = hello["d"]["d"]["h"]
                .as_str()
                .and_then(|host| endpoint.redirect(host))
            {
                endpoint = next;
            }
        }
        send(
            &mut socket,
            json!({"t":"d","d":{"r":0,"a":"auth","b":{"cred":token}}}),
        )
        .await?;
        loop {
            let value = packet(&mut socket, &mut frames).await?;
            if value["t"] == "d" && value["d"]["r"] == 0 {
                if value["d"]["b"]["s"] != "ok" {
                    if let Some(db) = db.upgrade() {
                        db.session.invalidate().await;
                    }
                    return Err(());
                }
                let now = Instant::now();
                return Ok((
                    Live {
                        socket,
                        frames,
                        auth_at,
                        auth_pending: None,
                        received: now,
                        ping: now,
                        started: now,
                        _open: opened,
                    },
                    endpoint,
                ));
            }
            if value["t"] == "c" && value["d"]["t"] != "o" {
                return Err(());
            }
        }
    }
    Err(())
}

struct Group {
    spec: Spec,
    tag: u64,
    subscribers: BTreeMap<u64, mpsc::UnboundedSender<Notice>>,
    data: Value,
    loaded: bool,
    retry: Option<Instant>,
}
struct Manager {
    active: Arc<AtomicU64>,
    empty: Arc<tokio::sync::Notify>,
    db: Weak<Database>,
    endpoint: Option<Endpoint>,
    timing: Timing,
    groups: BTreeMap<String, Group>,
    members: HashMap<u64, String>,
    pending: HashMap<u64, (String, Instant)>,
    request: u64,
    tag: u64,
    generation: u64,
    fallback: bool,
    /// Si el WebSocket ya se abrió alguna vez en esta sesión del Manager.
    connected_once: bool,
    /// Desde cuándo lleva caído; `None` mientras la conexión funciona.
    lost: Option<Instant>,
}
impl Manager {
    fn new(db: Weak<Database>, endpoint: Option<Endpoint>, timing: Timing) -> Self {
        Self {
            active: Arc::default(),
            empty: Arc::default(),
            db,
            endpoint,
            timing,
            groups: BTreeMap::new(),
            members: HashMap::new(),
            pending: HashMap::new(),
            request: 0,
            tag: 0,
            generation: 0,
            fallback: false,
            connected_once: false,
            lost: None,
        }
    }
    /// Abre el respaldo SSE cuando la caída ya superó la gracia. Antes de la primera conexión
    /// se abre al primer fallo: una red que bloquea el WebSocket no debe hacer esperar.
    fn maybe_fallback(&mut self, now: Instant) {
        if self.fallback {
            return;
        }
        let expired = self
            .lost
            .is_some_and(|lost| now.duration_since(lost) >= self.timing.fallback_after);
        if !self.connected_once || expired {
            self.mode(true);
        }
    }
    fn mode(&mut self, sse: bool) {
        self.generation += 1;
        self.fallback = sse;
        if sse {
            metrics::add(&metrics::FALLBACKS, 1);
        }
        for group in self.groups.values() {
            for tx in group.subscribers.values() {
                let _ = tx.send(Notice::Mode {
                    generation: self.generation,
                    sse,
                });
            }
        }
    }
    async fn query(&mut self, key: &str, live: &mut Live) -> Result<(), ()> {
        let group = self.groups.get_mut(key).ok_or(())?;
        group.loaded = false;
        group.data = Value::Null;
        group.retry = None;
        self.request += 1;
        self.pending
            .insert(self.request, (key.into(), Instant::now()));
        send(
            &mut live.socket,
            json!({"t":"d","d":{"r":self.request,"a":"q","b":group.spec.body(group.tag,true)}}),
        )
        .await
    }
    fn publish(&self, group: &Group) {
        for tx in group.subscribers.values() {
            let _ = tx.send(Notice::Data {
                generation: self.generation,
                value: group.data.clone(),
            });
        }
    }
    fn denied(&mut self, key: &str) {
        if let Some(group) = self.groups.get_mut(key) {
            group.retry = Some(Instant::now() + Duration::from_secs(20));
            group.loaded = false;
            group.data = Value::Null;
            let message = DbError::Permission(group.spec.path.clone()).to_string();
            for tx in group.subscribers.values() {
                let _ = tx.send(Notice::Error {
                    generation: self.generation,
                    error: message.clone(),
                });
            }
        }
    }
    fn data(&mut self, body: &Value, merge: bool) {
        let path = body["p"].as_str().unwrap_or("/");
        let tag = body["t"].as_u64();
        for group in self.groups.values_mut() {
            if group.retry.is_some() {
                continue;
            }
            if tag
                .map(|tag| tag != group.tag)
                .unwrap_or(!group.spec.query.is_empty())
            {
                continue;
            }
            if update(&mut group.data, &group.spec.path, path, &body["d"], merge) {
                group.loaded = true;
                group.spec.trim(&mut group.data);
                for tx in group.subscribers.values() {
                    let _ = tx.send(Notice::Data {
                        generation: self.generation,
                        value: group.data.clone(),
                    });
                }
            }
        }
    }
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>) {
        let mut live: Option<Live> = None;
        let mut reconnect = Instant::now();
        let mut delay = Duration::from_millis(250);
        let mut refresh: Option<tokio::task::JoinHandle<Result<(String, Instant), String>>> = None;
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if self.groups.is_empty() || self.active.load(Ordering::SeqCst) == 0 {
                live = None;
                self.pending.clear();
                reconnect = Instant::now();
                if let Some(task) = refresh.take() {
                    task.abort();
                }
            } else if live.is_none() && Instant::now() >= reconnect {
                let result = match self.endpoint.clone() {
                    Some(endpoint) => {
                        tokio::select! {
                            result = tokio::time::timeout(self.timing.connect, open(&self.db, endpoint)) => {
                                result.ok().and_then(Result::ok)
                            }
                            _ = async {
                                loop {
                                    self.empty.notified().await;
                                    if self.active.load(Ordering::SeqCst) == 0 {
                                        break;
                                    }
                                }
                            } => None,
                        }
                    }
                    None => None,
                };
                if self.active.load(Ordering::SeqCst) == 0 {
                    drop(result);
                } else if let Some((connection, endpoint)) = result {
                    self.endpoint = Some(endpoint);
                    live = Some(connection);
                    if !self.connected_once {
                        self.connected_once = true;
                        // La primera conexión deja atrás el fallo que abrió SSE.
                        self.lost = None;
                    }
                    self.pending.clear();
                    self.mode(false);
                    let keys: Vec<_> = self.groups.keys().cloned().collect();
                    for key in keys {
                        let connection = live.as_mut().unwrap();
                        if self.query(&key, connection).await.is_err() {
                            live = None;
                            break;
                        }
                    }
                } else {
                    // Sin conexión previa no se hace esperar: el respaldo abre al primer fallo.
                    self.lost.get_or_insert_with(Instant::now);
                    self.maybe_fallback(Instant::now());
                    reconnect = Instant::now() + self.timing.retry;
                }
            }
            let mut broken = false;
            tokio::select! {
                command = commands.recv() => match command {
                    None => break,
                    Some(Command::Add(id, spec, tx)) => {
                        let key = spec.key();
                        let _ = tx.send(Notice::Mode {
                            generation: self.generation,
                            sse: self.fallback,
                        });
                        if let Some(group) = self.groups.get_mut(&key) {
                            if group.loaded && !self.fallback {
                                let _ = tx.send(Notice::Data {
                                    generation: self.generation,
                                    value: group.data.clone(),
                                });
                            }
                            group.subscribers.insert(id, tx);
                        } else {
                            self.tag += 1;
                            self.groups.insert(
                                key.clone(),
                                Group {
                                    spec,
                                    tag: self.tag,
                                    subscribers: BTreeMap::from([(id, tx)]),
                                    data: Value::Null,
                                    loaded: false,
                                    retry: None,
                                },
                            );
                            if let Some(connection) = live.as_mut() {
                                broken = self.query(&key, connection).await.is_err();
                            }
                        }
                        self.members.insert(id, key);
                    }
                    Some(Command::Remove(id)) => {
                        if let Some(key) = self.members.remove(&id) {
                            if let Some(group) = self.groups.get_mut(&key) {
                                group.subscribers.remove(&id);
                            }
                            if self
                                .groups
                                .get(&key)
                                .is_some_and(|group| group.subscribers.is_empty())
                            {
                                let group = self.groups.remove(&key).unwrap();
                                self.pending.retain(|_, (pending, _)| pending != &key);
                                if let Some(connection) = live.as_mut() {
                                    self.request += 1;
                                    broken = send(
                                        &mut connection.socket,
                                        json!({"t":"d","d":{"r":self.request,"a":"n","b":group.spec.body(group.tag, false)}}),
                                    )
                                    .await
                                    .is_err();
                                }
                            }
                        }
                    }
                },
                message = async {
                    match live.as_mut() {
                        Some(connection) => connection.socket.next().await,
                        None => std::future::pending().await,
                    }
                } => {
                    let connection = live.as_mut().unwrap();
                    connection.received = Instant::now();
                    match message {
                        Some(Ok(Message::Text(text))) => {
                            metrics::add(&metrics::STREAM_BYTES, text.len() as u64);
                            match connection.frames.push(text.as_str()) {
                                Ok(Some(value)) => match value["t"].as_str() {
                                    Some("d") => {
                                        let packet = &value["d"];
                                        if let Some(request) = packet["r"].as_u64() {
                                            if request == 0 {
                                                connection.auth_pending = None;
                                                if packet["b"]["s"] != "ok" {
                                                    broken = true;
                                                    if let Some(db) = self.db.upgrade() {
                                                        db.session.invalidate().await;
                                                    }
                                                }
                                            } else if let Some((key, _)) = self.pending.remove(&request) {
                                                // Una respuesta confirma que el socket volvió a servir.
                                                self.lost = None;
                                                if packet["b"]["s"] != "ok" {
                                                    self.denied(&key);
                                                } else if let Some(group) = self.groups.get_mut(&key) {
                                                    if !group.loaded {
                                                        group.loaded = true;
                                                        self.publish(self.groups.get(&key).unwrap());
                                                    }
                                                }
                                            }
                                        } else {
                                            match packet["a"].as_str() {
                                                Some("d") => {
                                                    metrics::add(&metrics::EVENTS, 1);
                                                    self.data(&packet["b"], false);
                                                }
                                                Some("m") => {
                                                    metrics::add(&metrics::EVENTS, 1);
                                                    self.data(&packet["b"], true);
                                                }
                                                Some("ac") => {
                                                    broken = true;
                                                    if let Some(db) = self.db.upgrade() {
                                                        db.session.invalidate().await;
                                                    }
                                                }
                                                Some("c") => {
                                                    let keys: Vec<_> = self
                                                        .groups
                                                        .iter()
                                                        .filter(|(_, group)| {
                                                            group.spec.path
                                                                == packet["b"]["p"].as_str().unwrap_or("")
                                                                && group.spec.cancelled(&packet["b"]["q"])
                                                        })
                                                        .map(|(key, _)| key.clone())
                                                        .collect();
                                                    for key in keys {
                                                        self.denied(&key);
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    Some("c") => match value["d"]["t"].as_str() {
                                        Some("r") => {
                                            self.endpoint = value["d"]["d"]
                                                .as_str()
                                                .and_then(|host| self.endpoint.as_ref()?.redirect(host));
                                            broken = true;
                                        }
                                        Some("s" | "e") => broken = true,
                                        _ => {}
                                    },
                                    _ => broken = true,
                                },
                                Ok(None) => {}
                                Err(()) => broken = true,
                            }
                        }
                        Some(Ok(Message::Ping(bytes))) => {
                            broken = connection.socket.send(Message::Pong(bytes)).await.is_err();
                        }
                        Some(Ok(Message::Pong(_))) => {}
                        _ => broken = true,
                    }
                },
                result = async {
                    match refresh.as_mut() {
                        Some(task) => task.await,
                        None => std::future::pending().await,
                    }
                } => {
                    refresh = None;
                    if let Some(connection) = live.as_mut() {
                        match result {
                            Ok(Ok((token, expires))) => {
                                connection.auth_at = expires;
                                connection.auth_pending = Some(Instant::now());
                                broken = send(
                                    &mut connection.socket,
                                    json!({"t":"d","d":{"r":0,"a":"auth","b":{"cred":token}}}),
                                )
                                .await
                                .is_err();
                            }
                            _ => broken = true,
                        }
                    }
                }
                _ = tick.tick() => {
                    if let Some(connection) = live.as_mut() {
                        let now = Instant::now();
                        broken = now.duration_since(connection.received) > self.timing.idle
                            || connection
                                .auth_pending
                                .is_some_and(|at| now.duration_since(at) > self.timing.response)
                            || self
                                .pending
                                .values()
                                .any(|(_, at)| now.duration_since(*at) > self.timing.response);
                        if !broken && now.duration_since(connection.ping) >= self.timing.ping {
                            connection.ping = now;
                            broken = send(
                                &mut connection.socket,
                                json!({"t":"c","d":{"t":"p","d":{}}}),
                            )
                            .await
                            .is_err();
                        }
                        if !broken
                            && now >= connection.auth_at
                            && connection.auth_pending.is_none()
                            && refresh.is_none()
                        {
                            let db = self.db.clone();
                            refresh = Some(tokio::spawn(async move {
                                let db = db.upgrade().ok_or_else(|| "sesión cerrada".to_string())?;
                                Ok((db.session.token().await?, db.session.valid_until().await))
                            }));
                        }
                        let keys: Vec<_> = self
                            .groups
                            .iter()
                            .filter(|(_, group)| group.retry.is_some_and(|at| now >= at))
                            .map(|(key, _)| key.clone())
                            .collect();
                        for key in keys {
                            if self.query(&key, connection).await.is_err() {
                                broken = true;
                                break;
                            }
                        }
                    } else if !self.groups.is_empty()
                        && self.active.load(Ordering::SeqCst) > 0
                    {
                        // Entre reintentos, la gracia vence sin esperar al siguiente intento.
                        self.maybe_fallback(Instant::now());
                    }
                }
            }
            if broken {
                // Un socket que duró un rato vuelve a reintentar rápido.
                if live.as_ref().is_some_and(|connection| {
                    connection.started.elapsed() >= Duration::from_secs(30)
                }) {
                    delay = Duration::from_millis(250);
                }
                live = None;
                self.pending.clear();
                // La gracia cuenta desde el primer corte, aunque haya reintentos fallidos.
                self.lost.get_or_insert_with(Instant::now);
                self.maybe_fallback(Instant::now());
                if let Some(task) = refresh.take() {
                    task.abort();
                }
                reconnect = Instant::now() + delay.min(self.timing.retry);
                delay = (delay * 2).min(self.timing.retry);
            }
        }
        if let Some(task) = refresh {
            task.abort();
        }
    }
}

fn parts(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}
/// Despacha también eventos agrupados de una rama antecesora, sin mezclar consultas con etiquetas.
fn update(data: &mut Value, base: &str, path: &str, value: &Value, merge: bool) -> bool {
    if merge {
        if let Some(object) = value.as_object() {
            let mut touched = false;
            for (key, value) in object {
                touched |= update(
                    data,
                    base,
                    &format!("{}/{key}", path.trim_end_matches('/')),
                    value,
                    false,
                );
            }
            return touched;
        }
    }
    let base = parts(base);
    let path = parts(path);
    if path.starts_with(&base) {
        super::set_at(data, &path[base.len()..], value.clone());
        true
    } else if base.starts_with(&path) {
        let mut nested = value;
        for part in &base[path.len()..] {
            nested = &nested[*part];
        }
        *data = nested.clone();
        true
    } else {
        false
    }
}

#[cfg(test)]
#[path = "firebase_ws_tests.rs"]
mod tests;
