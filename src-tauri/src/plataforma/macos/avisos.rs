//! Avisos de Mac (los del Centro de Notificaciones) con la API de Apple, UserNotifications. Mismas
//! funciones que plataforma/windows/avisos.rs.
//!
//! macOS solo deja usarla a una .app (Parabellum.app; ver la tarea 10 de docs/mac.md): con el
//! programa suelto (cargo build), `show` dice por qué no sale el aviso, sin cerrar Parabellum. Y no
//! a cualquier .app: si está en una carpeta temporal (/private/tmp), macOS contesta «Notifications
//! are not allowed for this application». La primera vez pregunta si Parabellum puede mostrarlos.

use std::path::Path;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread};
use objc2_foundation::{NSArray, NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions, UNNotificationRequest,
    UNNotificationResponse, UNNotificationSound, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

type Click = Box<dyn Fn() + Send + Sync>;

/// Qué hacer al pulsar cada aviso (por su identificador). Se guardan los últimos, como en Windows.
static CLICKS: Mutex<Vec<(String, Click)>> = Mutex::new(Vec::new());

define_class!(
    // SAFETY: NSObject no pide nada a sus subclases y Delegate no implementa Drop.
    #[unsafe(super(NSObject))]
    #[name = "ParabellumAvisos"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        /// El jugador pulsó un aviso.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, completion: &DynBlock<dyn Fn()>) {
            let id = response.notification().request().identifier().to_string();
            let click = {
                let mut clicks = CLICKS.lock().unwrap();
                clicks.iter().position(|(saved, _)| *saved == id).map(|i| clicks.remove(i).1)
            };
            if let Some(click) = click {
                click();
            }
            completion.call(());
        }

        /// Con Parabellum al frente macOS no los muestra si no se le pide.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(&self, _center: &UNUserNotificationCenter, _notification: &UNNotification, completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
            completion.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List | UNNotificationPresentationOptions::Sound,));
        }
    }
);

impl Delegate {
    fn new() -> Retained<Self> {
        let this = Self::alloc().set_ivars(());
        // SAFETY: init de NSObject sobre un objeto recién reservado.
        unsafe { msg_send![super(this), init] }
    }
}

/// El centro de avisos, si Parabellum corre como .app (fuera de una, pedirlo cierra el programa).
fn center() -> Result<Retained<UNUserNotificationCenter>, String> {
    let bundle = NSBundle::mainBundle();
    if bundle.bundleIdentifier().is_none() || !bundle.bundlePath().to_string().ends_with(".app") {
        return Err(t!("En Mac los avisos solo salen desde Parabellum.app.", "On Mac, notices only show from Parabellum.app."));
    }
    let center = UNUserNotificationCenter::currentNotificationCenter();
    static DELEGATE: std::sync::Once = std::sync::Once::new();
    DELEGATE.call_once(|| {
        let delegate = Delegate::new();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // El centro no lo retiene: vive lo que viva Parabellum.
        std::mem::forget(delegate);
    });
    Ok(center)
}

/// Pide permiso para mostrar avisos (macOS pregunta solo la primera vez).
pub fn register(_cache: &Path) {
    let Ok(center) = center() else { return };
    let answer = RcBlock::new(|_granted: Bool, error: *mut NSError| report("sin permiso", error));
    center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &answer);
}

/// Muestra un aviso. `on_click` se llama si el jugador lo pulsa. `silent`: sin el sonido del sistema
/// (los mensajes tienen el suyo).
pub fn show(title: &str, body: &str, silent: bool, on_click: impl Fn() + Send + Sync + 'static) -> Result<(), String> {
    let center = center()?;
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    if !silent {
        content.setSound(Some(&UNNotificationSound::defaultSound()));
    }
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let id = format!("parabellum-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str(&id), &content, None);
    {
        let mut clicks = CLICKS.lock().unwrap();
        clicks.push((id, Box::new(on_click)));
        let excess = clicks.len().saturating_sub(10);
        clicks.drain(..excess);
    }
    // macOS contesta después: si no lo muestra (por ejemplo, sin permiso), queda en la consola.
    let added = RcBlock::new(|error: *mut NSError| report("no se mostró", error));
    center.addNotificationRequest_withCompletionHandler(&request, Some(&added));
    Ok(())
}

fn report(what: &str, error: *mut NSError) {
    // SAFETY: macOS pasa nulo o un NSError válido durante la llamada.
    if let Some(error) = unsafe { error.as_ref() } {
        eprintln!("avisos: {what} ({})", error.localizedDescription());
    }
}

/// Para usar los avisos desde un hilo propio (en Windows inicia WinRT; en Mac no hace falta).
pub fn init_thread() {}

/// Cuántos avisos de Parabellum hay en el Centro de Notificaciones (para comprobar que llegaron).
pub fn history_count() -> Result<u32, String> {
    let center = center()?;
    let (sender, receiver) = std::sync::mpsc::channel();
    let answer = RcBlock::new(move |list: NonNull<NSArray<UNNotification>>| {
        // SAFETY: macOS pasa una lista válida durante la llamada.
        let _ = sender.send(unsafe { list.as_ref() }.count() as u32);
    });
    center.getDeliveredNotificationsWithCompletionHandler(&answer);
    receiver.recv_timeout(Duration::from_secs(5)).map_err(|_| t!("macOS no respondió.", "macOS did not answer."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_app_dice_por_que_y_no_se_cae() {
        // `cargo test` corre fuera de una .app: pedir el centro de avisos cerraría el programa.
        assert!(show("Parabellum", "Prueba", true, || {}).is_err());
        assert!(history_count().is_err());
    }
}
