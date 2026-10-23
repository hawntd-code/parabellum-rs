//! Copias ligeras para escuchar la lista de chats en una rama. Las autorizaciones siguen usando
//! chats/{id}: este buzón no concede pertenencia ni permisos sobre una conversación.
use serde_json::{json, Map, Value};

fn assign(node: &mut Value, parts: &[&str], value: Value) {
    if parts.is_empty() { *node = value; return; }
    if !node.is_object() { *node = json!({}); }
    let object = node.as_object_mut().unwrap();
    if parts.len() == 1 {
        if value.is_null() { object.remove(parts[0]); } else { object.insert(parts[0].into(), value); }
    } else { assign(object.entry(parts[0]).or_insert(Value::Null), &parts[1..], value); }
}

pub fn after(id: &str, before: &Value, update: &Map<String, Value>) -> Value {
    let mut data = before.clone();
    let root = format!("chats/{id}");
    for (path, value) in update {
        if path == &root { data = value.clone(); }
        else if let Some(tail) = path.strip_prefix(&format!("{root}/")) { assign(&mut data, &tail.split('/').collect::<Vec<_>>(), value.clone()); }
    }
    data
}

pub fn members(data: &Value) -> Vec<String> {
    data["members"].as_object().into_iter().flatten().filter(|(_, v)| **v == true).map(|(k, _)| k.clone()).collect()
}

pub fn add(id: &str, uid: &str, before: &Value, update: &mut Map<String, Value>) {
    let data = after(id, before, update);
    let mut recipients = members(before);
    recipients.extend(members(&data));
    recipients.sort(); recipients.dedup();
    for recipient in recipients {
        let hidden = update.get(&format!("userChats/{recipient}/{id}")).is_some_and(Value::is_null);
        let entry = if hidden || data["members"][&recipient] != true { Value::Null }
            else { json!({"protocol":1,"by":uid,"meta":data["meta"],"members":data["members"]}) };
        update.insert(format!("chatInbox/{recipient}/{id}"), entry);
    }
}

pub fn valid(data: &Value, uid: &str) -> bool {
    data["protocol"] == 1 && data["members"][uid] == true && data["meta"].is_object()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conserva_metadatos_y_quita_el_buzon_del_miembro_que_sale() {
        let before = json!({"members":{"a":true,"b":true},"meta":{"kind":"grupo","name":"Amigos","lastAt":10}});
        let mut update = Map::from_iter([("chats/g/members/b".into(),Value::Null),("chats/g/meta/lastText".into(),json!("Hola"))]);
        add("g","a",&before,&mut update);
        assert_eq!(update["chatInbox/b/g"],Value::Null);
        assert_eq!(update["chatInbox/a/g"]["meta"]["name"],"Amigos");
        assert_eq!(update["chatInbox/a/g"]["meta"]["lastText"],"Hola");
        assert!(valid(&update["chatInbox/a/g"],"a"));
        assert!(!valid(&update["chatInbox/a/g"],"b"));
    }
}
