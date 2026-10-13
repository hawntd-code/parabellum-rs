//! Conexión directa entre PCs (P2P): para entrar a los servidores y mundos abiertos de los amigos sin
//! pasar por el túnel de playit.gg. Usa iroh: QUIC sobre UDP con perforación de NAT (las dos PCs se
//! hablan directo, también por IPv6) y, si las redes no lo permiten, un relevo de n0 en medio.
//!
//! - Anfitrión: acepta solo a sus amigos (cada Parabellum publica su dirección P2P en la presencia) y
//!   pasa cada conexión a un servidor encendido o al mundo abierto en LAN de este PC.
//! - Quien se une: abre un puerto en 127.0.0.1 y Minecraft se conecta ahí, como a un servidor de su
//!   propio PC. Sirve en todas las versiones y loaders, sin mods. Cada conexión del juego viaja como un
//!   flujo QUIC hacia el anfitrión.
//!
//! La identidad P2P es nueva en cada arranque (no se guarda nada): se publica en la presencia.

use iroh::endpoint::{Connection, RecvStream, SendStream, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl, TransportAddr};
use std::collections::{HashMap, VecDeque};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const ALPN: &[u8] = b"parabellum/minecraft/1";

/// datos/logs/p2p.log: quién se conectó, si fue directo o por relevo y los fallos.
static LOG: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

pub fn log_to(path: std::path::PathBuf) {
    let _ = LOG.set(path);
}

pub(crate) fn note(text: String) {
    eprintln!("p2p: {text}");
    if let Some(path) = LOG.get() {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(file, "{} {text}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    }
}
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Para cada flujo entrante: (id P2P de quien se conecta, destino pedido) -> puerto local al que va.
/// Err: no se deja pasar, con el motivo (no es amigo, o ese servidor no está encendido) para p2p.log.
pub type Gate = Arc<dyn Fn(&str, &str) -> Result<u16, String> + Send + Sync>;

/// Alguien que está jugando ahora en este PC por P2P.
#[derive(Clone, Debug)]
pub struct Guest {
    /// Su identidad P2P (la que publica en su presencia).
    pub id: String,
    /// "world" o la clave del servidor.
    pub target: String,
    /// Directo entre las dos PCs (si no, por el relevo).
    pub direct: bool,
    pub rtt_ms: Option<u32>,
    /// El ping más alto del último minuto y el % de paquetes perdidos (de la telemetría).
    pub peak_ms: Option<u32>,
    pub loss_pct: f32,
}

/// Una conexión entrante y cuántas conexiones del juego lleva abiertas a cada destino.
struct Visit {
    connection: Connection,
    streams: HashMap<String, usize>,
    health: Arc<Mutex<Health>>,
}

/// Cómo le fue a una conexión en el último minuto (lo llena `monitor` cada segundo).
#[derive(Default, Clone, Copy)]
struct Health {
    peak_ms: Option<u32>,
    loss_pct: f32,
}

/// Un túnel abierto hacia la partida de un amigo.
#[derive(Clone, Debug)]
pub struct Link {
    /// Puerto en 127.0.0.1 al que se conecta Minecraft.
    pub port: u16,
    /// Directo entre las dos PCs (si no, por el relevo).
    pub direct: bool,
    pub rtt_ms: Option<u32>,
}

pub struct P2p {
    endpoint: Endpoint,
    gate: Gate,
    /// Una conexión por amigo (se reutiliza para todas las conexiones del juego).
    connections: tokio::sync::Mutex<HashMap<EndpointId, Connection>>,
    /// La última dirección conocida de cada amigo (su relevo puede cambiar).
    addresses: Mutex<HashMap<EndpointId, EndpointAddr>>,
    /// (amigo, destino) -> puerto local ya abierto.
    tunnels: Mutex<HashMap<(EndpointId, String), u16>>,
    /// Quien se une: cuántas conexiones del juego van ahora hacia cada amigo (para la telemetría).
    playing: Arc<Mutex<HashMap<EndpointId, usize>>>,
    /// Anfitrión: quién está conectado a este PC ahora (por su identidad P2P).
    visits: Mutex<HashMap<String, Visit>>,
}

impl P2p {
    pub async fn start(gate: Gate) -> Result<Arc<P2p>, String> {
        let endpoint = Endpoint::builder(presets::Minimal)
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(RelayMode::Default)
            .bind()
            .await
            .map_err(|e| e.to_string())?;
        let p2p = Arc::new(P2p {
            endpoint,
            gate,
            connections: Default::default(),
            addresses: Default::default(),
            tunnels: Default::default(),
            playing: Default::default(),
            visits: Default::default(),
        });
        tokio::spawn(p2p.clone().serve());
        note(format!("listo ({})", p2p.endpoint.id().fmt_short()));
        Ok(p2p)
    }

    /// Dirección P2P para publicar: "id|relevo". None mientras no hay relevo (sin internet).
    pub fn address(&self) -> Option<String> {
        let addr = self.endpoint.addr();
        let relay = addr.relay_urls().next()?;
        Some(format!("{}|{relay}", addr.id))
    }

    /// Espera (como mucho `limit`) a tener relevo, para poder publicar la dirección.
    pub async fn online(&self, limit: Duration) -> bool {
        tokio::time::timeout(limit, self.endpoint.online()).await.is_ok()
    }

    pub async fn close(&self) {
        let _ = tokio::time::timeout(Duration::from_secs(2), self.endpoint.close()).await;
    }

    // ================= Anfitrión

    async fn serve(self: Arc<Self>) {
        while let Some(incoming) = self.endpoint.accept().await {
            let p2p = self.clone();
            tokio::spawn(async move {
                let Ok(connection) = incoming.await else { return };
                let remote = connection.remote_id().to_string();
                let stable = connection.stable_id();
                let health = Arc::new(Mutex::new(Health::default()));
                p2p.visits.lock().unwrap().insert(remote.clone(), Visit { connection: connection.clone(), streams: HashMap::new(), health: health.clone() });
                tokio::spawn(note_path(connection.clone(), remote.clone()));
                // Telemetría mientras haya juego por esta conexión.
                let (watch, who) = (p2p.clone(), remote.clone());
                let active = move || watch.visits.lock().unwrap().get(&who).is_some_and(|v| !v.streams.is_empty());
                tokio::spawn(monitor(connection.clone(), "anfitrion", short(&remote).to_string(), active, health));
                // Los flujos de cada conexión del juego (y los pings de la lista de servidores).
                while let Ok((send, recv)) = connection.accept_bi().await {
                    let (p2p, remote) = (p2p.clone(), remote.clone());
                    tokio::spawn(async move {
                        let _ = p2p.serve_stream(&remote, send, recv).await;
                    });
                }
                // Se fue (si no volvió a entrar con otra conexión mientras tanto).
                let mut visits = p2p.visits.lock().unwrap();
                if visits.get(&remote).is_some_and(|v| v.connection.stable_id() == stable) {
                    visits.remove(&remote);
                }
            });
        }
    }

    async fn serve_stream(&self, remote: &str, send: SendStream, mut recv: RecvStream) -> std::io::Result<()> {
        // Cabecera: largo del destino (1 byte) y el destino ("world" o la clave del servidor).
        let mut length = [0u8; 1];
        AsyncReadExt::read_exact(&mut recv, &mut length).await?;
        let mut target = vec![0u8; length[0] as usize];
        AsyncReadExt::read_exact(&mut recv, &mut target).await?;
        let target = String::from_utf8_lossy(&target).to_string();
        let port = match (self.gate)(remote, &target) {
            Ok(port) => port,
            Err(why) => {
                note(format!("rechazado {} -> {target}: {why}", short(remote)));
                return Ok(());
            }
        };
        self.count(remote, &target, true);
        let result = async {
            let tcp = TcpStream::connect(("127.0.0.1", port)).await?;
            pipe(tcp, send, recv).await;
            Ok(())
        }
        .await;
        self.count(remote, &target, false);
        result
    }

    /// Una conexión del juego más (o menos) de ese amigo hacia ese destino.
    fn count(&self, remote: &str, target: &str, add: bool) {
        if let Some(visit) = self.visits.lock().unwrap().get_mut(remote) {
            let streams = visit.streams.entry(target.to_string()).or_default();
            if add {
                *streams += 1;
            } else {
                *streams = streams.saturating_sub(1);
            }
            if *streams == 0 {
                visit.streams.remove(target);
            }
        }
    }

    /// Quienes están jugando ahora en este PC por P2P: a qué destino y por dónde va su conexión.
    pub fn guests(&self) -> Vec<Guest> {
        self.visits
            .lock()
            .unwrap()
            .iter()
            .flat_map(|(id, visit)| {
                let (direct, rtt_ms) = path_info(&visit.connection);
                let health = *visit.health.lock().unwrap();
                visit.streams.keys().map(move |target| Guest { id: id.clone(), target: target.clone(), direct, rtt_ms, peak_ms: health.peak_ms, loss_pct: health.loss_pct })
            })
            .collect()
    }

    // ================= Quien se une

    /// Abre (o reutiliza) el túnel a la partida de un amigo: `address` es su dirección P2P publicada
    /// y `target`, "world" o la clave del servidor. Espera unos segundos a que la conexión pase a
    /// directa, para que el tiempo medido sea el de verdad.
    pub async fn open(self: &Arc<Self>, address: &str, target: &str) -> Result<Link, String> {
        if target.is_empty() || target.len() > 64 {
            return Err("destino no válido".into());
        }
        let addr = parse_address(address)?;
        let id = addr.id;
        self.addresses.lock().unwrap().insert(id, addr);
        let connection = self.connection(id).await?;
        let existing = self.tunnels.lock().unwrap().get(&(id, target.to_string())).copied();
        let port = match existing {
            Some(port) => port,
            None => self.listen(id, target).await?,
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path_info(&connection).0 && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let (direct, rtt_ms) = path_info(&connection);
        note(format!("{} -> {target}: {} ({} ms)", id.fmt_short(), if direct { "directo" } else { "relevo" }, rtt_ms.map_or("?".into(), |ms| ms.to_string())));
        Ok(Link { port, direct, rtt_ms })
    }

    /// La conexión con un amigo: la que ya hay o una nueva.
    async fn connection(&self, id: EndpointId) -> Result<Connection, String> {
        let mut connections = self.connections.lock().await;
        if let Some(connection) = connections.get(&id).filter(|c| c.close_reason().is_none()) {
            return Ok(connection.clone());
        }
        let addr = self.addresses.lock().unwrap().get(&id).cloned().ok_or("sin dirección")?;
        let connection = tokio::time::timeout(CONNECT_TIMEOUT, self.endpoint.connect(addr, ALPN))
            .await
            .map_err(|_| t!("No se pudo conectar directo (tardó demasiado).", "Could not connect directly (it took too long)."))?
            .map_err(|e| {
                note(format!("no se pudo conectar con {} ({e})", id.fmt_short()));
                e.to_string()
            })?;
        connections.insert(id, connection.clone());
        // Telemetría mientras haya juego por esta conexión (también del lado de quien se une).
        let playing = self.playing.clone();
        let active = move || playing.lock().unwrap().get(&id).is_some_and(|n| *n > 0);
        tokio::spawn(monitor(connection.clone(), "invitado", id.fmt_short().to_string(), active, Default::default()));
        Ok(connection)
    }

    /// Un puerto local que lleva cada conexión de Minecraft al destino en el PC del amigo.
    async fn listen(self: &Arc<Self>, id: EndpointId, target: &str) -> Result<u16, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        self.tunnels.lock().unwrap().insert((id, target.to_string()), port);
        let (p2p, target) = (self.clone(), target.to_string());
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let (p2p, target) = (p2p.clone(), target.clone());
                tokio::spawn(async move {
                    let connection = match p2p.connection(id).await {
                        Ok(connection) => connection,
                        Err(_) => return,
                    };
                    let Ok((mut send, recv)) = connection.open_bi().await else { return };
                    let mut header = vec![target.len() as u8];
                    header.extend(target.as_bytes());
                    if send.write_all(&header).await.is_ok() {
                        *p2p.playing.lock().unwrap().entry(id).or_default() += 1;
                        pipe(tcp, send, recv).await;
                        if let Some(n) = p2p.playing.lock().unwrap().get_mut(&id) {
                            *n = n.saturating_sub(1);
                        }
                    }
                });
            }
        });
        Ok(port)
    }
}

/// Copia en los dos sentidos hasta que se cierren los dos lados.
async fn pipe(tcp: TcpStream, mut send: SendStream, mut recv: RecvStream) {
    let _ = tcp.set_nodelay(true);
    let (mut tcp_read, mut tcp_write) = tcp.into_split();
    let up = async {
        let _ = tokio::io::copy(&mut tcp_read, &mut send).await;
        let _ = send.finish();
    };
    let down = async {
        let _ = tokio::io::copy(&mut recv, &mut tcp_write).await;
        let _ = tcp_write.shutdown().await;
    };
    tokio::join!(up, down);
}

/// Cada segundo, mientras haya juego por la conexión: ping, camino y paquetes perdidos. Lo deja en
/// p2p-latencia.csv (junto a p2p.log) y anota en p2p.log lo que causa saltos en el juego: que pase al
/// relevo, un pico de ping o que se pierdan paquetes. `health` guarda el resumen del último minuto.
async fn monitor(connection: Connection, role: &'static str, who: String, active: impl Fn() -> bool + Send + 'static, health: Arc<Mutex<Health>>) {
    let csv = LOG.get().map(|log| log.with_file_name("p2p-latencia.csv"));
    let mut window: VecDeque<(Instant, u32, u64, u64)> = VecDeque::new();
    let mut last = connection.stats();
    let mut was_direct: Option<bool> = None;
    let mut quiet_until = Instant::now();
    // Al conectar se prueban caminos y esos paquetes de prueba cuentan como perdidos: no son lag.
    let warm_until = Instant::now() + Duration::from_secs(10);
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if connection.close_reason().is_some() {
            return;
        }
        let stats = connection.stats();
        let lost = if Instant::now() < warm_until { 0 } else { stats.lost_packets.saturating_sub(last.lost_packets) };
        let sent = stats.udp_tx.datagrams.saturating_sub(last.udp_tx.datagrams);
        last = stats;
        if !active() {
            continue;
        }
        let (direct, rtt) = path_info(&connection);
        let Some(rtt) = rtt else { continue };
        let now = Instant::now();
        // Lo normal: la mediana del último minuto (antes de sumar esta medida).
        let mut previous: Vec<u32> = window.iter().map(|(_, r, _, _)| *r).collect();
        previous.sort_unstable();
        let normal = (previous.len() >= 5).then(|| previous[previous.len() / 2]);
        window.push_back((now, rtt, lost, sent));
        while window.front().is_some_and(|(at, ..)| now.duration_since(*at) > Duration::from_secs(60)) {
            window.pop_front();
        }
        let (lost_minute, sent_minute) = window.iter().fold((0, 0), |(l, s), (_, _, lost, sent)| (l + lost, s + sent));
        *health.lock().unwrap() = Health {
            peak_ms: window.iter().map(|(_, r, _, _)| *r).max(),
            loss_pct: if sent_minute > 0 { lost_minute as f32 * 100.0 / sent_minute as f32 } else { 0.0 },
        };
        if let Some(path) = &csv {
            use std::io::Write as _;
            let new = !path.exists();
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                if new {
                    let _ = writeln!(file, "hora,rol,par,camino,ping_ms,perdidos,enviados");
                }
                let _ = writeln!(file, "{},{role},{who},{},{rtt},{lost},{sent}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), if direct { "directo" } else { "relevo" });
            }
        }
        // Avisos (como mucho uno cada 5 s, salvo el cambio de camino, que siempre se anota).
        if was_direct.is_some_and(|d| d != direct) {
            note(format!("{who}: {} ({rtt} ms)", if direct { "volvió a directo" } else { "pasó al relevo" }));
        }
        was_direct = Some(direct);
        if now >= quiet_until {
            if let Some(normal) = normal.filter(|n| rtt > (n * 5 / 2).max(n + 40)) {
                note(format!("{who}: pico de ping {rtt} ms (lo normal: {normal} ms)"));
                quiet_until = now + Duration::from_secs(5);
            } else if lost >= 3 && lost * 50 > sent {
                note(format!("{who}: se perdieron {lost} de {sent} paquetes en 1 s"));
                quiet_until = now + Duration::from_secs(5);
            }
        }
    }
}

/// Anfitrión: anota por dónde quedó la conexión de quien entra. Espera unos segundos a que pase a
/// directa (al principio casi siempre va por el relevo), como hace quien se une.
async fn note_path(connection: Connection, remote: String) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !path_info(&connection).0 && Instant::now() < deadline && connection.close_reason().is_none() {
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let (direct, rtt_ms) = path_info(&connection);
    note(format!("{} se conectó: {} ({} ms)", short(&remote), if direct { "directo" } else { "relevo" }, rtt_ms.map_or("?".into(), |ms| ms.to_string())));
}

/// (directo, tiempo de ida y vuelta) del camino que usa ahora la conexión.
fn path_info(connection: &Connection) -> (bool, Option<u32>) {
    let paths = connection.paths();
    let selected = paths.iter().find(|p| p.is_selected());
    selected.map(|p| (p.is_ip(), Some(p.rtt().as_millis() as u32))).unwrap_or((false, None))
}

fn parse_address(address: &str) -> Result<EndpointAddr, String> {
    let (id, relay) = address.split_once('|').unwrap_or((address, ""));
    let id = EndpointId::from_str(id.trim()).map_err(|_| "dirección P2P no válida".to_string())?;
    let relay = RelayUrl::from_str(relay.trim()).ok();
    Ok(EndpointAddr::from_parts(id, relay.map(TransportAddr::Relay)))
}

fn short(id: &str) -> &str {
    &id[..id.len().min(10)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Dos Parabellum en este PC: uno sirve un «servidor» de eco y el otro entra por el túnel.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "usa internet (relevos de n0)"]
    async fn tunel_de_ida_y_vuelta() {
        let logs = std::env::temp_dir().join(format!("parabellum-p2p-{}", std::process::id()));
        std::fs::create_dir_all(&logs).unwrap();
        log_to(logs.join("p2p.log"));
        let server = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_port = server.local_addr().unwrap().port();
        tokio::spawn(async move {
            while let Ok((mut tcp, _)) = server.accept().await {
                tokio::spawn(async move {
                    let (mut r, mut w) = tcp.split();
                    let _ = tokio::io::copy(&mut r, &mut w).await;
                });
            }
        });
        let host = P2p::start(Arc::new(move |_, target| (target == "world").then_some(server_port).ok_or_else(|| "no".to_string()))).await.unwrap();
        let guest = P2p::start(Arc::new(|_, _| Err("no".to_string()))).await.unwrap();
        assert!(host.online(Duration::from_secs(15)).await);
        let address = host.address().unwrap();
        // Un destino que el anfitrión no deja pasar: se cierra sin respuesta.
        let denied = guest.open(&address, "s000000000000").await.unwrap();
        let mut tcp = TcpStream::connect(("127.0.0.1", denied.port)).await.unwrap();
        tcp.write_all(b"hola").await.unwrap();
        let mut buf = Vec::new();
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), tcp.read_to_end(&mut buf)).await.unwrap().unwrap(), 0);
        // El permitido: eco.
        let link = guest.open(&address, "world").await.unwrap();
        let mut tcp = TcpStream::connect(("127.0.0.1", link.port)).await.unwrap();
        tcp.write_all(b"hola p2p").await.unwrap();
        let mut buf = [0u8; 8];
        tcp.read_exact(&mut buf).await.unwrap();
        assert_eq!(&buf, b"hola p2p");
        println!("directo: {} rtt: {:?}", link.direct, link.rtt_ms);
        // El anfitrión ve quién está entrando, a qué destino y por dónde (el rechazado no cuenta).
        let guests = host.guests();
        assert_eq!(guests.len(), 1, "{guests:?}");
        assert_eq!(guests[0].target, "world");
        assert!(address.starts_with(&host.endpoint.id().to_string()));
        println!("anfitrión ve: {guests:?}");
        // Telemetría: con el juego abierto unos segundos, cada lado anota su ping cada segundo.
        for _ in 0..3 {
            tcp.write_all(b"ping").await.unwrap();
            let mut back = [0u8; 4];
            tcp.read_exact(&mut back).await.unwrap();
            tokio::time::sleep(Duration::from_millis(1000)).await;
        }
        let guests = host.guests();
        assert!(guests[0].peak_ms.is_some(), "{guests:?}");
        let csv = std::fs::read_to_string(LOG.get().unwrap().with_file_name("p2p-latencia.csv")).unwrap();
        assert!(csv.contains(",anfitrion,") && csv.contains(",invitado,"), "{csv}");
        println!("telemetría:
{csv}");
        // Al salir del juego (se cierra la conexión del juego) ya no aparece.
        drop(tcp);
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(host.guests().is_empty(), "{:?}", host.guests());
        guest.close().await;
        host.close().await;
    }

    #[test]
    fn direcciones() {
        let id = iroh::SecretKey::generate().public().to_string();
        let addr = parse_address(&format!("{id}|https://use1-1.relay.n0.iroh.link./")).unwrap();
        assert_eq!(addr.id.to_string(), id);
        assert_eq!(addr.relay_urls().count(), 1);
        assert!(parse_address("no-es-un-id|x").is_err());
    }
}
