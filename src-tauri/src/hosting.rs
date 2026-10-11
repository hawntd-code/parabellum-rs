//! «Hostear partida»: los servidores en marcha (consola, jugadores, memoria y procesador), el túnel de
//! playit.gg que les da una dirección pública fija, el mundo abierto en LAN compartido por el mismo
//! túnel y lo que se publica en Firebase para que los invitados vean la dirección actual.
//!
//! Solo hay un túnel: apunta al primer servidor listo o, si no hay ninguno, al mundo abierto en LAN.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, Write};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::diagnostico::{self, Diagnosis, Fix};
use crate::playit;
use crate::proceso;
use crate::servidor::{self, Guest, LeftOut, ServerConfig};
use crate::{cuenta, mojang, AppState};

const CONSOLE_LINES: usize = 4000;
/// Arranques automáticos tras quitar lo que tumbaba el servidor.
const AUTO_FIXES: u32 = 4;
const SAVED_FILE: &str = "hostear.json";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Starting,
    Ready,
    Stopping,
}

struct Running {
    pid: u32,
    port: u16,
    stdin: Option<std::process::ChildStdin>,
    phase: Phase,
    players: Vec<String>,
    started: i64,
    console: VecDeque<String>,
    pending: Vec<String>,
    output: Option<std::io::BufWriter<std::fs::File>>,
    cpu: proceso::CpuMeter,
    cpu_percent: f32,
    memory: u64,
    stop_requested: bool,
    /// Se cambió algo que el servidor solo lee al arrancar (server.properties, memoria…).
    restart_needed: bool,
    /// Al apagarse, se vuelve a encender (Reiniciar).
    restart: bool,
}

/// Cómo terminó la última vez (si fue mal): se muestra hasta el próximo arranque.
#[derive(Clone, Default)]
struct Outcome {
    error: Option<String>,
    diagnosis: Option<Diagnosis>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Target {
    Server(String),
    World,
}

#[derive(Default)]
struct TunnelRuntime {
    agent: Option<playit::Agent>,
    target: Option<(Target, u16)>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayitView {
    pub connected: bool,
    pub agent_installed: bool,
    /// Página de playit.gg que hay que aceptar (mientras se vincula).
    pub claim_url: Option<String>,
    /// "off", "starting", "on" o "error".
    pub state: String,
    /// Dirección pública (fija: la misma siempre que se use el túnel).
    pub address: Option<String>,
    /// A qué apunta ahora: "world" o el id del servidor.
    pub target: Option<String>,
    /// "guest", "email-not-verified" o "verified".
    pub account: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
struct Saved {
    address: Option<String>,
    account: Option<String>,
    /// Compartir por playit.gg el mundo que se abra en LAN.
    share_world: bool,
}

impl Default for Saved {
    fn default() -> Self {
        Saved { address: None, account: None, share_world: true }
    }
}

#[derive(Clone, PartialEq, Eq)]
struct WorldShare {
    port: u16,
    instance: String,
}

#[derive(Default)]
pub struct Hosting {
    running: Mutex<HashMap<String, Running>>,
    outcomes: Mutex<HashMap<String, Outcome>>,
    auto_fixes: Mutex<HashMap<String, u32>>,
    tunnel: tokio::sync::Mutex<TunnelRuntime>,
    playit: Mutex<PlayitView>,
    claim: Mutex<Option<String>>,
    world: Mutex<Option<WorldShare>>,
    saved: Mutex<Saved>,
    /// Los nombres (anfitrión y amigos) la última vez que se revisaron los servidores.
    names_seen: Mutex<String>,
}

// ================= Vista para la interfaz

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ServerView {
    #[serde(flatten)]
    config: ServerConfig,
    /// "off", "starting", "ready" o "stopping".
    state: &'static str,
    players: Vec<String>,
    started: Option<i64>,
    cpu: f32,
    memory_mb: u64,
    mods: usize,
    max_players: u32,
    /// Dirección pública si el túnel apunta a este servidor.
    address: Option<String>,
    /// IP:puerto en la red local.
    lan: Option<String>,
    error: Option<String>,
    diagnosis: Option<Diagnosis>,
    /// Hay cambios que se aplican al reiniciar.
    restart_needed: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorldView {
    port: u16,
    instance: String,
    address: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HostView {
    playit: PlayitView,
    share_world: bool,
    world: Option<WorldView>,
    servers: Vec<ServerView>,
    total_mb: u64,
    free_mb: u64,
}

fn hosting(app: &AppHandle) -> &Hosting {
    &app.state::<AppState>().inner().hosting
}

pub fn view(app: &AppHandle) -> HostView {
    let state = app.state::<AppState>();
    let hosting = &state.hosting;
    let playit = hosting.playit.lock().unwrap().clone();
    let public = |target: &str| (playit.state == "on" && playit.target.as_deref() == Some(target)).then(|| playit.address.clone()).flatten();
    let local_ip = crate::actividad::local_address();
    let running = hosting.running.lock().unwrap();
    let outcomes = hosting.outcomes.lock().unwrap();
    let servers = servidor::list(&state.paths)
        .into_iter()
        .map(|config| {
            let run = running.get(&config.id);
            let outcome = outcomes.get(&config.id).cloned().unwrap_or_default();
            let max_players = servidor::property(&config.folder, "max-players").and_then(|v| v.parse().ok()).unwrap_or(20);
            ServerView {
                state: match run.map(|r| r.phase) {
                    None => "off",
                    Some(Phase::Starting) => "starting",
                    Some(Phase::Ready) => "ready",
                    Some(Phase::Stopping) => "stopping",
                },
                players: run.map(|r| r.players.clone()).unwrap_or_default(),
                started: run.map(|r| r.started),
                cpu: run.map(|r| r.cpu_percent).unwrap_or(0.0),
                memory_mb: run.map(|r| r.memory / 1_048_576).unwrap_or(0),
                mods: servidor::mod_count(&config.folder),
                max_players,
                address: public(&config.id),
                lan: run.and(local_ip.as_ref()).map(|ip| format!("{ip}:{}", config.port)),
                error: outcome.error,
                diagnosis: outcome.diagnosis,
                restart_needed: run.is_some_and(|r| r.restart_needed),
                config,
            }
        })
        .collect();
    drop(outcomes);
    drop(running);
    let (total, free) = proceso::system_memory();
    let share_world = hosting.saved.lock().unwrap().share_world;
    let world = hosting.world.lock().unwrap().clone();
    let view = HostView {
        world: world.map(|w| WorldView { port: w.port, instance: w.instance, address: public("world") }),
        playit,
        share_world,
        servers,
        total_mb: total / 1_048_576,
        free_mb: free / 1_048_576,
    };
    view
}

fn emit(app: &AppHandle) {
    let _ = app.emit("hosting", view(app));
}

fn update_playit(app: &AppHandle, change: impl FnOnce(&mut PlayitView)) {
    change(&mut hosting(app).playit.lock().unwrap());
    emit(app);
}

fn save_saved(app: &AppHandle) {
    let state = app.state::<AppState>();
    let saved = state.hosting.saved.lock().unwrap().clone();
    let _ = std::fs::write(state.paths.data.join(SAVED_FILE), serde_json::to_vec_pretty(&saved).unwrap());
}

// ================= Arranque de Parabellum

/// Lee lo guardado y empieza a medir los servidores en marcha (cada 2 s).
pub fn init(app: &AppHandle) {
    let state = app.state::<AppState>();
    let saved: Saved = std::fs::read_to_string(state.paths.data.join(SAVED_FILE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    {
        let mut playit = state.hosting.playit.lock().unwrap();
        playit.connected = playit::load_secret(&state.paths).is_some();
        playit.agent_installed = playit::agent_installed(&state.paths);
        playit.state = "off".into();
        playit.address = saved.address.clone();
        playit.account = saved.account.clone();
    }
    *state.hosting.saved.lock().unwrap() = saved;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick: u64 = 0;
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            tick += 1;
            let ids: Vec<String> = {
                let mut running = hosting(&app).running.lock().unwrap();
                for run in running.values_mut() {
                    if let Some((memory, cpu)) = proceso::usage(run.pid) {
                        run.memory = memory;
                        run.cpu_percent = run.cpu.sample(cpu);
                    }
                }
                running.keys().cloned().collect()
            };
            if !ids.is_empty() {
                emit(&app);
                // Los invitados ven el servidor encendido mientras llegue una señal por minuto.
                if tick % 30 == 0 {
                    for id in &ids {
                        publish(&app, id).await;
                    }
                }
            }
            // Si el agente de playit se cerró solo, se vuelve a abrir.
            if tick % 5 == 0 {
                let dead = {
                    let mut tunnel = hosting(&app).tunnel.lock().await;
                    tunnel.target.is_some() && !tunnel.agent.as_mut().is_some_and(|a| a.running())
                };
                if dead {
                    retarget(&app).await;
                }
            }
        }
    });
}

/// El servidor encendido (listo) que se anuncia a los amigos en la presencia.
pub fn hosted_name(app: &AppHandle) -> Option<String> {
    let ids: Vec<String> = {
        let running = hosting(app).running.lock().unwrap();
        let mut ready: Vec<(&String, &Running)> = running.iter().filter(|(_, r)| r.phase == Phase::Ready).collect();
        ready.sort_by_key(|(_, r)| r.started);
        ready.into_iter().map(|(id, _)| id.clone()).collect()
    };
    let paths = app.state::<AppState>().paths.clone();
    ids.into_iter().find_map(|id| servidor::find(&paths, &id)).map(|config| config.name)
}

/// ¿Hay algo que se cortaría al cerrar Parabellum? (servidores encendidos o el mundo compartido)
pub fn busy(app: &AppHandle) -> bool {
    !hosting(app).running.lock().unwrap().is_empty()
}

/// Al salir: cada servidor guarda el mundo y se apaga (hasta 40 s), y se cierra el agente.
pub fn shutdown(app: &AppHandle) {
    let ids: Vec<String> = hosting(app).running.lock().unwrap().keys().cloned().collect();
    for id in &ids {
        stop_server(app, id);
    }
    for _ in 0..200 {
        if hosting(app).running.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    for run in hosting(app).running.lock().unwrap().values() {
        crate::launch::kill(run.pid);
    }
    let agent = tauri::async_runtime::block_on(async { hosting(app).tunnel.lock().await.agent.take() });
    if let Some(agent) = agent {
        agent.stop();
    }
    tauri::async_runtime::block_on(async {
        for id in &ids {
            let _ = tokio::time::timeout(Duration::from_secs(3), publish(app, id)).await;
        }
    });
}

// ================= playit.gg

/// Vincula Parabellum con una cuenta de playit.gg (se acepta en el navegador).
pub async fn connect_playit(app: &AppHandle, progress: impl Fn(String, f64) + Send + Sync + 'static) -> Result<(), String> {
    let paths = app.state::<AppState>().paths.clone();
    playit::download_agent(&paths, progress).await?;
    update_playit(app, |p| {
        p.agent_installed = true;
        p.message = None;
    });
    let code = playit::claim_code();
    let url = playit::claim_url(&code);
    *hosting(app).claim.lock().unwrap() = Some(code.clone());
    update_playit(app, |p| p.claim_url = Some(url.clone()));
    let cancelled = || hosting(app).claim.lock().unwrap().as_deref() != Some(code.as_str());
    let finish = |app: &AppHandle| {
        *hosting(app).claim.lock().unwrap() = None;
        update_playit(app, |p| p.claim_url = None);
    };
    // El código se registra antes de abrir la página (si no, playit.gg no lo encuentra).
    let mut opened = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(15 * 60);
    loop {
        if cancelled() {
            return Err(t!("Se canceló la conexión con playit.gg.", "The playit.gg connection was cancelled."));
        }
        if std::time::Instant::now() > deadline {
            finish(app);
            return Err(t!("Pasó mucho tiempo sin aceptar en playit.gg. Vuelve a intentarlo.", "Too much time passed without accepting on playit.gg. Try again."));
        }
        match playit::claim_poll(&code).await {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) => {
                finish(app);
                return Err(error);
            }
        }
        if !opened {
            opened = true;
            let _ = crate::google::open_browser(&url);
        }
        tokio::time::sleep(Duration::from_millis(1500)).await;
    }
    let mut secret = None;
    for _ in 0..30 {
        match playit::claim_exchange(&code).await {
            Ok(Some(key)) => {
                secret = Some(key);
                break;
            }
            Ok(None) => tokio::time::sleep(Duration::from_secs(2)).await,
            Err(error) => {
                finish(app);
                return Err(error);
            }
        }
    }
    finish(app);
    let secret = secret.ok_or(t!("playit.gg aceptó, pero no entregó la clave. Vuelve a intentarlo.", "playit.gg accepted but did not hand over the key. Try again."))?;
    playit::save_secret(&paths, &secret)?;
    let account = playit::run_data(&secret).await.ok().map(|d| d.account);
    {
        let mut saved = hosting(app).saved.lock().unwrap();
        saved.account = account.clone();
    }
    save_saved(app);
    update_playit(app, |p| {
        p.connected = true;
        p.account = account;
        p.state = "off".into();
        p.message = None;
    });
    retarget(app).await;
    Ok(())
}

pub fn cancel_claim(app: &AppHandle) {
    *hosting(app).claim.lock().unwrap() = None;
    update_playit(app, |p| p.claim_url = None);
}

/// Olvida la clave de este PC (el agente sigue en la cuenta de playit.gg hasta que lo borres allí).
pub async fn disconnect_playit(app: &AppHandle) {
    let paths = app.state::<AppState>().paths.clone();
    {
        let mut tunnel = hosting(app).tunnel.lock().await;
        if let Some(agent) = tunnel.agent.take() {
            agent.stop();
        }
        tunnel.target = None;
    }
    playit::clear_secret(&paths);
    *hosting(app).saved.lock().unwrap() = Saved { share_world: hosting(app).saved.lock().unwrap().share_world, ..Default::default() };
    save_saved(app);
    update_playit(app, |p| {
        *p = PlayitView { agent_installed: p.agent_installed, state: "off".into(), ..Default::default() };
    });
}

pub fn set_share_world(app: &AppHandle, share: bool) {
    hosting(app).saved.lock().unwrap().share_world = share;
    save_saved(app);
    emit(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move { retarget(&app).await });
}

/// Cada 2 s, desde la presencia: el puerto del mundo abierto en LAN (si hay juego y lo abrió).
pub fn world_tick(app: &AppHandle, lan: Option<(u16, String)>) {
    let changed = {
        let mut world = hosting(app).world.lock().unwrap();
        let new = lan.map(|(port, instance)| WorldShare { port, instance });
        let changed = *world != new;
        *world = new;
        changed
    };
    if changed {
        emit(app);
        let app = app.clone();
        tauri::async_runtime::spawn(async move { retarget(&app).await });
    }
}

/// Dirección pública del mundo abierto (para la presencia: tus amigos pulsan «Unirse»).
pub fn world_address(app: &AppHandle) -> Option<String> {
    let playit = hosting(app).playit.lock().unwrap();
    (playit.state == "on" && playit.target.as_deref() == Some("world")).then(|| playit.address.clone()).flatten()
}

/// Apunta el túnel a lo que toca: el primer servidor listo o, si no hay, el mundo abierto.
pub async fn retarget(app: &AppHandle) {
    let state = app.state::<AppState>();
    let hosting = &state.hosting;
    let desired: Option<(Target, u16)> = {
        let running = hosting.running.lock().unwrap();
        let server = running.iter().filter(|(_, r)| r.phase == Phase::Ready).min_by_key(|(_, r)| r.started).map(|(id, r)| (Target::Server(id.clone()), r.port));
        let share = hosting.saved.lock().unwrap().share_world;
        server.or_else(|| hosting.world.lock().unwrap().as_ref().filter(|_| share).map(|w| (Target::World, w.port)))
    };
    let mut tunnel = hosting.tunnel.lock().await;
    let alive = tunnel.agent.as_mut().is_some_and(|a| a.running());
    if desired == tunnel.target && (desired.is_none() || alive) {
        return;
    }
    let previous = tunnel.target.clone();
    let Some((target, port)) = desired else {
        if let Some(agent) = tunnel.agent.take() {
            agent.stop();
        }
        tunnel.target = None;
        drop(tunnel);
        update_playit(app, |p| {
            p.state = "off".into();
            p.target = None;
            p.message = None;
        });
        if let Some((Target::Server(id), _)) = previous {
            publish(app, &id).await;
        }
        return;
    };
    let target_id = match &target {
        Target::Server(id) => id.clone(),
        Target::World => "world".to_string(),
    };
    let Some(secret) = playit::load_secret(&state.paths) else {
        tunnel.target = None;
        drop(tunnel);
        update_playit(app, |p| {
            p.state = "off".into();
            p.target = None;
            p.message = Some(t!("Conecta playit.gg para tener una dirección pública.", "Connect playit.gg to get a public address."));
        });
        return;
    };
    update_playit(app, |p| {
        p.state = "starting".into();
        p.target = Some(target_id.clone());
        p.message = Some(t!("Abriendo tu dirección pública…", "Opening your public address…"));
    });
    if !alive {
        if let Some(agent) = tunnel.agent.take() {
            // Se cerró solo: se anota por qué (queda en datos/logs/playit.log).
            if let Some(line) = agent.last_line() {
                eprintln!("playit: {line}");
            }
            agent.stop();
        }
        match playit::Agent::start(&state.paths, &secret) {
            Ok(agent) => tunnel.agent = Some(agent),
            Err(error) => {
                tunnel.target = None;
                drop(tunnel);
                update_playit(app, |p| {
                    p.state = "error".into();
                    p.message = Some(error);
                });
                return;
            }
        }
    }
    let report = app.clone();
    let result = playit::ensure_tunnel(&secret, port, move |message| update_playit(&report, |p| p.message = Some(message))).await;
    match result {
        Ok((address, data)) => {
            tunnel.target = Some((target, port));
            drop(tunnel);
            {
                let mut saved = hosting.saved.lock().unwrap();
                saved.address = Some(address.clone());
                saved.account = Some(data.account.clone());
            }
            save_saved(app);
            update_playit(app, |p| {
                p.state = "on".into();
                p.address = Some(address);
                p.account = Some(data.account);
                p.message = data.notices.first().cloned();
            });
            if let Some((Target::Server(id), _)) = &previous {
                publish(app, id).await;
            }
            if target_id != "world" {
                publish(app, &target_id).await;
            }
        }
        Err(error) => {
            if error.invalid_key() {
                playit::clear_secret(&state.paths);
                if let Some(agent) = tunnel.agent.take() {
                    agent.stop();
                }
            }
            tunnel.target = None;
            drop(tunnel);
            let message = error.explain();
            let invalid = error.invalid_key();
            update_playit(app, |p| {
                p.state = "error".into();
                p.message = Some(message);
                if invalid {
                    p.connected = false;
                }
            });
        }
    }
}

// ================= Firebase: lo que ven los invitados

/// Publica el servidor para los amigos invitados (encendido, dirección y jugadores).
async fn publish(app: &AppHandle, id: &str) {
    let Some(social) = cuenta::social_handle(app) else { return };
    let paths = app.state::<AppState>().paths.clone();
    let Some(config) = servidor::find(&paths, id) else { return };
    let guests: Vec<String> = config.guests.iter().filter_map(|g| g.uid.clone()).collect();
    if guests.is_empty() {
        return;
    }
    let (online, players) = {
        let running = hosting(app).running.lock().unwrap();
        running.get(id).map(|r| (r.phase == Phase::Ready, r.players.len())).unwrap_or((false, 0))
    };
    let address = {
        let playit = hosting(app).playit.lock().unwrap();
        (playit.state == "on" && playit.target.as_deref() == Some(id)).then(|| playit.address.clone()).flatten()
    };
    let max = servidor::property(&config.folder, "max-players").and_then(|v| v.parse::<u32>().ok()).unwrap_or(20);
    let mut data = json!({
        "name": config.name,
        "pack": config.instance_name,
        "packFolder": config.instance,
        "mc": config.minecraft,
        "loader": config.loader,
        "online": online,
        "players": players,
        "max": max,
        "guests": guests.iter().map(|g| (g.clone(), json!(true))).collect::<serde_json::Map<_, _>>(),
    });
    for (field, value) in [("address", address), ("modpack", config.modpack.clone()), ("project", config.modpack_project.clone())] {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            data[field] = json!(value.chars().take(200).collect::<String>());
        }
    }
    let _ = social.publish_server(&config.key, data).await;
}

/// Los invitados: lista blanca, recarga en caliente y aviso a los amigos de Parabellum.
pub async fn set_guests(app: &AppHandle, id: &str, guests: Vec<Guest>) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let mut config = servidor::find(&state.paths, id).ok_or(missing())?;
    let mut clean: Vec<Guest> = Vec::new();
    for guest in guests {
        let name = guest.name.trim().to_string();
        if !servidor::valid_name(&name) {
            return Err(t!(
                "«{name}» no es un nombre de Minecraft válido (de 1 a 16 letras, números o _).",
                "«{name}» is not a valid Minecraft name (1 to 16 letters, numbers or _)."
            ));
        }
        if !clean.iter().any(|g| g.name.eq_ignore_ascii_case(&name)) {
            clean.push(Guest { name, uid: guest.uid.filter(|u| !u.is_empty()) });
        }
    }
    let before: Vec<String> = config.guests.iter().filter_map(|g| g.uid.clone()).collect();
    config.guests = clean;
    servidor::save(&config)?;
    let host = state.settings.lock().unwrap().username.clone();
    servidor::write_access_lists(&config, &host)?;
    send_command(app, id, "whitelist reload");
    let after: Vec<String> = config.guests.iter().filter_map(|g| g.uid.clone()).collect();
    let mut warning = None;
    if let Some(social) = cuenta::social_handle(app) {
        // Primero el servidor (con su lista de invitados) y después la invitación: si no, el
        // invitado intentaría leer un servidor que todavía no lo deja pasar.
        publish(app, id).await;
        for uid in after.iter().filter(|u| !before.contains(u)) {
            if let Err(error) = social.set_invite(&config.key, uid, true).await {
                warning = Some(t!(
                    "La lista blanca quedó guardada, pero no se pudo avisar a tus amigos por Parabellum ({error}). Revisa que las reglas de Firebase estén publicadas.",
                    "The whitelist was saved, but your friends could not be notified through Parabellum ({error}). Check that the Firebase rules are published."
                ));
            }
        }
        for uid in before.iter().filter(|u| !after.contains(u)) {
            let _ = social.set_invite(&config.key, uid, false).await;
        }
    }
    emit(app);
    Ok(warning)
}

// ================= Servidores

fn missing() -> String {
    t!("Ese servidor ya no existe.", "That server no longer exists.")
}

pub fn console(app: &AppHandle, id: &str) -> Vec<String> {
    hosting(app).running.lock().unwrap().get(id).map(|r| r.console.iter().cloned().collect()).unwrap_or_else(|| {
        // Apagado: lo último que quedó en el registro.
        let paths = app.state::<AppState>().paths.clone();
        servidor::find(&paths, id)
            .and_then(|c| std::fs::read(c.as_instance().state_dir().join("salida-java.log")).ok())
            .map(|bytes| String::from_utf8_lossy(&bytes).lines().rev().take(CONSOLE_LINES).collect::<Vec<_>>().into_iter().rev().map(String::from).collect())
            .unwrap_or_default()
    })
}

pub fn start_server(app: &AppHandle, id: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let host_state = &state.hosting;
    if host_state.running.lock().unwrap().contains_key(id) {
        return Ok(());
    }
    let mut config = servidor::find(&state.paths, id).ok_or(missing())?;
    // e4mc y compañía abren su propia dirección (y e4mc 6.2 tumba los servidores de 26.2): fuera.
    let tunnel_mods = servidor::remove_tunnel_mods(&mut config);
    if !config.eula {
        return Err(t!("Para encender el servidor hay que aceptar el EULA de Minecraft.", "To start the server you must accept the Minecraft EULA."));
    }
    // El anfitrión o los amigos invitados pueden haber cambiado de nombre.
    let host = state.settings.lock().unwrap().username.clone();
    let friends = cuenta::social_handle(app).map(|s| s.friend_names());
    let name_notes = follow_names(app, &mut config, &host, friends.as_ref());
    // server.properties manda (se puede editar a mano); si el puerto está ocupado, se cambia.
    if let Some(port) = servidor::property(&config.folder, "server-port").and_then(|p| p.parse::<u16>().ok()) {
        config.port = port;
    }
    let port = servidor::free_port(config.port);
    if port != config.port || servidor::property(&config.folder, "server-port").is_none() {
        config.port = port;
        servidor::set_properties(&config.folder, &[("server-port", port.to_string())])?;
    }
    servidor::write_eula(&config.folder, true)?;
    servidor::write_access_lists(&config, &host)?;
    config.last_started = Some(chrono::Utc::now().timestamp_millis());
    servidor::save(&config)?;

    let mut child = servidor::command(&config, &state.paths)?.spawn().map_err(|e| t!("No se pudo abrir Java: {e}", "Could not open Java: {e}"))?;
    proceso::tie_to_launcher(&child);
    let state_dir = config.as_instance().state_dir();
    let _ = std::fs::create_dir_all(&state_dir);
    let output = std::fs::File::create(state_dir.join("salida-java.log")).ok().map(std::io::BufWriter::new);
    host_state.outcomes.lock().unwrap().remove(id);
    let mut notes: Vec<String> = tunnel_mods
        .iter()
        .map(|file| {
            t!(
                "[Parabellum] Se quitó {file} del servidor: abre su propia dirección y aquí la dirección la da playit.gg.",
                "[Parabellum] {file} was removed from the server: it opens its own address and here the address comes from playit.gg."
            )
        })
        .collect();
    notes.extend(name_notes);
    host_state.running.lock().unwrap().insert(
        id.to_string(),
        Running {
            pid: child.id(),
            port: config.port,
            stdin: child.stdin.take(),
            phase: Phase::Starting,
            players: Vec::new(),
            started: config.last_started.unwrap_or_default(),
            console: notes.iter().cloned().collect(),
            pending: notes,
            output,
            cpu: proceso::CpuMeter::default(),
            cpu_percent: 0.0,
            memory: 0,
            stop_requested: false,
            restart_needed: false,
            restart: false,
        },
    );
    let streams: Vec<Box<dyn std::io::Read + Send>> = [child.stdout.take().map(|s| Box::new(s) as Box<dyn std::io::Read + Send>), child.stderr.take().map(|s| Box::new(s) as _)]
        .into_iter()
        .flatten()
        .collect();
    let mut readers = Vec::new();
    for stream in streams {
        let (app, id) = (app.clone(), id.to_string());
        readers.push(std::thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stream);
            let mut buffer = Vec::new();
            while reader.read_until(b'\n', &mut buffer).is_ok_and(|n| n > 0) {
                let line = playit::strip_ansi(String::from_utf8_lossy(&buffer).trim_end());
                buffer.clear();
                on_line(&app, &id, line);
            }
        }));
    }
    // La consola llega a la interfaz en tandas (los modpacks escriben miles de líneas al arrancar).
    {
        let (app, id) = (app.clone(), id.to_string());
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(150));
            let (lines, alive) = {
                let mut running = hosting(&app).running.lock().unwrap();
                match running.get_mut(&id) {
                    Some(run) => {
                        if let Some(output) = run.output.as_mut() {
                            let _ = output.flush();
                        }
                        (std::mem::take(&mut run.pending), true)
                    }
                    None => (Vec::new(), false),
                }
            };
            if !lines.is_empty() {
                let _ = app.emit("servidor-consola", json!({ "id": id, "lines": lines }));
            }
            if !alive {
                return;
            }
        });
    }
    {
        let (app, id) = (app.clone(), id.to_string());
        std::thread::spawn(move || {
            let code = child.wait().ok().and_then(|s| s.code());
            for reader in readers {
                let _ = reader.join();
            }
            on_exit(&app, &id, code);
        });
    }
    emit(app);
    let (app, id) = (app.clone(), id.to_string());
    tauri::async_runtime::spawn(async move { publish(&app, &id).await });
    Ok(())
}

fn on_line(app: &AppHandle, id: &str, line: String) {
    let event = servidor::parse_line(&line);
    let mut ready = false;
    let mut players_changed = false;
    let mut joined = None;
    let mut left = false;
    {
        let mut running = hosting(app).running.lock().unwrap();
        let Some(run) = running.get_mut(id) else { return };
        if let Some(output) = run.output.as_mut() {
            let _ = writeln!(output, "{line}");
        }
        match &event {
            Some(servidor::LineEvent::Ready) if run.phase == Phase::Starting => {
                run.phase = Phase::Ready;
                ready = true;
            }
            Some(servidor::LineEvent::Joined(name)) if !run.players.contains(name) => {
                run.players.push(name.clone());
                players_changed = true;
                joined = Some(name.clone());
            }
            Some(servidor::LineEvent::Left(name)) => {
                run.players.retain(|p| p != name);
                players_changed = true;
                left = true;
            }
            _ => {}
        }
        run.console.push_back(line.clone());
        while run.console.len() > CONSOLE_LINES {
            run.console.pop_front();
        }
        run.pending.push(line);
    }
    let message = match event {
        Some(servidor::LineEvent::BindFailed) => Some(t!(
            "El puerto del servidor está ocupado (¿otro servidor abierto?). Apágalo y vuelve a encenderlo: Parabellum buscará otro.",
            "The server port is busy (another server open?). Stop it and start it again: Parabellum will look for another one."
        )),
        Some(servidor::LineEvent::NeedsEula) => Some(t!("El servidor pide aceptar el EULA de Minecraft.", "The server asks to accept the Minecraft EULA.")),
        _ => None,
    };
    if let Some(message) = message {
        hosting(app).outcomes.lock().unwrap().insert(id.to_string(), Outcome { error: Some(message), diagnosis: None });
    }
    if let Some(name) = joined {
        let (app, id) = (app.clone(), id.to_string());
        std::thread::spawn(move || verify_player(&app, &id, &name));
    }
    if left {
        retry_moves(app, id);
    }
    if ready {
        hosting(app).auto_fixes.lock().unwrap().remove(id);
        let paths = app.state::<AppState>().paths.clone();
        if let Some(mut config) = servidor::find(&paths, id) {
            config.startup_seconds = config.last_started.map(|s| ((chrono::Utc::now().timestamp_millis() - s) / 1000) as u64);
            let _ = servidor::save(&config);
            // Reglas del juego elegidas cuando todavía no había mundo (o mientras arrancaba).
            for command in crate::opciones::pending_commands(&config) {
                send_command(app, id, &command);
            }
        }
    }
    if ready || players_changed {
        emit(app);
        let (app, id) = (app.clone(), id.to_string());
        tauri::async_runtime::spawn(async move {
            if ready {
                retarget(&app).await;
            }
            publish(&app, &id).await;
        });
    }
}

fn on_exit(app: &AppHandle, id: &str, code: Option<i32>) {
    let state = app.state::<AppState>();
    let Some(run) = state.hosting.running.lock().unwrap().remove(id) else { return };
    // Las reglas cambiadas con el servidor encendido quedan escritas en el mundo.
    if let Some(mut config) = servidor::find(&state.paths, id) {
        crate::opciones::settle_pending(&mut config);
    }
    let (app, id) = (app.clone(), id.to_string());
    tauri::async_runtime::spawn(async move {
        let paths = app.state::<AppState>().paths.clone();
        if run.restart {
            emit(&app);
            tokio::time::sleep(Duration::from_millis(500)).await;
            if let Err(error) = start_server(&app, &id) {
                hosting(&app).outcomes.lock().unwrap().insert(id.clone(), Outcome { error: Some(error), diagnosis: None });
                emit(&app);
            }
            return;
        }
        if !run.stop_requested {
            if let Some(config) = servidor::find(&paths, &id) {
                let context = diagnostico::CrashContext {
                    since_ms: run.started,
                    exit_code: code,
                    ram_gb: config.ram_gb,
                    system_ram_gb: (proceso::system_memory().0 / 1_073_741_824) as u32,
                };
                let instance = config.as_instance();
                let mut diagnosis = tauri::async_runtime::spawn_blocking(move || diagnostico::after_crash(&instance, &context)).await.unwrap_or_default();
                for finding in diagnosis.findings.iter_mut() {
                    finding.fixes.retain(|o| !matches!(o.fix, Fix::SetLoader { .. } | Fix::Repair));
                }
                // No llegó a estar listo: se quita lo que lo tumba y se vuelve a intentar solo.
                if run.phase == Phase::Starting {
                    let tries = *hosting(&app).auto_fixes.lock().unwrap().get(&id).unwrap_or(&0);
                    let fixes: Vec<(String, Fix)> = diagnostico::recommended(&diagnosis, &[])
                        .into_iter()
                        .filter(|(_, fix)| matches!(fix, Fix::DisableMod { .. } | Fix::AddJvmArg { .. } | Fix::SetRam { .. } | Fix::UseJava { .. } | Fix::ResetConfig { .. }))
                        .collect();
                    if tries < AUTO_FIXES && !fixes.is_empty() {
                        hosting(&app).auto_fixes.lock().unwrap().insert(id.clone(), tries + 1);
                        let mut notes = Vec::new();
                        for (label, fix) in fixes {
                            match apply_server_fix(&app, &id, fix).await {
                                Ok(done) => notes.push(format!("[Parabellum] {done}")),
                                Err(error) => notes.push(format!("[Parabellum] {label}: {error}")),
                            }
                        }
                        notes.push(t!("[Parabellum] Vuelvo a encender el servidor…", "[Parabellum] Starting the server again…"));
                        let _ = app.emit("servidor-consola", json!({ "id": id, "lines": notes }));
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        if let Err(error) = start_server(&app, &id) {
                            hosting(&app).outcomes.lock().unwrap().insert(id.clone(), Outcome { error: Some(error), diagnosis: None });
                            emit(&app);
                        }
                        return;
                    }
                }
                let error = match (run.phase, code) {
                    (Phase::Starting, _) => t!("El servidor no pudo arrancar.", "The server could not start."),
                    (_, Some(code)) => t!("El servidor se cerró solo (código {code}).", "The server closed by itself (code {code})."),
                    _ => t!("El servidor se cerró solo.", "The server closed by itself."),
                };
                let mut outcomes = hosting(&app).outcomes.lock().unwrap();
                let previous = outcomes.remove(&id).and_then(|o| o.error);
                outcomes.insert(id.clone(), Outcome { error: Some(previous.unwrap_or(error)), diagnosis: Some(diagnosis) });
            }
        }
        hosting(&app).auto_fixes.lock().unwrap().remove(&id);
        emit(&app);
        retarget(&app).await;
        publish(&app, &id).await;
    });
}

/// Apaga y vuelve a encender (para aplicar lo que el servidor solo lee al arrancar).
pub fn restart_server(app: &AppHandle, id: &str) {
    if let Some(run) = hosting(app).running.lock().unwrap().get_mut(id) {
        run.restart = true;
    }
    stop_server(app, id);
}

/// Anota que hay cambios que se aplican al reiniciar.
pub fn needs_restart(app: &AppHandle, id: &str) {
    if let Some(run) = hosting(app).running.lock().unwrap().get_mut(id) {
        run.restart_needed = true;
    }
    emit(app);
}

/// Apaga con "stop" (guarda el mundo). Si en 60 s no se cerró, se cierra a la fuerza.
pub fn stop_server(app: &AppHandle, id: &str) {
    let pid = {
        let mut running = hosting(app).running.lock().unwrap();
        let Some(run) = running.get_mut(id) else { return };
        run.stop_requested = true;
        run.phase = Phase::Stopping;
        if let Some(stdin) = run.stdin.as_mut() {
            let _ = stdin.write_all(b"stop\n");
            let _ = stdin.flush();
        }
        run.pid
    };
    emit(app);
    let (app, id) = (app.clone(), id.to_string());
    std::thread::spawn(move || {
        for _ in 0..300 {
            std::thread::sleep(Duration::from_millis(200));
            if !hosting(&app).running.lock().unwrap().get(&id).is_some_and(|r| r.pid == pid) {
                return;
            }
        }
        crate::launch::kill(pid);
    });
}

pub fn send_command(app: &AppHandle, id: &str, text: &str) {
    let command = text.trim().trim_start_matches('/');
    if command.is_empty() {
        return;
    }
    let mut running = hosting(app).running.lock().unwrap();
    let Some(run) = running.get_mut(id) else { return };
    if let Some(stdin) = run.stdin.as_mut() {
        let _ = stdin.write_all(format!("{command}\n").as_bytes());
        let _ = stdin.flush();
    }
    let echo = format!("> {command}");
    run.console.push_back(echo.clone());
    run.pending.push(echo);
}

/// Una nota de Parabellum en la consola del servidor (no va al servidor).
fn console_note(app: &AppHandle, id: &str, text: String) {
    let mut running = hosting(app).running.lock().unwrap();
    let Some(run) = running.get_mut(id) else { return };
    let line = format!("[Parabellum] {text}");
    run.console.push_back(line.clone());
    run.pending.push(line);
}

// ================= Los nombres siguen a la cuenta de cada jugador

/// Pone en el servidor los nombres de ahora y pasa las cosas de quien cambió de nombre (si no está
/// conectado; si lo está, al salir). Devuelve las notas para la consola.
fn follow_names(app: &AppHandle, config: &mut ServerConfig, host: &str, friends: Option<&HashMap<String, String>>) -> Vec<String> {
    let renames = servidor::follow_names(config, host, friends);
    let mut notes = Vec::new();
    for rename in &renames {
        notes.push(if rename.host {
            t!(
                "[Parabellum] Ahora te llamas {}: sigues siendo el administrador y entras con tu nombre nuevo.",
                "[Parabellum] You are now called {}: you are still the admin and you join with your new name.",
                rename.new
            )
        } else {
            t!("[Parabellum] {} ahora se llama {}: sigue invitado.", "[Parabellum] {} is now called {}: they are still invited.", rename.old, rename.new)
        });
        config.pending_moves.retain(|(_, new)| *new != rename.old);
        config.pending_moves.push((rename.old.clone(), rename.new.clone()));
    }
    notes.extend(move_pending(app, config));
    notes
}

/// Pasa las cosas pendientes de los cambios de nombre cuyo jugador no está conectado.
fn move_pending(app: &AppHandle, config: &mut ServerConfig) -> Vec<String> {
    let online: Vec<String> = hosting(app).running.lock().unwrap().get(&config.id).map(|r| r.players.clone()).unwrap_or_default();
    let is_online = |name: &str| online.iter().any(|p| p.eq_ignore_ascii_case(name));
    let mut notes = Vec::new();
    config.pending_moves.retain(|(old, new)| {
        if is_online(old) || is_online(new) {
            return true;
        }
        if servidor::move_player_data(&config.folder, old, new) > 0 {
            notes.push(t!(
                "[Parabellum] El inventario, la posición y los logros de {old} pasaron a {new}.",
                "[Parabellum] The inventory, position and advancements of {old} moved to {new}."
            ));
        }
        false
    });
    notes
}

/// Revisa los nombres de todos los servidores propios (al cambiar el nombre del anfitrión o el de un
/// amigo). Con el servidor encendido, recarga la lista blanca y pasa el permiso de administrador.
pub fn sync_names(app: &AppHandle) {
    let state = app.state::<AppState>();
    let host = state.settings.lock().unwrap().username.clone();
    let friends = cuenta::social_handle(app).map(|s| s.friend_names());
    let seen = {
        let mut sorted: Vec<(&String, &String)> = friends.iter().flatten().collect();
        sorted.sort();
        format!("{host}|{sorted:?}")
    };
    {
        let mut last = hosting(app).names_seen.lock().unwrap();
        if *last == seen {
            return;
        }
        *last = seen;
    }
    let mut changed = false;
    for mut config in servidor::list(&state.paths) {
        let before = serde_json::to_string(&config).unwrap_or_default();
        let old_host = config.host_name.clone();
        let notes = follow_names(app, &mut config, &host, friends.as_ref());
        if serde_json::to_string(&config).unwrap_or_default() == before {
            continue;
        }
        changed = true;
        let _ = servidor::save(&config);
        if servidor::write_access_lists(&config, &host).is_err() || !is_running(app, &config.id) {
            continue;
        }
        send_command(app, &config.id, "whitelist reload");
        if let Some(old) = old_host.filter(|old| *old != host && servidor::valid_name(old)) {
            send_command(app, &config.id, &format!("deop {old}"));
            send_command(app, &config.id, &format!("op {host}"));
        }
        for note in notes {
            console_note(app, &config.id, note.trim_start_matches("[Parabellum] ").to_string());
        }
    }
    if changed {
        emit(app);
    }
}

/// Al salir un jugador se pasan las cosas que tenía pendientes por un cambio de nombre.
fn retry_moves(app: &AppHandle, id: &str) {
    let paths = app.state::<AppState>().paths.clone();
    let Some(mut config) = servidor::find(&paths, id).filter(|c| !c.pending_moves.is_empty()) else { return };
    let notes = move_pending(app, &mut config);
    let _ = servidor::save(&config);
    for note in notes {
        console_note(app, id, note.trim_start_matches("[Parabellum] ").to_string());
    }
}

/// Quien entra con el nombre del anfitrión o de un amigo invitado tiene que estar jugando desde
/// Parabellum con esa cuenta (su Parabellum lo publica en la presencia). Si no, se le saca: así no
/// basta con ponerse el nombre de otro, en Parabellum o en cualquier otro launcher.
fn verify_player(app: &AppHandle, id: &str, name: &str) {
    let state = app.state::<AppState>();
    let Some(config) = servidor::find(&state.paths, id).filter(|c| c.verify_players) else { return };
    let host = state.settings.lock().unwrap().username.clone();
    let check: Box<dyn Fn() -> bool> = if name.eq_ignore_ascii_case(&host) {
        // El anfitrión: con un juego abierto desde este Parabellum.
        let app = app.clone();
        Box::new(move || !app.state::<AppState>().games.lock().unwrap().is_empty())
    } else {
        // Invitados por nombre (sin cuenta de Parabellum): no hay con qué comprobarlo.
        let Some(uid) = config.guests.iter().find(|g| g.name.eq_ignore_ascii_case(name)).and_then(|g| g.uid.clone()) else { return };
        let Some(social) = cuenta::social_handle(app) else {
            console_note(
                app,
                id,
                t!(
                    "No se pudo comprobar que {name} sea tu amigo de Parabellum: no hay conexión con el servidor de amigos.",
                    "Could not check that {name} is your Parabellum friend: there is no connection to the friends server."
                ),
            );
            return;
        };
        let name = name.to_string();
        Box::new(move || social.playing_as(&uid, &name))
    };
    // La presencia puede tardar unos segundos en llegar.
    for _ in 0..10 {
        if check() {
            return;
        }
        std::thread::sleep(Duration::from_secs(1));
        if !hosting(app).running.lock().unwrap().get(id).is_some_and(|r| r.players.iter().any(|p| p == name)) {
            return;
        }
    }
    let reason = t!("Entra desde Parabellum con tu cuenta.", "Join from Parabellum with your account.");
    send_command(app, id, &format!("kick {name} {reason}"));
    console_note(
        app,
        id,
        t!(
            "Se sacó a {name}: no está jugando desde Parabellum con la cuenta de {name}. Si es tu amigo, que entre desde Parabellum (o quita la comprobación en Ajustes).",
            "{name} was kicked: they are not playing from Parabellum with {name}'s account. If it is your friend, have them join from Parabellum (or turn off the check in Settings)."
        ),
    );
}

pub fn is_running(app: &AppHandle, id: &str) -> bool {
    hosting(app).running.lock().unwrap().contains_key(id)
}

/// Encendido y listo para recibir comandos.
pub fn is_ready(app: &AppHandle, id: &str) -> bool {
    hosting(app).running.lock().unwrap().get(id).is_some_and(|r| r.phase == Phase::Ready)
}

/// Vuelve a mandar a Firebase lo que ven los amigos (p. ej. tras cambiar el nombre).
pub async fn republish(app: &AppHandle, id: &str) {
    publish(app, id).await;
}

pub async fn delete_server(app: &AppHandle, id: &str) -> Result<(), String> {
    if is_running(app, id) {
        return Err(t!("Apaga el servidor antes de borrarlo.", "Stop the server before deleting it."));
    }
    let paths = app.state::<AppState>().paths.clone();
    let config = servidor::find(&paths, id).ok_or(missing())?;
    if let Some(social) = cuenta::social_handle(app) {
        let guests: Vec<String> = config.guests.iter().filter_map(|g| g.uid.clone()).collect();
        if !guests.is_empty() {
            let _ = social.remove_server(&config.key, &guests).await;
        }
    }
    servidor::delete(&config)?;
    hosting(app).outcomes.lock().unwrap().remove(id);
    emit(app);
    Ok(())
}

/// Vuelve a copiar los mods y la configuración de la instancia (después de actualizarla).
pub async fn sync_server(app: &AppHandle, id: &str) -> Result<usize, String> {
    if is_running(app, id) {
        return Err(t!("Apaga el servidor antes de actualizar sus mods.", "Stop the server before updating its mods."));
    }
    let paths = app.state::<AppState>().paths.clone();
    let mut config = servidor::find(&paths, id).ok_or(missing())?;
    let instance = crate::instances::find(&paths, &config.instance).ok_or(t!("La instancia de este servidor ya no existe.", "This server's instance no longer exists."))?;
    // Lo que quitó el doctor se mantiene fuera; lo de solo cliente se vuelve a calcular.
    let mut left: Vec<LeftOut> = config.left_out.iter().filter(|l| l.kind == "doctor").cloned().collect();
    for item in servidor::client_only(&instance).await {
        if !left.iter().any(|l| l.file.eq_ignore_ascii_case(&item.file)) {
            left.push(item);
        }
    }
    let (folder, copy_left) = (config.folder.clone(), left.clone());
    let copied = tauri::async_runtime::spawn_blocking(move || servidor::copy_content(&instance, &folder, &copy_left)).await.map_err(|e| e.to_string())??;
    config.left_out = left;
    servidor::save(&config)?;
    emit(app);
    Ok(copied)
}

/// Aplica un arreglo del doctor al servidor (no a la instancia).
pub async fn apply_server_fix(app: &AppHandle, id: &str, fix: Fix) -> Result<String, String> {
    let paths = app.state::<AppState>().paths.clone();
    let mut config = servidor::find(&paths, id).ok_or(missing())?;
    let mods = config.folder.join("mods");
    let applied = fix.clone();
    let done = match fix {
        Fix::DisableMod { file } => {
            if file.contains(['/', '\\']) {
                return Err(missing());
            }
            let from = mods.join(&file);
            if from.exists() {
                std::fs::rename(&from, mods.join(format!("{file}.disabled"))).map_err(|e| e.to_string())?;
            }
            if !config.left_out.iter().any(|l| l.file.eq_ignore_ascii_case(&file)) {
                config.left_out.push(LeftOut::new(&file, "doctor", None));
            }
            t!("Se quitó {file} del servidor.", "{file} was removed from the server.")
        }
        Fix::EnableMod { file } => {
            let base = file.strip_suffix(".disabled").unwrap_or(&file).to_string();
            if base.contains(['/', '\\']) {
                return Err(missing());
            }
            let disabled = mods.join(format!("{base}.disabled"));
            if disabled.exists() {
                std::fs::rename(&disabled, mods.join(&base)).map_err(|e| e.to_string())?;
            } else if let Some(instance) = crate::instances::find(&paths, &config.instance) {
                let source = instance.game_dir.join("mods").join(&base);
                let source = if source.exists() { source } else { instance.game_dir.join("mods").join(format!("{base}.disabled")) };
                std::fs::copy(&source, mods.join(&base)).map_err(|_| t!("{base} no está en la instancia.", "{base} is not in the instance."))?;
            }
            config.left_out.retain(|l| !l.file.eq_ignore_ascii_case(&base));
            t!("Se puso {base} en el servidor.", "{base} was put on the server.")
        }
        Fix::AddJvmArg { arg } => {
            if !config.jvm_args.contains(&arg) {
                config.jvm_args.push(arg.clone());
            }
            t!("Se agregó {arg} a los argumentos de Java del servidor.", "{arg} was added to the server's Java arguments.")
        }
        Fix::SetRam { gb } => {
            config.ram_gb = gb;
            t!("El servidor ahora tiene {gb} GB.", "The server now has {gb} GB.")
        }
        Fix::UseJava { major } => {
            mojang::ensure_java(major, &paths, |_, _| {}).await?;
            config.java_major = major;
            t!("El servidor ahora usa Java {major}.", "The server now uses Java {major}.")
        }
        Fix::ResetConfig { path } => {
            if path.contains("..") {
                return Err(missing());
            }
            let target = config.folder.join(&path);
            if target.exists() {
                trash::delete(&target).map_err(|e| e.to_string())?;
            }
            t!("{path} se mandó a la papelera (se crea de nuevo al arrancar).", "{path} was moved to the Recycle Bin (it is created again on startup).")
        }
        Fix::InstallMod { .. } | Fix::SetLoader { .. } | Fix::Repair | Fix::InstallProject { .. } => {
            return Err(t!("Esto se arregla en la instancia.", "This is fixed in the instance."));
        }
    };
    servidor::save(&config)?;
    {
        let mut outcomes = hosting(app).outcomes.lock().unwrap();
        if let Some(outcome) = outcomes.get_mut(id) {
            if let Some(diagnosis) = outcome.diagnosis.as_mut() {
                diagnosis.findings.retain(|f| !f.fixes.iter().any(|o| o.fix == applied));
            }
        }
    }
    emit(app);
    Ok(done)
}

/// Para el panel de crear servidor: mundos, mods y memoria recomendada de la instancia.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    worlds: Vec<String>,
    mods: usize,
    recommended_ram: u32,
    game_ram: u32,
    total_mb: u64,
    free_mb: u64,
}

pub fn prepare(app: &AppHandle, instance_id: &str) -> Result<Preparation, String> {
    let state = app.state::<AppState>();
    let instance = crate::instances::find(&state.paths, instance_id).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?;
    let mods = std::fs::read_dir(instance.game_dir.join("mods")).into_iter().flatten().flatten().filter(|e| e.file_name().to_string_lossy().to_lowercase().ends_with(".jar")).count();
    let (total, free) = proceso::system_memory();
    let default_ram = state.settings.lock().unwrap().ram_gb;
    Ok(Preparation {
        worlds: servidor::worlds(&instance),
        mods,
        recommended_ram: servidor::recommended_ram(mods),
        game_ram: instance.ram_gb.unwrap_or(default_ram),
        total_mb: total / 1_048_576,
        free_mb: free / 1_048_576,
    })
}

pub fn emit_view(app: &AppHandle) {
    emit(app);
}
