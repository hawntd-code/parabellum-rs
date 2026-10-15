//! Windows: lo que Parabellum necesita del sistema (ver el contrato en plataforma/mod.rs).

pub mod accesos;
pub mod arrastre;
pub mod avisos;
pub mod lanzador;

use std::os::windows::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::OnceLock;

use super::AgentDownload;

pub const NAME: &str = "Windows";
pub const MOJANG_OS: &str = "windows";
pub const JAVA_RUNTIME_PLATFORM: &str = if cfg!(target_arch = "aarch64") { "windows-arm64" } else { "windows-x64" };
pub const EXECUTABLE: &str = "Parabellum.exe";
pub const SELF_UPDATE: bool = true;
pub const CLASSPATH_SEPARATOR: &str = ";";
pub const FORGE_SERVER_ARGS: &str = "win_args.txt";
pub const PLAYIT_AGENT: Option<AgentDownload> = Some(AgentDownload {
    url: "https://github.com/playit-cloud/playit-agent/releases/download/v0.17.1/playit-windows-x86_64-signed.exe",
    sha256: "9b00d6ff7d37d1052e5ae097e1348e11deae8617cd7a8ba39d1777f2006316a3",
    size: 5_094_152,
    file_name: "playit-0.17.1.exe",
});

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Carpeta portable: todo cuelga de la carpeta del .exe (instancias, base, datos…).
pub fn data_root(exe: &Path) -> PathBuf {
    exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

// ================= Java

/// javaw.exe abre el juego sin ventana de consola; java.exe para todo lo demás.
pub fn java_binary(home: &Path, windowed: bool) -> PathBuf {
    let javaw = home.join("bin").join("javaw.exe");
    if windowed && javaw.exists() {
        javaw
    } else {
        home.join("bin").join("java.exe")
    }
}

/// En Windows un .exe se puede abrir tal como se descarga.
pub fn make_executable(_path: &Path) {}

/// Mojang tiene todos sus Java para Windows.
pub fn java_runtime_platform(_major: u32) -> &'static str {
    JAVA_RUNTIME_PLATFORM
}

/// En Windows las bibliotecas de Mojang sirven tal cual.
pub fn adapt_libraries(libraries: Vec<serde_json::Value>, _java_major: u32) -> Vec<serde_json::Value> {
    libraries
}

/// En Windows no hay Java ajeno (ver java_runtime_platform).
pub fn foreign_java_ready() -> bool {
    true
}

pub fn install_foreign_java() -> Result<(), String> {
    Ok(())
}

// ================= Procesos

pub trait CommandExt {
    /// Sin ventana negra de consola.
    fn no_console(&mut self) -> &mut Self;
    /// Independiente de Parabellum (el juego sigue abierto aunque se cierre el launcher).
    fn detached(&mut self) -> &mut Self;
}

impl CommandExt for Command {
    fn no_console(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW)
    }

    fn detached(&mut self) -> &mut Self {
        self.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
    }
}

/// Un "job" de Windows que cierra sus procesos cuando Parabellum termina (aunque sea de golpe).
fn job() -> Option<isize> {
    use windows_sys::Win32::System::JobObjects::{
        CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
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
pub fn tie_to_launcher(child: &Child) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;
    if let Some(job) = job() {
        // SAFETY: el handle del proceso es válido mientras exista `child`.
        unsafe {
            AssignProcessToJobObject(job as HANDLE, child.as_raw_handle() as HANDLE);
        }
    }
}

/// Memoria en uso (bytes) y tiempo de procesador acumulado (en unidades de 100 ns) de un proceso.
pub fn process_usage(pid: u32) -> Option<(u64, u64)> {
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
    use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
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

/// Cierra el proceso y los que haya abierto.
pub fn kill_tree(pid: u32) {
    let _ = Command::new("taskkill").args(["/PID", &pid.to_string(), "/T", "/F"]).no_console().output();
}

/// Si ese número de proceso es un Java (Windows reutiliza los números). tasklist evita depender de
/// más APIs; se llama como mucho cada pocos segundos. Su línea es "javaw.exe","19476","Console",…
pub fn is_java_process(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .no_console()
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().any(|line| line.contains(&format!("\"{pid}\"")) && line.to_lowercase().starts_with("\"java")))
        .unwrap_or(false)
}

/// (pid, línea de comandos) de los Java abiertos.
pub fn java_command_lines() -> Vec<(u32, String)> {
    let script = "Get-CimInstance Win32_Process -Filter \"Name='javaw.exe' or Name='java.exe'\" | ForEach-Object { \"$($_.ProcessId)`t$($_.CommandLine)\" }";
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .no_console()
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    let (pid, command) = line.split_once('\t')?;
                    Some((pid.trim().parse().ok()?, command.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

// ================= Firewall (conexión directa a los servidores)

const FIREWALL_RULE: &str = "Parabellum (servidores de Minecraft)";
/// Los puertos que usan los servidores de Parabellum (el 25565 y los que se buscan si está ocupado).
const SERVER_PORTS: &str = "25565-25600";

/// El firewall de Windows ya deja entrar conexiones a los servidores (la regla de Parabellum existe).
pub fn firewall_open() -> bool {
    Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={FIREWALL_RULE}")])
        .no_console()
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Agrega la regla (Windows pide permiso de administrador una vez).
pub fn open_firewall() -> Result<(), String> {
    let arguments = format!("advfirewall firewall add rule name=\"{FIREWALL_RULE}\" dir=in action=allow protocol=TCP localport={SERVER_PORTS} profile=any");
    let script = format!("Start-Process netsh -ArgumentList '{arguments}' -Verb RunAs -WindowStyle Hidden -Wait");
    let status = Command::new("powershell").args(["-NoProfile", "-NonInteractive", "-Command", &script]).no_console().status().map_err(|e| e.to_string())?;
    if firewall_open() {
        Ok(())
    } else if status.success() {
        Err(t!("Windows no agregó el permiso. Vuelve a intentarlo y acepta el aviso de administrador.", "Windows did not add the permission. Try again and accept the administrator prompt."))
    } else {
        Err(t!("No se dio permiso de administrador.", "Administrator permission was not given."))
    }
}

// ================= Argumentos de Java

/// El texto en la codificación de Windows (ANSI), o None si tiene letras que no caben en ella.
pub fn ansi(text: &str) -> Option<Vec<u8>> {
    use windows_sys::Win32::Globalization::{WideCharToMultiByte, CP_ACP, WC_NO_BEST_FIT_CHARS};
    let wide: Vec<u16> = text.encode_utf16().collect();
    if wide.is_empty() {
        return Some(Vec::new());
    }
    let mut used_default = 0;
    // SAFETY: búferes válidos con su tamaño; la primera llamada solo mide.
    unsafe {
        let size = WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS, wide.as_ptr(), wide.len() as i32, std::ptr::null_mut(), 0, std::ptr::null(), &mut used_default);
        if size <= 0 || used_default != 0 {
            return None;
        }
        let mut out = vec![0u8; size as usize];
        let written = WideCharToMultiByte(CP_ACP, WC_NO_BEST_FIT_CHARS, wide.as_ptr(), wide.len() as i32, out.as_mut_ptr(), size, std::ptr::null(), &mut used_default);
        (written == size && used_default == 0).then_some(out)
    }
}

/// El nombre corto (8.3, solo letras inglesas) de una ruta que existe.
fn short_path(path: &str) -> Option<String> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetShortPathNameW;
    let long = HSTRING::from(path);
    let mut buffer = vec![0u16; 1024];
    // SAFETY: el búfer tiene el tamaño que se indica.
    let length = unsafe { GetShortPathNameW(&long, Some(&mut buffer)) } as usize;
    (length > 0 && length < buffer.len()).then(|| String::from_utf16_lossy(&buffer[..length]))
}

/// Un argumento que Java pueda leer: Java lee los argumentos (y el archivo @argumentos) en ANSI, no
/// en UTF-8; con UTF-8, «Prominence™» o «José» llegaban rotos y el juego abría otra carpeta. Si tiene
/// letras que no caben (una carpeta con nombre en chino en un Windows en español), sus rutas se
/// cambian por su nombre corto.
pub fn java_argument(argument: &str) -> Result<String, String> {
    if ansi(argument).is_some() {
        return Ok(argument.to_string());
    }
    let shorten = |piece: &str| if Path::new(piece).exists() { short_path(piece).unwrap_or_else(|| piece.to_string()) } else { piece.to_string() };
    let rebuilt = match argument.split_once('=') {
        Some((key, value)) if key.starts_with('-') => format!("{key}={}", value.split(';').map(shorten).collect::<Vec<_>>().join(";")),
        _ => argument.split(';').map(shorten).collect::<Vec<_>>().join(";"),
    };
    if ansi(&rebuilt).is_some() {
        return Ok(rebuilt);
    }
    Err(t!(
        "Java no puede abrir el juego desde una carpeta con letras como las de «{}». Cambia el nombre de la instancia o mueve Parabellum a otra carpeta.",
        "Java cannot open the game from a folder with characters like those in «{}». Rename the instance or move Parabellum to another folder.",
        argument.chars().filter(|c| !c.is_ascii()).take(6).collect::<String>()
    ))
}

/// El contenido del archivo @argumentos (en ANSI, como lo lee Java en Windows).
pub fn argfile_bytes(text: &str) -> Vec<u8> {
    ansi(text).unwrap_or_default()
}

// ================= Cifrado con la cuenta de Windows (DPAPI)

fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: los punteros apuntan a buffers válidos durante la llamada; el resultado lo reserva
    // Windows y se libera con LocalFree después de copiarlo.
    unsafe {
        let ok = if protect {
            CryptProtectData(&input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        };
        if ok == 0 || output.pbData.is_null() {
            return None;
        }
        let result = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Some(result)
    }
}

/// Cifra para el usuario de Windows: solo él, en este PC, lo puede leer (como la versión C#).
pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
    dpapi(data, true)
}

pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    dpapi(data, false)
}

// ================= Sistema

/// El idioma de Windows: español si Windows está en español; si no, inglés.
pub fn ui_language() -> &'static str {
    // SAFETY: función sin argumentos que solo lee la configuración del usuario.
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    if id & 0x3ff == 0x0a { "es" } else { "en" }
}

/// Abre una carpeta (o archivo) en el Explorador.
pub fn open_path(path: &Path) -> Result<(), String> {
    Command::new("explorer").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Abre el Explorador con el archivo seleccionado.
pub fn reveal_path(path: &Path) -> Result<(), String> {
    Command::new("explorer").arg(format!("/select,{}", path.display())).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Abre un enlace en el navegador predeterminado.
pub fn open_url(url: &str) -> Result<(), String> {
    Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Memoria del PC: (total, disponible) en bytes.
pub fn system_memory() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
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

/// Las tarjetas de video del PC (su nombre), leídas del registro de Windows.
pub fn graphics_cards() -> Vec<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let wide = |text: &str| text.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let value = wide("DriverDesc");
    let mut cards = Vec::new();
    for index in 0..16 {
        let key = wide(&format!(r"SYSTEM\CurrentControlSet\Control\Class\{{4d36e968-e325-11ce-bfc1-08002be10318}}\{index:04}"));
        let mut buffer = [0u16; 256];
        let mut size = (buffer.len() * 2) as u32;
        // SAFETY: búfer y tamaño válidos; la función escribe como mucho `size` bytes.
        let status = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key.as_ptr(), value.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr().cast(), &mut size) };
        if status == 0 {
            let length = (size as usize / 2).saturating_sub(1).min(buffer.len());
            cards.push(String::from_utf16_lossy(&buffer[..length]));
        }
    }
    cards
}

/// Registro SRV de Minecraft: (destino, puerto).
pub fn srv_lookup(host: &str) -> Option<(String, u16)> {
    use windows_sys::Win32::NetworkManagement::Dns::{DnsFree, DnsFreeRecordList, DnsQuery_W, DNS_QUERY_STANDARD, DNS_RECORDW, DNS_TYPE_SRV};
    let name: Vec<u16> = format!("_minecraft._tcp.{host}").encode_utf16().chain(std::iter::once(0)).collect();
    let mut results: *mut DNS_RECORDW = std::ptr::null_mut();
    // SAFETY: nombre UTF-16 terminado en cero; la lista que devuelve Windows se recorre mientras vive
    // y se libera con DnsFree al final.
    unsafe {
        if DnsQuery_W(name.as_ptr(), DNS_TYPE_SRV, DNS_QUERY_STANDARD, std::ptr::null_mut(), (&mut results as *mut *mut DNS_RECORDW).cast(), std::ptr::null_mut()) != 0 {
            return None;
        }
        let mut found = None;
        let mut record = results;
        while !record.is_null() {
            if (*record).wType == DNS_TYPE_SRV {
                let data = (*record).Data.SRV;
                let target = data.pNameTarget;
                if !target.is_null() {
                    let length = (0..).take_while(|&i| *target.add(i) != 0).count();
                    let text = String::from_utf16_lossy(std::slice::from_raw_parts(target, length));
                    found = Some((text.trim_end_matches('.').to_string(), data.wPort));
                    break;
                }
            }
            record = (*record).pNext;
        }
        DnsFree(results.cast(), DnsFreeRecordList);
        found
    }
}

/// Toca un WAV que está en memoria. `wait`: esperar a que termine.
pub fn play_wav(wav: &'static [u8], wait: bool) -> bool {
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    let flags = SND_MEMORY | SND_NODEFAULT | if wait { 0 } else { SND_ASYNC };
    // SAFETY: con SND_MEMORY el primer argumento es el WAV en memoria; es estático, así que sigue vivo
    // mientras Windows lo toca en segundo plano (SND_ASYNC).
    unsafe { PlaySoundW(wav.as_ptr().cast(), std::ptr::null_mut(), flags) != 0 }
}

/// Cualquier imagen que Windows sepa abrir (WIC: HEIC, TIFF, ICO, JPEG XR…) → (ancho, alto, RGBA),
/// reducida a `max_side` de lado como mucho.
pub fn decode_image(bytes: &[u8], max_side: u32) -> Result<(u32, u32, Vec<u8>), String> {
    use windows::core::GUID;
    use windows::Win32::Graphics::Imaging::{
        CLSID_WICImagingFactory, GUID_WICPixelFormat32bppBGRA, IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapInterpolationModeFant, WICBitmapPaletteTypeCustom,
        WICDecodeMetadataCacheOnDemand,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    let decode = || -> windows::core::Result<(u32, u32, Vec<u8>)> {
        // SAFETY: COM se inicia en este hilo (si ya estaba iniciado no pasa nada); todos los objetos son
        // de WIC y viven dentro de esta función; el búfer de píxeles tiene el tamaño exacto pedido.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
            let stream = factory.CreateStream()?;
            stream.InitializeFromMemory(bytes)?;
            let decoder = factory.CreateDecoderFromStream(&stream, std::ptr::null::<GUID>(), WICDecodeMetadataCacheOnDemand)?;
            let frame = decoder.GetFrame(0)?;
            let (mut width, mut height) = (0u32, 0u32);
            frame.GetSize(&mut width, &mut height)?;
            if width == 0 || height == 0 {
                return Err(windows::core::Error::from_hresult(windows::core::HRESULT(-1)));
            }
            let scale = (max_side as f64 / width.max(height) as f64).min(1.0);
            let (out_w, out_h) = (((width as f64 * scale).round() as u32).max(1), ((height as f64 * scale).round() as u32).max(1));
            let scaler = factory.CreateBitmapScaler()?;
            scaler.Initialize(&frame, out_w, out_h, WICBitmapInterpolationModeFant)?;
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(&scaler, &GUID_WICPixelFormat32bppBGRA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom)?;
            let stride = out_w * 4;
            let mut pixels = vec![0u8; (stride * out_h) as usize];
            converter.CopyPixels(std::ptr::null(), stride, &mut pixels)?;
            // BGRA → RGBA
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Ok((out_w, out_h, pixels))
        }
    };
    decode().map_err(|e| e.message())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumentos_de_java_en_ansi() {
        // «™» y las letras con acento caben en la codificación de Windows en español/inglés (1252).
        assert_eq!(ansi("Prominence™ II").unwrap().len(), "Prominence™ II".chars().count());
        assert_eq!(ansi("C:\\Users\\José\\Iñaki").unwrap().len(), "C:\\Users\\José\\Iñaki".chars().count());
        assert!(ansi("plain").is_some());
        // Una ruta que existe con letras que no caben se cambia por su nombre corto.
        let folder = std::env::temp_dir().join(format!("parabellum-漢字-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let argument = format!("-Dcarpeta={}", folder.display());
        if ansi(&argument).is_none() {
            let text = java_argument(&argument).unwrap();
            assert!(text.is_ascii(), "{text}");
        }
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn mide_este_proceso() {
        // Windows cuenta el tiempo de procesador en saltos de ~15 ms: primero se trabaja un poco.
        let started = std::time::Instant::now();
        let mut n: u64 = 0;
        while started.elapsed() < std::time::Duration::from_millis(60) {
            n = std::hint::black_box(n.wrapping_mul(31).wrapping_add(7));
        }
        let (memory, cpu) = process_usage(std::process::id()).expect("uso del proceso");
        assert!(memory > 0 && cpu > 0);
        let (total, available) = system_memory();
        assert!(total > 0 && available <= total);
    }

    #[test]
    fn cifrado_de_ida_y_vuelta() {
        let secret = protect(b"1//token").unwrap();
        assert_ne!(secret, b"1//token");
        assert_eq!(unprotect(&secret).unwrap(), b"1//token");
    }
}
