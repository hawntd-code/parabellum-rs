//! Gestor de descargas: cada instalación (un modpack, mods, una versión, el catálogo de Drive…) es una
//! tarea con su progreso, su velocidad y un botón para cancelarla, y todas se ven juntas en el panel de
//! descargas (la flecha de arriba a la derecha).
//!
//! - Las instalaciones grandes (`Job::queued`) esperan su turno: por defecto va una a la vez, que así
//!   cada una termina antes. Los mods y lo que se baja al jugar no esperan.
//! - Cancelar no corta el código en cualquier punto: las descargas lo notan entre pedazos (`check` y
//!   `received`) y fallan como si se cayera la conexión, así cada instalación limpia lo suyo igual que
//!   siempre. Al reintentar se aprovecha lo que ya se bajó.
//! - La velocidad de todas las descargas juntas se puede limitar (para no dejar sin internet la casa).

use serde::Serialize;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// Lo que devuelve una tarea cancelada (la interfaz no lo muestra como error).
pub fn cancelled_error() -> String {
    t!("Descarga cancelada.", "Download cancelled.")
}

/// Cuántas terminadas se recuerdan en el panel.
const HISTORY: usize = 30;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    /// "modpack", "contenido", "version", "catalogo", "compartida", "importar", "verificar" o "jugar".
    pub kind: &'static str,
    pub title: String,
    /// La etapa en curso («Descargando librerías… 12 de 80»).
    pub detail: String,
    pub fraction: f64,
    /// "cola", "activa", "lista", "error" o "cancelada".
    pub status: &'static str,
    pub error: Option<String>,
    /// Bytes por segundo (de los últimos segundos).
    pub speed: u64,
    /// Bytes que bajó esta tarea.
    pub bytes: u64,
    /// La instancia que creó o en la que instala (para «Abrir»).
    pub instance: Option<String>,
    pub icon: Option<String>,
    /// Milisegundos desde 1970.
    pub started: u64,
    pub ended: Option<u64>,
    /// Para «Reintentar»: { command, args } (el mismo comando de la interfaz).
    pub retry: Option<serde_json::Value>,
    /// Espera su turno con las demás grandes.
    #[serde(skip)]
    queued: bool,
    /// Solo sale en el panel si de verdad baja algo (p. ej. lo que falta antes de jugar).
    #[serde(skip)]
    quiet: bool,
}

/// Lo que comparten la tarea y sus descargas (las que corren dentro de `Job::run`).
struct Live {
    cancel: AtomicBool,
    bytes: AtomicU64,
}

struct Slot {
    task: Task,
    live: Arc<Live>,
    /// Para la velocidad: (cuándo, bytes en ese momento).
    sample: (Instant, u64),
}

struct Manager {
    app: AppHandle,
    slots: Mutex<Vec<Slot>>,
    dirty: AtomicBool,
    parallel: AtomicUsize,
}

static MANAGER: OnceLock<Manager> = OnceLock::new();
/// Bytes por segundo para todas las descargas juntas (0 = sin límite).
static LIMIT: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

tokio::task_local! {
    static CURRENT: Arc<Live>;
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Arranca el gestor (una vez, al abrir la ventana): avisa a la interfaz de los cambios y mide la velocidad.
pub fn init(app: &AppHandle, parallel: u32, limit_mb: f64) {
    configure(parallel, limit_mb);
    let manager = Manager { app: app.clone(), slots: Mutex::default(), dirty: AtomicBool::new(false), parallel: AtomicUsize::new(parallel.clamp(1, 3) as usize) };
    if MANAGER.set(manager).is_err() {
        return;
    }
    tauri::async_runtime::spawn(async {
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            tick();
        }
    });
}

/// Descargas simultáneas (de las grandes) y límite de velocidad en MB/s (0 = sin límite).
pub fn configure(parallel: u32, limit_mb: f64) {
    if let Some(manager) = MANAGER.get() {
        manager.parallel.store(parallel.clamp(1, 3) as usize, Ordering::Relaxed);
    }
    let limit = if limit_mb.is_finite() && limit_mb > 0.0 { (limit_mb * 1048576.0) as u64 } else { 0 };
    LIMIT.store(limit, Ordering::Relaxed);
}

fn visible(slot: &Slot) -> bool {
    !slot.task.quiet || slot.task.bytes > 0 || slot.task.status == "error"
}

/// Las tareas para el panel (las más nuevas primero).
pub fn list() -> Vec<Task> {
    let Some(manager) = MANAGER.get() else { return Vec::new() };
    let slots = manager.slots.lock().unwrap();
    slots.iter().rev().filter(|s| visible(s)).map(|s| s.task.clone()).collect()
}

fn emit_now() {
    if let Some(manager) = MANAGER.get() {
        manager.dirty.store(false, Ordering::Relaxed);
        let _ = manager.app.emit("descargas", list());
    }
}

fn tick() {
    let Some(manager) = MANAGER.get() else { return };
    {
        let mut slots = manager.slots.lock().unwrap();
        let now = Instant::now();
        for slot in slots.iter_mut().filter(|s| s.task.status == "activa") {
            let bytes = slot.live.bytes.load(Ordering::Relaxed);
            if bytes != slot.task.bytes {
                slot.task.bytes = bytes;
                manager.dirty.store(true, Ordering::Relaxed);
            }
            let elapsed = now.duration_since(slot.sample.0).as_secs_f64();
            if elapsed >= 0.9 {
                let rate = (bytes - slot.sample.1) as f64 / elapsed;
                // Suavizada, para que no salte con cada pedazo.
                let speed = if slot.task.speed == 0 { rate } else { slot.task.speed as f64 * 0.4 + rate * 0.6 };
                let speed = if rate == 0.0 && speed < 1024.0 { 0 } else { speed as u64 };
                if speed != slot.task.speed {
                    slot.task.speed = speed;
                    manager.dirty.store(true, Ordering::Relaxed);
                }
                slot.sample = (now, bytes);
            }
        }
    }
    if manager.dirty.load(Ordering::Relaxed) {
        emit_now();
    }
}

fn with_slot(id: &str, change: impl FnOnce(&mut Slot)) {
    let Some(manager) = MANAGER.get() else { return };
    let mut slots = manager.slots.lock().unwrap();
    if let Some(slot) = slots.iter_mut().find(|s| s.task.id == id) {
        change(slot);
        manager.dirty.store(true, Ordering::Relaxed);
    }
}

/// Cancela una tarea (en la cola sale de inmediato; en marcha, en el siguiente pedazo que llegue).
pub fn cancel(id: &str) -> bool {
    let Some(manager) = MANAGER.get() else { return false };
    let slots = manager.slots.lock().unwrap();
    match slots.iter().find(|s| s.task.id == id && matches!(s.task.status, "cola" | "activa")) {
        Some(slot) => {
            slot.live.cancel.store(true, Ordering::Relaxed);
            true
        }
        None => false,
    }
}

/// Quita del panel una terminada; sin id, todas las terminadas.
pub fn remove(id: Option<&str>) {
    let Some(manager) = MANAGER.get() else { return };
    manager.slots.lock().unwrap().retain(|s| matches!(s.task.status, "cola" | "activa") || id.is_some_and(|id| s.task.id != id));
    emit_now();
}

/// Una tarea del gestor. Se crea en la cola; `run` la hace esperar su turno (si es grande) y la ejecuta.
/// Si se suelta sin haber corrido (falló antes), sale del panel.
pub struct Job {
    id: String,
    live: Arc<Live>,
    done: AtomicBool,
    ran: AtomicBool,
}

/// Registra una tarea. `id`: el que eligió la interfaz para seguirla (si no, uno nuevo).
pub fn start(id: Option<String>, kind: &'static str, title: impl Into<String>) -> Job {
    let id = id
        .filter(|id| !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
        .unwrap_or_else(|| format!("d{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    let live = Arc::new(Live { cancel: AtomicBool::new(false), bytes: AtomicU64::new(0) });
    if let Some(manager) = MANAGER.get() {
        let mut slots = manager.slots.lock().unwrap();
        // Un id repetido (un reintento con el mismo) reemplaza a la vieja si ya terminó.
        slots.retain(|s| s.task.id != id || matches!(s.task.status, "cola" | "activa"));
        let task = Task {
            id: id.clone(),
            kind,
            title: title.into(),
            detail: t!("En espera…", "Waiting…"),
            fraction: 0.0,
            status: "cola",
            error: None,
            speed: 0,
            bytes: 0,
            instance: None,
            icon: None,
            started: now_ms(),
            ended: None,
            retry: None,
            queued: false,
            quiet: false,
        };
        slots.push(Slot { task, live: live.clone(), sample: (Instant::now(), 0) });
        // Las terminadas más viejas se olvidan.
        let finished = slots.iter().filter(|s| !matches!(s.task.status, "cola" | "activa")).count();
        if finished > HISTORY {
            let mut extra = finished - HISTORY;
            slots.retain(|s| {
                let drop = extra > 0 && !matches!(s.task.status, "cola" | "activa");
                if drop {
                    extra -= 1;
                }
                !drop
            });
        }
        manager.dirty.store(true, Ordering::Relaxed);
    }
    Job { id, live, done: AtomicBool::new(false), ran: AtomicBool::new(false) }
}

impl Job {
    /// Espera su turno con las demás grandes (modpacks, versiones, el catálogo).
    pub fn queued(self) -> Self {
        with_slot(&self.id, |s| s.task.queued = true);
        self
    }

    /// Solo sale en el panel si baja algo.
    pub fn quiet(self) -> Self {
        with_slot(&self.id, |s| s.task.quiet = true);
        self
    }

    pub fn icon(self, icon: Option<String>) -> Self {
        with_slot(&self.id, |s| s.task.icon = icon);
        self
    }

    pub fn instance(&self, instance: Option<String>) {
        with_slot(&self.id, |s| s.task.instance = instance);
    }

    pub fn title(&self, title: impl Into<String>) {
        let title = title.into();
        with_slot(&self.id, |s| s.task.title = title);
    }

    /// «Reintentar» vuelve a llamar a este comando con estos argumentos.
    pub fn retry(self, command: &str, args: serde_json::Value) -> Self {
        let value = serde_json::json!({ "command": command, "args": args });
        with_slot(&self.id, |s| s.task.retry = Some(value));
        self
    }

    /// El aviso de progreso de siempre ("progreso", con el id de la tarea) y el del panel. A la ventana
    /// llegan como mucho unos 10 por segundo: bajar Minecraft llama esto por cada uno de sus miles de
    /// archivos y, en una PC lenta, repintar tanto trababa la ventana. Una etapa nueva (otro texto antes
    /// de «…») y el final se avisan siempre.
    pub fn progress(&self, task: &'static str) -> impl Fn(String, f64) + Send + Sync + Clone + 'static {
        let id = self.id.clone();
        let last: Arc<Mutex<(Option<Instant>, String)>> = Arc::default();
        move |stage: String, fraction: f64| {
            let send = {
                let mut last = last.lock().unwrap();
                let kind = stage.split('…').next().unwrap_or_default().to_string();
                let due = last.0.is_none_or(|at| at.elapsed() >= Duration::from_millis(100)) || fraction >= 1.0 || kind != last.1;
                if due {
                    *last = (Some(Instant::now()), kind);
                }
                due
            };
            if send {
                if let Some(manager) = MANAGER.get() {
                    let _ = manager.app.emit("progreso", serde_json::json!({ "task": task, "stage": stage, "fraction": fraction, "job": id }));
                }
            }
            let fraction = if fraction.is_finite() { fraction.clamp(0.0, 1.0) } else { 0.0 };
            with_slot(&id, |s| {
                s.task.detail = stage;
                s.task.fraction = fraction;
            });
        }
    }

    fn cancelled(&self) -> bool {
        self.live.cancel.load(Ordering::Relaxed)
    }

    async fn wait_turn(&self) -> Result<(), String> {
        let Some(manager) = MANAGER.get() else { return Ok(()) };
        loop {
            if self.cancelled() {
                return Err(cancelled_error());
            }
            {
                let mut slots = manager.slots.lock().unwrap();
                let running = slots.iter().filter(|s| s.task.queued && s.task.status == "activa").count();
                let first = slots.iter().find(|s| s.task.queued && s.task.status == "cola").map(|s| s.task.id.clone());
                if running < manager.parallel.load(Ordering::Relaxed) && first.as_deref() == Some(&self.id) {
                    if let Some(slot) = slots.iter_mut().find(|s| s.task.id == self.id) {
                        slot.task.status = "activa";
                    }
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    fn finish(&self, result: &Result<(), String>) {
        self.done.store(true, Ordering::Relaxed);
        let cancelled = self.cancelled();
        with_slot(&self.id, |s| {
            s.task.ended = Some(now_ms());
            s.task.speed = 0;
            s.task.bytes = s.live.bytes.load(Ordering::Relaxed);
            match result {
                Ok(()) => {
                    s.task.status = "lista";
                    s.task.fraction = 1.0;
                    s.task.detail = String::new();
                }
                Err(_) if cancelled => s.task.status = "cancelada",
                Err(error) => {
                    s.task.status = "error";
                    s.task.error = Some(error.clone());
                }
            }
        });
        // Una silenciosa que no bajó nada no se queda en el panel.
        if let Some(manager) = MANAGER.get() {
            manager.slots.lock().unwrap().retain(|s| !(s.task.id == self.id && s.task.quiet && s.task.bytes == 0 && s.task.status == "lista"));
        }
        emit_now();
    }

    /// Espera su turno (si es grande) y ejecuta el trabajo; las descargas de adentro cuentan para esta
    /// tarea y notan si se cancela. Si se canceló, devuelve `cancelled_error()` (aunque el trabajo no lo
    /// diga: p. ej. una verificación que anota lo que no pudo bajar).
    pub async fn run<T>(&self, work: impl Future<Output = Result<T, String>>) -> Result<T, String> {
        self.ran.store(true, Ordering::Relaxed);
        let queued = MANAGER.get().is_some_and(|m| m.slots.lock().unwrap().iter().any(|s| s.task.id == self.id && s.task.queued));
        if queued {
            if let Err(error) = self.wait_turn().await {
                self.finish(&Err(error.clone()));
                return Err(error);
            }
        } else {
            with_slot(&self.id, |s| s.task.status = "activa");
        }
        with_slot(&self.id, |s| {
            s.task.started = now_ms();
            s.task.detail = t!("Empezando…", "Starting…");
            s.sample = (Instant::now(), 0);
        });
        emit_now();
        let result = CURRENT.scope(self.live.clone(), work).await;
        let result = if self.cancelled() { Err(cancelled_error()) } else { result };
        self.finish(&result.as_ref().map(|_| ()).map_err(Clone::clone));
        result
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        if self.done.load(Ordering::Relaxed) {
            return;
        }
        if self.ran.load(Ordering::Relaxed) {
            // Se dejó a medias (p. ej. se cerró la ventana): que no quede «activa» para siempre.
            self.live.cancel.store(true, Ordering::Relaxed);
            self.finish(&Err(cancelled_error()));
        } else if let Some(manager) = MANAGER.get() {
            // Nunca empezó (falló antes de `run`): no hay nada que mostrar.
            manager.slots.lock().unwrap().retain(|s| s.task.id != self.id);
            emit_now();
        }
    }
}

// ================= Lo que usan las descargas

/// La tarea en la que corre un código, para pasarla a un hilo (`current` y `enter`).
#[derive(Clone)]
pub struct Scope(Option<Arc<Live>>);

/// La tarea en la que corre este código.
pub fn current() -> Scope {
    Scope(CURRENT.try_with(Arc::clone).ok())
}

/// Corre `work` (en un hilo aparte, p. ej. en `spawn_blocking`) como parte de la tarea `scope`.
pub fn enter<R>(scope: Scope, work: impl FnOnce() -> R) -> R {
    match scope.0 {
        Some(live) => CURRENT.sync_scope(live, work),
        None => work(),
    }
}

/// ¿Se canceló la tarea en la que corre este código?
pub fn is_cancelled() -> bool {
    CURRENT.try_with(|live| live.cancel.load(Ordering::Relaxed)).unwrap_or(false)
}

/// Error si se canceló la tarea en la que corre este código.
pub fn check() -> Result<(), String> {
    if is_cancelled() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

/// Cuánto esperar después de recibir `n` bytes para no pasar del límite (todas las descargas juntas
/// se reparten el límite: cada pedazo «reserva» su tiempo).
fn throttle(n: usize) -> Duration {
    let limit = LIMIT.load(Ordering::Relaxed);
    if limit == 0 {
        return Duration::ZERO;
    }
    static NEXT: Mutex<Option<Instant>> = Mutex::new(None);
    let mut next = NEXT.lock().unwrap();
    let now = Instant::now();
    let start = next.filter(|t| *t > now).unwrap_or(now);
    let end = start + Duration::from_secs_f64(n as f64 / limit as f64);
    *next = Some(end);
    end.saturating_duration_since(now)
}

/// Llegaron `n` bytes: cuentan para el límite de velocidad y para la velocidad de la tarea (después de
/// esperar su turno, para que la velocidad que se ve no pase del límite).
pub async fn received(n: usize) {
    let wait = throttle(n);
    if !wait.is_zero() {
        tokio::time::sleep(wait).await;
    }
    let _ = CURRENT.try_with(|live| live.bytes.fetch_add(n as u64, Ordering::Relaxed));
}

/// Lo mismo que `received`, para el código que corre en un hilo (no async).
pub fn received_blocking(n: usize) {
    let wait = throttle(n);
    if !wait.is_zero() {
        std::thread::sleep(wait);
    }
    let _ = CURRENT.try_with(|live| live.bytes.fetch_add(n as u64, Ordering::Relaxed));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limite_de_velocidad() {
        configure(1, 1.0);
        // 1 MB/s: el primer pedazo de 512 KB espera ~0,5 s y el siguiente ~1 s (se acumulan).
        let first = throttle(512 * 1024);
        let second = throttle(512 * 1024);
        assert!(first > Duration::from_millis(400) && first <= Duration::from_millis(510), "{first:?}");
        assert!(second > Duration::from_millis(900) && second <= Duration::from_millis(1010), "{second:?}");
        configure(1, 0.0);
        assert_eq!(throttle(10 * 1048576), Duration::ZERO);
    }

    #[test]
    fn cancelar_dentro_de_la_tarea() {
        let live = Arc::new(Live { cancel: AtomicBool::new(false), bytes: AtomicU64::new(0) });
        assert!(check().is_ok(), "fuera de una tarea nunca está cancelada");
        CURRENT.sync_scope(live.clone(), || {
            assert!(check().is_ok());
            live.cancel.store(true, Ordering::Relaxed);
            assert_eq!(check(), Err(cancelled_error()));
        });
        // Pasada a un hilo con `current`/`enter` sigue siendo la misma tarea.
        let scope = CURRENT.sync_scope(live.clone(), current);
        let seen = std::thread::spawn(move || enter(scope, is_cancelled)).join().unwrap();
        assert!(seen);
    }
}
