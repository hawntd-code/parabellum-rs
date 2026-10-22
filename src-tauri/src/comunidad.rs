//! Comunidades (docs/plan-diciembre.md, sección 6): la de Juan es la primera. Una comunidad tiene su
//! nombre, descripción e imagen, sus miembros con su rol y sus permisos, anuncios, reglas, servidores
//! (con el paquete que piden) y paquetes oficiales.
//!
//! Todo vive en Firebase y lo protegen sus reglas (`firebase-reglas.json`, probadas en
//! docs/probar-reglas.py): se entra con un código vigente, solo el dueño nombra administradores y nadie
//! se da permisos a sí mismo. Aquí no hay escuchas abiertas: se lee al abrir la página (y al volver a
//! ella), para no sumar conexiones a Firebase (mejora 5 del plan).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

use crate::firebase::{self, Database};
use crate::AppState;

/// Letras de los códigos de invitación: sin I, O, 0 ni 1 (se confunden al dictarlos).
const CODE_LETTERS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn random(prefix: &str, len: usize, letters: &[u8]) -> String {
    let mut bytes = vec![0u8; len];
    getrandom::getrandom(&mut bytes).expect("números aleatorios");
    let body: String = bytes.iter().map(|b| letters[*b as usize % letters.len()] as char).collect();
    format!("{prefix}{body}")
}

pub(crate) fn id(prefix: &str, len: usize) -> String {
    random(prefix, len, b"0123456789abcdefghijklmnopqrstuvwxyz")
}

pub(crate) fn valid_id(prefix: char, len: usize, text: &str) -> bool {
    text.len() == len + 1 && text.starts_with(prefix) && text[1..].chars().all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Caps {
    /// Atiende las solicitudes de soporte.
    pub support: bool,
    /// Publica los paquetes oficiales.
    pub publish: bool,
    /// Edita los servidores de la comunidad.
    pub servers: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    /// owner, admin o member.
    pub role: String,
    pub caps: Caps,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Member {
    pub uid: String,
    pub name: String,
    pub role: String,
    pub caps: Caps,
    pub joined_at: i64,
    pub you: bool,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Post {
    pub id: String,
    pub title: String,
    pub text: String,
    pub author_name: String,
    pub at: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Server {
    /// La clave en Firebase (no se guarda como campo). Al editar, la ventana la manda: sin ella cada
    /// «Guardar» creaba otro servidor.
    pub id: String,
    pub name: String,
    /// Java: «mc.ejemplo.com» o «mc.ejemplo.com:25565».
    pub address: String,
    /// Bedrock (Geyser), solo para mostrarla: «mc.ejemplo.com:19132».
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrock: Option<String>,
    /// El paquete oficial que pide (id en communityPackages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Package {
    /// La clave en Firebase (no se guarda como campo). Sin ella, «usar la versión de Drive» duplicaba el
    /// paquete en vez de cambiarlo.
    pub id: String,
    pub name: String,
    /// El paquete del catálogo de Drive (su id): de ahí se instala y se actualiza.
    pub catalog_id: String,
    pub version: String,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: String,
    pub at: i64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Invite {
    pub code: String,
    pub expires_at: i64,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    #[serde(flatten)]
    pub summary: Summary,
    pub owner: String,
    pub members: Vec<Member>,
    pub posts: Vec<Post>,
    pub rules: String,
    pub servers: Vec<Server>,
    pub packages: Vec<Package>,
    /// Los códigos vigentes (solo el dueño y los administradores).
    pub invites: Vec<Invite>,
    /// Cómo está cada paquete en este PC (por su id).
    pub local: std::collections::HashMap<String, Readiness>,
    /// Lo que dice el catálogo de cada paquete: icono, mods, tamaño, versión publicada (por su id).
    pub catalog: std::collections::HashMap<String, PackageInfo>,
}

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct PackageInfo {
    pub icon: Option<String>,
    pub mods: u32,
    pub size: u64,
    pub description: String,
    /// La versión que está ahora en el catálogo (si es más nueva que la que piden, se puede pedir).
    pub version: String,
}

pub(crate) fn db(app: &AppHandle) -> Result<(Arc<Database>, String), String> {
    let social = crate::cuenta::social_handle(app).ok_or(t!("Primero inicia sesión con Google.", "Sign in with Google first."))?;
    let db = social.database();
    let uid = db.session.uid.clone();
    Ok((db, uid))
}

/// Tu nombre en la comunidad: el de Parabellum (o el de Minecraft si todavía no tienes).
pub(crate) fn my_name(app: &AppHandle) -> String {
    let social = crate::cuenta::social_handle(app);
    let name = social.and_then(|s| s.name()).unwrap_or_else(|| app.state::<AppState>().settings.lock().unwrap().username.clone());
    name.chars().take(16).collect()
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().to_string()
}

fn caps_of(value: &Value) -> Caps {
    serde_json::from_value(value["caps"].clone()).unwrap_or_default()
}

fn is_admin(role: &str) -> bool {
    role == "owner" || role == "admin"
}

fn summary(id: &str, community: &Value, me: &Value) -> Summary {
    Summary {
        id: id.to_string(),
        name: text(community, "name"),
        description: text(community, "description"),
        image: community["image"].as_str().map(String::from),
        role: text(me, "role"),
        caps: caps_of(me),
    }
}

/// Los errores de Firebase, con uno claro cuando las reglas no lo permiten.
pub(crate) fn db_error(error: firebase::DbError) -> String {
    match error {
        firebase::DbError::Permission(_) => t!("No tienes permiso para hacer eso en esta comunidad.", "You do not have permission to do that in this community."),
        other => other.to_string(),
    }
}

pub(crate) async fn get(db: &Database, path: String) -> Result<Value, String> {
    db.get(&path).await.map_err(db_error)
}

/// Tus comunidades.
#[tauri::command]
pub async fn comunidades(app: AppHandle) -> Result<Vec<Summary>, String> {
    let (db, uid) = db(&app)?;
    let ids: Vec<String> = get(&db, format!("userCommunities/{uid}")).await?.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
    let found = futures::future::join_all(ids.iter().map(|cid| {
        let db = db.clone();
        let uid = uid.clone();
        async move {
            let (community, me) = futures::join!(get(&db, format!("communities/{cid}")), get(&db, format!("communityMembers/{cid}/{uid}")));
            (cid.clone(), community.ok(), me.ok())
        }
    }))
    .await;
    let mut list: Vec<Summary> = found
        .into_iter()
        // Si te sacaron, la comunidad sigue en tu lista hasta que se limpie: no se muestra.
        .filter_map(|(cid, community, me)| {
            let community = community.filter(|c| c.is_object())?;
            let me = me.filter(|m| m.is_object())?;
            Some(summary(&cid, &community, &me))
        })
        .collect();
    list.sort_by_key(|c| c.name.to_lowercase());
    Ok(list)
}

/// Todo lo de una comunidad (tienes que ser miembro).
#[tauri::command]
pub async fn comunidad(app: AppHandle, id: String) -> Result<Detail, String> {
    let (db, uid) = db(&app)?;
    if !valid_id('c', 12, &id) {
        return Err(t!("Comunidad no válida.", "Invalid community."));
    }
    let (community, members) = futures::join!(get(&db, format!("communities/{id}")), get(&db, format!("communityMembers/{id}")));
    let community = community?;
    let members = members.map_err(|_| t!("Ya no eres miembro de esta comunidad.", "You are no longer a member of this community."))?;
    if !community.is_object() || !members[&uid].is_object() {
        return Err(t!("Ya no eres miembro de esta comunidad.", "You are no longer a member of this community."));
    }
    let me = members[&uid].clone();
    let admin = is_admin(&text(&me, "role"));
    let (posts, rules, servers, packages) = futures::join!(
        get(&db, format!("communityPosts/{id}")),
        get(&db, format!("communityRules/{id}")),
        get(&db, format!("communityServers/{id}")),
        get(&db, format!("communityPackages/{id}"))
    );
    let invites = if admin { get(&db, format!("communityInvites/{id}")).await.unwrap_or(Value::Null) } else { Value::Null };
    let now = chrono::Utc::now().timestamp_millis();
    let entries = |value: &Value| value.as_object().cloned().unwrap_or_default();
    let mut member_list: Vec<Member> = entries(&members)
        .into_iter()
        .map(|(member_uid, m)| Member {
            you: member_uid == uid,
            uid: member_uid,
            name: text(&m, "name"),
            role: text(&m, "role"),
            caps: caps_of(&m),
            joined_at: m["joinedAt"].as_i64().unwrap_or(0),
        })
        .collect();
    // El dueño, los administradores y después los demás por nombre.
    member_list.sort_by_key(|m| (match m.role.as_str() { "owner" => 0, "admin" => 1, _ => 2 }, m.name.to_lowercase()));
    let mut post_list: Vec<Post> = entries(&posts.unwrap_or_default())
        .into_iter()
        .map(|(post_id, p)| Post { id: post_id, title: text(&p, "title"), text: text(&p, "text"), author_name: text(&p, "authorName"), at: p["at"].as_i64().unwrap_or(0) })
        .collect();
    post_list.sort_by_key(|p| std::cmp::Reverse(p.at));
    let mut server_list: Vec<Server> = entries(&servers.unwrap_or_default())
        .into_iter()
        .filter_map(|(sid, s)| serde_json::from_value::<Server>(s).ok().map(|server| Server { id: sid, ..server }))
        .collect();
    server_list.sort_by(|a, b| a.order.unwrap_or(0.0).total_cmp(&b.order.unwrap_or(0.0)).then(a.name.cmp(&b.name)));
    let package_list: Vec<Package> = entries(&packages.unwrap_or_default())
        .into_iter()
        .filter_map(|(pk, p)| serde_json::from_value::<Package>(p).ok().map(|package| Package { id: pk, ..package }))
        .collect();
    let mut invite_list: Vec<Invite> =
        entries(&invites).into_iter().filter_map(|(code, at)| Some(Invite { code, expires_at: at.as_i64()? })).filter(|i| i.expires_at > now).collect();
    invite_list.sort_by_key(|i| std::cmp::Reverse(i.expires_at));
    let paths = app.state::<AppState>().paths.clone();
    // Revisar la huella lee los mods (la primera vez, todos): fuera del hilo de la ventana.
    let (packages_copy, paths_copy) = (package_list.clone(), paths.clone());
    let local = tauri::async_runtime::spawn_blocking(move || {
        let instances = crate::instances::list(&paths_copy);
        packages_copy.iter().map(|p| (p.id.clone(), readiness_for(p, &instances))).collect::<std::collections::HashMap<_, _>>()
    })
    .await
    .map_err(|e| e.to_string())?;
    let catalog = crate::catalog::current(&paths).await;
    let info = package_list
        .iter()
        .filter_map(|p| {
            let entry = catalog.entries.iter().find(|e| !e.is_launcher() && e.manifest.id.eq_ignore_ascii_case(&p.catalog_id))?;
            Some((
                p.id.clone(),
                PackageInfo {
                    icon: entry.icon.clone(),
                    mods: entry.manifest.mod_count,
                    size: entry.size,
                    description: entry.manifest.description.clone(),
                    version: entry.manifest.version.clone(),
                },
            ))
        })
        .collect();
    Ok(Detail {
        local,
        catalog: info,
        summary: summary(&id, &community, &me),
        owner: text(&community, "owner"),
        members: member_list,
        posts: post_list,
        rules: rules.ok().map(|r| text(&r, "text")).unwrap_or_default(),
        servers: server_list,
        packages: package_list,
        invites: invite_list,
    })
}

/// Crea una comunidad (quien la crea es su dueño).
#[tauri::command]
pub async fn crear_comunidad(app: AppHandle, nombre: String, descripcion: String) -> Result<String, String> {
    let (db, uid) = db(&app)?;
    let name = nombre.trim();
    if name.chars().count() < 2 || name.chars().count() > 40 {
        return Err(t!("El nombre va de 2 a 40 letras.", "The name must be 2 to 40 characters."));
    }
    let cid = id("c", 12);
    let update = json!({
        format!("communities/{cid}"): { "name": name, "description": descripcion.trim().chars().take(500).collect::<String>(), "owner": uid, "createdAt": firebase::server_timestamp() },
        format!("communityMembers/{cid}/{uid}"): { "role": "owner", "name": my_name(&app), "joinedAt": firebase::server_timestamp() },
        format!("userCommunities/{uid}/{cid}"): true,
    });
    db.patch("", &update).await.map_err(db_error)?;
    Ok(cid)
}

/// Cambia nombre, descripción o imagen (el dueño o un administrador). La imagen, como data URL ya
/// reducida por la ventana (máx. ~88 KB).
#[tauri::command]
pub async fn editar_comunidad(app: AppHandle, id: String, nombre: String, descripcion: String, imagen: Option<String>) -> Result<(), String> {
    let (db, _) = db(&app)?;
    let mut update = Map::new();
    update.insert(format!("communities/{id}/name"), json!(nombre.trim()));
    update.insert(format!("communities/{id}/description"), json!(descripcion.trim().chars().take(500).collect::<String>()));
    if let Some(image) = imagen {
        if !image.starts_with("data:image/") || image.len() > 120_000 {
            return Err(t!("La imagen es muy grande.", "The image is too big."));
        }
        update.insert(format!("communities/{id}/image"), json!(image));
    }
    db.patch("", &Value::Object(update)).await.map_err(db_error)?;
    Ok(())
}

/// Lo público de una comunidad a partir de un código (para ver a qué te invitan antes de unirte).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvitePreview {
    pub id: String,
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    /// Ya eres miembro.
    pub member: bool,
}

fn clean_code(code: &str) -> Result<String, String> {
    let code: String = code.trim().to_uppercase().chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if code.len() == 8 && code.bytes().all(|b| CODE_LETTERS.contains(&b)) {
        Ok(code)
    } else {
        Err(t!("Ese código no es válido: son 8 letras y números.", "That code is not valid: it has 8 letters and digits."))
    }
}

async fn invite_target(db: &Database, code: &str) -> Result<String, String> {
    let invite = get(db, format!("inviteCodes/{code}")).await?;
    let cid = text(&invite, "cid");
    let expired = invite["expiresAt"].as_i64().is_none_or(|at| at <= chrono::Utc::now().timestamp_millis());
    if cid.is_empty() || expired {
        return Err(t!("Ese código no existe o ya venció. Pide uno nuevo.", "That code does not exist or has expired. Ask for a new one."));
    }
    Ok(cid)
}

#[tauri::command]
pub async fn ver_invitacion(app: AppHandle, codigo: String) -> Result<InvitePreview, String> {
    let (db, uid) = db(&app)?;
    let code = clean_code(&codigo)?;
    let cid = invite_target(&db, &code).await?;
    let community = get(&db, format!("communities/{cid}")).await?;
    let member = get(&db, format!("communityMembers/{cid}/{uid}")).await.is_ok_and(|m| m.is_object());
    Ok(InvitePreview { id: cid, name: text(&community, "name"), description: text(&community, "description"), image: community["image"].as_str().map(String::from), member })
}

/// Entra con un código (como miembro, sin permisos). Devuelve el id de la comunidad.
#[tauri::command]
pub async fn unirse_comunidad(app: AppHandle, codigo: String) -> Result<String, String> {
    let (db, uid) = db(&app)?;
    let code = clean_code(&codigo)?;
    let cid = invite_target(&db, &code).await?;
    if get(&db, format!("communityMembers/{cid}/{uid}")).await.is_ok_and(|m| m.is_object()) {
        // Ya estabas: solo vuelve a tu lista.
        let _ = db.put(&format!("userCommunities/{uid}/{cid}"), &json!(true)).await;
        return Ok(cid);
    }
    let update = json!({
        format!("communityMembers/{cid}/{uid}"): { "role": "member", "name": my_name(&app), "joinedAt": firebase::server_timestamp(), "invite": code },
        format!("userCommunities/{uid}/{cid}"): true,
    });
    db.patch("", &update).await.map_err(db_error)?;
    Ok(cid)
}

#[tauri::command]
pub async fn salir_comunidad(app: AppHandle, id: String) -> Result<(), String> {
    let (db, uid) = db(&app)?;
    let update = json!({ format!("communityMembers/{id}/{uid}"): null, format!("userCommunities/{uid}/{id}"): null });
    db.patch("", &update).await.map_err(db_error)?;
    Ok(())
}

/// Un código nuevo de invitación (el dueño o un administrador), que vence en `dias` (1 a 30).
#[tauri::command]
pub async fn crear_invitacion(app: AppHandle, id: String, dias: u32) -> Result<Invite, String> {
    let (db, uid) = db(&app)?;
    let code = random("", 8, CODE_LETTERS);
    let expires_at = chrono::Utc::now().timestamp_millis() + dias.clamp(1, 30) as i64 * 86_400_000;
    let update = json!({
        format!("inviteCodes/{code}"): { "cid": id, "createdBy": uid, "expiresAt": expires_at },
        format!("communityInvites/{id}/{code}"): expires_at,
    });
    db.patch("", &update).await.map_err(db_error)?;
    Ok(Invite { code, expires_at })
}

#[tauri::command]
pub async fn borrar_invitacion(app: AppHandle, id: String, codigo: String) -> Result<(), String> {
    let (db, _) = db(&app)?;
    let code = clean_code(&codigo)?;
    let update = json!({ format!("inviteCodes/{code}"): null, format!("communityInvites/{id}/{code}"): null });
    db.patch("", &update).await.map_err(db_error)?;
    Ok(())
}

/// Cambia el rol («member» o «admin») y los permisos de alguien. Las reglas deciden quién puede.
#[tauri::command]
pub async fn cambiar_miembro(app: AppHandle, id: String, uid: String, rol: String, permisos: Caps) -> Result<(), String> {
    let (db, _) = db(&app)?;
    if rol != "member" && rol != "admin" {
        return Err(t!("Rol no válido.", "Invalid role."));
    }
    let current = get(&db, format!("communityMembers/{id}/{uid}")).await?;
    if !current.is_object() {
        return Err(t!("Ya no es miembro.", "They are no longer a member."));
    }
    let mut member = current.as_object().cloned().unwrap_or_default();
    member.insert("role".into(), json!(rol));
    if permisos == Caps::default() {
        member.remove("caps");
    } else {
        member.insert("caps".into(), serde_json::to_value(&permisos).unwrap());
    }
    db.put(&format!("communityMembers/{id}/{uid}"), &Value::Object(member)).await.map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn sacar_de_comunidad(app: AppHandle, id: String, uid: String) -> Result<(), String> {
    let (db, _) = db(&app)?;
    let update = json!({ format!("communityMembers/{id}/{uid}"): null, format!("userCommunities/{uid}/{id}"): null });
    db.patch("", &update).await.map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn publicar_anuncio(app: AppHandle, id: String, titulo: String, texto: String) -> Result<(), String> {
    let (db, uid) = db(&app)?;
    let title = titulo.trim();
    if title.is_empty() {
        return Err(t!("Escribe un título.", "Write a title."));
    }
    let post = json!({
        "title": title.chars().take(80).collect::<String>(),
        "text": texto.trim().chars().take(2000).collect::<String>(),
        "author": uid,
        "authorName": my_name(&app),
        "at": firebase::server_timestamp(),
    });
    db.put(&format!("communityPosts/{id}/{}", self::id("a", 12)), &post).await.map_err(db_error)?;
    Ok(())
}

#[tauri::command]
pub async fn borrar_anuncio(app: AppHandle, id: String, anuncio: String) -> Result<(), String> {
    let (db, _) = db(&app)?;
    db.delete(&format!("communityPosts/{id}/{anuncio}")).await.map_err(db_error)
}

#[tauri::command]
pub async fn guardar_reglas(app: AppHandle, id: String, texto: String) -> Result<(), String> {
    let (db, uid) = db(&app)?;
    let rules = json!({ "text": texto.trim().chars().take(4000).collect::<String>(), "at": firebase::server_timestamp(), "by": uid });
    db.put(&format!("communityRules/{id}"), &rules).await.map(|_| ()).map_err(db_error)
}

/// Crea o cambia un servidor de la comunidad (sin `server.id`, uno nuevo).
#[tauri::command]
pub async fn guardar_servidor_comunidad(app: AppHandle, id: String, servidor: Server) -> Result<String, String> {
    let (db, _) = db(&app)?;
    let sid = if valid_id('v', 8, &servidor.id) { servidor.id.clone() } else { self::id("v", 8) };
    let address = servidor.address.trim().to_string();
    if servidor.name.trim().is_empty() || address.is_empty() || address.contains(char::is_whitespace) {
        return Err(t!("Escribe el nombre y la dirección del servidor.", "Write the server's name and address."));
    }
    let server = Server {
        id: String::new(),
        name: servidor.name.trim().chars().take(40).collect(),
        address: address.chars().take(100).collect(),
        bedrock: servidor.bedrock.map(|b| b.trim().chars().take(100).collect::<String>()).filter(|b| !b.is_empty()),
        package: servidor.package.filter(|p| valid_id('p', 8, p)),
        order: servidor.order,
    };
    let mut value = serde_json::to_value(&server).unwrap();
    // El id es la clave en Firebase, no un campo (las reglas no aceptan campos de más).
    if let Some(object) = value.as_object_mut() {
        object.remove("id");
    }
    db.put(&format!("communityServers/{id}/{sid}"), &value).await.map_err(db_error)?;
    Ok(sid)
}

#[tauri::command]
pub async fn borrar_servidor_comunidad(app: AppHandle, id: String, servidor: String) -> Result<(), String> {
    let (db, _) = db(&app)?;
    db.delete(&format!("communityServers/{id}/{servidor}")).await.map_err(db_error)
}

/// El paquete oficial: un paquete del catálogo de Drive (por su id), con la versión que piden.
#[tauri::command]
pub async fn guardar_paquete_comunidad(app: AppHandle, id: String, paquete: Package) -> Result<String, String> {
    let (db, _) = db(&app)?;
    let pk = if valid_id('p', 8, &paquete.id) { paquete.id.clone() } else { self::id("p", 8) };
    let mut value = serde_json::to_value(&Package { id: String::new(), at: chrono::Utc::now().timestamp_millis(), ..paquete }).unwrap();
    if let Some(object) = value.as_object_mut() {
        object.remove("id");
        object.retain(|_, v| !v.as_str().is_some_and(str::is_empty));
    }
    db.put(&format!("communityPackages/{id}/{pk}"), &value).await.map_err(db_error)?;
    Ok(pk)
}

/// Cómo está en este PC el paquete oficial de un servidor de la comunidad.
#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Readiness {
    /// "lista" (instalada y al día), "falta" o "desactualizada"; "sin-paquete" si el servidor no pide uno.
    pub state: String,
    /// El paquete del catálogo que hay que instalar o actualizar.
    pub catalog_id: Option<String>,
    pub wanted: Option<String>,
    /// La instancia de este PC que viene de ese paquete.
    pub instance: Option<String>,
    pub installed: Option<String>,
    /// En este PC está la instancia de la que se publicó (la de quien lo publica): se juega con ella.
    pub source: bool,
    /// Qué cambió respecto del paquete (identidad.rs): mods de más o que faltan, otro loader. En la
    /// instancia original, lo que cambió desde que se publicó.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<crate::identidad::Diff>,
}

/// Cómo está un paquete en este PC: la instancia que se instaló desde él (no por su nombre) o, en el PC
/// de quien lo publica, la instancia original (no se instala otra encima).
fn readiness_for(package: &Package, instances: &[crate::instances::Instance]) -> Readiness {
    if let Some(source) = instances.iter().find(|i| crate::packs::installed_manifest(i).is_none() && crate::packs::slug(&i.id).eq_ignore_ascii_case(&package.catalog_id)) {
        let changes = Some(crate::identidad::check(source)).filter(|d| d.modified());
        return Readiness { state: "lista".into(), catalog_id: Some(package.catalog_id.clone()), wanted: Some(package.version.clone()).filter(|v| !v.is_empty()), instance: Some(source.id.clone()), installed: None, source: true, changes };
    }
    let local = instances
        .iter()
        .filter_map(|i| crate::packs::installed_manifest(i).filter(|m| m.id.eq_ignore_ascii_case(&package.catalog_id)).map(|m| (i.id.clone(), m.version)))
        .max_by(|a, b| crate::loaders::compare_versions(&a.1, &b.1));
    let mut state = match &local {
        None => "falta",
        Some((_, version)) if !package.version.is_empty() && crate::loaders::compare_versions(version, &package.version) == std::cmp::Ordering::Less => "desactualizada",
        Some(_) => "lista",
    };
    // Al día, pero con cambios: «modificada» (al jugar se ofrece restaurar el paquete oficial).
    let mut changes = None;
    if state == "lista" {
        if let Some(instance) = local.as_ref().and_then(|(id, _)| instances.iter().find(|i| &i.id == id)) {
            let diff = crate::identidad::check(instance);
            if diff.modified() {
                state = "modificada";
                changes = Some(diff);
            }
        }
    }
    Readiness {
        state: state.into(),
        catalog_id: Some(package.catalog_id.clone()),
        wanted: Some(package.version.clone()).filter(|v| !v.is_empty()),
        installed: local.as_ref().map(|(_, v)| v.clone()),
        instance: local.map(|(i, _)| i),
        source: false,
        changes,
    }
}

/// «Jugar»: qué falta para entrar al servidor con el paquete oficial. La ventana lo instala o actualiza
/// con el catálogo (`instalar_de_catalogo`, que conserva mundos, opciones y controles) y abre el juego
/// directo en el servidor.
#[tauri::command]
pub async fn paquete_comunidad(app: AppHandle, id: String, servidor: String) -> Result<Readiness, String> {
    let (db, _) = db(&app)?;
    let server: Server = serde_json::from_value(get(&db, format!("communityServers/{id}/{servidor}")).await?).map_err(|_| t!("Ese servidor ya no está.", "That server is no longer there."))?;
    let Some(pk) = server.package.filter(|p| !p.is_empty()) else {
        return Ok(Readiness { state: "sin-paquete".into(), catalog_id: None, wanted: None, instance: None, installed: None, source: false, changes: None });
    };
    let package: Package = serde_json::from_value(get(&db, format!("communityPackages/{id}/{pk}")).await?).map_err(|_| t!("El paquete oficial ya no está.", "The official package is no longer there."))?;
    let paths = app.state::<AppState>().paths.clone();
    tauri::async_runtime::spawn_blocking(move || readiness_for(&package, &crate::instances::list(&paths))).await.map_err(|e| e.to_string())
}

/// Al abrir la comunidad, su paquete oficial (`id`: el del catálogo) empieza a bajar si falta o hay
/// versión nueva (catalog::prefetch), así al pulsar «Jugar» ya va avanzado. Nunca mientras se juega.
/// Sale en Descargas solo si baja algo. Devuelve si quedó bajado (o ya lo estaba).
#[tauri::command]
pub async fn precargar_paquete(state: tauri::State<'_, AppState>, id: String) -> Result<bool, String> {
    if !state.games.lock().unwrap().is_empty() {
        return Ok(false);
    }
    let paths = state.paths.clone();
    let catalog = crate::catalog::current(&paths).await;
    let Some(entry) = catalog.entries.iter().find(|e| !e.is_launcher() && e.manifest.id.eq_ignore_ascii_case(&id)) else { return Ok(false) };
    let instances = crate::instances::list(&paths);
    let job = crate::descargas::start(None, "catalogo", t!("Preparar {} para jugar", "Get {} ready to play", entry.manifest.name)).quiet().icon(entry.icon.clone());
    job.run(crate::catalog::prefetch(entry, &paths, &instances, job.progress("catalogo"))).await
}

#[tauri::command]
pub async fn borrar_paquete_comunidad(app: AppHandle, id: String, paquete: String) -> Result<(), String> {
    let (db, _) = db(&app)?;
    db.delete(&format!("communityPackages/{id}/{paquete}")).await.map_err(db_error)
}

/// El paquete oficial desde una de tus instancias: se exporta (sin Minecraft, que sale del espejo), se
/// sube al catálogo de Drive y queda como paquete de la comunidad (`paquete`: el que se reemplaza, si
/// ya había uno). Hace falta «Conectar Drive» (Ajustes › Acerca de). Es una tarea de Descargas.
#[tauri::command]
pub async fn publicar_paquete_comunidad(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    id: String,
    instancia: String,
    paquete: Option<String>,
    tarea: Option<String>,
) -> Result<String, String> {
    let paths = state.paths.clone();
    let instance = crate::instances::find(&paths, &instancia).ok_or(t!("No se encontró la instancia.", "The instance was not found."))?;
    if state.games.lock().unwrap().contains_key(&instance.id) {
        return Err(t!("Cierra el juego antes de publicar la instancia.", "Close the game before publishing the instance."));
    }
    let publisher = crate::drive_publisher(&state)
        .await?
        .ok_or(t!("Para subirla hace falta conectar Drive: Ajustes › Acerca de › Conectar Drive.", "To upload it you need to connect Drive: Settings › About › Connect Drive."))?;
    let out = crate::remembered_export_folder(&paths).unwrap_or_else(|| paths.root.join(crate::catalog::LOCAL_FOLDER));
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let job = crate::descargas::start(tarea, "drive", t!("Publicar {} como paquete oficial", "Publish {} as the official package", instance.name));
    let progress = job.progress("drive");
    job.run(async {
        // Lo que se exporta normalmente (mods, configuración, opciones…), sin mundos.
        let include = crate::packs::exportable(&instance).into_iter().filter(|e| e.checked).map(|e| e.name).collect();
        // Los mods que están en Modrinth: con ellos sale también el paquete ligero (catalog::install_lite).
        progress(t!("Buscando qué mods están en Modrinth…", "Looking for which mods are on Modrinth…"), 0.0);
        let mut options = crate::packs::ExportOptions { include, skip_base: true, ..Default::default() };
        crate::packs::find_remote(&instance, &paths, &mut options).await;
        let (exported, report) = (instance.clone(), progress.clone());
        let paths_copy = paths.clone();
        let out_copy = out.clone();
        let result = tauri::async_runtime::spawn_blocking(move || crate::packs::export(&exported, &paths_copy, &out_copy, &options, move |stage, f| report(stage, f * 0.4)))
            .await
            .map_err(|e| e.to_string())??;
        let modpack = std::path::PathBuf::from(&result.modpack);
        let manifest = crate::packs::read_manifest(&modpack)?.ok_or(t!("El paquete no quedó bien.", "The package was not created correctly."))?;
        let files = crate::drive::files_in(modpack.parent().unwrap_or(&out));
        let report = progress.clone();
        publisher.publish(&out, &crate::upload_order(files), move |stage, f| report(stage, 0.4 + f * 0.6)).await?;
        // El catálogo ya trae la versión nueva.
        crate::catalog_snapshot(&state, true).await;
        let package = Package {
            id: paquete.unwrap_or_default(),
            name: instance.name.clone(),
            catalog_id: manifest.id.clone(),
            version: manifest.version.clone(),
            minecraft: instance.minecraft_version.clone(),
            loader: instance.loader.clone(),
            loader_version: instance.loader_version.clone().unwrap_or_default(),
            at: 0,
        };
        guardar_paquete_comunidad(app.clone(), id.clone(), package).await
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigos_e_ids() {
        let code = random("", 8, CODE_LETTERS);
        assert_eq!(code.len(), 8);
        assert!(clean_code(&code).is_ok());
        assert_eq!(clean_code(" abcd-efgh ").unwrap(), "ABCDEFGH");
        assert!(clean_code("ABCD1FGH").is_err()); // el 1 no se usa
        assert!(clean_code("corto").is_err());
        let cid = id("c", 12);
        assert!(valid_id('c', 12, &cid));
        assert!(!valid_id('c', 12, "c12"));
        assert!(!valid_id('v', 8, "v1234567X"));
    }
}
