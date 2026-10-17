//! Los Java incluidos en base\java (cada subcarpeta es un Java completo, como los de Mojang).

use std::path::{Path, PathBuf};

use crate::paths::Paths;

#[derive(Clone, Debug)]
pub struct JavaRuntime {
    pub home: PathBuf,
    pub name: String,
    pub major: u32,
    pub version: String,
}

impl JavaRuntime {
    /// El Java con el que se abre el juego (en Windows javaw.exe: sin ventana de consola).
    pub fn executable(&self) -> PathBuf {
        crate::plataforma::java_binary(&self.home, true)
    }
}

pub fn discover(paths: &Paths) -> Vec<JavaRuntime> {
    let Ok(entries) = std::fs::read_dir(&paths.java) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| crate::plataforma::java_binary(&e.path(), false).exists())
        .filter(|e| !interrupted(&e.path()))
        .filter_map(|e| {
            // «release» está junto a bin/ (en los Java de Mojang para Mac, dentro de jre.bundle/Contents/Home).
            let binary = crate::plataforma::java_binary(&e.path(), false);
            let home = binary.parent()?.parent()?;
            let release = std::fs::read_to_string(home.join("release")).ok()?;
            let version = release
                .lines()
                .find_map(|l| l.strip_prefix("JAVA_VERSION="))?
                .trim_matches('"')
                .to_string();
            // Un Java bajado antes de que se marcaran sus programas (en Mac, antes del PR #3) no abría
            // («Permission denied»): se arregla al encontrarlo. En Windows no hace nada.
            crate::plataforma::make_executable(&binary);
            crate::plataforma::make_executable(&home.join("lib").join("jspawnhelper"));
            Some(JavaRuntime { home: e.path(), name: e.file_name().to_string_lossy().into(), major: major_of(&version), version })
        })
        .collect()
}

/// Un Java cuya descarga se cortó: tiene la marca de mojang::DOWNLOADING o algún «.parte» sin su
/// archivo (lo que download dejaba a medias antes de la marca). Así no se usa (el juego no abriría:
/// «missing server JVM») y la próxima instalación lo completa. Un «.parte» junto a su archivo no
/// cuenta: el archivo llegó por otro lado (un paquete de Drive) y el «.parte» sobra.
fn interrupted(folder: &Path) -> bool {
    fn has_partial(folder: &Path) -> bool {
        std::fs::read_dir(folder).into_iter().flatten().flatten().any(|entry| {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                has_partial(&path)
            } else {
                path.extension().is_some_and(|e| e == "parte") && !path.with_extension("").exists()
            }
        })
    }
    folder.join(crate::mojang::DOWNLOADING).exists() || has_partial(folder)
}

/// Un paquete (base o modpack de Drive) terminó de instalarse: los Java que trae quedan completos, así
/// que se les quita la marca de mojang::DOWNLOADING y los «.parte» (las descargas no siguen donde
/// quedaron: un «.parte» nunca sirve). Si no, un Java que se cortó en casa y se completó en la escuela
/// desde Drive seguiría «a medias» y pediría Mojang, que ahí está bloqueado. `names`: las rutas del zip.
pub fn package_installed<'a>(paths: &Paths, names: impl IntoIterator<Item = &'a str>) {
    fn remove_partials(folder: &Path) {
        for entry in std::fs::read_dir(folder).into_iter().flatten().flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                remove_partials(&path);
            } else if path.extension().is_some_and(|e| e == "parte") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    let runtimes: std::collections::BTreeSet<String> = names
        .into_iter()
        .filter_map(|name| name.replace('\\', "/").strip_prefix("base/java/")?.split('/').next().map(String::from))
        .filter(|runtime| !runtime.is_empty() && runtime != "." && runtime != "..")
        .collect();
    for runtime in runtimes {
        let folder = paths.java.join(runtime);
        let _ = std::fs::remove_file(folder.join(crate::mojang::DOWNLOADING));
        remove_partials(&folder);
    }
}

/// La versión mayor de un Java: "17.0.15", "1.8.0_74" (Java 8) o "8u51-cacert462b08" (así nombra
/// Mojang a su Java 8 en la lista de Java).
pub fn major_of(version: &str) -> u32 {
    let parts: Vec<&str> = version.split(['.', '_', '+', '-', 'u']).collect();
    if parts.first() == Some(&"1") {
        if let Some(Ok(legacy)) = parts.get(1).map(|p| p.parse()) {
            return legacy; // 1.8.0_xx
        }
    }
    parts.first().and_then(|p| p.parse().ok()).unwrap_or(0)
}

/// El componente exacto que pide Mojang; si no, la misma versión mayor; si no, la más cercana por encima.
pub fn select<'a>(runtimes: &'a [JavaRuntime], component: Option<&str>, major: u32) -> Option<&'a JavaRuntime> {
    runtimes
        .iter()
        .find(|r| component.is_some_and(|c| r.name.eq_ignore_ascii_case(c)) && r.major == major)
        .or_else(|| runtimes.iter().filter(|r| r.major == major).max_by(|a, b| a.version.cmp(&b.version)))
        .or_else(|| runtimes.iter().filter(|r| r.major > major).min_by_key(|r| r.major))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_mayor() {
        assert_eq!(major_of("17.0.15"), 17);
        assert_eq!(major_of("1.8.0_74"), 8);
        assert_eq!(major_of("8u51-cacert462b08"), 8);
        assert_eq!(major_of("16.0.1.9.1_3"), 16);
        assert_eq!(major_of("25.0.1"), 25);
    }

    #[test]
    fn encuentra_el_java_de_mojang() {
        let paths = Paths::new(std::env::temp_dir().join(format!("parabellum-java-{}", std::process::id())));
        let runtime = paths.java.join("java-runtime-delta");
        // Como lo deja Mojang: en Mac, dentro de jre.bundle/Contents/Home.
        let (home, binary) = if cfg!(target_os = "macos") { (runtime.join("jre.bundle").join("Contents").join("Home"), "java") } else { (runtime.clone(), "java.exe") };
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(home.join("bin").join(binary), b"").unwrap();
        std::fs::write(home.join("release"), "JAVA_VERSION=\"21.0.7\"\n").unwrap();
        let found = discover(&paths);
        assert_eq!(found.iter().map(|r| (r.name.as_str(), r.major)).collect::<Vec<_>>(), [("java-runtime-delta", 21)]);
        // A medias no cuenta: con un «.parte» (se cortó la descarga) o con la marca de que se está bajando.
        let partial = home.join("lib").join("server");
        std::fs::create_dir_all(&partial).unwrap();
        std::fs::write(partial.join("libjvm.dylib.parte"), b"").unwrap();
        assert!(discover(&paths).is_empty());
        std::fs::remove_file(partial.join("libjvm.dylib.parte")).unwrap();
        std::fs::write(runtime.join(crate::mojang::DOWNLOADING), b"").unwrap();
        assert!(discover(&paths).is_empty());
        std::fs::remove_file(runtime.join(crate::mojang::DOWNLOADING)).unwrap();
        assert_eq!(discover(&paths).len(), 1);
        // Un «.parte» junto a su archivo (llegó completo de un paquete) no lo deja a medias.
        std::fs::write(home.join("release.parte"), b"").unwrap();
        assert_eq!(discover(&paths).len(), 1);
        // Cortado en casa (marca y «.parte» sin su archivo) y completado desde Drive: vuelve a contar.
        std::fs::write(runtime.join(crate::mojang::DOWNLOADING), b"").unwrap();
        std::fs::write(partial.join("libjvm.dylib.parte"), b"").unwrap();
        assert!(discover(&paths).is_empty());
        let zip_name = |p: &std::path::Path| format!("base/java/{}", p.strip_prefix(&paths.java).unwrap().to_string_lossy().replace('\\', "/"));
        package_installed(&paths, [zip_name(&home.join("release")).as_str(), "base/versions/1.21.1/1.21.1.json"]);
        assert_eq!(discover(&paths).len(), 1);
        assert!(!partial.join("libjvm.dylib.parte").exists() && !home.join("release.parte").exists());
        let _ = std::fs::remove_dir_all(&paths.root);
    }
}
