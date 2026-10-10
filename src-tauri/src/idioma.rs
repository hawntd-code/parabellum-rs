//! Idioma de Parabellum: español o inglés. El motor arma sus mensajes con
//! `t!("texto en español", "text in English", argumentos…)`, igual que `format!`.
//! La interfaz tiene su propia `t()` en ui/i18n.js.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// "es" o "en" (cualquier otro valor cuenta como español).
pub fn set(language: &str) {
    ENGLISH.store(language == "en", Ordering::Relaxed);
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// El idioma de Windows: español si Windows está en español; si no, inglés.
pub fn detect() -> &'static str {
    // SAFETY: función sin argumentos que solo lee la configuración del usuario.
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    if id & 0x3ff == 0x0a { "es" } else { "en" }
}

/// Valida lo que llega de la interfaz.
pub fn normalize(language: &str) -> String {
    if language == "en" { "en".into() } else { "es".into() }
}

/// Texto en el idioma elegido; acepta los mismos argumentos que `format!`.
macro_rules! t {
    ($es:literal, $en:literal $(,)?) => {
        if $crate::idioma::english() { format!($en) } else { format!($es) }
    };
    ($es:literal, $en:literal, $($args:tt)*) => {
        if $crate::idioma::english() { format!($en, $($args)*) } else { format!($es, $($args)*) }
    };
}
