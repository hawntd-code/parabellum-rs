//! Soporte de una comunidad (docs/plan-diciembre.md, sección 7): solicitudes de ayuda que lee todo el
//! equipo de soporte, con el diagnóstico del jugador, y tarjetas de ajustes que el jugador aplica con un
//! clic.
//!
//! Vive aparte del chat (`supportTickets`, `supportMessages`, `supportByUser`): ser del equipo no da
//! acceso a los mensajes privados. Las reglas de Firebase deciden quién ve y escribe qué (probadas en
//! docs/probar-reglas.py); aquí se vuelve a comprobar antes de aplicar una tarjeta, porque es lo único
//! que cambia algo en este PC.
//!
//! Una tarjeta nunca se aplica sola: el jugador la pulsa. Se aplica con el servicio de ajustes
//! (`ajustes::apply`, con el valor «esperado» que había al pulsar) y queda en `datos/tarjetas.json`;
//! repetirla no cambia nada y «Deshacer» vuelve al valor anterior solo si nadie lo cambió después.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

use crate::comunidad::{db, db_error, get, id, my_name, valid_id};
use crate::firebase;
use crate::AppState;

const RECORD: &str = "tarjetas.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Ticket {
    #[serde(skip_deserializing)]
    pub id: String,
    pub by: String,
    pub by_name: String,
    pub topic: String,
    /// «abierto», «atendido» o «cerrado».
    pub state: String,
    pub at: i64,
    pub updated: i64,
    pub assignee: Option<String>,
    pub assignee_name: Option<String>,
    pub instance: Option<String>,
    pub last: String,
    pub last_at: i64,
    pub last_from: String,
    /// Solo para quien la pidió: hay respuesta que todavía no leyó.
    #[serde(skip_deserializing)]
    pub unread: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Card {
    pub v: u32,
    pub op: String,
    pub kind: String,
    pub setting: String,
    pub value: Value,
    pub scope: String,
    pub official: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Ack {
    pub op: String,
    pub result: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Message {
    #[serde(skip_deserializing)]
    pub id: String,
    pub from: String,
    pub name: String,
    pub text: String,
    pub at: i64,
    /// «texto», «diagnostico», «tarjeta» o «acuse».
    pub kind: String,
    pub card: Option<Card>,
    pub ack: Option<Ack>,
    #[serde(skip_deserializing)]
    pub mine: bool,
}

/// Lo que pasó con una tarjeta en este PC (`datos/tarjetas.json`).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Applied {
    pub community: String,
    pub ticket: String,
    pub message: String,
    pub setting: String,
    pub previous: Value,
    pub value: Value,
    pub at: i64,
    /// «aplicada», «ya-estaba» o «deshecha».
    pub result: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// Eres del equipo de soporte (o dueño o administrador): ves la cola entera.
    pub staff: bool,
    pub tickets: Vec<Ticket>,
}

fn record_path(app: &AppHandle) -> std::path::PathBuf {
    app.state::<AppState>().paths.data.join(RECORD)
}

pub fn record(app: &AppHandle) -> HashMap<String, Applied> {
    std::fs::read(record_path(app)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_record(app: &AppHandle, record: &HashMap<String, Applied>) {
    let _ = std::fs::write(record_path(app), serde_json::to_vec_pretty(record).unwrap_or_default());
}

fn is_staff(member: &Value) -> bool {
    matches!(member["role"].as_str(), Some("owner" | "admin")) || member["caps"]["support"].as_bool() == Some(true)
}

fn check_ids(cid: &str, tid: Option<&str>) -> Result<(), String> {
    if !valid_id('c', 12, cid) || tid.is_some_and(|t| !valid_id('t', 10, t)) {
        return Err(t!("Solicitud no válida.", "Invalid request."));
    }
    Ok(())
}

fn tickets_from(value: &Value) -> Vec<Ticket> {
    value
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(tid, t)| serde_json::from_value::<Ticket>(t.clone()).ok().map(|ticket| Ticket { id: tid.clone(), ..ticket }))
        .collect()
}

fn sort_tickets(list: &mut [Ticket]) {
    // Las abiertas primero, después las atendidas y al final las cerradas; las más recientes arriba.
    let rank = |s: &str| match s {
        "abierto" => 0,
        "atendido" => 1,
        _ => 2,
    };
    list.sort_by_key(|t| (rank(&t.state), std::cmp::Reverse(t.last_at.max(t.updated))));
}

/// Las solicitudes: todas si eres del equipo; si no, las tuyas.
#[tauri::command]
pub async fn soporte(app: AppHandle, id: String) -> Result<View, String> {
    check_ids(&id, None)?;
    let (db, uid) = db(&app)?;
    let me = get(&db, format!("communityMembers/{id}/{uid}")).await?;
    if !me.is_object() {
        return Err(t!("Ya no eres miembro de esta comunidad.", "You are no longer a member of this community."));
    }
    if let Some(watcher) = crate::cuenta::support_watcher(&app) {
        watcher.set_staff(&id, is_staff(&me));
    }
    if is_staff(&me) {
        let mut tickets = tickets_from(&get(&db, format!("supportTickets/{id}")).await?);
        sort_tickets(&mut tickets);
        return Ok(View { staff: true, tickets });
    }
    let seen = get(&db, format!("supportByUser/{id}/{uid}")).await.unwrap_or(Value::Null);
    let ids: Vec<(String, i64)> = seen.as_object().into_iter().flatten().map(|(t, at)| (t.clone(), at.as_i64().unwrap_or(0))).collect();
    let found = futures::future::join_all(ids.iter().map(|(tid, _)| get(&db, format!("supportTickets/{id}/{tid}")))).await;
    let mut tickets: Vec<Ticket> = ids
        .iter()
        .zip(found)
        .filter_map(|((tid, seen_at), t)| {
            let ticket = serde_json::from_value::<Ticket>(t.ok()?).ok()?;
            let unread = ticket.last_from != uid && ticket.last_at > *seen_at;
            Some(Ticket { id: tid.clone(), unread, ..ticket })
        })
        .collect();
    sort_tickets(&mut tickets);
    Ok(View { staff: false, tickets })
}

fn preview(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    if line.chars().count() > 140 {
        format!("{}…", line.chars().take(139).collect::<String>())
    } else {
        line.to_string()
    }
}

fn message_value(uid: &str, name: &str, text: &str, kind: &str) -> Value {
    json!({ "from": uid, "name": name, "text": text, "at": firebase::server_timestamp(), "kind": kind })
}

/// Pide ayuda: el tema, lo que pasó y, si el jugador lo deja, su diagnóstico (ya revisado por él).
#[tauri::command]
pub async fn soporte_abrir(app: AppHandle, id: String, tema: String, texto: String, instancia: Option<String>, diagnostico: Option<String>) -> Result<String, String> {
    check_ids(&id, None)?;
    let (db, uid) = db(&app)?;
    let topic: String = tema.trim().chars().take(80).collect();
    let text: String = texto.trim().chars().take(2000).collect();
    if topic.is_empty() || text.is_empty() {
        return Err(t!("Escribe el tema y qué pasó.", "Write the topic and what happened."));
    }
    let name = my_name(&app);
    let tid = id_for("t", 10);
    let mut update = serde_json::Map::new();
    let mut ticket = json!({
        "by": uid, "byName": name, "topic": topic, "state": "abierto", "at": firebase::server_timestamp(), "updated": firebase::server_timestamp(),
        "last": preview(&text), "lastAt": firebase::server_timestamp(), "lastFrom": uid,
    });
    if let Some(instance) = instancia.map(|i| i.trim().chars().take(60).collect::<String>()).filter(|i| !i.is_empty()) {
        ticket["instance"] = json!(instance);
    }
    update.insert(format!("supportTickets/{id}/{tid}"), ticket);
    // Hasta cuándo la leyó, con la hora del servidor (la del PC puede ir atrasada).
    update.insert(format!("supportByUser/{id}/{uid}/{tid}"), firebase::server_timestamp());
    update.insert(format!("supportMessages/{id}/{tid}/{}", id_for("m", 12)), message_value(&uid, &name, &text, "texto"));
    // El diagnóstico se vuelve a limpiar aquí (sin la carpeta del usuario, correos ni claves).
    if let Some(diagnosis) = diagnostico.map(|d| crate::reporte::redact(d.trim())).filter(|d| !d.is_empty()) {
        let diagnosis: String = diagnosis.chars().take(4000).collect();
        update.insert(format!("supportMessages/{id}/{tid}/{}", id_for("m", 12)), message_value(&uid, &name, &diagnosis, "diagnostico"));
    }
    db.patch("", &Value::Object(update)).await.map_err(db_error)?;
    Ok(tid)
}

fn id_for(prefix: &str, len: usize) -> String {
    id(prefix, len)
}

fn messages_from(value: &Value, uid: &str) -> Vec<Message> {
    let mut list: Vec<Message> = value
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(mid, m)| serde_json::from_value::<Message>(m.clone()).ok().map(|message| Message { id: mid.clone(), mine: message.from == uid, ..message }))
        .collect();
    list.sort_by(|a, b| a.at.cmp(&b.at).then(a.id.cmp(&b.id)));
    list
}

/// Los mensajes de una solicitud. Si es tuya, queda leída.
#[tauri::command]
pub async fn soporte_mensajes(app: AppHandle, id: String, solicitud: String) -> Result<Vec<Message>, String> {
    check_ids(&id, Some(&solicitud))?;
    let (db, uid) = db(&app)?;
    let messages = messages_from(&get(&db, format!("supportMessages/{id}/{solicitud}")).await?, &uid);
    mark_seen(&app, &id, &solicitud).await;
    Ok(messages)
}

/// Leída (la ventana la tiene a la vista y llegó algo nuevo).
#[tauri::command]
pub async fn soporte_visto(app: AppHandle, id: String, solicitud: String) -> Result<(), String> {
    check_ids(&id, Some(&solicitud))?;
    mark_seen(&app, &id, &solicitud).await;
    Ok(())
}

async fn mark_seen(app: &AppHandle, cid: &str, tid: &str) {
    let Ok((db, uid)) = db(app) else { return };
    // Solo existe para quien la pidió (para el equipo, la regla lo niega y no importa).
    if get(&db, format!("supportByUser/{cid}/{uid}/{tid}")).await.is_ok_and(|v| !v.is_null()) {
        let _ = db.put(&format!("supportByUser/{cid}/{uid}/{tid}"), &firebase::server_timestamp()).await;
        if let Some(watcher) = crate::cuenta::support_watcher(app) {
            watcher.seen(cid, tid);
        }
    }
}

/// La solicitud abierta en la ventana: sus mensajes llegan solos (`soporte-mensajes`) hasta cerrarla.
static OPEN: Mutex<Option<firebase::Listener>> = Mutex::new(None);

#[tauri::command]
pub fn soporte_escuchar(app: AppHandle, id: Option<String>, solicitud: Option<String>) -> Result<(), String> {
    *OPEN.lock().unwrap() = None;
    let (Some(cid), Some(tid)) = (id, solicitud) else { return Ok(()) };
    check_ids(&cid, Some(&tid))?;
    let (db, uid) = db(&app)?;
    let (app_copy, cid_copy, tid_copy) = (app.clone(), cid.clone(), tid.clone());
    let listener = db.listen(
        format!("supportMessages/{cid}/{tid}"),
        move |value| {
            let messages = messages_from(&value, &uid);
            let _ = app_copy.emit("soporte-mensajes", json!({ "community": cid_copy, "ticket": tid_copy, "messages": messages }));
        },
        |_| {},
    );
    *OPEN.lock().unwrap() = Some(listener);
    Ok(())
}

/// Escribe en una solicitud (quien la pidió o el equipo). Si escribe el equipo, queda «atendida».
#[tauri::command]
pub async fn soporte_enviar(app: AppHandle, id: String, solicitud: String, texto: String) -> Result<(), String> {
    check_ids(&id, Some(&solicitud))?;
    let (db, uid) = db(&app)?;
    let text: String = texto.trim().chars().take(2000).collect();
    if text.is_empty() {
        return Ok(());
    }
    let ticket = get(&db, format!("supportTickets/{id}/{solicitud}")).await?;
    let mine = ticket["by"].as_str() == Some(uid.as_str());
    let mut update = serde_json::Map::new();
    update.insert(format!("supportMessages/{id}/{solicitud}/{}", id_for("m", 12)), message_value(&uid, &my_name(&app), &text, "texto"));
    update.insert(format!("supportTickets/{id}/{solicitud}/last"), json!(preview(&text)));
    update.insert(format!("supportTickets/{id}/{solicitud}/lastAt"), firebase::server_timestamp());
    update.insert(format!("supportTickets/{id}/{solicitud}/lastFrom"), json!(uid));
    update.insert(format!("supportTickets/{id}/{solicitud}/updated"), firebase::server_timestamp());
    // Si quien la pidió escribe en una cerrada, se vuelve a abrir.
    if mine && ticket["state"].as_str() == Some("cerrado") {
        update.insert(format!("supportTickets/{id}/{solicitud}/state"), json!("abierto"));
    } else if !mine && ticket["state"].as_str() == Some("abierto") {
        update.insert(format!("supportTickets/{id}/{solicitud}/state"), json!("atendido"));
    }
    // Contesta el equipo: el aviso para quien la pidió (lo escucha su vigía, sin abrir Comunidades).
    let owner = ticket["by"].as_str().filter(|_| !mine);
    patch_with_notice(&db, Value::Object(update), &id, owner, &solicitud).await?;
    if mine {
        mark_seen(&app, &id, &solicitud).await;
    }
    Ok(())
}

/// El equipo la toma (así se sabe quién la atiende).
#[tauri::command]
pub async fn soporte_tomar(app: AppHandle, id: String, solicitud: String) -> Result<(), String> {
    check_ids(&id, Some(&solicitud))?;
    let (db, uid) = db(&app)?;
    let update = json!({
        format!("supportTickets/{id}/{solicitud}/assignee"): uid,
        format!("supportTickets/{id}/{solicitud}/assigneeName"): my_name(&app),
        format!("supportTickets/{id}/{solicitud}/state"): "atendido",
        format!("supportTickets/{id}/{solicitud}/updated"): firebase::server_timestamp(),
    });
    db.patch("", &update).await.map_err(db_error).map(|_| ())
}

/// Cerrar o volver a abrir (quien la pidió o el equipo).
#[tauri::command]
pub async fn soporte_estado(app: AppHandle, id: String, solicitud: String, estado: String) -> Result<(), String> {
    check_ids(&id, Some(&solicitud))?;
    if !matches!(estado.as_str(), "abierto" | "atendido" | "cerrado") {
        return Err(t!("Estado no válido.", "Invalid state."));
    }
    let (db, _) = db(&app)?;
    let update = json!({
        format!("supportTickets/{id}/{solicitud}/state"): estado,
        format!("supportTickets/{id}/{solicitud}/updated"): firebase::server_timestamp(),
    });
    db.patch("", &update).await.map_err(db_error).map(|_| ())
}

/// El texto de una tarjeta: va siempre, para los clientes viejos y el panel del juego.
fn card_text(setting: &str, value: &Value) -> String {
    let label = crate::ajustes::find(setting).map(|d| if crate::idioma::english() { d.en } else { d.es }).unwrap_or(setting);
    // Sin lo de entre paréntesis («Memoria para el juego (GB)»): el valor ya lleva su unidad.
    let label = label.split(" (").next().unwrap_or(label);
    let shown = match (setting, value) {
        (_, Value::Bool(true)) => t!("Activado", "On"),
        (_, Value::Bool(false)) => t!("Desactivado", "Off"),
        ("launcher.ram_gb", v) => format!("{v} GB"),
        ("launcher.download_limit_mb", v) if v.as_f64() == Some(0.0) => t!("Sin límite", "No limit"),
        ("launcher.download_limit_mb", v) => format!("{v} MB/s"),
        (_, v) => v.to_string(),
    };
    t!("🛠 Ajuste propuesto: {label} → {shown} (pulsa «Aplicar» en Parabellum)", "🛠 Proposed setting: {label} → {shown} (press «Apply» in Parabellum)")
}

/// El equipo propone un ajuste (de la lista de ajustes.rs, con un valor válido).
#[tauri::command]
pub async fn soporte_tarjeta(app: AppHandle, id: String, solicitud: String, ajuste: String, valor: Value) -> Result<(), String> {
    check_ids(&id, Some(&solicitud))?;
    let (db, uid) = db(&app)?;
    // Se valida aquí con las mismas reglas que al aplicarla (y otra vez en las reglas de Firebase).
    let (_, outcome) = crate::ajustes::plan(&crate::settings::Settings::default(), &crate::ajustes::Operation { id: ajuste.clone(), value: valor.clone(), expected: None })?;
    let value = match outcome {
        crate::ajustes::Outcome::Applied { value } | crate::ajustes::Outcome::Unchanged { value } => value,
        crate::ajustes::Outcome::Conflict { current } => current,
    };
    // 5.0 → 5 (se lee mejor en el texto y en la base de datos).
    let value = match value.as_f64() {
        Some(n) if n.fract() == 0.0 && !value.is_boolean() => json!(n as i64),
        _ => value,
    };
    let card = Card { v: 1, op: id_for("o", 12), kind: "setting".into(), setting: ajuste.clone(), value: value.clone(), scope: "launcher".into(), official: id.clone() };
    let text = card_text(&ajuste, &value);
    let mut message = message_value(&uid, &my_name(&app), &text, "tarjeta");
    message["card"] = serde_json::to_value(&card).unwrap();
    let update = json!({
        format!("supportMessages/{id}/{solicitud}/{}", id_for("m", 12)): message,
        format!("supportTickets/{id}/{solicitud}/last"): preview(&text),
        format!("supportTickets/{id}/{solicitud}/lastAt"): firebase::server_timestamp(),
        format!("supportTickets/{id}/{solicitud}/lastFrom"): uid,
        format!("supportTickets/{id}/{solicitud}/updated"): firebase::server_timestamp(),
    });
    // El aviso para quien la pidió, como al contestar.
    let owner = get(&db, format!("supportTickets/{id}/{solicitud}/by")).await?;
    patch_with_notice(&db, update, &id, owner.as_str().filter(|o| *o != uid), &solicitud).await
}

/// Escribe la respuesta del equipo junto con el aviso para quien la pidió (`supportNotify`). Si Firebase
/// todavía no tiene la regla del aviso (se publica antes que la 0.35.0, pero por si acaso), la respuesta
/// se manda igual, sin el aviso: solo no le llegaría al jugador hasta que abra Comunidades.
async fn patch_with_notice(db: &firebase::Database, mut update: Value, cid: &str, owner: Option<&str>, tid: &str) -> Result<(), String> {
    let Some(owner) = owner else { return db.patch("", &update).await.map_err(db_error).map(|_| ()) };
    let notice = format!("supportNotify/{cid}/{owner}/{tid}");
    update[notice.as_str()] = firebase::server_timestamp();
    match db.patch("", &update).await {
        Err(firebase::DbError::Permission(_)) => {
            update.as_object_mut().unwrap().remove(&notice);
            db.patch("", &update).await.map_err(db_error).map(|_| ())
        }
        other => other.map_err(db_error).map(|_| ()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CardResult {
    /// «aplicada», «ya-estaba» o «deshecha».
    pub result: String,
    pub previous: Value,
    pub value: Value,
}

/// Aplica una tarjeta en este PC. Se vuelve a comprobar todo (no se confía en lo que muestra la ventana):
/// que la solicitud sea tuya, que la mandó alguien que **hoy** es del equipo de esa comunidad, y el
/// ajuste y su valor. Repetirla no cambia nada.
#[tauri::command]
pub async fn aplicar_tarjeta(app: AppHandle, id: String, solicitud: String, mensaje: String) -> Result<CardResult, String> {
    check_ids(&id, Some(&solicitud))?;
    if !valid_id('m', 12, &mensaje) {
        return Err(t!("Tarjeta no válida.", "Invalid card."));
    }
    let (db, uid) = db(&app)?;
    let (message, ticket) = futures::join!(get(&db, format!("supportMessages/{id}/{solicitud}/{mensaje}")), get(&db, format!("supportTickets/{id}/{solicitud}")));
    let message: Message = serde_json::from_value(message?).map_err(|_| t!("La tarjeta ya no está.", "The card is no longer there."))?;
    let ticket = ticket?;
    let card = message.card.filter(|c| message.kind == "tarjeta" && c.v == 1 && c.kind == "setting" && c.scope == "launcher" && c.official == id).ok_or(t!("Tarjeta no válida.", "Invalid card."))?;
    if ticket["by"].as_str() != Some(uid.as_str()) {
        return Err(t!("Solo quien pidió la ayuda puede aplicar sus tarjetas.", "Only whoever asked for help can apply its cards."));
    }
    let sender = get(&db, format!("communityMembers/{id}/{}", message.from)).await.unwrap_or(Value::Null);
    if !is_staff(&sender) {
        return Err(t!("Quien la mandó ya no es del equipo de soporte: no se aplica.", "Whoever sent it is no longer on the support team: it is not applied."));
    }
    let def = crate::ajustes::find(&card.setting).ok_or(t!("Esta versión de Parabellum no conoce ese ajuste: actualízala.", "This version of Parabellum does not know that setting: update it."))?;
    if crate::loaders::compare_versions(env!("CARGO_PKG_VERSION"), def.since) == std::cmp::Ordering::Less {
        return Err(t!("Actualiza Parabellum para aplicar esta tarjeta.", "Update Parabellum to apply this card."));
    }
    let mut record = record(&app);
    if let Some(done) = record.get(&card.op).filter(|r| r.result != "deshecha") {
        return Ok(CardResult { result: done.result.clone(), previous: done.previous.clone(), value: done.value.clone() });
    }
    let previous = crate::ajustes::read(&app, &card.setting)?;
    let outcome = crate::ajustes::apply(&app, &crate::ajustes::Operation { id: card.setting.clone(), value: card.value.clone(), expected: Some(previous.clone()) })?;
    let (result, value) = match outcome {
        crate::ajustes::Outcome::Applied { value } => ("aplicada", value),
        crate::ajustes::Outcome::Unchanged { value } => ("ya-estaba", value),
        crate::ajustes::Outcome::Conflict { current } => return Err(t!("El ajuste cambió mientras tanto (ahora está en {current}). Vuelve a pulsar.", "The setting changed meanwhile (now it is {current}). Press again.")),
    };
    record.insert(
        card.op.clone(),
        Applied { community: id.clone(), ticket: solicitud.clone(), message: mensaje, setting: card.setting.clone(), previous: previous.clone(), value: value.clone(), at: chrono::Utc::now().timestamp_millis(), result: result.into() },
    );
    save_record(&app, &record);
    acknowledge(&app, &id, &solicitud, &card.op, result).await;
    Ok(CardResult { result: result.into(), previous, value })
}

/// Vuelve al valor de antes de la tarjeta, solo si nadie lo cambió después.
#[tauri::command]
pub async fn deshacer_tarjeta(app: AppHandle, op: String) -> Result<CardResult, String> {
    let mut record = record(&app);
    let done = record.get(&op).cloned().ok_or(t!("Esa tarjeta no se aplicó en este PC.", "That card was not applied on this PC."))?;
    if done.result == "deshecha" {
        return Ok(CardResult { result: done.result, previous: done.previous, value: done.value });
    }
    let outcome = crate::ajustes::apply(&app, &crate::ajustes::Operation { id: done.setting.clone(), value: done.previous.clone(), expected: Some(done.value.clone()) })?;
    if let crate::ajustes::Outcome::Conflict { current } = outcome {
        return Err(t!("Lo cambiaste después de aplicarla (ahora está en {current}): no se deshace.", "You changed it after applying it (now it is {current}): it is not undone."));
    }
    let entry = record.get_mut(&op).unwrap();
    entry.result = "deshecha".into();
    let result = entry.clone();
    save_record(&app, &record);
    acknowledge(&app, &result.community, &result.ticket, &op, "deshecha").await;
    Ok(CardResult { result: result.result, previous: result.previous, value: result.value })
}

/// Lo que pasó con cada tarjeta en este PC (para mostrar «Aplicada» y «Deshacer»).
#[tauri::command]
pub fn tarjetas_aplicadas(app: AppHandle) -> HashMap<String, Applied> {
    record(&app)
}

/// El acuse para el equipo: «aplicada en este PC» (si falla, no importa: es informativo).
async fn acknowledge(app: &AppHandle, cid: &str, tid: &str, op: &str, result: &str) {
    let Ok((db, uid)) = db(app) else { return };
    let text = match result {
        "aplicada" => t!("✓ Ajuste aplicado en mi PC", "✓ Setting applied on my PC"),
        "ya-estaba" => t!("✓ Ya lo tenía así", "✓ I already had it like that"),
        _ => t!("↩ Deshice el ajuste", "↩ I undid the setting"),
    };
    let mut message = message_value(&uid, &my_name(app), &text, "acuse");
    message["ack"] = json!({ "op": op, "result": result });
    let _ = db.put(&format!("supportMessages/{cid}/{tid}/{}", id_for("m", 12)), &message).await;
}

// ================= El vigía: los avisos de soporte desde que se abre Parabellum

/// Lo que ve la ventana (`soporte-avisos`): cuántas cosas esperan, en total y por comunidad. Para quien
/// pidió ayuda, las respuestas sin leer; para el equipo, además, las solicitudes que esperan respuesta.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Notices {
    pub total: usize,
    pub communities: HashMap<String, usize>,
}

/// Una comunidad vigilada.
#[derive(Default)]
struct Watched {
    name: String,
    staff: bool,
    listeners: Vec<firebase::Listener>,
    /// Las respuestas del equipo a mis solicitudes (supportNotify): solicitud -> cuándo. None hasta que
    /// llega la primera lectura (lo que ya había no se avisa).
    replies: Option<HashMap<String, i64>>,
    /// Hasta cuándo leí cada una (supportByUser).
    seen: HashMap<String, i64>,
    /// Del equipo: las que esperan respuesta (`waits`), solicitud -> (cuándo, tema, quién, es nueva).
    waiting: Option<HashMap<String, (i64, String, String, bool)>>,
}

impl Watched {
    fn count(&self) -> usize {
        let unread = self.replies.iter().flatten().filter(|(tid, at)| **at > self.seen.get(*tid).copied().unwrap_or(0)).count();
        unread + if self.staff { self.waiting.as_ref().map_or(0, HashMap::len) } else { 0 }
    }
}

/// Espera respuesta del equipo: sin cerrar y lo último lo escribió quien la pidió (una nueva, o el
/// jugador que contestó). Las tuyas no cuentan: las ves como jugador.
fn waits(ticket: &Ticket, me: &str) -> bool {
    ticket.state != "cerrado" && ticket.last_from == ticket.by && ticket.by != me
}

/// El vigía de soporte (plan de noviembre, punto 2): arranca con la sesión y escucha, en cada comunidad
/// tuya, `supportNotify/{cid}/{uid}` (una rama: las respuestas a tus solicitudes) y, si eres del equipo,
/// `supportTickets/{cid}` (la cola). Así te enteras sin abrir Comunidades, con el número en su botón y
/// una notificación de Windows o Mac.
pub struct Watcher {
    app: AppHandle,
    db: std::sync::Arc<firebase::Database>,
    uid: String,
    membership: Mutex<Option<firebase::Listener>>,
    watched: Mutex<HashMap<String, Watched>>,
    /// Lo ya avisado con una notificación: (solicitud, cuándo).
    notified: Mutex<std::collections::HashSet<(String, i64)>>,
    last: Mutex<Notices>,
}

impl Watcher {
    pub fn start(app: &AppHandle, db: std::sync::Arc<firebase::Database>) -> std::sync::Arc<Self> {
        let uid = db.session.uid.clone();
        let watcher = std::sync::Arc::new(Self {
            app: app.clone(),
            db,
            uid,
            membership: Mutex::default(),
            watched: Mutex::default(),
            notified: Mutex::default(),
            last: Mutex::default(),
        });
        // Las comunidades: al entrar o salir de una se empieza o se deja de escucharla.
        let weak = std::sync::Arc::downgrade(&watcher);
        let listener = watcher.db.listen(
            format!("userCommunities/{}", watcher.uid),
            move |value| {
                if let Some(watcher) = weak.upgrade() {
                    watcher.sync(&value);
                }
            },
            |_| {},
        );
        *watcher.membership.lock().unwrap() = Some(listener);
        watcher
    }

    /// Lo que hay ahora (la ventana lo pide al abrir: el aviso pudo llegar antes).
    pub fn notices(&self) -> Notices {
        self.last.lock().unwrap().clone()
    }

    fn sync(self: &std::sync::Arc<Self>, value: &Value) {
        let wanted: std::collections::HashSet<String> =
            value.as_object().into_iter().flatten().filter(|(cid, v)| v.as_bool() == Some(true) && valid_id('c', 12, cid)).map(|(cid, _)| cid.clone()).collect();
        let added: Vec<String> = {
            let mut watched = self.watched.lock().unwrap();
            watched.retain(|cid, _| wanted.contains(cid));
            let added = wanted.iter().filter(|cid| !watched.contains_key(*cid)).cloned().collect::<Vec<_>>();
            for cid in &added {
                watched.insert(cid.clone(), Watched::default());
            }
            added
        };
        for cid in added {
            let watcher = self.clone();
            tauri::async_runtime::spawn(async move { watcher.watch(cid).await });
        }
        self.emit();
    }

    /// Empieza a escuchar una comunidad: si eres del equipo, también su cola.
    async fn watch(self: std::sync::Arc<Self>, cid: String) {
        let (member, community, seen) = futures::join!(
            get(&self.db, format!("communityMembers/{cid}/{}", self.uid)),
            get(&self.db, format!("communities/{cid}/name")),
            get(&self.db, format!("supportByUser/{cid}/{}", self.uid)),
        );
        let staff = member.as_ref().is_ok_and(is_staff);
        let seen: HashMap<String, i64> = seen.unwrap_or_default().as_object().into_iter().flatten().filter_map(|(t, at)| Some((t.clone(), at.as_i64()?))).collect();
        let weak = std::sync::Arc::downgrade(&self);
        let mut listeners = vec![{
            let (weak, cid) = (weak.clone(), cid.clone());
            self.db.listen(
                format!("supportNotify/{cid}/{}", self.uid),
                move |value| {
                    if let Some(watcher) = weak.upgrade() {
                        watcher.on_replies(&cid, &value);
                    }
                },
                |_| {},
            )
        }];
        if staff {
            let (weak, cid) = (weak.clone(), cid.clone());
            listeners.push(self.db.listen(
                format!("supportTickets/{cid}"),
                move |value| {
                    if let Some(watcher) = weak.upgrade() {
                        watcher.on_queue(&cid, &value);
                    }
                },
                |_| {},
            ));
        }
        let mut watched = self.watched.lock().unwrap();
        // Si saliste de la comunidad mientras tanto, las escuchas se sueltan aquí.
        if let Some(entry) = watched.get_mut(&cid) {
            *entry = Watched { name: community.ok().and_then(|n| n.as_str().map(String::from)).unwrap_or_default(), staff, listeners, seen, ..Watched::default() };
        }
    }

    /// Te volvieron a dar o quitar el soporte (lo nota la ventana al abrir la comunidad).
    pub fn set_staff(self: &std::sync::Arc<Self>, cid: &str, staff: bool) {
        let changed = {
            let mut watched = self.watched.lock().unwrap();
            let changed = watched.get(cid).is_some_and(|w| !w.listeners.is_empty() && w.staff != staff);
            if changed {
                watched.insert(cid.to_string(), Watched::default());
            }
            changed
        };
        if changed {
            let (watcher, cid) = (self.clone(), cid.to_string());
            tauri::async_runtime::spawn(async move { watcher.watch(cid).await });
        }
    }

    /// La leíste en este PC (soporte_mensajes, soporte_visto o al contestar).
    pub fn seen(&self, cid: &str, tid: &str) {
        if let Some(watched) = self.watched.lock().unwrap().get_mut(cid) {
            let at = watched.replies.as_ref().and_then(|r| r.get(tid)).copied().unwrap_or(0);
            let seen = watched.seen.entry(tid.to_string()).or_insert(0);
            *seen = (*seen).max(at);
        }
        self.emit();
    }

    fn on_replies(&self, cid: &str, value: &Value) {
        let replies: HashMap<String, i64> = value.as_object().into_iter().flatten().filter_map(|(t, at)| Some((t.clone(), at.as_i64()?))).collect();
        let (fresh, name) = {
            let mut watched = self.watched.lock().unwrap();
            let Some(entry) = watched.get_mut(cid) else { return };
            let fresh: Vec<(String, i64)> = match &entry.replies {
                Some(before) => replies
                    .iter()
                    .filter(|(tid, at)| **at > before.get(*tid).copied().unwrap_or(0) && **at > entry.seen.get(*tid).copied().unwrap_or(0))
                    .map(|(t, at)| (t.clone(), *at))
                    .collect(),
                None => Vec::new(),
            };
            entry.replies = Some(replies);
            (fresh, entry.name.clone())
        };
        self.emit();
        if !fresh.is_empty() {
            let _ = self.app.emit("soporte-cola", json!({ "community": cid }));
        }
        for (tid, at) in fresh {
            // El tema no viaja en el aviso: se lee de la solicitud.
            let (db, app, cid, name) = (self.db.clone(), self.app.clone(), cid.to_string(), name.clone());
            if !self.notified.lock().unwrap().insert((tid.clone(), at)) {
                continue;
            }
            tauri::async_runtime::spawn(async move {
                let topic = get(&db, format!("supportTickets/{cid}/{tid}/topic")).await.ok().and_then(|t| t.as_str().map(String::from)).unwrap_or_default();
                let title = if name.is_empty() { t!("Te respondió el soporte", "Support answered you") } else { t!("Te respondió el soporte de {name}", "{name}'s support answered you") };
                notify(&app, &title, &topic, &cid, &tid);
            });
        }
    }

    fn on_queue(&self, cid: &str, value: &Value) {
        let waiting: HashMap<String, (i64, String, String, bool)> =
            tickets_from(value).into_iter().filter(|t| waits(t, &self.uid)).map(|t| (t.id.clone(), (t.last_at, t.topic.clone(), t.by_name.clone(), t.last_at <= t.at))).collect();
        let (fresh, name) = {
            let mut watched = self.watched.lock().unwrap();
            let Some(entry) = watched.get_mut(cid) else { return };
            let fresh: Vec<(String, (i64, String, String, bool))> = match &entry.waiting {
                Some(before) => waiting.iter().filter(|(tid, (at, ..))| before.get(*tid).is_none_or(|(old, ..)| at > old)).map(|(t, w)| (t.clone(), w.clone())).collect(),
                None => Vec::new(),
            };
            let had = entry.waiting.is_some();
            entry.waiting = Some(waiting);
            (if had { fresh } else { Vec::new() }, entry.name.clone())
        };
        self.emit();
        let _ = self.app.emit("soporte-cola", json!({ "community": cid }));
        for (tid, (at, topic, by_name, new)) in fresh {
            if !self.notified.lock().unwrap().insert((tid.clone(), at)) {
                continue;
            }
            let title = match (new, name.is_empty()) {
                (true, false) => t!("Nueva solicitud de ayuda en {name}", "New help request in {name}"),
                (true, true) => t!("Nueva solicitud de ayuda", "New help request"),
                (false, _) => t!("{by_name} contestó en soporte", "{by_name} replied in support"),
            };
            let body = if new { format!("{by_name}: {topic}") } else { topic };
            notify(&self.app, &title, &body, cid, &tid);
        }
    }

    /// El número en el botón de Comunidades y el punto de cada comunidad, si cambió.
    fn emit(&self) {
        let notices = {
            let watched = self.watched.lock().unwrap();
            let communities: HashMap<String, usize> = watched.iter().map(|(cid, w)| (cid.clone(), w.count())).filter(|(_, n)| *n > 0).collect();
            Notices { total: communities.values().sum(), communities }
        };
        let mut last = self.last.lock().unwrap();
        if *last != notices {
            *last = notices.clone();
            drop(last);
            let _ = self.app.emit("soporte-avisos", notices);
        }
    }
}

/// La notificación de Windows o Mac: solo con los avisos activados, si Parabellum no está al frente y no
/// se está jugando con el panel (ahí no se ve). Al pulsarla se abre la solicitud.
fn notify(app: &AppHandle, title: &str, body: &str, cid: &str, tid: &str) {
    let enabled = app.state::<AppState>().settings.lock().unwrap().notifications;
    let in_front = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
    if !enabled || in_front || crate::puente::game_connected(app) {
        return;
    }
    let (handle, title, body, target) = (app.clone(), title.to_string(), body.to_string(), json!({ "community": cid, "ticket": tid }));
    // Los avisos usan WinRT: se muestran desde el hilo de la ventana, que ya lo tiene iniciado.
    let _ = app.run_on_main_thread(move || {
        let click = handle.clone();
        let _ = crate::avisos::show(&title, &body, false, move || {
            crate::cuenta::show_window(&click);
            let _ = click.emit("abrir-soporte", target.clone());
        });
    });
}

/// Los avisos de ahora (la ventana los pide al abrir; después llegan solos con `soporte-avisos`).
#[tauri::command]
pub fn soporte_avisos(app: AppHandle) -> Notices {
    crate::cuenta::support_watcher(&app).map(|w| w.notices()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cola_ordena_abiertas_primero() {
        let t = |id: &str, state: &str, at: i64| Ticket { id: id.into(), state: state.into(), last_at: at, ..Default::default() };
        let mut list = vec![t("a", "cerrado", 50), t("b", "atendido", 40), t("c", "abierto", 10), t("d", "abierto", 30)];
        sort_tickets(&mut list);
        let ids: Vec<&str> = list.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, vec!["d", "c", "b", "a"]);
    }

    #[test]
    fn el_equipo_de_soporte() {
        assert!(is_staff(&json!({ "role": "owner" })));
        assert!(is_staff(&json!({ "role": "admin" })));
        assert!(is_staff(&json!({ "role": "member", "caps": { "support": true } })));
        assert!(!is_staff(&json!({ "role": "member", "caps": { "publish": true } })));
        assert!(!is_staff(&Value::Null));
    }

    #[test]
    fn mensajes_en_orden_y_tarjeta() {
        let value = json!({
            "m000000000002": { "from": "F", "name": "F", "text": "x", "at": 20, "kind": "tarjeta", "card": { "v": 1, "op": "o000000000001", "kind": "setting", "setting": "launcher.ram_gb", "value": 6, "scope": "launcher", "official": "c1" } },
            "m000000000001": { "from": "E", "name": "E", "text": "hola", "at": 10, "kind": "texto" }
        });
        let list = messages_from(&value, "E");
        assert_eq!(list[0].text, "hola");
        assert!(list[0].mine);
        assert_eq!(list[1].card.as_ref().unwrap().value, json!(6));
        assert!(card_text("launcher.ram_gb", &json!(6)).contains("Memoria para el juego → 6 GB"));
        assert!(card_text("launcher.download_limit_mb", &json!(0)).contains("→ Sin límite"));
        assert_eq!(preview("una\ndos"), "una");
    }

    #[test]
    fn que_cuenta_el_aviso() {
        let t = |by: &str, state: &str, last_from: &str| Ticket { by: by.into(), state: state.into(), last_from: last_from.into(), ..Default::default() };
        // Para el equipo (F): las que esperan respuesta.
        assert!(waits(&t("E", "abierto", "E"), "F"), "una nueva");
        assert!(waits(&t("E", "atendido", "E"), "F"), "el jugador contestó");
        assert!(!waits(&t("E", "atendido", "F"), "F"), "ya contestó el equipo");
        assert!(!waits(&t("E", "cerrado", "E"), "F"), "cerrada");
        assert!(!waits(&t("F", "abierto", "F"), "F"), "la mía la veo como jugador");
        // Para quien la pidió: las respuestas posteriores a lo que leyó.
        let watched = Watched {
            replies: Some(HashMap::from([("t1".into(), 50), ("t2".into(), 20)])),
            seen: HashMap::from([("t1".into(), 40), ("t2".into(), 30)]),
            ..Default::default()
        };
        assert_eq!(watched.count(), 1);
        let staff = Watched { staff: true, waiting: Some(HashMap::from([("t3".into(), (1, String::new(), String::new(), true))])), ..watched };
        assert_eq!(staff.count(), 2);
    }
}
