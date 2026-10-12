//! Favoritos que siguen a la cuenta de Google (reglas en firebase-reglas.json):
//!   favorites/{uid} = { since, chats: { chat: true }, mods: { proyecto: { title, kind, icon?, at } } }
//! Con la sesión iniciada manda lo que hay en la cuenta: quitar uno en otro PC también lo quita aquí.
//! La primera vez se sube lo que ya había en este PC (si era de esta misma cuenta o de nadie: en un
//! PC compartido, los favoritos de otro compañero no se pasan a tu cuenta).
//! Sin sesión, los favoritos quedan en ajustes.json como siempre.

use serde_json::{json, Map, Value};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

use crate::firebase::{self, Database, Listener};
use crate::settings::{FavoriteMod, Settings};
use crate::AppState;

/// Lo que la base de datos acepta como clave (Firebase no deja . # $ [ ] /).
fn key_ok(key: &str) -> bool {
    !key.is_empty() && key.len() <= 200 && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn https_icon(icon: &Option<String>) -> Option<String> {
    icon.clone().filter(|u| u.starts_with("https://") && u.len() <= 500)
}

/// Los favoritos de este PC como se guardan en la cuenta.
fn to_remote(settings: &Settings, now: i64) -> Value {
    let chats: Map<String, Value> = settings.favorite_chats.iter().filter(|c| key_ok(c)).map(|c| (c.clone(), json!(true))).collect();
    // El orden (el más reciente primero) va en «at».
    let mods: Map<String, Value> = settings
        .favorite_mods
        .iter()
        .enumerate()
        .filter(|(_, m)| key_ok(&m.project_id) && m.project_id.len() <= 64)
        .map(|(i, m)| {
            let mut entry = json!({ "title": m.title.chars().take(100).collect::<String>(), "kind": m.kind, "at": now - i as i64 });
            if let Some(icon) = https_icon(&m.icon_url) {
                entry["icon"] = json!(icon);
            }
            (m.project_id.clone(), entry)
        })
        .collect();
    json!({ "since": now, "chats": chats, "mods": mods })
}

/// Lo de la cuenta como favoritos de este PC. Se conservan los iconos locales que ya se conocían.
fn from_remote(data: &Value, current: &[FavoriteMod]) -> (Vec<String>, Vec<FavoriteMod>) {
    let mut chats: Vec<String> = data["chats"].as_object().map(|o| o.iter().filter(|(_, v)| v.as_bool() == Some(true)).map(|(k, _)| k.clone()).collect()).unwrap_or_default();
    chats.sort();
    let mut mods: Vec<(i64, FavoriteMod)> = data["mods"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(id, m)| {
                    let kind = m["kind"].as_str().filter(|k| ["mod", "resourcepack", "shader"].contains(k))?.to_string();
                    let icon = m["icon"].as_str().map(String::from).or_else(|| current.iter().find(|c| c.project_id == *id).and_then(|c| c.icon_url.clone()));
                    Some((m["at"].as_i64().unwrap_or(0), FavoriteMod { project_id: id.clone(), title: m["title"].as_str().unwrap_or(id).to_string(), icon_url: icon, kind }))
                })
                .collect()
        })
        .unwrap_or_default();
    mods.sort_by_key(|(at, _)| std::cmp::Reverse(*at));
    (chats, mods.into_iter().map(|(_, m)| m).collect())
}

fn emit(app: &AppHandle, settings: &Settings) {
    let _ = app.emit("favoritos", json!({ "chats": settings.favorite_chats, "mods": settings.favorite_mods }));
}

/// Escucha los favoritos de la cuenta mientras haya sesión.
pub fn start(app: &AppHandle, db: Arc<Database>) -> Listener {
    let uid = db.session.uid.clone();
    let (app, writer) = (app.clone(), db.clone());
    db.listen(
        format!("favorites/{uid}"),
        move |data| {
            let state = app.state::<AppState>();
            if data.is_null() {
                // Primera vez con esta cuenta: se suben los de este PC (si no eran de otra cuenta).
                let upload = {
                    let mut settings = state.settings.lock().unwrap();
                    let mine = settings.favorites_owner.as_deref().is_none_or(|owner| owner == uid);
                    if !mine {
                        settings.favorite_chats.clear();
                        settings.favorite_mods.clear();
                    }
                    settings.favorites_owner = Some(uid.clone());
                    let _ = settings.save(&state.paths);
                    emit(&app, &settings);
                    to_remote(&settings, chrono::Utc::now().timestamp_millis())
                };
                let (writer, uid) = (writer.clone(), uid.clone());
                tauri::async_runtime::spawn(async move {
                    let _ = writer.put(&format!("favorites/{uid}"), &upload).await;
                });
                return;
            }
            let mut settings = state.settings.lock().unwrap();
            let (chats, mods) = from_remote(&data, &settings.favorite_mods);
            if settings.favorite_chats != chats || settings.favorite_mods != mods || settings.favorites_owner.as_deref() != Some(uid.as_str()) {
                settings.favorite_chats = chats;
                settings.favorite_mods = mods;
                settings.favorites_owner = Some(uid.clone());
                let _ = settings.save(&state.paths);
                emit(&app, &settings);
            }
        },
        |_| {},
    )
}

/// Guarda en la cuenta un cambio hecho en este PC (si hay sesión).
pub fn push_chat(app: &AppHandle, chat: &str, on: bool) {
    push(app, format!("chats/{chat}"), on.then(|| json!(true)));
}

pub fn push_mod(app: &AppHandle, favorite: &FavoriteMod, on: bool) {
    let value = on.then(|| {
        let mut entry = json!({ "title": favorite.title, "kind": favorite.kind, "at": chrono::Utc::now().timestamp_millis() });
        if let Some(icon) = https_icon(&favorite.icon_url) {
            entry["icon"] = json!(icon);
        }
        entry
    });
    push(app, format!("mods/{}", favorite.project_id), value);
}

fn push(app: &AppHandle, path: String, value: Option<Value>) {
    let Some(db) = crate::cuenta::social_handle(app).map(|s| s.database()) else { return };
    if path.rsplit('/').next().is_none_or(|key| !key_ok(key)) {
        return;
    }
    let uid = db.session.uid.clone();
    // Solo si estos favoritos ya son de esta cuenta (si no, la escucha los trae primero).
    if app.state::<AppState>().settings.lock().unwrap().favorites_owner.as_deref() != Some(uid.as_str()) {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let path = format!("favorites/{uid}/{path}");
        let _ = match value {
            Some(value) => db.put(&path, &value).await.map(|_| ()),
            None => db.delete(&path).await,
        };
        let _ = db.put(&format!("favorites/{uid}/since"), &firebase::server_timestamp()).await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favoritos_de_ida_y_vuelta() {
        let settings = Settings {
            favorite_chats: vec!["d_a_b".into(), "g_x".into(), "malo/ruta".into()],
            favorite_mods: vec![
                FavoriteMod { project_id: "AANobbMI".into(), title: "Sodium".into(), icon_url: Some("https://cdn.modrinth.com/s.png".into()), kind: "mod".into() },
                FavoriteMod { project_id: "mOgUt4GM".into(), title: "Mod Menu".into(), icon_url: Some("http://icono.localhost/x".into()), kind: "mod".into() },
            ],
            ..Settings::default()
        };
        let remote = to_remote(&settings, 1_000);
        assert!(remote["chats"].get("malo/ruta").is_none());
        assert_eq!(remote["mods"]["AANobbMI"]["icon"], "https://cdn.modrinth.com/s.png");
        assert!(remote["mods"]["mOgUt4GM"].get("icon").is_none(), "los iconos locales no suben");
        let (chats, mods) = from_remote(&remote, &settings.favorite_mods);
        assert_eq!(chats, vec!["d_a_b", "g_x"]);
        assert_eq!(mods, settings.favorite_mods, "mismo orden y el icono local se conserva");
    }
}
