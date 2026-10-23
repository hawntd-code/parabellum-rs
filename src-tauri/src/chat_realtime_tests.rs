//! Recorrido del motor real contra el emulador oficial, aislado de cuentas y datos de jugadores.
use crate::{amigos::Social, chat::Chat, firebase::{Database, Session, metrics}};
use serde_json::{json, Value};
use std::{sync::{Arc, Mutex, atomic::{AtomicUsize, Ordering}}, time::Duration};

const BASE: &str = "http://127.0.0.1:9399";
const NS: &str = "parabellum-chat37";

async fn admin(path: &str, value: &Value) {
    let response = reqwest::Client::new().put(format!("{BASE}/{path}.json?ns={NS}"))
        .bearer_auth("owner").json(value).send().await.expect("emulador en 127.0.0.1:9399");
    assert!(response.status().is_success(), "{}", response.text().await.unwrap_or_default());
}

async fn wait_for(label: &str, condition: impl Fn() -> bool) {
    for _ in 0..200 {
        if condition() { return; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("No se completó: {label}");
}

fn database(uid: &str) -> Arc<Database> { Arc::new(Database::new(&format!("{BASE}?ns={NS}"), Session::emulator(uid,NS))) }

#[tokio::test]
#[ignore = "requiere el emulador oficial en 127.0.0.1:9399"]
async fn chat37_buzones_envio_idempotente_escritura_y_cliente_anterior() {
    let rules: Value = serde_json::from_str(include_str!("../../firebase-reglas.json")).unwrap();
    admin(".settings/rules", &rules).await;
    let mut chats = serde_json::Map::new();
    let mut ids = serde_json::Map::new();
    for index in 0..8 {
        let id = format!("g_test{index}");
        chats.insert(id.clone(), json!({"members":{"A":true,"B":true},"meta":{"kind":"grupo","name":format!("Grupo {index}"),"owner":"A","lastAt":1,"lastText":"","lastUid":"","lastName":""}}));
        ids.insert(id,json!(true));
    }
    admin("", &json!({"chats":chats,"userChats":{"A":ids,"B":ids},"friends":{"A":{"B":true},"B":{"A":true}},
        "users":{"A":{"name":"TestA","uuid":"","streamVersion":1},"B":{"name":"TestB","uuid":"","streamVersion":1},"C":{"name":"TestC","uuid":""}},
        "names":{"testa":"A","testb":"B","testc":"C"}})).await;
    let db_a = database("A"); let db_b = database("B"); let db_c = database("C");
    let social_a = Social::from_database(db_a.clone(), || {}, |e| eprintln!("amigos A: {e}"));
    let social_b = Social::from_database(db_b.clone(), || {}, |e| eprintln!("amigos B: {e}"));
    assert!(social_a.claim_name("TestA", "").await.is_ok());
    assert!(social_b.claim_name("TestB", "").await.is_ok());
    let arrived = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(Mutex::new(Vec::new()));
    let typing = Arc::new(Mutex::new(Value::Null));
    let root = std::env::temp_dir().join(format!("parabellum-chat37-{}",std::process::id()));
    let chat_a = Chat::new(db_a.clone(),"TestA".into(),root.join("a"),|| {}, |_| {}, |_,_| {}, |_,_| {});
    let (new_arrived,new_received,new_typing) = (arrived.clone(),received.clone(),typing.clone());
    let chat_b = Chat::new(db_b.clone(),"TestB".into(),root.join("b"),|| {}, move |_| {new_arrived.fetch_add(1,Ordering::SeqCst);},
        move |_,messages| {*new_received.lock().unwrap()=messages;},move |_,data| {*new_typing.lock().unwrap()=data;});
    social_a.start(); social_b.start(); chat_a.start(); chat_b.start();
    wait_for("listas sincronizadas",|| chat_a.synced() && chat_b.synced() && social_a.synced() && social_b.synced()).await;
    wait_for("buzones sin escuchas por amigo ni conversación",|| metrics::LISTENERS.load(Ordering::SeqCst) == 14).await;
    println!("8 conversaciones y 1 amigo por usuario: 14 escuchas entre dos clientes (7 por cliente). Antes: 14 por cliente.");
    chat_b.open("g_test0").unwrap();
    wait_for("solo el chat abierto agrega 3 escuchas",|| metrics::LISTENERS.load(Ordering::SeqCst) == 17).await;
    let key = format!("{}aaaaaaa",db_a.server_now());
    chat_a.send_text_tracked("g_test0","Hola",None,Some(&key)).await.unwrap();
    chat_a.send_text_tracked("g_test0","Hola",None,Some(&key)).await.unwrap();
    wait_for("mensaje recibido",|| received.lock().unwrap().iter().any(|m| m.id == key && m.text.as_deref()==Some("Hola"))).await;
    wait_for("un solo aviso",|| arrived.load(Ordering::SeqCst) == 1).await;
    assert_eq!(db_a.get("messages/g_test0").await.unwrap().as_object().unwrap().len(),1);
    assert!(chat_a.send_text_tracked("g_test0","Otro texto",None,Some(&key)).await.is_err());
    assert!(chat_a.send_text_tracked("g_test0","Hola",None,Some("éééééééééé")).await.is_err());
    chat_a.set_typing(Some("g_test0")).await;
    wait_for("indicador de escritura del miembro",|| typing.lock().unwrap()["A"].is_number()).await;
    chat_a.set_typing(None).await;
    wait_for("indicador retirado",|| typing.lock().unwrap()["A"].is_null()).await;
    chat_a.edit("g_test0",&key,"Editado").await.unwrap();
    wait_for("edición actualiza la vista previa",|| chat_b.summary("g_test0").is_some_and(|s| s.last_text == "Editado")).await;
    let reply = format!("{}bbbbbbb",db_a.server_now());
    chat_a.send_text_tracked("g_test0","Contesto",Some(&key),Some(&reply)).await.unwrap();
    chat_a.delete("g_test0",&key).await.unwrap();
    // Reintentar una respuesta ya enviada sigue funcionando si después se borró el original.
    chat_a.send_text_tracked("g_test0","Contesto",Some(&key),Some(&reply)).await.unwrap();
    chat_a.delete("g_test0",&reply).await.unwrap();
    wait_for("borrado actualiza los mensajes",|| received.lock().unwrap().is_empty()).await;
    assert!(db_a.get("chatInbox/A/g_test0").await.unwrap().is_object());
    assert!(db_c.get("chatInbox/A").await.is_err());
    assert!(db_c.put("typing/g_test0/C",&crate::firebase::server_timestamp()).await.is_err());
    admin("users/B/email", &json!("privado@example.invalid")).await;
    assert!(chat_a.profile("B").await.unwrap().get("email").is_none());
    let mut archive = serde_json::Map::new();
    for index in 0..305 {
        let mut message = json!({"from":"A","name":"TestA","at":index});
        if [5,160,300].contains(&index) { message["image"] = json!(true); } else { message["text"] = json!("Mensaje"); }
        archive.insert(format!("{:013}abcdefg",1000+index),message);
    }
    admin("messages/g_test1", &Value::Object(archive)).await;
    let mut cursor: Option<String> = None;
    let mut photos = std::collections::HashSet::new();
    loop {
        let page = chat_a.media_page("g_test1",cursor.as_deref()).await.unwrap();
        for image in page["images"].as_array().unwrap() { assert!(photos.insert(image["id"].as_str().unwrap().to_string())); }
        if page["more"] != true { break; }
        let next = page["cursor"].as_str().unwrap().to_string(); assert_ne!(cursor.as_ref(),Some(&next)); cursor=Some(next);
    }
    assert_eq!(photos.len(),3,"La galería recorre el historial sin perder ni repetir imágenes");
    // Un miembro de una versión anterior sigue escribiendo solo las rutas canónicas.
    admin("chats/g_legacy", &json!({"members":{"A":true,"C":true},"meta":{"kind":"grupo","name":"Anterior","owner":"A","lastAt":1}})).await;
    admin("userChats/A/g_legacy", &json!(true)).await;
    admin("friends/A/C", &json!(true)).await; admin("friends/C/A", &json!(true)).await;
    admin("presence/C", &json!({"name":"TestC","state":"online","updated":db_c.server_now()})).await;
    wait_for("respaldo para conversación y amigo anteriores",|| metrics::LISTENERS.load(Ordering::SeqCst) == 19).await;
    db_c.put("presence/C",&json!({"name":"TestC","state":"playing","pack":"Anterior","updated":crate::firebase::server_timestamp()})).await.unwrap();
    wait_for("presencia anterior en tiempo real",|| social_a.view().friends.iter().any(|f|f.uid=="C" && f.presence.as_ref().is_some_and(|p|p.pack.as_deref()==Some("Anterior")))).await;
    db_c.patch("",&json!({"chats/g_legacy/meta/lastAt":crate::firebase::server_timestamp(),"chats/g_legacy/meta/lastText":"Desde la anterior","chats/g_legacy/meta/lastUid":"C","chats/g_legacy/meta/lastName":"TestC"})).await.unwrap();
    wait_for("mensaje de cliente anterior",|| chat_a.summary("g_legacy").is_some_and(|s| s.last_text=="Desde la anterior")).await;
    social_a.remove_friend("C").await.unwrap();
    chat_a.remove_member("g_legacy","C").await.unwrap();
    wait_for("eliminar escuchas al quitar amigo y miembro",|| metrics::LISTENERS.load(Ordering::SeqCst) == 17).await;
    chat_b.close(); chat_a.stop(); chat_b.stop();
    social_a.go_offline().await; social_b.go_offline().await;
    wait_for("cero escuchas al cerrar sesión",|| metrics::LISTENERS.load(Ordering::SeqCst)==0).await;
    // Las reglas anteriores no deben romper envíos ni presencia durante la actualización gradual.
    let mut old_rules = rules.clone();
    for branch in ["chatInbox","presenceInbox","typing"] { old_rules["rules"].as_object_mut().unwrap().remove(branch); }
    old_rules["rules"]["users"]["$uid"].as_object_mut().unwrap().remove("streamVersion");
    admin(".settings/rules", &old_rules).await;
    admin("", &json!({"chats":{"g_old":{"members":{"A":true,"B":true},"meta":{"kind":"grupo","name":"Anterior","owner":"A"}}},"userChats":{"A":{"g_old":true},"B":{"g_old":true}},"friends":{"A":{"B":true},"B":{"A":true}},"users":{"A":{"name":"TestA"},"B":{"name":"TestB"}},"names":{"testa":"A","testb":"B"}})).await;
    let old_a = Chat::new(db_a.clone(),"TestA".into(),root.join("old-a"),|| {}, |_| {}, |_,_| {}, |_,_| {});
    let old_b = Chat::new(db_b.clone(),"TestB".into(),root.join("old-b"),|| {}, |_| {}, |_,_| {}, |_,_| {});
    old_a.start(); old_b.start();
    wait_for("listas con reglas anteriores",|| old_a.synced() && old_b.synced()).await;
    let old_key = format!("{}ccccccc",db_a.server_now());
    old_a.send_text_tracked("g_old","Con reglas anteriores",None,Some(&old_key)).await.unwrap();
    wait_for("envío con reglas anteriores",|| old_b.summary("g_old").is_some_and(|s|s.last_text=="Con reglas anteriores")).await;
    old_a.stop(); old_b.stop();
    wait_for("limpieza del respaldo anterior",|| metrics::LISTENERS.load(Ordering::SeqCst)==0).await;
    admin(".settings/rules", &rules).await;
    println!("Envío, reintento, escritura, editar, borrar, permisos, compatibilidad y limpieza: correctos.");
}
