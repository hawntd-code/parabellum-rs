//! Servicio de ajustes (docs/plan-diciembre.md, sección 5): una sola forma de cambiar un ajuste, con
//! sus efectos. La ventana de Ajustes (`guardar_ajustes`), y después las tarjetas del soporte (etapa E3),
//! pasan por aquí: se valida en el motor, nunca solo en la interfaz.
//!
//! El registro dice qué ajustes se pueden compartir (mandar en una tarjeta): su tipo, sus valores
//! permitidos y su texto. Una operación `{ id, valor, esperado }` se aplica solo si el valor actual
//! sigue siendo el esperado; repetirla no cambia nada (si ya tiene ese valor, «ya estaba»).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::settings::Settings;
use crate::AppState;

/// Qué valores acepta un ajuste.
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Bool,
    /// Un número entero entre dos límites.
    Range(i64, i64),
    /// Uno de estos números.
    Choice(&'static [f64]),
}

/// Un ajuste que se puede compartir.
pub struct Shareable {
    pub id: &'static str,
    pub kind: Kind,
    /// Desde qué versión de Parabellum existe (una tarjeta para uno más nuevo se rechaza).
    pub since: &'static str,
    pub es: &'static str,
    pub en: &'static str,
    get: fn(&Settings) -> Value,
    set: fn(&mut Settings, &Value),
}

pub const REGISTRY: &[Shareable] = &[
    Shareable { id: "launcher.message_sound", kind: Kind::Bool, since: "0.33.0", es: "Sonido de mensajes", en: "Message sound", get: |s| json!(s.message_sound), set: |s, v| s.message_sound = v.as_bool().unwrap_or(s.message_sound) },
    Shareable { id: "launcher.notifications", kind: Kind::Bool, since: "0.33.0", es: "Avisos de amigos", en: "Friend notifications", get: |s| json!(s.notifications), set: |s, v| s.notifications = v.as_bool().unwrap_or(s.notifications) },
    Shareable { id: "launcher.ram_gb", kind: Kind::Range(1, 64), since: "0.33.0", es: "Memoria para el juego (GB)", en: "Game memory (GB)", get: |s| json!(s.ram_gb), set: |s, v| s.ram_gb = v.as_u64().map(|n| n as u32).unwrap_or(s.ram_gb) },
    Shareable {
        id: "launcher.download_limit_mb",
        kind: Kind::Choice(&[0.0, 1.0, 2.0, 5.0, 10.0, 20.0]),
        since: "0.33.0",
        es: "Límite de descarga (MB/s, 0 sin límite)",
        en: "Download limit (MB/s, 0 no limit)",
        get: |s| json!(s.download_limit_mb),
        set: |s, v| s.download_limit_mb = v.as_f64().unwrap_or(s.download_limit_mb),
    },
    Shareable { id: "launcher.download_parallel", kind: Kind::Range(1, 3), since: "0.33.0", es: "Descargas a la vez", en: "Simultaneous downloads", get: |s| json!(s.download_parallel), set: |s, v| s.download_parallel = v.as_u64().map(|n| n as u32).unwrap_or(s.download_parallel) },
    Shareable { id: "launcher.game_panel", kind: Kind::Bool, since: "0.33.0", es: "Parabellum dentro del juego", en: "Parabellum inside the game", get: |s| json!(s.game_panel), set: |s, v| s.game_panel = v.as_bool().unwrap_or(s.game_panel) },
    Shareable { id: "launcher.school_mode", kind: Kind::Bool, since: "0.33.0", es: "Modo escuela", en: "School mode", get: |s| json!(s.school_mode), set: |s, v| s.school_mode = v.as_bool().unwrap_or(s.school_mode) },
];

fn pick(es: &'static str, en: &'static str) -> &'static str {
    if crate::idioma::english() { en } else { es }
}

pub fn find(id: &str) -> Option<&'static Shareable> {
    REGISTRY.iter().find(|s| s.id == id)
}

/// ¿Es un valor permitido para ese ajuste? Devuelve el valor normalizado (p. ej. 6.0 → 6).
fn validate(def: &Shareable, value: &Value) -> Result<Value, String> {
    let invalid = || t!("Valor no permitido para «{}».", "Value not allowed for «{}».", pick(def.es, def.en));
    match def.kind {
        Kind::Bool => value.as_bool().map(Value::Bool).ok_or_else(invalid),
        Kind::Range(min, max) => {
            let n = value.as_f64().filter(|n| n.fract() == 0.0).map(|n| n as i64).ok_or_else(invalid)?;
            (min..=max).contains(&n).then(|| json!(n)).ok_or_else(invalid)
        }
        Kind::Choice(options) => {
            let n = value.as_f64().ok_or_else(invalid)?;
            options.iter().any(|o| (o - n).abs() < 1e-9).then(|| json!(n)).ok_or_else(invalid)
        }
    }
}

/// Valores iguales para el ajuste (6 y 6.0 son lo mismo).
fn same(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

/// Una operación sobre un ajuste. `id_operacion` identifica la tarjeta que la manda (para el registro).
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub id: String,
    pub value: Value,
    /// Si viene, solo se aplica si el valor actual es este.
    #[serde(default)]
    pub expected: Option<Value>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", tag = "result")]
pub enum Outcome {
    /// Se aplicó: este es el valor nuevo.
    Applied { value: Value },
    /// Ya tenía ese valor: no se cambió nada.
    Unchanged { value: Value },
    /// El valor actual ya no es el esperado (alguien lo cambió): no se aplicó.
    Conflict { current: Value },
}

/// Sin efectos: los ajustes que resultarían y qué pasa. Lo usa `apply` y las pruebas.
pub fn plan(settings: &Settings, op: &Operation) -> Result<(Settings, Outcome), String> {
    let def = find(&op.id).ok_or(t!("Ese ajuste no se puede cambiar así.", "That setting cannot be changed this way."))?;
    let value = validate(def, &op.value)?;
    let current = (def.get)(settings);
    if same(&current, &value) {
        return Ok((settings.clone(), Outcome::Unchanged { value: current }));
    }
    if let Some(expected) = &op.expected {
        if !same(&current, expected) {
            return Ok((settings.clone(), Outcome::Conflict { current }));
        }
    }
    let mut next = settings.clone();
    (def.set)(&mut next, &value);
    let value = (def.get)(&next);
    Ok((next, Outcome::Applied { value }))
}

pub fn read(app: &AppHandle, id: &str) -> Result<Value, String> {
    let def = find(id).ok_or(t!("Ajuste desconocido.", "Unknown setting."))?;
    Ok((def.get)(&app.state::<AppState>().settings.lock().unwrap()))
}

/// Aplica una operación con todos sus efectos (como si se cambiara en la ventana de Ajustes).
pub fn apply(app: &AppHandle, op: &Operation) -> Result<Outcome, String> {
    let settings = app.state::<AppState>().settings.lock().unwrap().clone();
    let (next, outcome) = plan(&settings, op)?;
    if matches!(outcome, Outcome::Applied { .. }) {
        let saved = save_all(app, next)?;
        let _ = tauri::Emitter::emit(app, "ajustes", &saved);
    }
    Ok(outcome)
}

/// Guarda todos los ajustes y aplica sus efectos: memoria, descargas, modo escuela, canal, idioma,
/// tamaño de la ventana. Es lo que hacía `guardar_ajustes`; ahora lo usan la ventana y las tarjetas.
pub fn save_all(app: &AppHandle, mut ajustes: Settings) -> Result<Settings, String> {
    let state = app.state::<AppState>();
    ajustes.normalize()?;
    {
        let current = state.settings.lock().unwrap();
        ajustes.favorite_chats = current.favorite_chats.clone();
        ajustes.favorite_mods = current.favorite_mods.clone();
        ajustes.favorites_owner = current.favorites_owner.clone();
    }
    ajustes.ram_gb = ajustes.ram_gb.clamp(1, 64);
    ajustes.ui_scale = if ajustes.ui_scale.is_finite() { ajustes.ui_scale.clamp(0.8, 1.5) } else { 1.0 };
    ajustes.language = crate::idioma::normalize(&ajustes.language);
    let language_changed = ajustes.language != state.settings.lock().unwrap().language;
    if (ajustes.ui_scale - state.settings.lock().unwrap().ui_scale).abs() > f64::EPSILON {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_zoom(ajustes.ui_scale);
        }
    }
    ajustes.save(&state.paths).map_err(|e| t!("No se pudieron guardar los ajustes: {e}", "Could not save the settings: {e}"))?;
    *state.settings.lock().unwrap() = ajustes.clone();
    crate::descargas::configure(ajustes.download_parallel, ajustes.download_limit_mb);
    crate::espejo::set_school_mode(ajustes.school_mode);
    crate::canal::set(&ajustes.update_channel);
    if language_changed {
        crate::idioma::set(&ajustes.language);
        // El menú del icono junto al reloj se vuelve a crear con los textos nuevos.
        crate::cuenta::rebuild_tray(app);
    }
    crate::cuenta::settings_changed(app);
    Ok(ajustes)
}

/// Lo que la interfaz (y las tarjetas) necesitan saber de cada ajuste compartible.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Described {
    pub id: &'static str,
    pub label: String,
    pub kind: &'static str,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub options: Vec<f64>,
    pub since: &'static str,
    pub value: Value,
}

pub fn describe(settings: &Settings) -> Vec<Described> {
    REGISTRY
        .iter()
        .map(|d| {
            let (kind, min, max, options) = match d.kind {
                Kind::Bool => ("bool", None, None, vec![]),
                Kind::Range(a, b) => ("range", Some(a), Some(b), vec![]),
                Kind::Choice(o) => ("choice", None, None, o.to_vec()),
            };
            Described { id: d.id, label: pick(d.es, d.en).to_string(), kind, min, max, options, since: d.since, value: (d.get)(settings) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: &str, value: Value, expected: Option<Value>) -> Operation {
        Operation { id: id.into(), value, expected }
    }

    #[test]
    fn aplica_valida_y_es_idempotente() {
        let base = Settings { ram_gb: 4, message_sound: true, download_limit_mb: 0.0, ..Default::default() };
        let (next, outcome) = plan(&base, &op("launcher.ram_gb", json!(6), None)).unwrap();
        assert_eq!(outcome, Outcome::Applied { value: json!(6) });
        assert_eq!(next.ram_gb, 6);
        // Repetirla no cambia nada.
        let (again, outcome) = plan(&next, &op("launcher.ram_gb", json!(6), Some(json!(4)))).unwrap();
        assert_eq!(outcome, Outcome::Unchanged { value: json!(6) });
        assert_eq!(again.ram_gb, 6);
        // Fuera de rango, de otro tipo o con un valor que no está en la lista: error.
        assert!(plan(&base, &op("launcher.ram_gb", json!(99), None)).is_err());
        assert!(plan(&base, &op("launcher.ram_gb", json!(2.5), None)).is_err());
        assert!(plan(&base, &op("launcher.message_sound", json!("sí"), None)).is_err());
        assert!(plan(&base, &op("launcher.download_limit_mb", json!(3), None)).is_err());
        assert!(plan(&base, &op("launcher.no_existe", json!(true), None)).is_err());
        let (next, _) = plan(&base, &op("launcher.download_limit_mb", json!(5), None)).unwrap();
        assert_eq!(next.download_limit_mb, 5.0);
    }

    #[test]
    fn no_pisa_un_cambio_ajeno() {
        let base = Settings { ram_gb: 8, ..Default::default() };
        // La tarjeta esperaba 4 (cuando se mandó), pero ya está en 8: no se aplica.
        let (next, outcome) = plan(&base, &op("launcher.ram_gb", json!(6), Some(json!(4)))).unwrap();
        assert_eq!(outcome, Outcome::Conflict { current: json!(8) });
        assert_eq!(next.ram_gb, 8);
    }

    #[test]
    fn el_registro_tiene_lo_del_plan() {
        for id in ["launcher.message_sound", "launcher.notifications", "launcher.ram_gb", "launcher.download_limit_mb", "launcher.download_parallel", "launcher.game_panel", "launcher.school_mode"] {
            assert!(find(id).is_some(), "{id}");
        }
        let described = describe(&Settings::default());
        assert_eq!(described.len(), REGISTRY.len());
    }
}
