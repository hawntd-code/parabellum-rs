//! Selección automática por servicio. No identifica escuelas por nombre ni cambia el proxy global.
//! Un puente solo se elige tras reproducir un fallo directo con internet disponible.
use std::{collections::HashMap, sync::{atomic::{AtomicU64, Ordering}, Mutex, OnceLock}, time::{Duration, Instant}};
use serde::Serialize;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum Service { Mojang, Fabric, Quilt, Forge, NeoForge, Modrinth, Portal }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Access { Direct, Restricted, Offline, Unavailable }

pub struct Detector {
    client: reqwest::Client,
    probes: HashMap<Service, Vec<String>>,
    control: String,
    cache: Mutex<HashMap<Service, (Instant, Access)>>,
    checking: tokio::sync::Mutex<()>,
    generation: AtomicU64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Probe { Reachable, Filtered, Failed, Unavailable }

impl Detector {
    fn new(probes: HashMap<Service, Vec<String>>, control: String) -> Self {
        Self {
            client: reqwest::Client::builder().redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(3)).timeout(Duration::from_secs(5))
                .user_agent(concat!("Parabellum/", env!("CARGO_PKG_VERSION"))).build().expect("sonda de red"),
            probes, control, cache: Mutex::new(HashMap::new()), checking: tokio::sync::Mutex::new(()),
            generation: AtomicU64::new(0),
        }
    }

    fn cached(&self, service: Service) -> Option<Access> {
        self.cache.lock().unwrap().get(&service)
            .filter(|(at, _)| at.elapsed() < Duration::from_secs(60)).map(|(_, access)| *access)
    }

    fn invalidate(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.cache.lock().unwrap().clear();
    }

    async fn probe(&self, url: &str) -> Probe {
        let Ok(response) = self.client.head(url).send().await else { return Probe::Failed };
        let status = response.status();
        if status.is_server_error() { return Probe::Unavailable; }
        // Un rechazo JSON, 404 o 429 demuestra que el servidor es alcanzable. No es un bloqueo de red.
        let html = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.to_ascii_lowercase().contains("text/html"));
        let challenge = response.headers().contains_key("cf-ray")
            || response.headers().get("cf-mitigated").is_some_and(|v| v == "challenge");
        let page = reqwest::Url::parse(url).ok().is_some_and(|u| matches!(u.host_str(), Some("www.curseforge.com" | "curseforge.com")));
        if status.is_redirection() || status.as_u16() == 451
            || (html && ((!page && status.is_success()) || (status.as_u16() == 403 && !challenge))) {
            return Probe::Filtered;
        }
        Probe::Reachable
    }

    async fn direct(&self, service: Service) -> Probe {
        let Some(urls) = self.probes.get(&service) else { return Probe::Unavailable };
        for url in urls {
            let result = self.probe(url).await;
            if result != Probe::Reachable { return result; }
        }
        Probe::Reachable
    }

    pub async fn access(&self, service: Service) -> Access {
        if let Some(access) = self.cached(service) { return access; }
        let _checking = self.checking.lock().await;
        if let Some(access) = self.cached(service) { return access; }
        let generation = self.generation.load(Ordering::Relaxed);
        let result = match self.direct(service).await {
            Probe::Reachable => Access::Direct,
            Probe::Unavailable => Access::Unavailable,
            Probe::Failed | Probe::Filtered => {
                let connected = self.client.head(&self.control).send().await
                    .is_ok_and(|r| r.status() == reqwest::StatusCode::NO_CONTENT);
                if !connected { Access::Offline } else {
                    match self.direct(service).await {
                        Probe::Failed | Probe::Filtered => Access::Restricted,
                        Probe::Reachable => Access::Direct,
                        Probe::Unavailable => Access::Unavailable,
                    }
                }
            }
        };
        if generation == self.generation.load(Ordering::Relaxed) {
            self.cache.lock().unwrap().insert(service, (Instant::now(), result));
        }
        result
    }
}

fn detector() -> &'static Detector {
    static DETECTOR: OnceLock<Detector> = OnceLock::new();
    DETECTOR.get_or_init(|| Detector::new([
        (Service::Mojang, vec![
            "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json".into(),
            "https://piston-data.mojang.com/".into(),
            "https://resources.download.minecraft.net/".into(),
            "https://libraries.minecraft.net/".into(),
            "https://launchermeta.mojang.com/".into(),
        ]),
        (Service::Fabric, vec!["https://meta.fabricmc.net/v2/versions/game".into()]),
        (Service::Quilt, vec!["https://meta.quiltmc.org/v3/versions/game".into()]),
        (Service::Forge, vec!["https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml".into()]),
        (Service::NeoForge, vec!["https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml".into()]),
        (Service::Modrinth, vec!["https://api.modrinth.com/v2/tag/loader".into()]),
        // Se comprueban también los hosts de recursos: que abra el HTML no basta.
        (Service::Portal, vec!["https://www.curseforge.com/".into(), "https://static-beta.curseforge.com/robots.txt".into(), "https://media.forgecdn.net/robots.txt".into()]),
    ].into(), "https://www.gstatic.com/generate_204".into()))
}

pub async fn alternative(service: Service) -> bool { detector().access(service).await == Access::Restricted }
pub fn invalidate() { detector().invalidate(); }
pub async fn diagnostic() -> String {
    let mut lines = vec!["API CurseForge: Cloud (arquitectura conservada)".to_string()];
    for service in [Service::Mojang, Service::Fabric, Service::Quilt, Service::Forge, Service::NeoForge, Service::Modrinth, Service::Portal] {
        lines.push(format!("{service:?}: {:?}", detector().access(service).await));
    }
    lines.join("\n")
}
pub fn loader(loader: crate::loaders::Loader) -> Service {
    use crate::loaders::Loader;
    match loader { Loader::Fabric => Service::Fabric, Loader::Quilt => Service::Quilt, Loader::Forge => Service::Forge, Loader::NeoForge => Service::NeoForge }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpListener};
    use std::sync::{Arc, atomic::AtomicUsize};

    async fn fixture() -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let code = Arc::new(AtomicUsize::new(200));
        let state = code.clone();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else { break };
                let mut buffer = [0; 2048];
                let size = stream.read(&mut buffer).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buffer[..size]);
                let state = state.load(Ordering::Relaxed);
                let (status, content) = if request.starts_with("HEAD /control ") { (204, "text/plain") }
                    else if request.starts_with("HEAD /ok ") { (200, "application/json") }
                    else { match state {
                        299 => (200, "text/html"), 403 => (403, "application/json"),
                        398 | 399 => (403, "text/html"),
                        451 => (451, "text/html"), status => (status, "application/json"),
                    }};
                let challenge = if state == 398 { "cf-ray: prueba\r\ncf-mitigated: challenge\r\n" } else { "" };
                let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: {content}\r\n{challenge}Content-Length: 0\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        (url, code, task)
    }

    #[tokio::test]
    async fn restringida_solo_con_fallo_repetido_y_control_disponible() {
        let (url, code, task) = fixture().await;
        let detector = Detector::new([(Service::Modrinth, vec![format!("{url}/api")])].into(), format!("{url}/control"));
        code.store(299, Ordering::Relaxed);
        assert_eq!(detector.access(Service::Modrinth).await, Access::Restricted);
        // Simula volver a casa: la decisión anterior no queda pegada a toda la sesión.
        code.store(200, Ordering::Relaxed);
        detector.invalidate();
        assert_eq!(detector.access(Service::Modrinth).await, Access::Direct);
        task.abort();
    }

    #[tokio::test]
    async fn errores_del_proveedor_y_sin_internet_no_activan_puentes() {
        let (url, code, task) = fixture().await;
        let detector = Detector::new([(Service::Modrinth, vec![format!("{url}/api")])].into(), format!("{url}/control"));
        for status in [401, 403, 404, 429] {
            code.store(status, Ordering::Relaxed); detector.invalidate();
            assert_eq!(detector.access(Service::Modrinth).await, Access::Direct);
        }
        code.store(503, Ordering::Relaxed); detector.invalidate();
        assert_eq!(detector.access(Service::Modrinth).await, Access::Unavailable);
        code.store(299, Ordering::Relaxed);
        let offline = Detector::new([(Service::Modrinth, vec![format!("{url}/api")])].into(), format!("{url}/no-control"));
        assert_eq!(offline.access(Service::Modrinth).await, Access::Offline);
        task.abort();
    }

    #[tokio::test]
    async fn decision_por_servicio_y_desafio_no_es_bloqueo() {
        let (url, code, task) = fixture().await;
        let detector = Detector::new([
            (Service::Portal, vec![format!("{url}/portal")]),
            (Service::Mojang, vec![format!("{url}/ok")]),
        ].into(), format!("{url}/control"));
        code.store(399, Ordering::Relaxed);
        assert_eq!(detector.access(Service::Portal).await, Access::Restricted);
        assert_eq!(detector.access(Service::Mojang).await, Access::Direct);
        code.store(398, Ordering::Relaxed);
        assert_eq!(detector.access(Service::Portal).await, Access::Restricted); // Caché acotada.
        detector.invalidate();
        assert_eq!(detector.access(Service::Portal).await, Access::Direct); // El usuario resuelve el desafío.
        task.abort();
    }
}
