//! Avisos de Windows (las notificaciones de la esquina): solicitud de amistad nueva, un amigo abrió
//! su mundo… Parabellum es portable (sin instalador), así que se registra solo para el usuario
//! (HKCU\Software\Classes\AppUserModelId) con su nombre e icono; así Windows muestra los avisos
//! como de "Parabellum".

use std::path::Path;

use windows::core::HSTRING;
use windows::Data::Xml::Dom::XmlDocument;
use windows::Foundation::TypedEventHandler;
use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};

pub const APP_ID: &str = "mx.parabellum.launcher";
const ICON: &[u8] = include_bytes!("../icons/icon.ico");

/// Registra la app para los avisos (nombre e icono). Se llama al arrancar; no hace falta instalar nada.
pub fn register(cache: &Path) {
    let icon = cache.join("parabellum.ico");
    if std::fs::read(&icon).ok().as_deref() != Some(ICON) {
        let _ = std::fs::create_dir_all(cache);
        let _ = std::fs::write(&icon, ICON);
    }
    let key = format!("Software\\Classes\\AppUserModelId\\{APP_ID}");
    set_value(&key, "DisplayName", "Parabellum");
    set_value(&key, "IconUri", &icon.display().to_string());
}

fn set_value(key: &str, name: &str, value: &str) {
    use windows_sys::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};
    let wide = |text: &str| text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let (key, name, data) = (wide(key), wide(name), wide(value));
    // SAFETY: cadenas UTF-16 terminadas en cero que viven durante la llamada; el tamaño va en bytes.
    unsafe {
        RegSetKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr(), REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32);
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

thread_local! {
    /// Los avisos se guardan un rato: así su clic llega mientras Parabellum sigue abierto.
    static RECENT: std::cell::RefCell<Vec<ToastNotification>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Muestra un aviso. `on_click` se llama si el jugador lo pulsa. Debe llamarse desde un hilo con
/// COM iniciado (el hilo principal de la ventana, o tras `init_thread`).
pub fn show(title: &str, body: &str, on_click: impl Fn() + Send + Sync + 'static) -> windows::core::Result<()> {
    let xml = XmlDocument::new()?;
    xml.LoadXml(&HSTRING::from(format!(
        "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
        escape(title),
        escape(body)
    )))?;
    let toast = ToastNotification::CreateToastNotification(&xml)?;
    toast.Activated(&TypedEventHandler::new(move |_, _| {
        on_click();
        Ok(())
    }))?;
    ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?.Show(&toast)?;
    RECENT.with(|recent| {
        let mut recent = recent.borrow_mut();
        recent.push(toast);
        let excess = recent.len().saturating_sub(10);
        recent.drain(..excess);
    });
    Ok(())
}

/// Para usar los avisos desde un hilo propio (modo sin ventana).
pub fn init_thread() {
    // SAFETY: inicia WinRT en este hilo; si ya estaba iniciado no pasa nada.
    unsafe {
        let _ = windows::Win32::System::WinRT::RoInitialize(windows::Win32::System::WinRT::RO_INIT_MULTITHREADED);
    }
}

/// Cuántos avisos de Parabellum hay en el centro de notificaciones (para comprobar que llegaron).
pub fn history_count() -> windows::core::Result<u32> {
    let history = ToastNotificationManager::History()?;
    history.GetHistoryWithId(&HSTRING::from(APP_ID))?.Size()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapa_el_texto_del_aviso() {
        assert_eq!(escape("A&B <x> \"y\""), "A&amp;B &lt;x&gt; &quot;y&quot;");
    }
}
