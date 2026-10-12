//! Lo que depende del sistema operativo, en un solo lugar. Todo lo demás de Parabellum (interfaz,
//! amigos, chat, Modrinth, catálogo, servidores, armar el comando de Java…) es igual en todos.
//!
//!   plataforma/windows/   la versión que ya funciona (Windows 10/11).
//!   plataforma/macos/     la de Mac con chip Apple (en construcción: ver docs/mac.md).
//!
//! Cada carpeta da exactamente lo mismo (las mismas funciones, constantes y módulos, con los mismos
//! tipos). Si se agrega algo en una, se agrega en la otra (aunque sea con un TODO): así las dos
//! compilan siempre. El contrato:
//!
//! Constantes
//!   NAME                    nombre del sistema para mostrar ("Windows", "macOS").
//!   MOJANG_OS               nombre del sistema en los JSON de Mojang ("windows", "osx").
//!   JAVA_RUNTIME_PLATFORM   plataforma de los Java de Mojang ("windows-x64", "mac-os-arm64"…).
//!   EXECUTABLE              nombre del programa al actualizarse ("Parabellum.exe").
//!   SELF_UPDATE             si se actualiza solo con el catálogo (si no, nunca pide actualizarse).
//!   CLASSPATH_SEPARATOR     separador de rutas del classpath de Java (";" o ":").
//!   PLAYIT_AGENT            el agente de playit.gg para este sistema (None si no hay).
//!
//! Funciones
//!   data_root(exe)                 la carpeta de Parabellum (instancias, base, datos…).
//!   java_binary(home, windowed)    el ejecutable de Java dentro de un runtime.
//!   protect(datos) / unprotect     cifrar con la cuenta del usuario (sesiones, claves).
//!   ui_language()                  idioma del sistema: "es" o "en".
//!   open_path / reveal_path / open_url
//!   system_memory()                (total, disponible) en bytes.
//!   graphics_cards()               nombres de las tarjetas de video.
//!   srv_lookup(host)               registro SRV de Minecraft (_minecraft._tcp.host).
//!   play_wav(wav, wait)            tocar un sonido WAV que está en memoria.
//!   decode_image(bytes, lado)      cualquier imagen → (ancho, alto, píxeles RGBA), reducida.
//!   tie_to_launcher(child)         que el proceso se cierre si Parabellum se cierra de golpe.
//!   process_usage(pid)             (memoria en bytes, tiempo de CPU en unidades de 100 ns).
//!   kill_tree(pid)                 cerrar un proceso y los que abrió.
//!   is_java_process(pid)           si ese número de proceso es un Java.
//!   java_command_lines()           (pid, línea de comandos) de los Java abiertos.
//!   java_argument(texto)           un argumento como Java lo puede leer en este sistema.
//!   argfile_bytes(texto)           el contenido del archivo @argumentos en la codificación que lee Java.
//!   firewall_open() / open_firewall()   si el firewall deja entrar a los servidores (conexión directa)
//!                                  y abrirlo (pide permiso de administrador una vez).
//!   trait CommandExt               .no_console() (sin ventana negra) y .detached() (el juego sigue
//!                                  abierto aunque se cierre Parabellum).
//!
//! Módulos
//!   avisos    notificaciones del sistema (register, show, init_thread, history_count).
//!   accesos   accesos directos (Place, Status, status, create, remove, supported).

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

/// El agente de playit.gg que se descarga para hostear (el oficial, comprobado con su SHA-256).
pub struct AgentDownload {
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
    /// Nombre del archivo en base/playit (con la versión).
    pub file_name: &'static str,
}
