//! Procesos que Parabellum abre en segundo plano (el servidor de Minecraft y el agente de playit.gg):
//! van atados a Parabellum para no quedar abiertos sin consola si se cierra de golpe, y se mide
//! cuánta memoria y procesador usan.

use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Un "job" de Windows que cierra sus procesos cuando Parabellum termina (aunque sea de golpe).
fn job() -> Option<isize> {
    static JOB: OnceLock<Option<isize>> = OnceLock::new();
    *JOB.get_or_init(|| {
        // SAFETY: llamadas a la API de Windows con estructuras inicializadas; el handle vive mientras
        // viva Parabellum (al cerrarse, Windows cierra el job y con él sus procesos).
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            (ok != 0).then_some(job as isize)
        }
    })
}

/// El proceso se cierra solo si Parabellum termina sin cerrarlo antes.
pub fn tie_to_launcher(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    if let Some(job) = job() {
        // SAFETY: el handle del proceso es válido mientras exista `child`.
        unsafe {
            AssignProcessToJobObject(job as HANDLE, child.as_raw_handle() as HANDLE);
        }
    }
}

/// Memoria en uso (bytes) y tiempo de procesador acumulado (en unidades de 100 ns) de un proceso.
pub fn usage(pid: u32) -> Option<(u64, u64)> {
    // SAFETY: el handle se abre y se cierra aquí; las estructuras van inicializadas a cero.
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut memory: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
        memory.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let memory_ok = K32GetProcessMemoryInfo(process, &mut memory, memory.cb) != 0;
        let (mut created, mut exited, mut kernel, mut user): (FILETIME, FILETIME, FILETIME, FILETIME) = std::mem::zeroed();
        let times_ok = GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0;
        CloseHandle(process);
        let ticks = |t: FILETIME| ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64;
        (memory_ok && times_ok).then(|| (memory.WorkingSetSize as u64, ticks(kernel) + ticks(user)))
    }
}

/// Memoria del PC: (total, disponible) en bytes.
pub fn system_memory() -> (u64, u64) {
    // SAFETY: estructura inicializada con su tamaño, como pide la API.
    unsafe {
        let mut status: MEMORYSTATUSEX = std::mem::zeroed();
        status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
        if GlobalMemoryStatusEx(&mut status) == 0 {
            return (0, 0);
        }
        (status.ullTotalPhys, status.ullAvailPhys)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mide_este_proceso() {
        // Windows cuenta el tiempo de procesador en saltos de ~15 ms: primero se trabaja un poco.
        let started = std::time::Instant::now();
        let mut n: u64 = 0;
        while started.elapsed() < std::time::Duration::from_millis(60) {
            n = std::hint::black_box(n.wrapping_mul(31).wrapping_add(7));
        }
        let (memory, cpu) = usage(std::process::id()).expect("uso del proceso");
        assert!(memory > 0 && cpu > 0);
        let (total, available) = system_memory();
        assert!(total > 0 && available <= total);
    }
}
