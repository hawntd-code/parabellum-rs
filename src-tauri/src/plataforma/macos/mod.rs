//! macOS (Mac con chip Apple, M1 en adelante): lo que Parabellum necesita del sistema. Mismo contrato
//! que plataforma/windows (ver plataforma/mod.rs). Guía y lista de tareas: docs/mac.md.
//!
//! Lo que ya está hecho usa herramientas que trae macOS (open, sysctl, ps, kill, afplay, sips, dig,
//! security, sh). Lo marcado con TODO(mac) falta: cada uno dice qué hacer. Mientras tanto devuelve
//! algo seguro (por ejemplo, `firewall_open` dice que sí: no revisa el firewall de macOS).

pub mod accesos;
pub mod avisos;
pub mod lanzador;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};

use super::AgentDownload;

pub const NAME: &str = "macOS";
pub const MOJANG_OS: &str = "osx";
pub const JAVA_RUNTIME_PLATFORM: &str = if cfg!(target_arch = "aarch64") { "mac-os-arm64" } else { "mac-os" };
// El programa, dentro de Parabellum.app/Contents/MacOS. Al actualizarse se cambia la app entera
// (lanzador.rs), con su propio id en el catálogo: nunca baja el .exe de Windows.
pub const EXECUTABLE: &str = "Parabellum";
pub const SELF_UPDATE: bool = true;
pub const CLASSPATH_SEPARATOR: &str = ":";
// El de Windows separa las rutas con «;» y aquí Java no encontraría la clase principal de Forge.
pub const FORGE_SERVER_ARGS: &str = "unix_args.txt";
// playit.gg dejó de publicar su agente para Mac después de la 0.15.13 (que lee PLAYIT_SECRET al
// compilar, no al abrirse). Este es la misma 0.17.1 que en Windows, compilada para chip Apple por
// GitHub Actions a partir de la etiqueta oficial, sin cambios, en el fork diegovc20/playit-agent
// (rama parabellum-macos, .github/workflows/macos-arm64.yml). Para otra versión: una etiqueta
// v<versión>-macos-arm64 sobre la oficial y poner aquí lo que diga su release.
pub const PLAYIT_AGENT: Option<AgentDownload> = Some(AgentDownload {
    url: "https://github.com/diegovc20/playit-agent/releases/download/v0.17.1-macos-arm64/playit-darwin-arm64",
    sha256: "57fd00e8085097e439e8e141ca947620e179065848cd63d27b933db7a953d00e",
    size: 5_390_736,
    file_name: "playit-0.17.1",
});

/// Dónde viven instancias, base y datos. Dentro de Parabellum.app no se puede escribir (y macOS la
/// mueve a una carpeta temporal si viene de Descargas), así que van en ~/Library/Application Support.
/// Fuera de una .app (compilando con cargo) queda junto al programa, como en Windows.
pub fn data_root(exe: &Path) -> PathBuf {
    let inside_app = exe.to_string_lossy().contains(".app/Contents/MacOS");
    match std::env::var_os("HOME") {
        Some(home) if inside_app => PathBuf::from(home).join("Library").join("Application Support").join("Parabellum"),
        _ => exe.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")),
    }
}

// ================= Java

/// Los Java de Mojang para Mac traen el ejecutable en jre.bundle/Contents/Home/bin/java; los demás,
/// en bin/java.
pub fn java_binary(home: &Path, _windowed: bool) -> PathBuf {
    let bundled = home.join("jre.bundle").join("Contents").join("Home").join("bin").join("java");
    if bundled.exists() {
        bundled
    } else {
        home.join("bin").join("java")
    }
}

/// Lo descargado llega sin permiso de ejecución: se le pone (como chmod +x). Solo a archivos normales:
/// si en su lugar hubiera un enlace, no se toca a lo que apunta.
pub fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let Ok(metadata) = std::fs::symlink_metadata(path) else { return };
    let mut permissions = metadata.permissions();
    // Si ya lo tiene, no se toca (java::discover lo pide cada vez que busca los Java).
    if !metadata.file_type().is_file() || permissions.mode() & 0o111 == 0o111 {
        return;
    }
    permissions.set_mode(permissions.mode() | 0o111);
    let _ = std::fs::set_permissions(path, permissions);
}

// ================= Versiones viejas (1.18 y anteriores)

/// Mojang tiene para chip Apple los Java 17, 21 y 25, pero no el 8 (hasta 1.16) ni el 16 (1.17). Esos
/// se bajan de Intel y se abren con Rosetta, y el juego usa sus nativos de Intel tal cual.
pub fn java_runtime_platform(major: u32) -> &'static str {
    if cfg!(target_arch = "aarch64") && major < 17 {
        "mac-os"
    } else {
        JAVA_RUNTIME_PLATFORM
    }
}

/// 1.18 pide Java 17, que sí hay para chip Apple, pero trae LWJGL 3.2, sin nativos arm64: se cambia
/// por la 3.3.1 de 1.19 (la misma de Mojang) y java-objc-bridge por la 1.1. Antes de 1.18 no se toca,
/// porque va con el Java de Intel: con LWJGL 3.3.1 se caerían al ponerle ícono a la ventana (su GLFW
/// da el error 65548, «Regular windows do not have icons on macOS», y Minecraft lo trata como fatal).
pub fn adapt_libraries(libraries: Vec<serde_json::Value>, java_major: u32) -> Vec<serde_json::Value> {
    let name = |library: &serde_json::Value| library["name"].as_str().unwrap_or_default().to_string();
    // LWJGL 3.0 a 3.2 (la 2 es de versiones que piden Java 8).
    let old_lwjgl = libraries.iter().map(name).any(|n| {
        n.strip_prefix("org.lwjgl:").and_then(|rest| rest.split(':').nth(1)).is_some_and(|version| {
            let mut parts = version.split('.').map(|p| p.parse::<u32>().unwrap_or(0));
            parts.next() == Some(3) && parts.next().is_some_and(|minor| minor < 3)
        })
    });
    if java_runtime_platform(java_major) != JAVA_RUNTIME_PLATFORM || !old_lwjgl {
        return libraries;
    }
    let replaced = |n: &str| n.starts_with("org.lwjgl:") || n.starts_with("ca.weblite:java-objc-bridge:");
    let mut adapted: Vec<serde_json::Value> = libraries.into_iter().filter(|l| !replaced(&name(l))).collect();
    for (coordinate, sha1, size) in ARM64_LWJGL {
        let Some(path) = crate::version::Maven::parse(coordinate).map(|m| m.relative_path()) else { continue };
        let url = format!("https://libraries.minecraft.net/{path}");
        adapted.push(serde_json::json!({ "name": coordinate, "downloads": { "artifact": { "path": path, "sha1": sha1, "size": size, "url": url } } }));
    }
    adapted
}

/// Las bibliotecas de 1.19 para Mac con chip Apple (de su JSON en Mojang): LWJGL 3.3.1 con sus nativos
/// arm64 y java-objc-bridge 1.1, que ya trae los suyos.
const ARM64_LWJGL: [(&str, &str, u64); 15] = [
    ("org.lwjgl:lwjgl:3.3.1", "ae58664f88e18a9bb2c77b063833ca7aaec484cb", 724243),
    ("org.lwjgl:lwjgl:3.3.1:natives-macos-arm64", "71d0d5e469c9c95351eb949064497e3391616ac9", 42693),
    ("org.lwjgl:lwjgl-jemalloc:3.3.1", "a817bcf213db49f710603677457567c37d53e103", 36601),
    ("org.lwjgl:lwjgl-jemalloc:3.3.1:natives-macos-arm64", "e577b87d8ad2ade361aaea2fcf226c660b15dee8", 103475),
    ("org.lwjgl:lwjgl-openal:3.3.1", "2623a6b8ae1dfcd880738656a9f0243d2e6840bd", 88237),
    ("org.lwjgl:lwjgl-openal:3.3.1:natives-macos-arm64", "23d55e7490b57495320f6c9e1936d78fd72c4ef8", 346125),
    ("org.lwjgl:lwjgl-opengl:3.3.1", "831a5533a21a5f4f81bbc51bb13e9899319b5411", 921563),
    ("org.lwjgl:lwjgl-opengl:3.3.1:natives-macos-arm64", "eafe34b871d966292e8db0f1f3d6b8b110d4e91d", 41665),
    ("org.lwjgl:lwjgl-glfw:3.3.1", "cbac1b8d30cb4795149c1ef540f912671a8616d0", 128801),
    ("org.lwjgl:lwjgl-glfw:3.3.1:natives-macos-arm64", "cac0d3f712a3da7641fa174735a5f315de7ffe0a", 129077),
    ("org.lwjgl:lwjgl-stb:3.3.1", "b119297cf8ed01f247abe8685857f8e7fcf5980f", 112380),
    ("org.lwjgl:lwjgl-stb:3.3.1:natives-macos-arm64", "fcf073ed911752abdca5f0b00a53cfdf17ff8e8b", 178408),
    ("org.lwjgl:lwjgl-tinyfd:3.3.1", "0ff1914111ef2e3e0110ef2dabc8d8cdaad82347", 6767),
    ("org.lwjgl:lwjgl-tinyfd:3.3.1:natives-macos-arm64", "972ecc17bad3571e81162153077b4d47b7b9eaa9", 41380),
    ("ca.weblite:java-objc-bridge:1.1", "1227f9e0666314f9de41477e3ec277e542ed7f7b", 1330045),
];

/// Rosetta está instalado si se puede abrir un programa de Intel (uno del sistema, con `arch`).
pub fn foreign_java_ready() -> bool {
    !cfg!(target_arch = "aarch64")
        || Command::new("/usr/bin/arch").args(["-x86_64", "/usr/bin/true"]).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Instala Rosetta con el instalador de Apple (acepta su licencia). macOS pide la contraseña de un
/// administrador con su propia ventana.
pub fn install_foreign_java() -> Result<(), String> {
    if foreign_java_ready() {
        return Ok(());
    }
    let script = r#"do shell script "/usr/sbin/softwareupdate --install-rosetta --agree-to-license" with administrator privileges"#;
    let output = Command::new("/usr/bin/osascript").args(["-e", script]).output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // -128: el usuario cerró la ventana de la contraseña.
        return Err(if detail.contains("-128") {
            t!("No se instaló Rosetta: se canceló la contraseña.", "Rosetta was not installed: the password was cancelled.")
        } else {
            t!("No se pudo instalar Rosetta: {detail}", "Could not install Rosetta: {detail}")
        });
    }
    if foreign_java_ready() {
        Ok(())
    } else {
        Err(t!("El instalador de Apple terminó, pero Rosetta no funciona. Reinicia la Mac y vuelve a intentarlo.", "Apple's installer finished, but Rosetta does not work. Restart the Mac and try again."))
    }
}

// ================= Procesos

pub trait CommandExt {
    /// Sin ventana de consola (en Mac no aplica).
    fn no_console(&mut self) -> &mut Self;
    /// Independiente de Parabellum (el juego sigue abierto aunque se cierre el launcher).
    fn detached(&mut self) -> &mut Self;
}

impl CommandExt for Command {
    fn no_console(&mut self) -> &mut Self {
        self
    }

    fn detached(&mut self) -> &mut Self {
        use std::os::unix::process::CommandExt as _;
        // Su propio grupo de procesos: no le llegan las señales que reciba Parabellum.
        self.process_group(0)
    }
}

/// El proceso se cierra solo si Parabellum termina sin cerrarlo antes (en Windows lo hace un «job»).
/// Lo cuida un vigilante (`sh`) que espera leyendo una tubería que solo Parabellum tiene abierta para
/// escribir: cuando Parabellum termina, aunque sea de golpe, macOS la cierra, el vigilante deja de
/// esperar y le pide al proceso que se cierre (SIGTERM: el servidor guarda el mundo); si a los 40 s
/// sigue abierto, lo cierra a la fuerza. Si el proceso termina antes, un hilo de Parabellum se entera
/// con kqueue y cierra al vigilante. Por si macOS ya le dio el número a otro programa (en el instante
/// en que Parabellum se cae, o mientras el vigilante espera los 40 s), el vigilante solo manda una
/// señal si el proceso con ese número empezó a la misma hora.
pub fn tie_to_launcher(child: &Child) {
    use std::os::unix::process::CommandExt as _;
    let pid = child.id().to_string();
    // Mientras exista `child` nadie lo ha recogido, así que el número todavía es suyo.
    let Some(started) = start_time(&pid) else { return };
    let Some(exit) = ExitWatch::new(child.id()) else { return };
    let Some(lifeline) = lifeline() else { return };
    let watcher = Command::new("/bin/sh")
        .args(["-c", WATCHER, "parabellum-vigilante", &pid, &started])
        .stdin(lifeline)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Su propio grupo de procesos: no le llegan las señales que reciba Parabellum (Ctrl+C).
        .process_group(0)
        .spawn();
    let Ok(mut watcher) = watcher else { return };
    std::thread::spawn(move || {
        exit.wait();
        let _ = watcher.kill();
        let _ = watcher.wait();
    });
}

/// El vigilante ($1: el número del proceso; $2: cuándo empezó). `read` vuelve cuando se cierra la
/// tubería, es decir, cuando Parabellum terminó.
const WATCHER: &str = r#"read -r _
same() { [ "$(ps -o lstart= -p "$1")" = "$2" ]; }
same "$1" "$2" || exit 0
kill -TERM "$1"
i=0
while [ $i -lt 40 ] && same "$1" "$2"; do sleep 1; i=$((i + 1)); done
same "$1" "$2" && kill -KILL "$1""#;

/// Cuándo empezó un proceso, como lo escribe `ps` (lo que compara el vigilante).
fn start_time(pid: &str) -> Option<String> {
    let output = Command::new("ps").args(["-o", "lstart=", "-p", pid]).output().ok()?;
    // Igual que `$(…)` en sh: sin los saltos de línea del final.
    let text = String::from_utf8_lossy(&output.stdout).trim_end_matches('\n').to_string();
    (!text.trim().is_empty()).then_some(text)
}

/// La tubería de los vigilantes. Parabellum guarda el extremo de escritura sin escribir nunca, y nadie
/// más lo tiene: std abre las tuberías con close-on-exec, así que no pasa a los programas que se
/// abren después (el juego, el servidor). Cada vigilante recibe su copia del extremo de lectura.
fn lifeline() -> Option<std::io::PipeReader> {
    static PIPE: std::sync::OnceLock<Option<(std::io::PipeReader, std::io::PipeWriter)>> = std::sync::OnceLock::new();
    PIPE.get_or_init(|| std::io::pipe().ok()).as_ref()?.0.try_clone().ok()
}

// kqueue (sys/event.h), declarado aquí para no agregar el crate libc solo por esto.
#[repr(C)]
struct KEvent {
    ident: usize,
    filter: i16,
    flags: u16,
    fflags: u32,
    data: isize,
    udata: *mut std::ffi::c_void,
}
const NO_EVENT: KEvent = KEvent { ident: 0, filter: 0, flags: 0, fflags: 0, data: 0, udata: std::ptr::null_mut() };
const EVFILT_PROC: i16 = -5;
const EV_ADD: u16 = 0x0001;
const EV_ONESHOT: u16 = 0x0010;
const EV_RECEIPT: u16 = 0x0040;
const EV_ERROR: u16 = 0x4000;
const NOTE_EXIT: u32 = 0x8000_0000;

unsafe extern "C" {
    fn kqueue() -> i32;
    fn kevent(kq: i32, changes: *const KEvent, change_count: i32, events: *mut KEvent, event_count: i32, timeout: *const std::ffi::c_void) -> i32;
}

/// Avisa cuando termina un proceso, sin recogerlo (eso lo hace quien lo abrió).
struct ExitWatch(std::os::fd::OwnedFd);

impl ExitWatch {
    /// None si el proceso ya terminó (o kqueue falla).
    fn new(pid: u32) -> Option<Self> {
        use std::os::fd::{AsRawFd, FromRawFd};
        // SAFETY: kqueue no recibe nada, y el descriptor nuevo (si lo hay) solo es de OwnedFd.
        let queue = unsafe { kqueue() };
        if queue < 0 {
            return None;
        }
        let queue = unsafe { std::os::fd::OwnedFd::from_raw_fd(queue) };
        let change = KEvent { ident: pid as usize, filter: EVFILT_PROC, flags: EV_ADD | EV_ONESHOT | EV_RECEIPT, fflags: NOTE_EXIT, ..NO_EVENT };
        let mut receipt = NO_EVENT;
        // SAFETY: un cambio y espacio para una respuesta, los dos válidos durante la llamada. Con
        // EV_RECEIPT responde al momento: EV_ERROR con data 0 si quedó anotado, o el error (ESRCH si
        // el proceso ya terminó).
        let count = unsafe { kevent(queue.as_raw_fd(), &change, 1, &mut receipt, 1, std::ptr::null()) };
        (count == 1 && receipt.flags & EV_ERROR != 0 && receipt.data == 0).then_some(Self(queue))
    }

    /// Espera hasta que el proceso termine.
    fn wait(&self) {
        use std::os::fd::AsRawFd;
        let mut event = NO_EVENT;
        loop {
            // SAFETY: espacio para un evento, válido durante la llamada; sin límite de tiempo.
            let count = unsafe { kevent(self.0.as_raw_fd(), std::ptr::null(), 0, &mut event, 1, std::ptr::null()) };
            if count != -1 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
                return;
            }
        }
    }
}

/// Memoria en uso (bytes) y tiempo de CPU (en unidades de 100 ns) con `ps`.
pub fn process_usage(pid: u32) -> Option<(u64, u64)> {
    let output = Command::new("ps").args(["-o", "rss=,cputime=", "-p", &pid.to_string()]).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let mut parts = text.split_whitespace();
    let rss_kb: u64 = parts.next()?.parse().ok()?;
    // cputime: [[dd-]hh:]mm:ss(.cc)
    let time = parts.next()?;
    let (days, clock) = time.split_once('-').map(|(d, c)| (d.parse::<u64>().unwrap_or(0), c)).unwrap_or((0, time));
    let seconds = clock.split(':').fold(0f64, |total, piece| total * 60.0 + piece.parse::<f64>().unwrap_or(0.0)) + days as f64 * 86_400.0;
    Some((rss_kb * 1024, (seconds * 10_000_000.0) as u64))
}

/// Cierra el proceso y los que haya abierto.
pub fn kill_tree(pid: u32) {
    let _ = Command::new("pkill").args(["-9", "-P", &pid.to_string()]).output();
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
}

pub fn is_java_process(pid: u32) -> bool {
    Command::new("ps")
        .args(["-o", "comm=", "-p", &pid.to_string()])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_lowercase().ends_with("java"))
        .unwrap_or(false)
}

/// (pid, línea de comandos) de los Java abiertos.
pub fn java_command_lines() -> Vec<(u32, String)> {
    Command::new("ps")
        .args(["-axo", "pid=,command="])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|line| {
                    let line = line.trim_start();
                    let (pid, command) = line.split_once(' ')?;
                    let program = command.split_whitespace().next().unwrap_or_default();
                    program.ends_with("/java").then(|| Some((pid.parse().ok()?, command.to_string()))).flatten()
                })
                .collect()
        })
        .unwrap_or_default()
}

// ================= Firewall (conexión directa a los servidores)

/// El firewall de macOS viene apagado y, si está encendido, pregunta por cada programa la primera
/// vez que abre un puerto. TODO(mac): si está encendido, revisar que deje entrar a Java.
pub fn firewall_open() -> bool {
    true
}

pub fn open_firewall() -> Result<(), String> {
    Ok(())
}

// ================= Argumentos de Java

/// En Mac Java lee los argumentos en UTF-8: no hay que cambiar nada.
pub fn java_argument(argument: &str) -> Result<String, String> {
    Ok(argument.to_string())
}

pub fn argfile_bytes(text: &str) -> Vec<u8> {
    text.as_bytes().to_vec()
}

// ================= Cifrado con el Llavero

// Una clave aleatoria por usuario del Mac, guardada en su Llavero de inicio de sesión. Se guarda y se
// lee con el comando `security` (no con la API del Llavero): así la entrada queda a nombre de un
// programa firmado por Apple y macOS no pide la contraseña cada vez que Parabellum se recompila o se
// actualiza (sin firma de Apple, para macOS cada versión sería otro programa). Como DPAPI en Windows:
// lo cifrado solo lo lee este usuario, en esta Mac.
const KEYCHAIN_SERVICE: &str = "Parabellum";
// Las pruebas usan su propia clave para no tocar la de verdad.
const KEYCHAIN_ACCOUNT: &str = if cfg!(test) { "pruebas" } else { "clave" };
/// Primer byte de lo cifrado: si un día cambia el formato, se distingue lo viejo.
const CIPHER_VERSION: u8 = 1;

/// La clave del Llavero. Con `create`, la crea si todavía no existe (solo al cifrar: al descifrar,
/// sin clave no hay nada que leer). Se lee una vez por ejecución.
fn key(create: bool) -> Option<[u8; 32]> {
    static KEY: std::sync::OnceLock<[u8; 32]> = std::sync::OnceLock::new();
    if let Some(key) = KEY.get() {
        return Some(*key);
    }
    let key = match read_key() {
        Ok(key) => key,
        // 44: no existe (errSecItemNotFound). Con otro error (Llavero bloqueado…) no se crea.
        Err(Some(44)) if create => {
            store_new_key();
            // Se vuelve a leer: si otro Parabellum la creó al mismo tiempo, vale la que quedó guardada.
            read_key().ok()?
        }
        Err(_) => return None,
    };
    Some(*KEY.get_or_init(|| key))
}

/// Lee la clave; si no se puede, el código de salida de `security`.
fn read_key() -> Result<[u8; 32], Option<i32>> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", KEYCHAIN_SERVICE, "-a", KEYCHAIN_ACCOUNT, "-w"])
        .output()
        .map_err(|_| None)?;
    if !output.status.success() {
        return Err(output.status.code());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    hex::decode(text.trim()).ok().and_then(|bytes| bytes.try_into().ok()).ok_or(None)
}

/// Guarda una clave nueva. Va por la entrada de `security -i` (no como argumento) para que no se vea
/// en la lista de procesos. Sin -U: si ya hay una clave no la reemplaza (lo cifrado quedaría ilegible).
fn store_new_key() {
    use std::io::Write;
    use std::process::Stdio;
    let mut key = [0u8; 32];
    if getrandom::getrandom(&mut key).is_err() {
        return;
    }
    let child = Command::new("security").arg("-i").stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    let Ok(mut child) = child else { return };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = writeln!(stdin, "add-generic-password -s {KEYCHAIN_SERVICE} -a {KEYCHAIN_ACCOUNT} -w {}", hex::encode(key));
    } // Al cerrar la entrada, `security` termina.
    let _ = child.wait();
}

fn cipher(create: bool) -> Option<LessSafeKey> {
    Some(LessSafeKey::new(UnboundKey::new(&CHACHA20_POLY1305, &key(create)?).ok()?))
}

/// Cifra para el usuario del Mac (ChaCha20-Poly1305 con la clave del Llavero):
/// [versión][nonce de 12 bytes al azar][cifrado][etiqueta de 16 bytes].
pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
    let cipher = cipher(true)?;
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::getrandom(&mut nonce).ok()?;
    let mut sealed = [&[CIPHER_VERSION][..], &nonce[..], data].concat();
    let tag = cipher.seal_in_place_separate_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut sealed[1 + NONCE_LEN..]).ok()?;
    sealed.extend_from_slice(tag.as_ref());
    Some(sealed)
}

/// None si no se cifró para este usuario en esta Mac o si se alteró.
pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    let (&version, rest) = data.split_first()?;
    if version != CIPHER_VERSION || rest.len() < NONCE_LEN {
        return None;
    }
    let (nonce, sealed) = rest.split_at(NONCE_LEN);
    let mut buffer = sealed.to_vec();
    let plain = cipher(false)?.open_in_place(Nonce::try_assume_unique_for_key(nonce).ok()?, Aad::empty(), &mut buffer).ok()?;
    Some(plain.to_vec())
}

// ================= Sistema

/// El idioma del Mac: español si el primero de la lista de idiomas es español.
pub fn ui_language() -> &'static str {
    let output = Command::new("defaults").args(["read", "-g", "AppleLanguages"]).output();
    let text = output.map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase()).unwrap_or_default();
    let first = text.split(['"', ',', '(', ')', '\n']).map(str::trim).find(|piece| !piece.is_empty()).unwrap_or_default().to_string();
    if first.starts_with("es") { "es" } else { "en" }
}

pub fn open_path(path: &Path) -> Result<(), String> {
    Command::new("open").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn reveal_path(path: &Path) -> Result<(), String> {
    Command::new("open").arg("-R").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

pub fn open_url(url: &str) -> Result<(), String> {
    Command::new("open").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Memoria: (total, disponible) en bytes. La disponible se calcula con vm_stat (páginas libres,
/// inactivas y especulativas).
pub fn system_memory() -> (u64, u64) {
    let read = |args: &[&str]| Command::new(args[0]).args(&args[1..]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let total: u64 = read(&["sysctl", "-n", "hw.memsize"]).trim().parse().unwrap_or(0);
    let stats = read(&["vm_stat"]);
    let page: u64 = stats.split("page size of ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(16384);
    let pages = |name: &str| -> u64 {
        stats.lines().find(|l| l.starts_with(name)).and_then(|l| l.split(':').nth(1)).map(|n| n.trim().trim_end_matches('.').parse().unwrap_or(0)).unwrap_or(0)
    };
    let available = (pages("Pages free") + pages("Pages inactive") + pages("Pages speculative")) * page;
    (total, available.min(total))
}

/// TODO(mac): leer el nombre de la GPU (system_profiler SPDisplaysDataType). Solo se usa para un
/// arreglo de gráficos Intel en Windows; en Mac con chip Apple no hace falta.
pub fn graphics_cards() -> Vec<String> {
    Vec::new()
}

/// Registro SRV de Minecraft con `dig` (viene con macOS).
pub fn srv_lookup(host: &str) -> Option<(String, u16)> {
    let output = Command::new("dig").args(["+short", "SRV", &format!("_minecraft._tcp.{host}")]).output().ok()?;
    // Cada línea: «prioridad peso puerto destino.»
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?;
    let parts: Vec<&str> = line.split_whitespace().collect();
    let port = parts.get(2)?.parse().ok()?;
    let target = parts.get(3)?.trim_end_matches('.').to_string();
    (!target.is_empty()).then_some((target, port))
}

/// Toca el WAV con `afplay` (lo guarda primero en un archivo temporal).
pub fn play_wav(wav: &'static [u8], wait: bool) -> bool {
    let file = std::env::temp_dir().join("parabellum-mensaje.wav");
    if !file.exists() && std::fs::write(&file, wav).is_err() {
        return false;
    }
    let mut command = Command::new("afplay");
    command.arg(&file);
    if wait {
        command.status().map(|s| s.success()).unwrap_or(false)
    } else {
        command.spawn().is_ok()
    }
}

/// Cualquier imagen que abra el Mac (HEIC, TIFF, WebP…) → (ancho, alto, RGBA), reducida a `max_side`
/// de lado como mucho. La convierte `sips` (viene con macOS) a PNG en un proceso aparte: si una imagen
/// mala del chat lo hace caer, Parabellum sigue igual. Como en Windows, no se gira según su EXIF.
pub fn decode_image(bytes: &[u8], max_side: u32) -> Result<(u32, u32, Vec<u8>), String> {
    use std::os::unix::fs::DirBuilderExt;
    // Carpeta nueva, con nombre al azar y solo para este usuario. `create` (no `create_dir_all`)
    // falla si ya hay algo con ese nombre (una carpeta o un enlace) en vez de usarlo.
    let mut random = [0u8; 8];
    getrandom::getrandom(&mut random).map_err(|e| e.to_string())?;
    let folder = std::env::temp_dir().join(format!("parabellum-imagen-{}", hex::encode(random)));
    std::fs::DirBuilder::new().mode(0o700).create(&folder).map_err(|e| e.to_string())?;
    let result = sips_to_png(bytes, max_side, &folder).and_then(|png| read_png(&png, max_side));
    let _ = std::fs::remove_dir_all(&folder);
    result
}

/// La imagen como PNG, reducida si hace falta (`sips -Z` también agranda: antes se mide).
fn sips_to_png(bytes: &[u8], max_side: u32, folder: &Path) -> Result<Vec<u8>, String> {
    let (source, target) = (folder.join("imagen"), folder.join("imagen.png"));
    std::fs::write(&source, bytes).map_err(|e| e.to_string())?;
    let size = Command::new("sips").args(["-g", "pixelWidth", "-g", "pixelHeight"]).arg(&source).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&size.stdout);
    let side = |name: &str| text.lines().find_map(|l| l.trim().strip_prefix(name)?.trim().parse::<u32>().ok());
    let (width, height) = size.status.success().then(|| side("pixelWidth:").zip(side("pixelHeight:"))).flatten().ok_or("sips no la reconoce como imagen")?;
    if width == 0 || height == 0 {
        return Err("imagen vacía".into());
    }
    let mut convert = Command::new("sips");
    convert.args(["-s", "format", "png"]);
    if width.max(height) > max_side {
        convert.args(["-Z", &max_side.to_string()]);
    }
    let converted = convert.arg(&source).arg("--out").arg(&target).output().map_err(|e| e.to_string())?;
    if !converted.status.success() {
        return Err("sips no pudo convertirla".into());
    }
    std::fs::read(&target).map_err(|e| e.to_string())
}

/// Un PNG de cualquier tipo de color → RGBA de 8 bits.
fn read_png(png: &[u8], max_side: u32) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    // Antes de reservar memoria, que sea del tamaño pedido (no basta con lo que dijo `sips -g`).
    let (width, height) = (reader.info().width, reader.info().height);
    if width == 0 || height == 0 || width.max(height) > max_side {
        return Err(format!("sips la dejó de {width}×{height}"));
    }
    let mut buffer = vec![0; reader.output_buffer_size().ok_or("PNG demasiado grande")?];
    let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        // EXPAND ya convierte la paleta a RGB(A).
        png::ColorType::Indexed => return Err("PNG con paleta".into()),
    };
    Ok((info.width, info.height, rgba))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cifrado_de_ida_y_vuelta() {
        let secret = protect(b"1//token").unwrap();
        assert_ne!(secret, b"1//token");
        assert_eq!(unprotect(&secret).unwrap(), b"1//token");
        // Cada vez con otro nonce: el mismo texto no da los mismos bytes.
        assert_ne!(protect(b"1//token").unwrap(), secret);
        // Alterado (o de otra Mac) no se descifra.
        let mut altered = secret.clone();
        *altered.last_mut().unwrap() ^= 1;
        assert!(unprotect(&altered).is_none());
        assert!(unprotect(&[]).is_none());
    }

    #[test]
    fn java_sin_permiso_se_arregla_al_encontrarlo() {
        use std::os::unix::fs::PermissionsExt;
        // Como quedaba un Java bajado antes de que se marcaran sus programas: sin +x.
        let paths = crate::paths::Paths::new(std::env::temp_dir().join(format!("parabellum-java-mac-{}", std::process::id())));
        let home = paths.java.join("java-runtime-delta").join("jre.bundle").join("Contents").join("Home");
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::create_dir_all(home.join("lib")).unwrap();
        for program in [home.join("bin").join("java"), home.join("lib").join("jspawnhelper")] {
            std::fs::write(&program, b"").unwrap();
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        std::fs::write(home.join("release"), "JAVA_VERSION=\"21.0.7\"\n").unwrap();
        assert_eq!(crate::java::discover(&paths).len(), 1);
        for program in [home.join("bin").join("java"), home.join("lib").join("jspawnhelper")] {
            assert_eq!(std::fs::metadata(&program).unwrap().permissions().mode() & 0o777, 0o755, "{}", program.display());
        }
        let _ = std::fs::remove_dir_all(&paths.root);
    }

    fn alive(pid: u32) -> bool {
        Command::new("kill").args(["-0", &pid.to_string()]).stderr(Stdio::null()).status().unwrap().success()
    }

    fn closes_within(seconds: u64, open: impl Fn() -> bool) -> bool {
        let started = std::time::Instant::now();
        while started.elapsed().as_secs() < seconds {
            if !open() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        false
    }

    #[test]
    fn atado_se_cierra_si_parabellum_se_cae() {
        use std::os::unix::process::CommandExt as _;
        // Otra copia de esta prueba hace de Parabellum: ata un proceso, abre otro aparte (como el juego,
        // que no debe quedarse con la tubería del vigilante) y espera a que esta prueba la cierre de golpe.
        if std::env::var_os("PARABELLUM_PRUEBA_ATADO").is_some() {
            let tied = Command::new("sleep").arg("60").spawn().unwrap();
            tie_to_launcher(&tied);
            let other = Command::new("sleep").arg("60").process_group(0).spawn().unwrap();
            println!("PID {} {}", tied.id(), other.id());
            std::thread::sleep(std::time::Duration::from_secs(60));
            return;
        }
        use std::io::BufRead;
        let mut parabellum = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "plataforma::macos::tests::atado_se_cierra_si_parabellum_se_cae", "--nocapture"])
            .env("PARABELLUM_PRUEBA_ATADO", "1")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let line = std::io::BufReader::new(parabellum.stdout.take().unwrap()).lines().map_while(Result::ok).find(|l| l.starts_with("PID ")).unwrap();
        let pids: Vec<u32> = line.split_whitespace().skip(1).map(|p| p.parse().unwrap()).collect();
        let (tied, other) = (pids[0], pids[1]);
        assert!(alive(tied));
        parabellum.kill().unwrap();
        parabellum.wait().unwrap();
        let closed = closes_within(10, || alive(tied));
        let _ = Command::new("kill").arg(other.to_string()).status();
        assert!(closed, "el proceso atado siguió abierto");
    }

    #[test]
    fn el_vigilante_se_va_con_el_proceso() {
        let mut child = Command::new("sleep").arg("1").spawn().unwrap();
        let pattern = format!("parabellum-vigilante {} ", child.id());
        let watcher = || Command::new("pgrep").args(["-f", &pattern]).stdout(Stdio::null()).status().unwrap().success();
        tie_to_launcher(&child);
        assert!(watcher());
        child.wait().unwrap();
        assert!(closes_within(5, watcher), "el vigilante siguió abierto");
    }

    #[test]
    fn el_vigilante_no_toca_otro_programa_con_el_mismo_numero() {
        // Como si macOS ya le hubiera dado el número a otro programa: no empezó a esa hora.
        let mut other = Command::new("sleep").arg("30").spawn().unwrap();
        let (reader, writer) = std::io::pipe().unwrap();
        let mut watcher =
            Command::new("/bin/sh").args(["-c", WATCHER, "parabellum-vigilante", &other.id().to_string(), "Mon Jan  1 00:00:00 2001"]).stdin(reader).spawn().unwrap();
        // Parabellum «se cae».
        drop(writer);
        watcher.wait().unwrap();
        let untouched = other.try_wait().unwrap().is_none();
        let _ = other.kill();
        let _ = other.wait();
        assert!(untouched, "el vigilante cerró otro programa");
    }

    #[test]
    fn versiones_viejas_en_chip_apple() {
        if !cfg!(target_arch = "aarch64") {
            return;
        }
        assert_eq!(java_runtime_platform(8), "mac-os");
        assert_eq!(java_runtime_platform(16), "mac-os");
        assert_eq!(java_runtime_platform(17), "mac-os-arm64");
        assert_eq!(java_runtime_platform(25), "mac-os-arm64");
        let library = |name: &str| serde_json::json!({ "name": name });
        // 1.18.2: LWJGL 3.2 y java-objc-bridge 1.0.0 pasan a los de 1.19; lo demás queda igual.
        let old = vec![library("org.lwjgl:lwjgl:3.2.1"), library("org.lwjgl:lwjgl-glfw:3.2.1"), library("ca.weblite:java-objc-bridge:1.0.0"), library("com.mojang:brigadier:1.0.18")];
        let adapted = adapt_libraries(old.clone(), 17);
        let names: Vec<&str> = adapted.iter().filter_map(|l| l["name"].as_str()).collect();
        assert!(names.contains(&"com.mojang:brigadier:1.0.18"));
        assert!(names.contains(&"org.lwjgl:lwjgl-glfw:3.3.1:natives-macos-arm64"));
        assert!(names.contains(&"ca.weblite:java-objc-bridge:1.1"));
        assert!(!names.iter().any(|n| n.ends_with(":3.2.1") || n.ends_with(":1.0.0")));
        let glfw = adapted.iter().find(|l| l["name"] == "org.lwjgl:lwjgl-glfw:3.3.1:natives-macos-arm64").unwrap();
        assert_eq!(glfw.pointer("/downloads/artifact/url").and_then(|u| u.as_str()), Some("https://libraries.minecraft.net/org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos-arm64.jar"));
        // Con el Java de Intel (1.16.5) o si ya trae LWJGL 3.3 (1.19 en adelante), no se toca.
        assert_eq!(adapt_libraries(old.clone(), 8), old);
        let current = vec![library("org.lwjgl:lwjgl:3.3.3"), library("org.lwjgl:lwjgl:3.3.3:natives-macos-arm64")];
        assert_eq!(adapt_libraries(current.clone(), 21), current);
    }

    #[test]
    fn png_del_tamano_pedido_a_rgba() {
        let mut png = Vec::new();
        let mut encoder = png::Encoder::new(&mut png, 3, 2);
        encoder.set_color(png::ColorType::Rgb);
        encoder.write_header().unwrap().write_image_data(&[7; 3 * 2 * 3]).unwrap();
        // Más grande que lo pedido: se rechaza antes de reservar memoria.
        assert!(read_png(&png, 2).is_err());
        let (width, height, rgba) = read_png(&png, 3).unwrap();
        assert_eq!((width, height), (3, 2));
        assert_eq!(rgba, [7, 7, 7, 255].repeat(6));
    }
}
