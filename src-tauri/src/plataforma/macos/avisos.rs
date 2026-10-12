//! Avisos del sistema en Mac (las notificaciones de la esquina). Mismas funciones que
//! plataforma/windows/avisos.rs.
//!
//! TODO(mac): mostrarlos con el Centro de Notificaciones. Lo más directo es el plugin oficial
//! tauri-plugin-notification (sirve en Mac y en Windows), o el crate mac-notification-sys. Mientras
//! tanto no se muestran (el sonido de mensaje nuevo sí suena: ver sonido.rs).

use std::path::Path;

/// Nada que registrar en Mac: el sistema toma el nombre y el icono de la .app.
pub fn register(_cache: &Path) {}

/// Muestra un aviso. `on_click` se llama si el jugador lo pulsa. `silent`: sin sonido del sistema.
pub fn show(_title: &str, _body: &str, _silent: bool, _on_click: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
    Ok(())
}

/// Para usar los avisos desde un hilo propio (en Windows inicia WinRT; en Mac no hace falta).
pub fn init_thread() {}

/// Cuántos avisos hay en el centro de notificaciones (solo para la prueba --probar-aviso).
pub fn history_count() -> Result<u32, String> {
    Err("TODO(mac)".into())
}
