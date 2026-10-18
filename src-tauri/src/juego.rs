//! El mod de Parabellum dentro del juego (código en `mod/` del repositorio; los .jar compilados, en
//! `src-tauri/juego/mods/`). Al jugar se pone en la carpeta mods de la instancia el que toca según el
//! loader y la versión, y al cerrar el juego se quita: así no se cuela en los servidores, en los
//! modpacks exportados ni en las instancias compartidas. Las fuentes (Inter) van a datos/fuentes.

use crate::instances::Instance;
use crate::paths::Paths;

/// El nombre del .jar en la carpeta mods (los listados de Parabellum lo esconden).
pub const FILE_NAME: &str = "parabellum-juego.jar";

struct Build {
    /// "Fabric" (también sirve en Quilt), "Forge" o "NeoForge".
    loader: &'static str,
    /// Versiones de Minecraft: desde (incluida) hasta (sin incluir).
    from: &'static str,
    until: &'static str,
    jar: &'static [u8],
}

/// La tabla la genera mod/compilar.py junto con los .jar.
const BUILDS: &[Build] = include!("../juego/mods/lista.rs");

const FONTS: &[(&str, &[u8])] = &[
    ("Inter-Regular.ttf", include_bytes!("../juego/fuentes/Inter-Regular.ttf")),
    ("Inter-SemiBold.ttf", include_bytes!("../juego/fuentes/Inter-SemiBold.ttf")),
    ("OFL.txt", include_bytes!("../juego/fuentes/OFL.txt")),
];

/// "1.21.1" → [1, 21, 1]; "26.1-pre2" → [26, 1].
fn numbers(version: &str) -> Vec<u32> {
    version.split(['-', ' ', '+']).next().unwrap_or_default().split('.').map(|n| n.parse().unwrap_or(0)).collect()
}

fn compare(a: &str, b: &str) -> std::cmp::Ordering {
    let (a, b) = (numbers(a), numbers(b));
    for i in 0..a.len().max(b.len()) {
        let order = a.get(i).copied().unwrap_or(0).cmp(&b.get(i).copied().unwrap_or(0));
        if order.is_ne() {
            return order;
        }
    }
    std::cmp::Ordering::Equal
}

fn build_for(loader: &str, minecraft: &str) -> Option<&'static Build> {
    let loader = if loader == "Quilt" { "Fabric" } else { loader };
    BUILDS.iter().find(|b| b.loader == loader && compare(minecraft, b.from).is_ge() && compare(minecraft, b.until).is_lt())
}

/// Marca de que el mod hizo caer el juego en esta instancia (con la versión de Parabellum de ese momento).
const DISABLED_FILE: &str = "sin-panel.txt";

/// ¿Se apagó el mod en esta instancia porque hizo caer el juego? Solo cuenta con la misma versión de
/// Parabellum: una nueva puede traerlo arreglado y se vuelve a intentar.
fn disabled_by_crash(instance: &Instance) -> bool {
    std::fs::read_to_string(instance.state_dir().join(DISABLED_FILE)).is_ok_and(|v| v.trim() == env!("CARGO_PKG_VERSION"))
}

/// Antes de abrir el juego: pone el mod (si hay para su versión y está activado) o lo quita.
pub fn install(instance: &Instance, enabled: bool) {
    let path = instance.game_dir.join("mods").join(FILE_NAME);
    match build_for(&instance.loader, &instance.minecraft_version).filter(|_| enabled && !disabled_by_crash(instance)) {
        Some(build) => {
            free_panel_key(&instance.minecraft_version, &instance.game_dir.join("options.txt"));
            let same = std::fs::metadata(&path).is_ok_and(|m| m.len() == build.jar.len() as u64) && std::fs::read(&path).is_ok_and(|b| b == build.jar);
            if !same {
                let _ = std::fs::create_dir_all(instance.game_dir.join("mods"));
                if let Err(error) = std::fs::write(&path, build.jar) {
                    eprintln!("juego: no se pudo poner el mod ({error})");
                }
            }
        }
        None => remove(instance),
    }
}

/// Desde 26.1 la «Lista de amigos» de Mojang también viene en la O, la tecla del panel: mientras las dos
/// sigan ahí, la de Mojang pasa a «]» (como en el modpack de Drive; lo vio Diego, PR #15). Solo si ya hay
/// options.txt: la primera partida lo crea y se arregla en la siguiente.
fn free_panel_key(minecraft: &str, options: &std::path::Path) {
    let year = minecraft.split('.').next().and_then(|y| y.parse::<u32>().ok()).unwrap_or(0);
    if year < 26 {
        return;
    }
    let Ok(text) = std::fs::read_to_string(options) else { return };
    let value = |key: &str| text.lines().find_map(|l| l.strip_prefix(key)?.strip_prefix(':')).map(str::trim);
    const O: &str = "key.keyboard.o";
    if value("key_key.parabellum.abrir").unwrap_or(O) != O || value("key_key.friends").unwrap_or(O) != O {
        return;
    }
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.starts_with("key_key.friends:")).collect();
    lines.push("key_key.friends:key.keyboard.right.bracket");
    let _ = std::fs::write(options, lines.join("\n") + "\n");
}

/// Si el juego se cayó y el error menciona al mod, no se vuelve a poner en esa instancia: el juego de
/// cada quien importa más que el panel. Devuelve true si se apagó.
pub fn after_crash(instance: &Instance, started_ms: i64) -> bool {
    let mut texts = Vec::new();
    // El informe de fallo de esta partida, si lo hubo.
    if let Ok(entries) = std::fs::read_dir(instance.game_dir.join("crash-reports")) {
        for entry in entries.flatten() {
            let recent = entry.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).is_some_and(|d| d.as_millis() as i64 >= started_ms - 2000);
            if recent {
                texts.push(std::fs::read_to_string(entry.path()).unwrap_or_default());
            }
        }
    }
    // Y el registro (Forge muestra los errores de carga en una pantalla, sin informe).
    let log = std::fs::read(instance.game_dir.join("logs").join("latest.log")).unwrap_or_default();
    let tail = &log[log.len().saturating_sub(400_000)..];
    texts.push(String::from_utf8_lossy(tail).to_string());
    let ours = texts.iter().any(|t| t.lines().any(caused_by_us));
    if ours {
        let _ = std::fs::create_dir_all(instance.state_dir());
        let _ = std::fs::write(instance.state_dir().join(DISABLED_FILE), env!("CARGO_PKG_VERSION"));
    }
    ours
}

/// Un renglón del informe o del registro que culpa al mod: código suyo en la pila del error, o el
/// cargador diciendo que no pudo cargarlo. (Que solo se mencione el .jar, como hacen algunos avisos de
/// Forge, no cuenta.)
fn caused_by_us(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.contains("at mx.parabellum.juego") || (lower.contains("parabellum") && (lower.contains("failed to load") || lower.contains("mod loading has failed") || lower.contains("incompatible mod")))
}

/// Al cerrar el juego (y al abrir Parabellum, por si quedó de una vez que se cerró de golpe).
pub fn remove(instance: &Instance) {
    let path = instance.game_dir.join("mods").join(FILE_NAME);
    if path.exists() {
        let _ = std::fs::remove_file(path);
    }
}

/// Las fuentes del panel (Inter, como en Parabellum), en datos/fuentes.
pub fn write_fonts(paths: &Paths) {
    let folder = paths.data.join("fuentes");
    let _ = std::fs::create_dir_all(&folder);
    for (name, bytes) in FONTS {
        let path = folder.join(name);
        if std::fs::metadata(&path).map(|m| m.len()).ok() != Some(bytes.len() as u64) {
            let _ = std::fs::write(path, bytes);
        }
    }
}

/// Un archivo de la carpeta mods que es el de Parabellum (no se lista ni se copia a servidores).
pub fn is_ours(name: &str) -> bool {
    name.eq_ignore_ascii_case(FILE_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tecla_del_panel_en_26() {
        let folder = std::env::temp_dir().join(format!("parabellum-tecla-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let options = folder.join("options.txt");
        // 26.x con las dos en la O (por defecto): la de Mojang pasa a «]».
        std::fs::write(&options, "version:4556\nkey_key.jump:key.keyboard.space\n").unwrap();
        free_panel_key("26.3", &options);
        assert!(std::fs::read_to_string(&options).unwrap().contains("key_key.friends:key.keyboard.right.bracket"));
        // Si el jugador movió la del panel, no se toca nada.
        std::fs::write(&options, "key_key.parabellum.abrir:key.keyboard.p\nkey_key.friends:key.keyboard.o\n").unwrap();
        free_panel_key("26.3", &options);
        assert!(std::fs::read_to_string(&options).unwrap().contains("key_key.friends:key.keyboard.o"));
        // Antes de 26 no existe esa tecla de Mojang.
        std::fs::write(&options, "version:3955\n").unwrap();
        free_panel_key("1.21.1", &options);
        assert_eq!(std::fs::read_to_string(&options).unwrap(), "version:3955\n");
        let _ = std::fs::remove_dir_all(&folder);
    }

    #[test]
    fn culpa_del_mod() {
        assert!(caused_by_us("	at mx.parabellum.juego.Panel.pintar(Panel.java:120)"));
        assert!(caused_by_us("Mod parabellum failed to load correctly"));
        assert!(!caused_by_us("[ModernFix/]: PathResourcePack base class instantiated with root path of mod file parabellum-juego.jar."));
        assert!(!caused_by_us("java.lang.NullPointerException: Cannot invoke \"Object.toString()\""));
    }

    #[test]
    fn versiones() {
        assert!(build_for("Fabric", "1.21.1").is_some());
        assert!(build_for("Quilt", "1.21").is_some());
        assert!(build_for("Fabric", "1.21.2").is_some());
        assert!(build_for("Forge", "1.17.1").is_none());
        // NeoForge 1.20.1 todavía carga los mods de Forge.
        assert!(build_for("NeoForge", "1.20.1").is_some_and(|b| std::ptr::eq(b.jar, build_for("Forge", "1.20.1").unwrap().jar)));
        assert!(build_for("Vanilla", "1.21.1").is_none());
        assert!(compare("26.1.2", "1.21.11").is_gt());
        assert!(compare("1.21", "1.21.0").is_eq());
    }
}
