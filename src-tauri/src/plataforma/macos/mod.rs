//! macOS (Mac con chip Apple, M1 en adelante): lo que Parabellum necesita del sistema. Mismo contrato
//! que plataforma/windows (ver plataforma/mod.rs). Guía y lista de tareas: docs/mac.md.
//!
//! Lo que ya está hecho usa herramientas que trae macOS (open, sysctl, ps, kill, afplay, sips, dig,
//! security). Lo marcado con TODO(mac) falta: cada uno dice qué hacer. Mientras tanto devuelve algo
//! seguro (por ejemplo, `tie_to_launcher` no hace nada: si Parabellum se cierra de golpe, el
//! servidor sigue abierto).

pub mod accesos;
pub mod avisos;

use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use ring::aead::{Aad, LessSafeKey, Nonce, UnboundKey, CHACHA20_POLY1305, NONCE_LEN};

use super::AgentDownload;

pub const NAME: &str = "macOS";
pub const MOJANG_OS: &str = "osx";
pub const JAVA_RUNTIME_PLATFORM: &str = if cfg!(target_arch = "aarch64") { "mac-os-arm64" } else { "mac-os" };
// TODO(mac): la actualización automática reemplaza el ejecutable (catalog.rs, apply_launcher_update).
// En Mac el programa vive dentro de Parabellum.app/Contents/MacOS: hay que actualizar la .app entera.
pub const EXECUTABLE: &str = "Parabellum";
// Mientras eso falte, no se ofrece actualizar: el catálogo trae el .exe de Windows y la ventana de
// actualización obligatoria dejaría a Parabellum bloqueado. En Mac se actualiza recompilando.
pub const SELF_UPDATE: bool = false;
pub const CLASSPATH_SEPARATOR: &str = ":";
// TODO(mac): playit.gg no publica su agente para Mac (v0.17.1 solo trae Windows y Linux). Es de
// código abierto (github.com/playit-cloud/playit-agent): compilarlo para aarch64-apple-darwin,
// subirlo a un release propio y poner aquí su enlace, SHA-256 y tamaño. Sin agente, «Hostear
// partida» dice que playit.gg no está disponible en este sistema.
pub const PLAYIT_AGENT: Option<AgentDownload> = None;

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

/// TODO(mac): en Windows, el servidor y el agente de playit se cierran solos si Parabellum se cierra de
/// golpe (un «job»). En Mac no hay equivalente directo: guardar los PID y cerrarlos al arrancar si
/// quedaron vivos, o usar kqueue (NOTE_EXIT) en un proceso vigilante.
pub fn tie_to_launcher(_child: &Child) {}

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
