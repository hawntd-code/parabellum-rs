//! La cuenta de Google y los amigos en la ventana: une google.rs, amigos.rs y la actividad del juego
//! con la interfaz (comandos y el evento "social"). También la bandeja del sistema: con la sesión
//! iniciada, la X deja Parabellum junto al reloj para que los amigos te sigan viendo.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

use crate::actividad::{self, Tracker};
use crate::amigos::{ClaimError, Presence, Social, View};
use crate::chat::{self, Chat};
use crate::google::{self, GoogleAccount, Profile};
use crate::settings::{self, Account, Settings};
use crate::{instances, sesion_local, AppState};

#[derive(Default)]
pub struct SocialState {
    google: Mutex<Option<Arc<GoogleAccount>>>,
    social: Mutex<Option<Arc<Social>>>,
    chat: Mutex<Option<Arc<Chat>>>,
    status: Mutex<Status>,
    /// Puerto del servidor de sesión local (None si no se pudo abrir).
    pub session_port: Mutex<Option<u16>>,
    profile_generation: AtomicU64,
    tracker: Mutex<(Option<String>, Tracker)>,
    tray_hint_shown: AtomicBool,
    seen: Mutex<Seen>,
}

/// Lo que ya se conocía de los amigos, para avisar solo de lo nuevo.
#[derive(Default)]
struct Seen {
    /// Al conectar llega todo lo que ya había: hasta este momento solo se anota, sin avisar.
    quiet_until: Option<std::time::Instant>,
    requests: std::collections::HashSet<String>,
    open_worlds: std::collections::HashSet<String>,
}

#[derive(Default, Clone)]
struct Status {
    signing_in: bool,
    connecting: bool,
    error: Option<String>,
    warning: Option<String>,
    name_conflict: Option<String>,
    notice: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SocialView {
    /// Esta copia de Parabellum tiene las claves de Google y Firebase.
    available: bool,
    signed_in: bool,
    email: String,
    display_name: String,
    signing_in: bool,
    connecting: bool,
    connected: bool,
    error: Option<String>,
    warning: Option<String>,
    name_conflict: Option<String>,
    notice: Option<String>,
    me: Option<Presence>,
    friends: Option<View>,
}

fn available(state: &AppState) -> bool {
    let c = &state.config;
    !c.google_client_id.is_empty() && !c.google_client_secret.is_empty() && !c.firebase_api_key.is_empty() && !c.firebase_database_url.is_empty()
}

fn google_account(app: &AppHandle) -> Option<Arc<GoogleAccount>> {
    app.state::<AppState>().social.google.lock().unwrap().clone()
}

fn social(app: &AppHandle) -> Option<Arc<Social>> {
    app.state::<AppState>().social.social.lock().unwrap().clone()
}

/// La conexión con los amigos (None sin sesión de Google), para «Hostear partida».
pub fn social_handle(app: &AppHandle) -> Option<Arc<Social>> {
    social(app)
}

fn update_status(app: &AppHandle, change: impl FnOnce(&mut Status)) {
    change(&mut app.state::<AppState>().social.status.lock().unwrap());
    emit(app);
}

pub fn view(app: &AppHandle) -> SocialView {
    let state = app.state::<AppState>();
    let status = state.social.status.lock().unwrap().clone();
    let account = google_account(app);
    let social = social(app);
    SocialView {
        available: available(&state),
        signed_in: account.is_some(),
        email: account.as_ref().map(|a| a.email.clone()).unwrap_or_default(),
        display_name: account.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
        signing_in: status.signing_in,
        connecting: status.connecting,
        connected: social.is_some(),
        error: status.error,
        warning: status.warning,
        name_conflict: status.name_conflict,
        notice: status.notice,
        me: social.as_ref().map(|s| s.my_presence()),
        friends: social.as_ref().map(|s| s.view()),
    }
}

fn emit(app: &AppHandle) {
    let _ = app.emit("social", view(app));
}

#[tauri::command]
pub fn estado_social(app: AppHandle) -> SocialView {
    view(&app)
}

/// Abre el navegador para elegir la cuenta de Google; al volver conecta con los amigos.
#[tauri::command]
pub async fn iniciar_sesion_google(app: AppHandle, recordar: bool) -> Result<SocialView, String> {
    let config = {
        let state = app.state::<AppState>();
        if !available(&state) {
            return Err(t!("Esta copia de Parabellum no trae las claves de Google. Descarga la versión más reciente.", "This copy of Parabellum does not include the Google keys. Download the latest version."));
        }
        state.config.clone()
    };
    update_status(&app, |s| {
        s.signing_in = true;
        s.error = None;
        s.notice = None;
    });
    let result = GoogleAccount::sign_in(&config).await;
    update_status(&app, |s| s.signing_in = false);
    let account = Arc::new(result.inspect_err(|e| update_status(&app, |s| s.error = Some(e.clone())))?);
    let paths = app.state::<AppState>().paths.clone();
    if recordar {
        let _ = google::save_session(&paths, &account.refresh_token);
    } else {
        google::clear_session(&paths);
    }
    *app.state::<AppState>().social.google.lock().unwrap() = Some(account.clone());
    show_window(&app);
    apply_profile(&app, &account, true).await;
    connect(&app).await;
    Ok(view(&app))
}

#[tauri::command]
pub async fn cerrar_sesion_google(app: AppHandle) -> SocialView {
    disconnect(&app).await;
    *app.state::<AppState>().social.google.lock().unwrap() = None;
    google::clear_session(&app.state::<AppState>().paths);
    update_status(&app, |s| *s = Status::default());
    view(&app)
}

/// Vuelve a intentar conectar con los amigos (la sesión de Google sigue iniciada).
#[tauri::command]
pub async fn reconectar_amigos(app: AppHandle) -> SocialView {
    connect(&app).await;
    view(&app)
}

fn connected(app: &AppHandle) -> Result<Arc<Social>, String> {
    social(app).ok_or(t!("Primero inicia sesión con Google.", "Sign in with Google first."))
}

#[tauri::command]
pub async fn agregar_amigo(app: AppHandle, nombre: String) -> Result<String, String> {
    let name = nombre.trim();
    if let Some(problem) = settings::validate_username(name) {
        return Err(problem.into());
    }
    connected(&app)?.send_request(name).await
}

#[tauri::command]
pub async fn aceptar_solicitud(app: AppHandle, uid: String) -> Result<(), String> {
    connected(&app)?.accept(&uid).await
}

#[tauri::command]
pub async fn rechazar_solicitud(app: AppHandle, uid: String) -> Result<(), String> {
    connected(&app)?.decline(&uid).await
}

#[tauri::command]
pub async fn eliminar_amigo(app: AppHandle, uid: String) -> Result<(), String> {
    connected(&app)?.remove_friend(&uid).await
}

// ================= Conexión

async fn connect(app: &AppHandle) {
    let Some(account) = google_account(app) else { return };
    if social(app).is_some() {
        return;
    }
    update_status(app, |s| {
        s.connecting = true;
        s.error = None;
    });
    let config = app.state::<AppState>().config.clone();
    let result = async {
        let token = account.id_token().await?;
        let (changed, failed) = (app.clone(), app.clone());
        Social::connect(
            &config.firebase_api_key,
            &config.firebase_database_url,
            &token,
            move || {
                let state = changed.state::<AppState>();
                state.social.status.lock().unwrap().warning = None;
                emit(&changed);
                notify_changes(&changed);
            },
            move |error| update_status(&failed, |s| s.warning = Some(error)),
        )
        .await
    }
    .await;
    match result {
        Ok(social) => {
            // Si se cerró la sesión mientras tanto, no se conecta.
            if google_account(app).is_none_or(|a| !Arc::ptr_eq(&a, &account)) {
                update_status(app, |s| s.connecting = false);
                return;
            }
            *app.state::<AppState>().social.seen.lock().unwrap() =
                Seen { quiet_until: Some(std::time::Instant::now() + Duration::from_secs(10)), ..Seen::default() };
            *app.state::<AppState>().social.social.lock().unwrap() = Some(social.clone());
            claim_name(app, &social).await;
            social.set_presence(current_presence(app).map(|(p, _)| p).unwrap_or_else(Presence::online));
            social.start();
            start_chat(app, &social);
            update_status(app, |s| s.connecting = false);
        }
        Err(error) => update_status(app, |s| {
            s.connecting = false;
            s.error = Some(error);
        }),
    }
}

async fn disconnect(app: &AppHandle) {
    if let Some(chat) = app.state::<AppState>().social.chat.lock().unwrap().take() {
        chat.stop();
    }
    let _ = app.emit("chats", Vec::<chat::Summary>::new());
    let social = app.state::<AppState>().social.social.lock().unwrap().take();
    if let Some(social) = social {
        social.go_offline().await;
    }
}

/// Reserva el nombre de jugador en uso para esta cuenta, para que nadie más pueda usarlo.
async fn claim_name(app: &AppHandle, social: &Arc<Social>) {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    if settings::validate_username(&settings.username).is_some() {
        return;
    }
    let uuid = settings::dashed(&settings.effective_uuid());
    match social.claim_name(&settings.username, &uuid).await {
        Ok(()) => update_status(app, |s| s.name_conflict = None),
        Err(ClaimError::Taken) => update_status(app, |s| s.name_conflict = Some(settings.username.clone())),
        Err(ClaimError::Other(error)) => update_status(app, |s| s.warning = Some(t!("No se pudo reservar tu nombre: {error}", "Could not reserve your name: {error}"))),
    }
}

// ================= Perfil guardado en la cuenta

fn current_profile(settings: &Settings) -> Profile {
    Profile { username: settings.username.clone(), uuid: settings.uuid.clone(), ram_gb: settings.ram_gb, accounts: settings.accounts.clone() }
}

/// Al iniciar sesión: si la cuenta ya tiene perfil, se usa (la cuenta de este PC queda en la lista
/// para cambiar con un clic); si no, se guarda el de este PC.
async fn apply_profile(app: &AppHandle, account: &GoogleAccount, announce: bool) {
    let notice = match account.load_profile().await {
        Ok(Some(remote)) if settings::validate_username(&remote.username).is_none() => {
            let state = app.state::<AppState>();
            let mut current = state.settings.lock().unwrap().clone();
            let same = current.username == remote.username && current.uuid == settings::parse_uuid(&remote.uuid).map(|u| settings::dashed(&u)).unwrap_or_default();
            if same {
                None
            } else {
                let previous = Account { username: current.username.clone(), uuid: current.uuid.clone() };
                let mut accounts = remote.accounts.clone();
                if previous.username != Settings::default().username {
                    accounts.push(previous);
                }
                accounts.append(&mut current.accounts);
                current.username = remote.username.clone();
                current.uuid = remote.uuid.clone();
                current.accounts = accounts;
                if remote.ram_gb > 0 {
                    current.ram_gb = remote.ram_gb.clamp(1, 64);
                }
                if current.normalize().is_err() {
                    current.uuid.clear();
                    let _ = current.normalize();
                }
                let _ = current.save(&state.paths);
                *state.settings.lock().unwrap() = current.clone();
                let _ = app.emit("ajustes", current);
                announce.then(|| t!("Se cargó tu perfil guardado: ahora juegas como {}.", "Your saved profile was loaded: you now play as {}.", remote.username))
            }
        }
        Ok(_) => {
            let profile = current_profile(&app.state::<AppState>().settings.lock().unwrap());
            match account.save_profile(&profile).await {
                Ok(()) => announce.then(|| t!("Tu perfil de este PC quedó guardado en tu cuenta.", "Your profile on this PC was saved to your account.")),
                Err(error) => Some(t!("Sesión iniciada, pero no se pudo guardar tu perfil: {error}", "Signed in, but your profile could not be saved: {error}")),
            }
        }
        Err(error) => Some(t!("Sesión iniciada, pero no se pudo leer tu perfil: {error}", "Signed in, but your profile could not be read: {error}")),
    };
    update_status(app, |s| s.notice = notice);
}

/// Al abrir Parabellum con la sesión ya guardada manda lo de este PC: el nombre que el jugador
/// eligió aquí no lo cambia el perfil de la cuenta (que podía venir de una versión anterior).
async fn upload_profile(app: &AppHandle, account: &GoogleAccount) {
    let profile = current_profile(&app.state::<AppState>().settings.lock().unwrap());
    if let Err(error) = account.save_profile(&profile).await {
        update_status(app, |s| s.notice = Some(t!("No se pudo guardar tu perfil en la cuenta: {error}", "Could not save your profile to your account: {error}")));
    }
}

/// Tras cambiar el nombre o la cuenta en uso: se reserva el nombre y se guarda el perfil en la
/// cuenta (2 s después del último cambio, para no hacerlo en cada tecla).
pub fn settings_changed(app: &AppHandle) {
    if google_account(app).is_none() {
        return;
    }
    let generation = app.state::<AppState>().social.profile_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if app.state::<AppState>().social.profile_generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let settings = app.state::<AppState>().settings.lock().unwrap().clone();
        if let Some(social) = social(&app) {
            if social.name().as_deref() != Some(settings.username.as_str()) {
                claim_name(&app, &social).await;
            }
        }
        if let Some(chat) = chat_service(&app) {
            chat.set_my_name(&settings.username);
        }
        if let Some(account) = google_account(&app) {
            if let Err(error) = account.save_profile(&current_profile(&settings)).await {
                update_status(&app, |s| s.notice = Some(t!("No se pudo guardar tu perfil en la cuenta: {error}", "Could not save your profile to your account: {error}")));
            }
        }
    });
}

// ================= Chat

fn chat_service(app: &AppHandle) -> Option<Arc<Chat>> {
    app.state::<AppState>().social.chat.lock().unwrap().clone()
}

fn chat_ready(app: &AppHandle) -> Result<Arc<Chat>, String> {
    chat_service(app).ok_or(t!("Primero inicia sesión con Google.", "Sign in with Google first."))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChatMessages {
    chat: String,
    messages: Vec<chat::Message>,
}

fn start_chat(app: &AppHandle, social: &Arc<Social>) {
    let (list, arrived, messages) = (app.clone(), app.clone(), app.clone());
    let name = app.state::<AppState>().settings.lock().unwrap().username.clone();
    let images = app.state::<AppState>().paths.data.join("cache").join("chat");
    let service = Chat::new(
        social.database(),
        name,
        images,
        move || {
            if let Some(chat) = chat_service(&list) {
                let _ = list.emit("chats", chat.list());
            }
        },
        move |summary| notify_message(&arrived, summary),
        move |chat, list| {
            let _ = messages.emit("chat-mensajes", ChatMessages { chat, messages: list });
        },
    );
    service.start();
    *app.state::<AppState>().social.chat.lock().unwrap() = Some(service);
}

/// Aviso de Windows de un mensaje nuevo, salvo que esa conversación esté a la vista.
fn notify_message(app: &AppHandle, summary: chat::Summary) {
    if !app.state::<AppState>().settings.lock().unwrap().notifications {
        return;
    }
    let in_front = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    let open = chat_service(app).and_then(|c| c.open_chat());
    if in_front && open.as_deref() == Some(summary.id.as_str()) {
        return;
    }
    let title = if summary.kind == "grupo" { summary.name.clone() } else { t!("Mensaje de {}", "Message from {}", summary.name) };
    let body = if summary.kind == "grupo" { format!("{}: {}", summary.last_name, summary.last_text) } else { summary.last_text.clone() };
    let (handle, id) = (app.clone(), summary.id.clone());
    let _ = app.run_on_main_thread(move || {
        let click = handle.clone();
        let _ = crate::avisos::show(&title, &body, move || {
            show_window(&click);
            let _ = click.emit("abrir-chat", id.clone());
        });
    });
}

/// Una imagen del PC (soltada en la ventana) como data URL, para reducirla y enviarla.
#[tauri::command]
pub fn leer_imagen(ruta: String) -> Result<String, String> {
    use base64::Engine;
    let path = std::path::PathBuf::from(&ruta);
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        _ => return Err(t!("Eso no es una imagen (PNG, JPG, GIF, WEBP o BMP).", "That is not an image (PNG, JPG, GIF, WEBP or BMP).")),
    };
    if path.metadata().map(|m| m.len()).unwrap_or(0) > 25 * 1024 * 1024 {
        return Err(t!("La imagen pesa más de 25 MB.", "The image is larger than 25 MB."));
    }
    let bytes = std::fs::read(&path).map_err(|e| t!("No se pudo leer la imagen: {e}", "Could not read the image: {e}"))?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

#[tauri::command]
pub fn chats(app: AppHandle) -> Vec<chat::Summary> {
    chat_service(&app).map(|c| c.list()).unwrap_or_default()
}

#[tauri::command]
pub fn abrir_chat(app: AppHandle, chat: String) -> Result<(), String> {
    chat_ready(&app)?.open(&chat)
}

#[tauri::command]
pub fn cerrar_chat(app: AppHandle) {
    if let Some(chat) = chat_service(&app) {
        chat.close();
    }
}

#[tauri::command]
pub async fn marcar_leido(app: AppHandle, chat: String) -> Result<(), String> {
    chat_ready(&app)?.mark_read(&chat).await
}

#[tauri::command]
pub async fn enviar_mensaje(app: AppHandle, chat: String, texto: String) -> Result<(), String> {
    chat_ready(&app)?.send_text(&chat, &texto).await
}

#[tauri::command]
pub async fn enviar_imagen(app: AppHandle, chat: String, datos: String, ancho: u32, alto: u32) -> Result<(), String> {
    chat_ready(&app)?.send_image(&chat, &datos, ancho, alto).await
}

#[tauri::command]
pub async fn imagen_chat(app: AppHandle, chat: String, mensaje: String) -> Result<String, String> {
    chat_ready(&app)?.image(&chat, &mensaje).await
}

#[tauri::command]
pub async fn chat_con_amigo(app: AppHandle, uid: String) -> Result<String, String> {
    chat_ready(&app)?.direct(&uid).await
}

#[tauri::command]
pub async fn crear_grupo(app: AppHandle, nombre: String, amigos: Vec<String>) -> Result<String, String> {
    chat_ready(&app)?.create_group(&nombre, &amigos).await
}

#[tauri::command]
pub async fn agregar_al_grupo(app: AppHandle, chat: String, amigos: Vec<String>) -> Result<(), String> {
    chat_ready(&app)?.add_members(&chat, &amigos).await
}

#[tauri::command]
pub async fn renombrar_grupo(app: AppHandle, chat: String, nombre: String) -> Result<(), String> {
    chat_ready(&app)?.rename_group(&chat, &nombre).await
}

#[tauri::command]
pub async fn salir_del_chat(app: AppHandle, chat: String) -> Result<(), String> {
    chat_ready(&app)?.leave(&chat).await
}

// ================= Avisos de Windows

/// Avisa de solicitudes nuevas y de amigos que abren su mundo, si Parabellum no está al frente.
fn notify_changes(app: &AppHandle) {
    let Some(social) = social(app) else { return };
    let view = social.view();
    let requests: std::collections::HashSet<String> = view.requests.iter().map(|r| r.uid.clone()).collect();
    let open_worlds: std::collections::HashSet<String> = view.friends.iter().filter(|f| f.world_open).map(|f| f.uid.clone()).collect();
    let state = app.state::<AppState>();
    let mut toasts = Vec::new();
    {
        let mut seen = state.social.seen.lock().unwrap();
        let quiet = seen.quiet_until.is_none_or(|until| std::time::Instant::now() < until);
        if !quiet {
            for request in view.requests.iter().filter(|r| !seen.requests.contains(&r.uid)) {
                toasts.push((t!("Solicitud de amistad", "Friend request"), t!("{} quiere ser tu amigo en Parabellum.", "{} wants to be your friend on Parabellum.", request.name)));
            }
            for friend in view.friends.iter().filter(|f| f.world_open && !seen.open_worlds.contains(&f.uid)) {
                let pack = friend.presence.as_ref().and_then(|p| p.pack.clone()).unwrap_or_default();
                toasts.push((t!("{} abrió su mundo", "{} opened their world", friend.name), t!("Está jugando {pack}. Abre Parabellum y pulsa Unirse.", "They are playing {pack}. Open Parabellum and press Join.")));
            }
        }
        seen.requests = requests;
        seen.open_worlds = open_worlds;
    }
    let enabled = state.settings.lock().unwrap().notifications;
    let in_front = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if !enabled || in_front {
        return;
    }
    for (title, body) in toasts {
        let handle = app.clone();
        // Los avisos usan WinRT: se muestran desde el hilo de la ventana, que ya lo tiene iniciado.
        let _ = app.run_on_main_thread(move || {
            let click = handle.clone();
            let _ = crate::avisos::show(&title, &body, move || show_window(&click));
        });
    }
}

// ================= Presencia: a qué juegas y si tu mundo está abierto

/// La presencia y, si el jugador abrió su mundo en LAN, (puerto, instancia) para compartirlo por playit.gg.
fn current_presence(app: &AppHandle) -> Option<(Presence, Option<(u16, String)>)> {
    let state = app.state::<AppState>();
    let running = state.games.lock().unwrap().keys().next().cloned();
    let mut tracker = state.social.tracker.lock().unwrap();
    let Some(id) = running else {
        *tracker = (None, Tracker::default());
        return Some((Presence::online(), None));
    };
    let instance = instances::find(&state.paths, &id)?;
    let started = std::fs::read_to_string(instance.state_dir().join("ultima-partida.txt")).ok().and_then(|t| t.trim().parse::<i64>().ok()).unwrap_or(0);
    if tracker.0.as_deref() != Some(id.as_str()) {
        *tracker = (Some(id.clone()), Tracker::default());
    }
    tracker.1.poll(Some(&instance.game_dir), started);
    let activity = &tracker.1.activity;
    let lan = activity.lan_port.and_then(|port| actividad::local_address().map(|ip| format!("{ip}:{port}")));
    let shared = activity.lan_port.map(|port| (port, instance.id.clone()));
    let presence = Presence {
        state: "playing".into(),
        pack: Some(instance.name.clone()),
        pack_folder: Some(instance.id.clone()),
        minecraft: Some(instance.minecraft_version.clone()),
        world: activity.world.clone(),
        server: activity.server.clone(),
        // La dirección de playit.gg (fija) va antes que la de e4mc.
        address: activity.lan_port.and_then(|_| crate::hosting::world_address(app)).or_else(|| activity.address.clone()),
        lan,
    };
    Some((presence, shared))
}

// ================= Arranque, bandeja y salida

/// Servidor de sesión local, sesión de Google guardada y la presencia (cada 2 s).
pub fn startup(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let log = app.state::<AppState>().paths.logs.join("sesion-local.log");
        if let Ok(port) = sesion_local::start(log).await {
            *app.state::<AppState>().social.session_port.lock().unwrap() = Some(port);
        }
        let (paths, config, ok) = {
            let state = app.state::<AppState>();
            (state.paths.clone(), state.config.clone(), available(&state))
        };
        if ok {
            if let Some(refresh) = google::load_session(&paths) {
                update_status(&app, |s| s.connecting = true);
                match GoogleAccount::resume(&config, refresh).await {
                    Ok(account) => {
                        let account = Arc::new(account);
                        *app.state::<AppState>().social.google.lock().unwrap() = Some(account.clone());
                        upload_profile(&app, &account).await;
                        connect(&app).await;
                    }
                    Err(error) => update_status(&app, |s| {
                        s.connecting = false;
                        s.error = Some(t!("No se pudo recuperar tu sesión de Google ({error}). Puedes volver a iniciarla.", "Your Google session could not be restored ({error}). You can sign in again."));
                    }),
                }
            }
        }
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            // Se mira el juego aunque no haya sesión: el mundo abierto en LAN se comparte por playit.gg.
            let Some((presence, shared)) = current_presence(&app) else { continue };
            crate::hosting::world_tick(&app, shared);
            let Some(social) = social(&app) else { continue };
            if presence != social.my_presence() {
                social.set_presence(presence);
                emit(&app);
            }
        }
    });
}

/// Con la sesión iniciada (y la opción activada), la X deja Parabellum junto al reloj.
pub fn should_hide_on_close(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let close_to_tray = state.settings.lock().unwrap().close_to_tray;
    // Con un servidor encendido, cerrar la ventana no lo apaga: Parabellum sigue junto al reloj.
    (close_to_tray && google_account(app).is_some()) || crate::hosting::busy(app)
}

pub fn hid_to_tray(app: &AppHandle) {
    if !app.state::<AppState>().social.tray_hint_shown.swap(true, Ordering::SeqCst) {
        let _ = app.emit("bandeja", ());
    }
}

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Al cambiar de idioma: el icono junto al reloj se rehace con los textos nuevos.
pub fn rebuild_tray(app: &AppHandle) {
    if app.remove_tray_by_id("principal").is_some() {
        let _ = create_tray(app);
    }
}

/// Icono junto al reloj: clic para abrir, menú para salir del todo.
pub fn create_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let open = MenuItemBuilder::with_id("abrir", t!("Abrir Parabellum", "Open Parabellum")).build(app)?;
    let quit = MenuItemBuilder::with_id("salir", t!("Salir", "Quit")).build(app)?;
    let menu = MenuBuilder::new(app).item(&open).separator().item(&quit).build()?;
    let mut builder = TrayIconBuilder::with_id("principal").tooltip("Parabellum").menu(&menu).show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "abrir" => show_window(app),
            "salir" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Al salir de verdad: los amigos te ven desconectado al momento.
pub fn shutdown(app: &AppHandle) {
    if let Some(social) = app.state::<AppState>().social.social.lock().unwrap().take() {
        tauri::async_runtime::block_on(async move { social.go_offline().await });
    }
}

/// Argumentos de Java para usar el servidor de sesión local.
pub fn session_arguments(app: &AppHandle) -> Vec<String> {
    app.state::<AppState>().social.session_port.lock().unwrap().map(sesion_local::jvm_arguments).unwrap_or_default()
}
