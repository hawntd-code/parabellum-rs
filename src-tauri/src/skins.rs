//! Skins entre jugadores de Parabellum (reglas en firebase-reglas.json):
//!   skins/{uid} = { value, signature, from?, at }
//! `value` y `signature` son la propiedad «textures» tal como la firma Mojang: la imagen vive en
//! textures.minecraft.net y el juego la acepta sin mods (no deja skins de otros sitios). Sale de una cuenta
//! premium, por su nombre, o de un PNG propio que el puente de Cloud Run sube a Mojang por MineSkin
//! (puente-curseforge/main.py › skins; solo si el puente tiene su clave).
//! El servidor de sesión local se la da al juego: al anfitrión de un mundo cuando entra un jugador
//! (hasJoined, y de ahí a todos los que están dentro) y a cada jugador cuando pide un perfil (el suyo, o el
//! de un amigo en las versiones que lo piden así).

use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

use crate::settings;
use crate::AppState;

static APP: OnceLock<AppHandle> = OnceLock::new();
/// Nombre en minúsculas → (cuándo se leyó, su skin).
static CACHE: OnceLock<Mutex<HashMap<String, (Instant, Option<Skin>)>>> = OnceLock::new();
/// Cuánto vale lo leído de la base: poco, para que un cambio de skin se vea en la siguiente partida.
const FRESH: Duration = Duration::from_secs(120);
/// Lo más que espera el juego por una skin antes de seguir con la de siempre.
const LOOKUP: Duration = Duration::from_secs(4);

#[derive(Clone, Debug, PartialEq)]
pub struct Skin {
    pub value: String,
    pub signature: String,
    /// La cuenta premium de donde salió.
    pub from: Option<String>,
    pub at: i64,
}

/// Lo que ve la interfaz de una skin.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// La imagen (PNG de 64×64 o 64×32), como `data:` para dibujarla sin pedir nada más.
    pub image: String,
    pub slim: bool,
    pub from: Option<String>,
    pub at: i64,
}

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

fn cache() -> &'static Mutex<HashMap<String, (Instant, Option<Skin>)>> {
    CACHE.get_or_init(Default::default)
}

fn base64_ok(text: &str, max: usize) -> bool {
    !text.is_empty() && text.len() <= max && text.bytes().all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
}

/// La dirección de la imagen y si es de brazos delgados, si `value` es una skin de Mojang.
fn decode(value: &str) -> Option<(String, bool)> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(value).ok()?;
    let payload: Value = serde_json::from_slice(&bytes).ok()?;
    let skin = &payload["textures"]["SKIN"];
    let url = skin["url"].as_str()?;
    let hash = url.strip_prefix("http://textures.minecraft.net/texture/").or_else(|| url.strip_prefix("https://textures.minecraft.net/texture/"))?;
    if hash.is_empty() || hash.len() > 128 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((format!("https://textures.minecraft.net/texture/{hash}"), skin["metadata"]["model"] == "slim"))
}

/// Una skin guardada en la base, si está bien formada.
fn parse(data: &Value) -> Option<Skin> {
    let value = data["value"].as_str().filter(|v| base64_ok(v, 4096))?;
    let signature = data["signature"].as_str().filter(|s| base64_ok(s, 1400))?;
    decode(value)?;
    Some(Skin {
        value: value.into(),
        signature: signature.into(),
        from: data["from"].as_str().filter(|n| settings::validate_username(n).is_none()).map(String::from),
        at: data["at"].as_i64().unwrap_or(0),
    })
}

fn database(app: &AppHandle) -> Option<std::sync::Arc<crate::firebase::Database>> {
    crate::cuenta::social_handle(app).map(|s| s.database())
}

/// La skin de un jugador de Parabellum por su nombre (None si no tiene, o sin sesión).
pub async fn for_name(name: &str) -> Option<Skin> {
    if settings::validate_username(name).is_some() {
        return None;
    }
    let key = name.to_lowercase();
    if let Some((at, skin)) = cache().lock().unwrap().get(&key) {
        if at.elapsed() < FRESH {
            return skin.clone();
        }
    }
    let db = database(APP.get()?)?;
    let lookup = async {
        let uid = db.get(&format!("names/{key}")).await.map_err(|_| ())?;
        let Some(uid) = uid.as_str().filter(|u| !u.is_empty() && u.len() <= 128 && u.chars().all(|c| c.is_ascii_alphanumeric())) else {
            return Ok(None);
        };
        Ok::<_, ()>(parse(&db.get(&format!("skins/{uid}")).await.map_err(|_| ())?))
    };
    // Sin respuesta (sin internet, la base lenta) no se recuerda: se vuelve a intentar en la siguiente.
    let skin = tokio::time::timeout(LOOKUP, lookup).await.ok()?.ok()?;
    cache().lock().unwrap().insert(key, (Instant::now(), skin.clone()));
    skin
}

/// Las propiedades del perfil de ese jugador para el juego: su skin, si tiene.
pub async fn properties(name: &str) -> Vec<Value> {
    for_name(name)
        .await
        .map(|s| vec![json!({ "name": "textures", "value": s.value, "signature": s.signature })])
        .unwrap_or_default()
}

/// El nombre de un jugador conocido por su UUID: las cuentas de este PC y los amigos.
pub fn name_of(uuid: &str) -> Option<String> {
    let uuid = settings::parse_uuid(uuid)?;
    let app = APP.get()?;
    let state = app.state::<AppState>();
    let mut names: Vec<(String, String)> = {
        let s = state.settings.lock().unwrap();
        std::iter::once((s.username.clone(), s.uuid.clone())).chain(s.accounts.iter().map(|a| (a.username.clone(), a.uuid.clone()))).collect()
    };
    if let Some(social) = crate::cuenta::social_handle(app) {
        names.extend(social.friend_names().into_values().map(|n| (n, String::new())));
    }
    names
        .into_iter()
        .find(|(name, fixed)| settings::parse_uuid(fixed).unwrap_or_else(|| settings::offline_uuid(name)) == uuid)
        .map(|(name, _)| name)
}

fn own(app: &AppHandle) -> Result<(std::sync::Arc<crate::firebase::Database>, String), String> {
    let db = database(app).ok_or_else(|| t!("Inicia sesión con Google para guardar tu skin. ⟦accion:sesion⟧", "Sign in with Google to save your skin. ⟦accion:sesion⟧"))?;
    let uid = db.session.uid.clone();
    Ok((db, uid))
}

/// La skin de una cuenta premium, firmada por Mojang.
async fn premium(name: &str) -> Result<Skin, String> {
    if settings::validate_username(name).is_some() {
        return Err(t!("Escribe el nombre de una cuenta de Minecraft (letras, números y _).", "Type the name of a Minecraft account (letters, numbers and _)."));
    }
    let http = crate::download::client();
    let unreachable = |_| t!("No se pudo hablar con Mojang. Revisa tu internet.", "Could not reach Mojang. Check your internet.");
    let missing = || t!("No hay una cuenta premium con el nombre «{}».", "There is no premium account named \"{}\".", name);
    let found = http.get(format!("https://api.minecraftservices.com/minecraft/profile/lookup/name/{name}")).send().await.map_err(unreachable)?;
    if !found.status().is_success() {
        return Err(missing());
    }
    let profile: Value = found.json().await.map_err(|_| missing())?;
    let id = profile["id"].as_str().and_then(settings::parse_uuid).ok_or_else(missing)?;
    let session: Value = http
        .get(format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}?unsigned=false"))
        .send()
        .await
        .map_err(unreachable)?
        .json()
        .await
        .map_err(|_| missing())?;
    let textures = session["properties"].as_array().and_then(|list| list.iter().find(|p| p["name"] == "textures")).cloned().unwrap_or_default();
    let mut data = json!({ "value": textures["value"], "signature": textures["signature"], "from": profile["name"], "at": 0 });
    if data["from"].as_str().is_none_or(|n| settings::validate_username(n).is_some()) {
        data["from"] = json!(name);
    }
    parse(&data).ok_or_else(|| t!("«{}» usa la skin de siempre (Steve o Alex): no tiene una propia.", "\"{}\" uses the default skin (Steve or Alex): it has no skin of its own.", name))
}

/// La imagen de la skin, guardada en este PC la primera vez.
async fn view(skin: &Skin) -> Result<View, String> {
    let (url, slim) = decode(&skin.value).ok_or_else(|| t!("La skin no es válida.", "The skin is not valid."))?;
    let paths = &APP.get().ok_or("sin app")?.state::<AppState>().paths;
    let file = paths.data.join("skins").join(format!("{}.png", url.rsplit('/').next().unwrap_or_default()));
    let png = match std::fs::read(&file) {
        Ok(png) => png,
        Err(_) => {
            let bytes = crate::download::client()
                .get(&url)
                .send()
                .await
                .and_then(|r| r.error_for_status())
                .map_err(|_| t!("No se pudo bajar la imagen de la skin.", "Could not download the skin image."))?
                .bytes()
                .await
                .map_err(|e| e.to_string())?;
            if bytes.len() > 64 * 1024 || !bytes.starts_with(b"\x89PNG") {
                return Err(t!("La imagen de la skin no es válida.", "The skin image is not valid."));
            }
            let _ = std::fs::create_dir_all(file.parent().unwrap());
            let _ = std::fs::write(&file, &bytes);
            bytes.to_vec()
        }
    };
    Ok(View {
        image: format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png)),
        slim,
        from: skin.from.clone(),
        at: skin.at,
    })
}

/// Tu skin de Parabellum (None si no tienes).
#[tauri::command]
pub async fn skin_mia(app: AppHandle) -> Result<Option<View>, String> {
    let (db, uid) = own(&app)?;
    match db.get(&format!("skins/{uid}")).await.map_err(|e| e.to_string())? {
        Value::Null => Ok(None),
        data => match parse(&data) {
            Some(skin) => view(&skin).await.map(Some),
            None => Ok(None),
        },
    }
}

/// La skin de una cuenta premium, para verla antes de usarla.
#[tauri::command]
pub async fn skin_buscar(nombre: String) -> Result<View, String> {
    view(&premium(nombre.trim()).await?).await
}

/// Guarda esa skin como la del jugador.
async fn save(app: &AppHandle, mut skin: Skin) -> Result<View, String> {
    let (db, uid) = own(app)?;
    skin.at = db.server_now();
    let mut data = json!({ "value": skin.value, "signature": skin.signature, "at": crate::firebase::server_timestamp() });
    if let Some(from) = &skin.from {
        data["from"] = json!(from);
    }
    db.put(&format!("skins/{uid}"), &data).await.map_err(|e| e.to_string())?;
    forget_own(app);
    view(&skin).await
}

/// Usa la skin de esa cuenta premium: la ven los jugadores de Parabellum desde la siguiente partida.
#[tauri::command]
pub async fn skin_usar(app: AppHandle, nombre: String) -> Result<View, String> {
    own(&app)?;
    save(&app, premium(nombre.trim()).await?).await
}

/// Si el puente puede subir skins en PNG (tiene la clave de MineSkin).
#[tauri::command]
pub async fn skin_png_disponible() -> bool {
    static KNOWN: OnceLock<bool> = OnceLock::new();
    if let Some(known) = KNOWN.get() {
        return *known;
    }
    let answer = crate::download::client().get(format!("{}/skins/estado", crate::curseforge::base())).timeout(Duration::from_secs(8)).send().await;
    let Ok(response) = answer else { return false };
    let enabled = response.json::<Value>().await.ok().and_then(|v| v["enabled"].as_bool()).unwrap_or(false);
    // Lo que diga el puente vale para toda la sesión; sin respuesta se vuelve a preguntar.
    let _ = KNOWN.set(enabled);
    enabled
}

/// Un PNG de skin: 64×64 (o 64×32, el formato de antes) y de menos de 24 KB.
fn png_ok(bytes: &[u8]) -> bool {
    bytes.len() <= 24 * 1024
        && bytes.len() >= 24
        && bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        && &bytes[12..16] == b"IHDR"
        && u32::from_be_bytes(bytes[16..20].try_into().unwrap()) == 64
        && matches!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()), 64 | 32)
}

/// Sube una skin propia (PNG): el puente la sube a Mojang y devuelve la firmada. None si se canceló.
#[tauri::command]
pub async fn skin_subir(app: AppHandle, delgada: bool) -> Result<Option<View>, String> {
    use tauri_plugin_dialog::DialogExt;
    own(&app)?;
    let dialog = app.dialog().file().set_title(t!("Elegir tu skin (PNG de 64×64)", "Choose your skin (64×64 PNG)")).add_filter("PNG", &["png"]);
    let Some(picked) = tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_file()).await.map_err(|e| e.to_string())? else { return Ok(None) };
    let bytes = std::fs::read(picked.into_path().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if !png_ok(&bytes) {
        return Err(t!("La skin tiene que ser un PNG de 64×64 (o 64×32) de menos de 24 KB, como las de Minecraft.", "The skin has to be a 64×64 (or 64×32) PNG under 24 KB, like Minecraft's."));
    }
    let response = crate::download::client()
        .post(format!("{}/skins/generar?variant={}", crate::curseforge::base(), if delgada { "slim" } else { "classic" }))
        .header("Content-Type", "image/png")
        .timeout(Duration::from_secs(90))
        .body(bytes)
        .send()
        .await
        .map_err(|_| t!("No se pudo subir la skin. Revisa tu internet.", "Could not upload the skin. Check your internet."))?;
    let status = response.status();
    let answer: Value = response.json().await.unwrap_or_default();
    if !status.is_success() {
        return Err(answer["error"].as_str().map(String::from).unwrap_or_else(|| t!("No se pudo subir la skin.", "Could not upload the skin.")));
    }
    let skin = parse(&json!({ "value": answer["value"], "signature": answer["signature"], "at": 0 })).ok_or_else(|| t!("La skin que volvió no es válida.", "The skin that came back is not valid."))?;
    save(&app, skin).await.map(Some)
}

/// Vuelve a la skin de siempre.
#[tauri::command]
pub async fn skin_quitar(app: AppHandle) -> Result<(), String> {
    let (db, uid) = own(&app)?;
    db.delete(&format!("skins/{uid}")).await.map_err(|e| e.to_string())?;
    forget_own(&app);
    Ok(())
}

/// Las skins de esos jugadores de Parabellum que tienen una, por nombre en minúsculas: para ver sus caras
/// en la lista de amigos.
#[tauri::command]
pub async fn skins_de(nombres: Vec<String>) -> HashMap<String, View> {
    let lookups = nombres.into_iter().take(64).map(|name| async move {
        let skin = for_name(&name).await?;
        Some((name.to_lowercase(), view(&skin).await.ok()?))
    });
    futures::future::join_all(lookups).await.into_iter().flatten().collect()
}

/// El juego que se abra ahora pide la skin nueva, no la que se recordaba.
fn forget_own(app: &AppHandle) {
    let mut names = vec![app.state::<AppState>().settings.lock().unwrap().username.to_lowercase()];
    if let Some(name) = crate::cuenta::social_handle(app).and_then(|s| s.name()) {
        names.push(name.to_lowercase());
    }
    let mut cache = cache().lock().unwrap();
    for name in names {
        cache.remove(&name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(payload: Value) -> String {
        base64::engine::general_purpose::STANDARD.encode(payload.to_string())
    }

    #[test]
    fn solo_acepta_skins_de_mojang() {
        let skin = |url: &str, model: Option<&str>| {
            let mut s = json!({ "url": url });
            if let Some(model) = model {
                s["metadata"] = json!({ "model": model });
            }
            value(json!({ "timestamp": 1, "profileId": "a", "profileName": "Notch", "textures": { "SKIN": s } }))
        };
        assert_eq!(decode(&skin("http://textures.minecraft.net/texture/abc123", None)), Some(("https://textures.minecraft.net/texture/abc123".into(), false)));
        assert_eq!(decode(&skin("https://textures.minecraft.net/texture/abc123", Some("slim"))).unwrap().1, true);
        assert_eq!(decode(&skin("https://ejemplo.com/texture/abc123", None)), None, "solo textures.minecraft.net");
        assert_eq!(decode(&skin("http://textures.minecraft.net/texture/../x", None)), None);
        assert_eq!(decode(&value(json!({ "textures": {} }))), None, "sin skin propia");
        assert_eq!(decode("no es base64"), None);
    }

    #[test]
    fn solo_pngs_de_skin() {
        let png = |w: u32, h: u32| {
            let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
            bytes.extend(w.to_be_bytes());
            bytes.extend(h.to_be_bytes());
            bytes.extend([8, 6, 0, 0, 0]);
            bytes
        };
        assert!(png_ok(&png(64, 64)));
        assert!(png_ok(&png(64, 32)));
        assert!(!png_ok(&png(128, 128)), "las HD no las acepta Minecraft sin mods");
        assert!(!png_ok(b"GIF89a, no es un PNG ni de lejos"));
        let mut big = png(64, 64);
        big.resize(30 * 1024, 0);
        assert!(!png_ok(&big));
    }

    #[test]
    fn lo_de_la_base_se_revisa_antes_de_darselo_al_juego() {
        let good = json!({
            "value": value(json!({ "textures": { "SKIN": { "url": "http://textures.minecraft.net/texture/ff00" } } })),
            "signature": "c2lnbmF0dXJh",
            "from": "Notch",
            "at": 5
        });
        let skin = parse(&good).unwrap();
        assert_eq!(skin.from.as_deref(), Some("Notch"));
        assert_eq!(skin.at, 5);
        assert!(parse(&json!({ "value": good["value"] })).is_none(), "sin firma no");
        assert!(parse(&json!({ "value": good["value"], "signature": "no<base64>" })).is_none());
        assert!(parse(&json!({ "value": "x".repeat(5000), "signature": "c2ln" })).is_none(), "demasiado largo");
        let other = json!({ "value": good["value"], "signature": "c2ln", "from": "no válido!" });
        assert_eq!(parse(&other).unwrap().from, None);
    }
}
