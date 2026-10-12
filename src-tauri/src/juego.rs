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

/// Antes de abrir el juego: pone el mod (si hay para su versión y está activado) o lo quita.
pub fn install(instance: &Instance, enabled: bool) {
    let path = instance.game_dir.join("mods").join(FILE_NAME);
    match build_for(&instance.loader, &instance.minecraft_version).filter(|_| enabled) {
        Some(build) => {
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
    fn versiones() {
        assert!(build_for("Fabric", "1.21.1").is_some());
        assert!(build_for("Quilt", "1.21").is_some());
        assert!(build_for("Fabric", "1.21.2").is_none());
        assert!(build_for("Vanilla", "1.21.1").is_none());
        assert!(compare("26.1.2", "1.21.11").is_gt());
        assert!(compare("1.21", "1.21.0").is_eq());
    }
}
