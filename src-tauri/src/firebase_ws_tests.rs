use super::*;
use crate::firebase::Session;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize},
    Mutex,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::broadcast,
};

#[test]
fn tramas_partidas_unicode_y_limites() {
    let mut frames = Frames::default();
    assert_eq!(frames.push("0").unwrap(), None);
    assert_eq!(frames.push("2").unwrap(), None);
    assert_eq!(frames.push("{\"texto\":\"á🙂").unwrap(), None);
    assert_eq!(frames.push("\"}").unwrap(), Some(json!({"texto":"á🙂"})));
    assert_eq!(frames.push("{\"x\":1}").unwrap(), Some(json!({"x":1})));
    assert!(frames.push("999999").is_err());
    assert!(Frames::default().push("incorrecto").is_err());
    let mut frames = Frames::default();
    frames.push("2").unwrap();
    assert!(frames.push(&"x".repeat(MAX_PACKET + 1)).is_err());
}

#[test]
fn consultas_rutas_antecesoras_y_etiquetas_aisladas() {
    let query = Spec::parse("messages/g?orderBy=%22%24key%22&limitToLast=2").unwrap();
    assert_eq!(
        query.body(7, true),
        json!({"p":"/messages/g","h":"","q":{"i":".key","l":2,"vf":"r"},"t":7})
    );
    assert_eq!(
        query.body(7, false),
        json!({"p":"/messages/g","q":{"i":".key","l":2,"vf":"r"},"t":7})
    );
    assert!(query.cancelled(&json!([{"i":".key","l":2,"vf":"r"}])));
    assert!(!query.cancelled(&Value::Null));
    assert!(Spec::parse("x?orderBy=\"name\"").is_none());
    assert!(Spec::parse("x?limitToLast=2").is_none());
    let mut value = json!({"old":0});
    assert!(!update(&mut value, "/a", "/ab", &json!(1), false));
    assert!(update(
        &mut value,
        "/a/b",
        "/a",
        &json!({"b":{"x":1}}),
        false
    ));
    assert_eq!(value, json!({"x":1}));
    update(
        &mut value,
        "/a/b",
        "/a",
        &json!({"b/x":null,"b/y":2,"c":4}),
        true,
    );
    assert_eq!(value, json!({"y":2}));
    let mut value = json!({"a":1,"b":2,"c":3});
    query.trim(&mut value);
    assert_eq!(value, json!({"b":2,"c":3}));
    let mut manager = Manager::new(Weak::new(), None, Timing::default());
    let (plain_tx, mut plain_rx) = mpsc::unbounded_channel();
    let (query_tx, mut query_rx) = mpsc::unbounded_channel();
    for (tag, spec, tx) in [
        (1, Spec::parse("messages/g").unwrap(), plain_tx),
        (7, query, query_tx),
    ] {
        manager.groups.insert(
            spec.key(),
            Group {
                spec,
                tag,
                subscribers: BTreeMap::from([(tag, tx)]),
                data: Value::Null,
                loaded: false,
                retry: None,
            },
        );
    }
    manager.data(
        &json!({"p":"/messages/g","d":{"a":1,"b":2,"c":3},"t":7}),
        false,
    );
    assert!(plain_rx.try_recv().is_err());
    assert!(
        matches!(query_rx.try_recv(),Ok(Notice::Data {value,..}) if value==json!({"b":2,"c":3}))
    );
    manager.data(&json!({"p":"/messages/g/a","d":5}), false);
    assert!(matches!(plain_rx.try_recv(),Ok(Notice::Data {value,..}) if value==json!({"a":5})));
    assert!(query_rx.try_recv().is_err());
}

#[test]
fn redireccion_solo_al_servidor_local_o_a_firebase_por_tls() {
    let endpoint =
        Endpoint::parse("https://ejemplo-default-rtdb.europe-west1.firebasedatabase.app").unwrap();
    assert_eq!(
        endpoint
            .url
            .query_pairs()
            .find(|(k, _)| k == "ns")
            .unwrap()
            .1,
        "ejemplo-default-rtdb"
    );
    assert!(endpoint.redirect("s-usc1c-nss-1.firebaseio.com").is_some());
    for bad in [
        "servidor.invalid",
        "firebaseio.com.servidor.invalid",
        "s.firebaseio.com:80",
        "usuario@s.firebaseio.com",
        "s.firebaseio.com/?auth=secreto",
    ] {
        assert!(endpoint.redirect(bad).is_none(), "{bad}");
    }
    assert!(Endpoint::parse("http://base.firebaseio.com").is_none());
    let local = Endpoint::parse("http://127.0.0.1:9399?ns=prueba").unwrap();
    assert!(local.redirect("127.0.0.1:9399").is_some());
    assert!(local.redirect("127.0.0.1:9400").is_none());
}

#[derive(Clone)]
enum Event {
    Push(Value),
    Cut,
}
struct State {
    blocked: AtomicBool,
    quiet: AtomicBool,
    mute_query: AtomicBool,
    reset: AtomicBool,
    connects: AtomicUsize,
    queries: AtomicUsize,
    unlistens: AtomicUsize,
    auth: AtomicUsize,
    sse: AtomicUsize,
    events: broadcast::Sender<Event>,
    snapshot: Mutex<Value>,
}
struct Server {
    url: String,
    state: Arc<State>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn new() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (events, _) = broadcast::channel(32);
        let state = Arc::new(State {
            blocked: AtomicBool::new(false),
            quiet: AtomicBool::new(false),
            mute_query: AtomicBool::new(false),
            reset: AtomicBool::new(false),
            connects: AtomicUsize::new(0),
            queries: AtomicUsize::new(0),
            unlistens: AtomicUsize::new(0),
            auth: AtomicUsize::new(0),
            sse: AtomicUsize::new(0),
            events,
            snapshot: Mutex::new(json!({"root":{"child":{"value":1}}})),
        });
        let shared = state.clone();
        let task = tokio::spawn(async move {
            let mut tasks = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    result=listener.accept()=>{ let Ok((stream,_))=result else {break}; let state=shared.clone(); tasks.spawn(async move { serve(stream,state,address.to_string()).await; }); },
                    _=tasks.join_next(), if !tasks.is_empty()=>{},
                }
            }
        });
        Self {
            url: format!("http://{address}?ns=prueba"),
            state,
            task,
        }
    }
    fn database(&self, renew: bool) -> Arc<Database> {
        let mut session = Session::emulator("A", "prueba");
        if renew {
            session.refresh_url = Some(format!("{}/token", self.url.split('?').next().unwrap()));
            session.token.get_mut().expires = Instant::now() + Duration::from_secs(2);
        }
        let mut db = Database::new(&self.url, session);
        db.sse_only = false;
        db.timing = Timing {
            connect: Duration::from_secs(2),
            retry: Duration::from_millis(300),
            idle: Duration::from_secs(2),
            ping: Duration::from_millis(250),
            response: Duration::from_secs(2),
            fallback_after: Duration::from_secs(2),
        };
        Arc::new(db)
    }
    fn push(&self, path: &str, value: Value) {
        let mut snapshot = self.state.snapshot.lock().unwrap();
        update(&mut snapshot, "/", path, &value, false);
        let _ = self.state.events.send(Event::Push(
            json!({"t":"d","d":{"a":"d","b":{"p":path,"d":value}}}),
        ));
    }
}
fn at(value: &Value, path: &str) -> Value {
    let mut value = value;
    for key in parts(path) {
        value = &value[key];
    }
    value.clone()
}
async fn http_request(stream: &mut TcpStream) -> String {
    let mut request = Vec::new();
    let mut part = [0; 2048];
    loop {
        let size = stream.read(&mut part).await.unwrap_or(0);
        if size == 0 {
            break;
        }
        request.extend_from_slice(&part[..size]);
        if request.windows(4).any(|v| v == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&request).into_owned()
}
async fn serve(mut stream: TcpStream, state: Arc<State>, address: String) {
    let mut first = [0; 2048];
    let count = stream.peek(&mut first).await.unwrap_or(0);
    let line = String::from_utf8_lossy(&first[..count]);
    if line.starts_with("GET /.ws?") {
        if state.blocked.load(Ordering::SeqCst) {
            http_request(&mut stream).await;
            let _ = stream
                .write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\n\r\n")
                .await;
            return;
        }
        let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
            return;
        };
        state.connects.fetch_add(1, Ordering::SeqCst);
        if state.reset.swap(false, Ordering::SeqCst) {
            let _ = socket
                .send(Message::Text(
                    json!({"t":"c","d":{"t":"r","d":address}})
                        .to_string()
                        .into(),
                ))
                .await;
            return;
        }
        let hello = json!({"t":"c","d":{"t":"h","d":{"v":"5","h":address,"s":"prueba","ts":chrono::Utc::now().timestamp_millis()}}});
        if socket
            .send(Message::Text(hello.to_string().into()))
            .await
            .is_err()
        {
            return;
        }
        let mut events = state.events.subscribe();
        loop {
            tokio::select! {
                message=socket.next()=>{
                    let Some(Ok(Message::Text(text)))=message else {return};
                    if text=="0" {continue;}
                    let Ok(packet)=serde_json::from_str::<Value>(text.as_str()) else {return};
                    if packet["t"]=="c" {
                        if !state.quiet.load(Ordering::SeqCst) { let _=socket.send(Message::Text(json!({"t":"c","d":{"t":"o","d":{}}}).to_string().into())).await; }
                        continue;
                    }
                    let body=&packet["d"]["b"]; let request=&packet["d"]["r"];
                    let mut status="ok";
                    match packet["d"]["a"].as_str() {
                        Some("auth")=>{state.auth.fetch_add(1,Ordering::SeqCst);},
                        Some("n")=>{state.unlistens.fetch_add(1,Ordering::SeqCst);},
                        Some("q")=>{
                            if state.mute_query.load(Ordering::SeqCst) {continue;}
                            state.queries.fetch_add(1,Ordering::SeqCst);
                            if body["p"]=="/private" {status="permission_denied";}
                            else {
                                let value=at(&state.snapshot.lock().unwrap(),body["p"].as_str().unwrap());
                                if !value.is_null() {
                                    let mut push=json!({"t":"d","d":{"a":"d","b":{"p":body["p"],"d":value}}});
                                    if !body["t"].is_null() {push["d"]["b"]["t"]=body["t"].clone();}
                                    if socket.send(Message::Text(push.to_string().into())).await.is_err() {return;}
                                }
                            }
                        },
                        _=>{},
                    }
                    if socket.send(Message::Text(json!({"t":"d","d":{"r":request,"b":{"s":status,"d":null}}}).to_string().into())).await.is_err() {return;}
                },
                event=events.recv()=>match event {
                    Ok(Event::Push(value))=>if socket.send(Message::Text(value.to_string().into())).await.is_err() {return;},
                    _=>return,
                },
            }
        }
    } else {
        let request = http_request(&mut stream).await;
        if request.starts_with("POST /token") {
            let body = json!({"id_token":"token-renovado","refresh_token":"r","expires_in":"3600"})
                .to_string();
            let _=stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
            return;
        }
        state.sse.fetch_add(1, Ordering::SeqCst);
        let path = request
            .split_whitespace()
            .nth(1)
            .unwrap_or("/")
            .split('?')
            .next()
            .unwrap_or("/")
            .trim_end_matches(".json")
            .to_string();
        if stream
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .await
            .is_err()
        {
            return;
        }
        let mut events = state.events.subscribe();
        loop {
            let body =
                json!({"path":"/","data":at(&state.snapshot.lock().unwrap(),&path)}).to_string();
            if stream
                .write_all(format!("event: put\ndata: {body}\n\n").as_bytes())
                .await
                .is_err()
            {
                return;
            }
            match events.recv().await {
                Ok(Event::Push(_)) => {}
                _ => return,
            }
        }
    }
}

async fn wait(label: &str, mut check: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !check() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("No se cumplió: {label}"));
}
type Seen = Arc<Mutex<Vec<Value>>>;
fn listen(db: &Arc<Database>, path: &str) -> (crate::firebase::Listener, Seen) {
    let seen: Seen = Arc::default();
    let output = seen.clone();
    (
        db.listen(
            path.into(),
            move |value| output.lock().unwrap().push(value),
            |_| {},
        ),
        seen,
    )
}
fn last(seen: &Seen) -> Value {
    seen.lock().unwrap().last().cloned().unwrap_or(Value::Null)
}
async fn closed() {
    wait("cierre de todas las conexiones", || {
        metrics::OPEN.load(Ordering::SeqCst) == 0 && metrics::LISTENERS.load(Ordering::SeqCst) == 0
    })
    .await;
}
/// `metrics` es global: los recorridos de red se ejecutan de uno en uno para que sus cuentas
/// no se mezclen entre pruebas.
static NETWORK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test]
async fn websocket_compartido_reconecta_renueva_y_vuelve_de_sse() {
    let _network = NETWORK.lock().await;
    let server = Server::new().await;
    // Mismo path: una consulta del servidor, dos consumidores. Una rama hija comparte el socket.
    let db = server.database(true);
    let (one, seen) = listen(&db, "root");
    let (two, second) = listen(&db, "root");
    let child_seen: Seen = Arc::default();
    let values = child_seen.clone();
    let revoked = Arc::new(AtomicUsize::new(0));
    let revoked_count = revoked.clone();
    let child = db.listen(
        "root/child".into(),
        move |value| values.lock().unwrap().push(value),
        move |_| {
            revoked_count.fetch_add(1, Ordering::SeqCst);
        },
    );
    wait("instantáneas iniciales", || {
        last(&seen) == json!({"child":{"value":1}})
            && last(&second) == last(&seen)
            && last(&child_seen) == json!({"value":1})
    })
    .await;
    assert_eq!(server.state.connects.load(Ordering::SeqCst), 1);
    assert_eq!(server.state.queries.load(Ordering::SeqCst), 2);
    assert_eq!(metrics::WEBSOCKETS.load(Ordering::SeqCst), 1);
    let queries = server.state.queries.load(Ordering::SeqCst);
    let errors = Arc::new(AtomicUsize::new(0));
    let failures = errors.clone();
    let denied = db.listen(
        "private".into(),
        |_| panic!("Una consulta denegada no entrega datos"),
        move |_| {
            failures.fetch_add(1, Ordering::SeqCst);
        },
    );
    wait("permiso rechazado solo en su consulta", || {
        errors.load(Ordering::SeqCst) == 1
    })
    .await;
    assert_eq!(metrics::WEBSOCKETS.load(Ordering::SeqCst), 1);
    assert_eq!(server.state.queries.load(Ordering::SeqCst), queries + 1);
    drop(denied);
    wait("renovar el token en el mismo socket", || {
        server.state.auth.load(Ordering::SeqCst) >= 2
    })
    .await;
    assert_eq!(server.state.connects.load(Ordering::SeqCst), 1);
    let removed = server.state.unlistens.load(Ordering::SeqCst);
    drop(one);
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(server.state.unlistens.load(Ordering::SeqCst), removed);
    drop(two);
    wait("soltar la consulta cuando ya no tiene consumidores", || {
        server.state.unlistens.load(Ordering::SeqCst) == removed + 1
    })
    .await;
    server.push("/root/child/value", json!(2));
    wait("actualizar una rama descendiente", || {
        last(&child_seen) == json!({"value":2})
    })
    .await;
    let _ = server.state.events.send(Event::Push(
        json!({"t":"d","d":{"a":"c","b":{"p":"/root/child"}}}),
    ));
    wait("revocar el permiso de una escucha", || {
        revoked.load(Ordering::SeqCst) == 1
    })
    .await;
    let _ = server.state.events.send(Event::Push(
        json!({"t":"d","d":{"a":"d","b":{"p":"/root/child/value","d":888}}}),
    ));
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        last(&child_seen),
        json!({"value":2}),
        "Se descarta un dato tardío tras revocar el permiso"
    );
    drop(child);
    drop(db);
    closed().await;

    // Se corta el socket; el valor cambiado mientras no había conexión se recupera una sola vez.
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    wait("segunda sesión", || last(&seen) == json!({"value":2})).await;
    let before = server.state.connects.load(Ordering::SeqCst);
    server.state.snapshot.lock().unwrap()["root"]["child"]["value"] = json!(3);
    let _ = server.state.events.send(Event::Cut);
    wait("reconectar y recuperar el estado", || {
        last(&seen) == json!({"value":3})
            && server.state.connects.load(Ordering::SeqCst) > before
            && metrics::SSE.load(Ordering::SeqCst) == 0
    })
    .await;
    assert_eq!(
        seen.lock()
            .unwrap()
            .iter()
            .filter(|v| **v == json!({"value":3}))
            .count(),
        1
    );
    server.push("/root/child/value", json!(4));
    wait("recibir después de reconectar", || {
        last(&seen) == json!({"value":4})
    })
    .await;
    drop(listener);
    drop(db);
    closed().await;

    // Una red bloquea WebSocket, SSE sigue recibiendo, y vuelve al canal compartido al recuperarse.
    server.state.blocked.store(true, Ordering::SeqCst);
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    wait("respaldo SSE", || {
        last(&seen) == json!({"value":4}) && metrics::SSE.load(Ordering::SeqCst) == 1
    })
    .await;
    server.push("/root/child/value", json!(5));
    wait("recibir por SSE", || last(&seen) == json!({"value":5})).await;
    server.state.blocked.store(false, Ordering::SeqCst);
    wait("recuperar WebSocket y cerrar SSE", || {
        metrics::WEBSOCKETS.load(Ordering::SeqCst) == 1 && metrics::SSE.load(Ordering::SeqCst) == 0
    })
    .await;
    server.push("/root/child/value", json!(6));
    wait("recibir tras dejar SSE", || {
        last(&seen) == json!({"value":6})
    })
    .await;
    // Una conexión TCP muda también se detecta, aunque siga abierta; al reconectar dentro de
    // la gracia se conserva el WebSocket y no se abre SSE.
    let fallbacks = metrics::FALLBACKS.load(Ordering::SeqCst);
    let connects = server.state.connects.load(Ordering::SeqCst);
    server.state.quiet.store(true, Ordering::SeqCst);
    wait("detectar el socket mudo y reemplazarlo", || {
        server.state.connects.load(Ordering::SeqCst) > connects
    })
    .await;
    server.state.quiet.store(false, Ordering::SeqCst);
    wait("seguir sin SSE tras el socket mudo", || {
        metrics::WEBSOCKETS.load(Ordering::SeqCst) == 1
            && metrics::SSE.load(Ordering::SeqCst) == 0
            && metrics::FALLBACKS.load(Ordering::SeqCst) == fallbacks
    })
    .await;
    drop(listener);
    drop(db);
    closed().await;

    server.state.mute_query.store(true, Ordering::SeqCst);
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    wait("una consulta sin confirmación termina en SSE", || {
        last(&seen) == json!({"value":6}) && metrics::SSE.load(Ordering::SeqCst) == 1
    })
    .await;
    server.state.mute_query.store(false, Ordering::SeqCst);
    wait("recuperarse de una consulta muda", || {
        metrics::WEBSOCKETS.load(Ordering::SeqCst) == 1 && metrics::SSE.load(Ordering::SeqCst) == 0
    })
    .await;
    drop(listener);
    drop(db);
    closed().await;

    server.state.reset.store(true, Ordering::SeqCst);
    let before = server.state.connects.load(Ordering::SeqCst);
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    wait("seguir la redirección antes de autenticar", || {
        last(&seen) == json!({"value":6})
    })
    .await;
    assert_eq!(server.state.connects.load(Ordering::SeqCst), before + 2);
    drop(listener);
    drop(db);
    closed().await;
    println!("Socket compartido, consultas duplicadas, permisos, renovación, reconexión, SSE, vigilancia y redirección: correctos.");
}

#[tokio::test]
async fn corte_breve_reconecta_sin_abrir_sse() {
    let _network = NETWORK.lock().await;
    let server = Server::new().await;
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    wait("conexión inicial", || last(&seen) == json!({"value":1})).await;
    let (connects, queries) = (
        server.state.connects.load(Ordering::SeqCst),
        server.state.queries.load(Ordering::SeqCst),
    );
    let fallbacks = metrics::FALLBACKS.load(Ordering::SeqCst);
    let sse = server.state.sse.load(Ordering::SeqCst);
    let _ = server.state.events.send(Event::Cut);
    wait("reconectar y volver a consultar", || {
        server.state.connects.load(Ordering::SeqCst) > connects
            && server.state.queries.load(Ordering::SeqCst) > queries
    })
    .await;
    // Margen para que un aviso `sse: true` equivocado abriera su conexión.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        metrics::FALLBACKS.load(Ordering::SeqCst),
        fallbacks,
        "Un corte que reconecta dentro de la gracia no debe abrir SSE"
    );
    assert_eq!(
        server.state.sse.load(Ordering::SeqCst),
        sse,
        "No debe abrirse ninguna conexión SSE"
    );
    drop(listener);
    drop(db);
    closed().await;
}

#[tokio::test]
async fn corte_largo_abre_sse_tras_la_gracia() {
    let _network = NETWORK.lock().await;
    let server = Server::new().await;
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    wait("conexión inicial", || last(&seen) == json!({"value":1})).await;
    let fallbacks = metrics::FALLBACKS.load(Ordering::SeqCst);
    // La red deja de aceptar WebSocket: el corte supera la gracia.
    server.state.blocked.store(true, Ordering::SeqCst);
    let _ = server.state.events.send(Event::Cut);
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        metrics::FALLBACKS.load(Ordering::SeqCst),
        fallbacks,
        "Todavía no venció la gracia"
    );
    wait("abrir SSE al vencer la gracia", || {
        metrics::FALLBACKS.load(Ordering::SeqCst) > fallbacks
    })
    .await;
    wait("una conexión SSE", || {
        metrics::SSE.load(Ordering::SeqCst) == 1
    })
    .await;
    assert_eq!(
        last(&seen),
        json!({"value":1}),
        "Conserva los datos mientras espera la reconexión"
    );
    server.state.blocked.store(false, Ordering::SeqCst);
    wait("volver al WebSocket y cerrar SSE", || {
        metrics::WEBSOCKETS.load(Ordering::SeqCst) == 1 && metrics::SSE.load(Ordering::SeqCst) == 0
    })
    .await;
    drop(listener);
    drop(db);
    closed().await;
}

#[tokio::test]
async fn websocket_que_nunca_conecta_abre_sse_de_inmediato() {
    let _network = NETWORK.lock().await;
    let server = Server::new().await;
    server.state.blocked.store(true, Ordering::SeqCst);
    let db = server.database(false);
    let (listener, seen) = listen(&db, "root/child");
    let started = Instant::now();
    wait("SSE al primer fallo", || {
        metrics::SSE.load(Ordering::SeqCst) == 1
    })
    .await;
    assert!(
        started.elapsed() < Duration::from_millis(1500),
        "Sin conexión previa no se espera la gracia"
    );
    assert_eq!(
        server.state.connects.load(Ordering::SeqCst),
        0,
        "El WebSocket nunca llegó a abrir"
    );
    wait("datos por SSE", || last(&seen) == json!({"value":1})).await;
    drop(listener);
    drop(db);
    closed().await;
}
