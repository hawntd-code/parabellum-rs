//! Los Java incluidos en base\java (cada subcarpeta es un Java completo, como los de Mojang).

use std::path::PathBuf;

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
        .filter_map(|e| {
            let release = std::fs::read_to_string(e.path().join("release")).ok()?;
            let version = release
                .lines()
                .find_map(|l| l.strip_prefix("JAVA_VERSION="))?
                .trim_matches('"')
                .to_string();
            Some(JavaRuntime { home: e.path(), name: e.file_name().to_string_lossy().into(), major: major_of(&version), version })
        })
        .collect()
}

pub fn major_of(version: &str) -> u32 {
    let parts: Vec<&str> = version.split(['.', '_', '+', '-']).collect();
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
