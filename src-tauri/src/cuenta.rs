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
    /// La conversación en la que estás escribiendo y cuándo fue la última tecla (va en tu presencia).
    typing: Mutex<Option<(String, std::time::Instant)>>,
    google: Mutex<Option<Arc<GoogleAccount>>>,
    social: Mutex<Option<Arc<Social>>>,
    chat: Mutex<Option<Arc<Chat>>>,
    /// Escucha de los favoritos de la cuenta (favoritos.rs).
    favorites: Mutex<Option<crate::firebase::Listener>>,
    /// Los avisos de soporte de tus comunidades (soporte::Watcher).
    support: Mutex<Option<Arc<crate::soporte::Watcher>>>,
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
    /// Ya se intentó recuperar la sesión guardada al abrir (salió bien o mal).
    resumed: bool,
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
    /// Al abrir: se está recuperando la sesión de Google guardada (la pantalla de carga espera).
    restoring: bool,
    /// Ya llegaron los amigos y las conversaciones.
    synced: bool,
    error: Option<String>,
    warning: Option<String>,
    name_conflict: Option<String>,
    notice: Option<String>,
    me: Option<Presence>,
    friends: Option<View>,
    server_now: i64,
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

/// El vigía de soporte (None sin sesión de Google).
pub fn support_watcher(app: &AppHandle) -> Option<Arc<crate::soporte::Watcher>> {
    app.state::<AppState>().social.support.lock().unwrap().clone()
}

/// El correo de la cuenta de Google con la que se inició sesión (si hay).
pub fn google_email(app: &AppHandle) -> Option<String> {
    google_account(app).map(|a| a.email.clone()).filter(|e| !e.is_empty())
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
    let chat = state.social.chat.lock().unwrap().clone();
    let restoring = available(&state) && !status.resumed && status.error.is_none() && crate::google::has_session(&state.paths);
    SocialView {
        available: available(&state),
        signed_in: account.is_some(),
        email: account.as_ref().map(|a| a.email.clone()).unwrap_or_default(),
        display_name: account.as_ref().map(|a| a.name.clone()).unwrap_or_default(),
        signing_in: status.signing_in,
        connecting: status.connecting,
        connected: social.is_some(),
        restoring,
        synced: social.as_ref().is_some_and(|s| s.synced()) && chat.is_some_and(|c| c.synced()),
        error: status.error,
        warning: status.warning,
        name_conflict: status.name_conflict,
        notice: status.notice,
        me: social.as_ref().map(|s| s.my_presence()),
        friends: social.as_ref().map(|s| s.view()),
        server_now: social.as_ref().map(|s| s.database().server_now()).unwrap_or_else(|| chrono::Utc::now().timestamp_millis()),
    }
}

fn emit(app: &AppHandle) {
    let _ = app.emit("social", view(app));
    crate::puente::changed(app);
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
            *app.state::<AppState>().social.favorites.lock().unwrap() = Some(crate::favoritos::start(app, social.database()));
            *app.state::<AppState>().social.support.lock().unwrap() = Some(crate::soporte::Watcher::start(app, social.database()));
            crate::soporte::settle_pending(app,None);
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
    app.state::<AppState>().social.favorites.lock().unwrap().take();
    app.state::<AppState>().social.support.lock().unwrap().take();
    let _ = app.emit("soporte-avisos", crate::soporte::Notices::default());
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
    // Los servidores propios siguen al anfitrión aunque no haya sesión de Google.
    crate::hosting::sync_names(app);
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

/// El chat, para el puente con el juego.
pub fn chat_handle(app: &AppHandle) -> Option<Arc<Chat>> {
    chat_service(app)
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
    let (list, arrived, messages, typing) = (app.clone(), app.clone(), app.clone(), app.clone());
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
            crate::puente::changed(&list);
        },
        move |summary| notify_message(&arrived, summary),
        move |chat, list| {
            let _ = messages.emit("chat-mensajes", ChatMessages { chat, messages: list });
        },
        move |chat, users| { let _ = typing.emit("chat-escribiendo", serde_json::json!({"chat":chat,"users":users})); },
    );
    service.start();
    *app.state::<AppState>().social.chat.lock().unwrap() = Some(service);
}

/// Sonido y aviso de Windows de un mensaje nuevo, salvo que esa conversación esté a la vista.
fn notify_message(app: &AppHandle, summary: chat::Summary) {
    let state = app.state::<AppState>();
    let (sound, toast) = {
        let settings = state.settings.lock().unwrap();
        (settings.message_sound, settings.notifications)
    };
    let in_front = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    let open = chat_service(app).and_then(|c| c.open_chat());
    if in_front && open.as_deref() == Some(summary.id.as_str()) {
        return;
    }
    if sound {
        crate::sonido::message(false);
    }
    let title = if summary.kind == "grupo" { summary.name.clone() } else { t!("Mensaje de {}", "Message from {}", summary.name) };
    let body = if summary.kind == "grupo" { format!("{}: {}", summary.last_name, summary.last_text) } else { summary.last_text.clone() };
    // Dentro del juego (si tiene el mod): ahí se avisa en vez de con Windows.
    let kind = if summary.last_text.starts_with("🎮") { "invitacion" } else { "mensaje" };
    crate::puente::notice(app, kind, &title, &body, serde_json::json!({ "chat": summary.id }));
    if !toast || crate::puente::game_connected(app) {
        return;
    }
    let (handle, id) = (app.clone(), summary.id.clone());
    let _ = app.run_on_main_thread(move || {
        let click = handle.clone();
        let _ = crate::avisos::show(&title, &body, sound, move || {
            show_window(&click);
            let _ = click.emit("abrir-chat", id.clone());
        });
    });
}

/// Comparte una instancia en una conversación (su receta: ver compartir.rs).
#[tauri::command]
pub async fn compartir_instancia(app: AppHandle, chat: String, id: String) -> Result<(), String> {
    let service = chat_ready(&app)?;
    let paths = app.state::<AppState>().paths.clone();
    let instance = crate::instances::find(&paths, &id).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?;
    let recipe = crate::compartir::recipe(&instance).await?;
    let text = crate::compartir::encode(&recipe)?;
    service.send_share(&chat, &text, &recipe.name).await
}

/// Una imagen del PC (soltada en la ventana) como data URL, para reducirla y enviarla.
#[tauri::command]
pub fn leer_imagen(ruta: String) -> Result<String, String> {
    use base64::Engine;
    let path = std::path::PathBuf::from(&ruta);
    let extension = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mime = match extension.as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" | "jfif" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "avif" => Some("image/avif"),
        // Los que la ventana no abre los convierte Windows (abajo).
        "heic" | "heif" | "tif" | "tiff" | "ico" | "jxr" | "wdp" | "dds" => None,
        _ => return Err(t!("Eso no es una imagen.", "That is not an image.")),
    };
    if path.metadata().map(|m| m.len()).unwrap_or(0) > 25 * 1024 * 1024 {
        return Err(t!("La imagen pesa más de 25 MB.", "The image is larger than 25 MB."));
    }
    // Un enlace no, y tiene que ser imagen por dentro, no solo por el nombre (como en Mac, PR #15).
    if path.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(t!("Eso no es una imagen.", "That is not an image."));
    }
    let bytes = std::fs::read(&path).map_err(|e| t!("No se pudo leer la imagen: {e}", "Could not read the image: {e}"))?;
    if !crate::imagen::is_image(&bytes) {
        return Err(t!("Eso no es una imagen.", "That is not an image."));
    }
    match mime {
        Some(mime) => Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))),
        None => Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(crate::imagen::to_png(&bytes)?))),
    }
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

/// `respuesta`: el mensaje al que responde.
#[tauri::command]
pub async fn enviar_mensaje(app: AppHandle, chat: String, texto: String, respuesta: Option<String>, envio: Option<String>) -> Result<String, String> {
    chat_ready(&app)?.send_text_tracked(&chat, &texto, respuesta.as_deref(), envio.as_deref()).await
}

#[tauri::command]
pub async fn perfil_jugador(app: AppHandle, uid: String) -> Result<serde_json::Value, String> { chat_ready(&app)?.profile(&uid).await }

#[tauri::command]
pub async fn imagenes_chat(app: AppHandle, chat: String, antes: Option<String>) -> Result<serde_json::Value, String> { chat_ready(&app)?.media_page(&chat, antes.as_deref()).await }

/// Pone tu reacción a un mensaje, o la quita (`emoji` vacío).
#[tauri::command]
pub async fn reaccionar(app: AppHandle, chat: String, mensaje: String, emoji: Option<String>) -> Result<(), String> {
    chat_ready(&app)?.react(&chat, &mensaje, emoji.as_deref()).await
}

#[tauri::command]
pub async fn enviar_imagen(app: AppHandle, chat: String, datos: String, ancho: u32, alto: u32) -> Result<(), String> {
    chat_ready(&app)?.send_image(&chat, &datos, ancho, alto).await
}

#[tauri::command]
pub async fn editar_mensaje(app: AppHandle, chat: String, mensaje: String, texto: String) -> Result<(), String> {
    chat_ready(&app)?.edit(&chat, &mensaje, &texto).await
}

#[tauri::command]
pub async fn borrar_mensaje(app: AppHandle, chat: String, mensaje: String) -> Result<(), String> {
    chat_ready(&app)?.delete(&chat, &mensaje).await
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
pub async fn crear_grupo(app: AppHandle, nombre: String, amigos: Vec<String>, foto: Option<String>) -> Result<String, String> {
    chat_ready(&app)?.create_group(&nombre, &amigos, foto.as_deref()).await
}

/// Nombre y foto de un grupo (solo el dueño). `foto`: la nueva; `quitar_foto`: sin foto.
#[tauri::command]
pub async fn editar_grupo(app: AppHandle, chat: String, nombre: Option<String>, foto: Option<String>, quitar_foto: bool) -> Result<(), String> {
    let photo = if quitar_foto { Some(None) } else { foto.as_deref().map(Some) };
    chat_ready(&app)?.edit_group(&chat, nombre.as_deref(), photo).await
}

#[tauri::command]
pub async fn sacar_del_grupo(app: AppHandle, chat: String, uid: String) -> Result<(), String> {
    chat_ready(&app)?.remove_member(&chat, &uid).await
}

/// Invita a un amigo a tu servidor encendido o a tu mundo abierto (lo mismo que «Invitar» en el juego).
#[tauri::command]
pub async fn invitar_amigo(app: AppHandle, uid: String) -> Result<String, String> {
    crate::puente::invite(&app, &uid).await
}

#[tauri::command]
pub async fn pasar_grupo(app: AppHandle, chat: String, uid: String) -> Result<(), String> {
    chat_ready(&app)?.transfer_group(&chat, &uid).await
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
    // Un amigo invitado a un servidor propio pudo cambiar de nombre.
    crate::hosting::sync_names(app);
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
                crate::puente::notice(app, "solicitud", &t!("Solicitud de amistad", "Friend request"), &t!("{} quiere ser tu amigo.", "{} wants to be your friend.", request.name), serde_json::json!({ "uid": request.uid }));
            }
            for friend in view.friends.iter().filter(|f| f.world_open && !seen.open_worlds.contains(&f.uid)) {
                let pack = friend.presence.as_ref().and_then(|p| p.pack.clone()).unwrap_or_default();
                toasts.push((t!("{} abrió su mundo", "{} opened their world", friend.name), t!("Está jugando {pack}. Abre Parabellum y pulsa Unirse.", "They are playing {pack}. Open Parabellum and press Join.")));
                crate::puente::notice(app, "mundo", &t!("{} abrió su mundo", "{} opened their world", friend.name), &pack, serde_json::json!({ "uid": friend.uid }));
            }
        }
        seen.requests = requests;
        seen.open_worlds = open_worlds;
    }
    let enabled = state.settings.lock().unwrap().notifications;
    let in_front = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if !enabled || in_front || crate::puente::game_connected(app) {
        return;
    }
    for (title, body) in toasts {
        let handle = app.clone();
        // Los avisos usan WinRT: se muestran desde el hilo de la ventana, que ya lo tiene iniciado.
        let _ = app.run_on_main_thread(move || {
            let click = handle.clone();
            let _ = crate::avisos::show(&title, &body, false, move || show_window(&click));
        });
    }
}

// ================= Presencia: a qué juegas y si tu mundo está abierto

/// La presencia y, si el jugador abrió su mundo en LAN, (puerto, instancia) para compartirlo por playit.gg.
fn current_presence(app: &AppHandle) -> Option<(Presence, Option<(u16, String)>)> {
    let state = app.state::<AppState>();
    let running = state.games.lock().unwrap().keys().next().cloned();
    let mut tracker = state.social.tracker.lock().unwrap();
    let hosting = crate::hosting::hosted_name(app);
    // «Está escribiendo» dura 6 s desde la última tecla.
    let typing = state.social.typing.lock().unwrap().as_ref().filter(|(_, at)| at.elapsed() < Duration::from_secs(6)).map(|(chat, _)| chat.clone());
    let Some(id) = running else {
        *tracker = (None, Tracker::default());
        return Some((Presence { hosting, typing, p2p: p2p_address(app), ..Presence::online() }, None));
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
        hosting,
        player: Some(state.settings.lock().unwrap().username.clone()),
        p2p: p2p_address(app),
        typing,
    };
    Some((presence, shared))
}

/// La ventana avisa que estás escribiendo en esa conversación (None: la vaciaste o mandaste el mensaje).
/// Va en tu presencia: tus amigos ven «está escribiendo…».
#[tauri::command]
pub async fn escribiendo(app: AppHandle, chat: Option<String>) {
    let chat = chat.filter(|c| !c.is_empty() && c.len() <= 120 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || "-_".contains(ch)));
    *app.state::<AppState>().social.typing.lock().unwrap() = chat.clone().map(|c| (c, std::time::Instant::now()));
    if let Some(service) = chat_service(&app) { service.set_typing(chat.as_deref()).await; }
    // Se publica ya (no a los 2 s de la vuelta de la presencia), solo si cambió.
    if let (Some((presence, _)), Some(social)) = (current_presence(&app), social(&app)) {
        if presence != social.my_presence() {
            social.set_presence(presence);
            emit(&app);
        }
    }
}

/// La dirección P2P de este Parabellum (se publica siempre: también sirve para que tus servidores
/// sepan qué amigo se conecta).
pub fn p2p_address(app: &AppHandle) -> Option<String> {
    app.state::<AppState>().p2p.lock().unwrap().as_ref().and_then(|p2p| p2p.address())
}

// ================= Arranque, bandeja y salida

/// Servidor de sesión local, sesión de Google guardada y la presencia (cada 2 s).
pub fn startup(app: &AppHandle) {
    let app = app.clone();
    // La sesión de Google guardada, a la vez que lo demás: la pantalla de carga espera a los amigos y los
    // mensajes, no a la conexión directa.
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
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
            update_status(&app, |s| s.resumed = true);
        });
    }
    // Firebase, medido: cada minuto una línea en datos/logs/firebase.csv, y cuánto tardó en sincronizar.
    {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let started = std::time::Instant::now();
            let path = app.state::<AppState>().paths.logs.join("firebase.csv");
            let transport_path = app.state::<AppState>().paths.logs.join("firebase-transporte.csv");
            let _ = std::fs::create_dir_all(path.parent().unwrap());
            let write = |line: String| {
                use std::io::Write as _;
                let new = !path.exists();
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
                    if new {
                        let _ = writeln!(file, "hora,segundos,amigos,conversaciones,escuchas,abiertas,conexiones,eventos,bytes_escuchas,peticiones,bytes_peticiones,nota");
                    }
                    let _ = writeln!(file, "{line}");
                }
                let new = !transport_path.exists();
                if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&transport_path) {
                    if new { let _ = writeln!(file, "hora,websockets,sse,entradas_respaldo"); }
                    let _ = writeln!(file, "{},{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), crate::firebase::metrics::transport());
                }
            };
            let row = |note: &str| {
                let (friends, chats) = {
                    let view = social(&app).map(|s| s.view().friends.len()).unwrap_or(0);
                    let chats = app.state::<AppState>().social.chat.lock().unwrap().as_ref().map(|c| c.list().len()).unwrap_or(0);
                    (view, chats)
                };
                format!(
                    "{},{},{friends},{chats},{},{note}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    started.elapsed().as_secs(),
                    crate::firebase::metrics::line()
                )
            };
            let mut synced = false;
            let mut last = std::time::Instant::now();
            loop {
                tokio::time::sleep(Duration::from_millis(250)).await;
                if !synced && view(&app).synced {
                    synced = true;
                    write(row(&format!("sincronizado en {} ms", started.elapsed().as_millis())));
                }
                if last.elapsed() >= Duration::from_secs(60) {
                    last = std::time::Instant::now();
                    write(row(""));
                }
            }
        });
    }
    tauri::async_runtime::spawn(async move {
        // Conexión directa con los amigos: entran a tus servidores y mundos abiertos (los que estén
        // encendidos) sin pasar por playit.gg.
        crate::p2p::log_to(app.state::<AppState>().paths.logs.join("p2p.log"));
        let gate_app = app.clone();
        match crate::p2p::P2p::start(Arc::new(move |remote: &str, target: &str| crate::hosting::p2p_port(&gate_app, remote, target))).await {
            Ok(p2p) => *app.state::<AppState>().p2p.lock().unwrap() = Some(p2p),
            Err(error) => eprintln!("p2p: no arrancó ({error})"),
        }
        // El puente con el mod dentro del juego (amigos y chat en Minecraft).
        if let Err(error) = crate::puente::start(&app).await {
            eprintln!("puente: no arrancó ({error})");
        }
        let log = app.state::<AppState>().paths.logs.join("sesion-local.log");
        if let Ok(port) = sesion_local::start(log).await {
            *app.state::<AppState>().social.session_port.lock().unwrap() = Some(port);
        }
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            // Alguien volvió a abrir Parabellum (este seguía junto al reloj): se muestra este, no otro.
            if std::fs::remove_file(app.state::<AppState>().paths.data.join("mostrar-ventana")).is_ok() {
                show_window(&app);
            }
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
    // Los amigos conectados por P2P se enteran al momento (y no cuando se agota la espera).
    if let Some(p2p) = app.state::<AppState>().p2p.lock().unwrap().take() {
        tauri::async_runtime::block_on(async move { p2p.close().await });
    }
}

/// Argumentos de Java para usar el servidor de sesión local.
pub fn session_arguments(app: &AppHandle) -> Vec<String> {
    let mut arguments = app.state::<AppState>().social.session_port.lock().unwrap().map(sesion_local::jvm_arguments).unwrap_or_default();
    // Y dónde está Parabellum para el mod del juego.
    arguments.extend(crate::puente::jvm_arguments(app));
    arguments
}
