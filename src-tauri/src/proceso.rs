//! Procesos que Parabellum abre en segundo plano (el servidor de Minecraft y el agente de playit.gg):
//! van atados a Parabellum para no quedar abiertos sin consola si se cierra de golpe, y se mide
//! cuánta memoria y procesador usan.

use std::sync::OnceLock;

use crate::plataforma;

/// El proceso se cierra solo si Parabellum termina sin cerrarlo antes.
pub fn tie_to_launcher(child: &std::process::Child) {
    plataforma::tie_to_launcher(child);
}

/// Memoria en uso (bytes) y tiempo de procesador acumulado (en unidades de 100 ns) de un proceso.
pub fn usage(pid: u32) -> Option<(u64, u64)> {
    plataforma::process_usage(pid)
}

/// Memoria del PC: (total, disponible) en bytes.
pub fn system_memory() -> (u64, u64) {
    plataforma::system_memory()
}

/// Las tarjetas de video del PC (su nombre), leídas una vez.
pub fn graphics_cards() -> &'static [String] {
    static CARDS: OnceLock<Vec<String>> = OnceLock::new();
    CARDS.get_or_init(plataforma::graphics_cards)
}

/// El PC tiene gráficos Intel (con ellos se juega casi siempre en las laptops).
pub fn intel_graphics() -> bool {
    graphics_cards().iter().any(|card| card.to_lowercase().contains("intel"))
}

/// Núcleos del procesador (para pasar el tiempo de CPU a porcentaje del PC).
pub fn cores() -> u32 {
    std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(1)
}

/// Mide el uso de procesador entre dos lecturas.
#[derive(Default)]
pub struct CpuMeter {
    last: Option<(std::time::Instant, u64)>,
}

impl CpuMeter {
    /// Porcentaje del PC entero (0–100) desde la lectura anterior.
    pub fn sample(&mut self, cpu_ticks: u64) -> f32 {
        let now = std::time::Instant::now();
        let percent = match self.last {
            Some((at, ticks)) => {
                let elapsed = now.duration_since(at).as_secs_f64() * 10_000_000.0;
                if elapsed <= 0.0 {
                    0.0
                } else {
                    (cpu_ticks.saturating_sub(ticks) as f64 / elapsed / cores() as f64 * 100.0).clamp(0.0, 100.0) as f32
                }
            }
            None => 0.0,
        };
        self.last = Some((now, cpu_ticks));
        percent
    }
}
