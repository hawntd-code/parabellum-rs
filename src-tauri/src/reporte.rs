//! «Reportar problema»: un resumen corto de lo que hace falta para ayudar con un fallo, que cabe en un
//! mensaje del chat (lo escribe el jugador arriba): versión de Parabellum y del sistema, la instancia y
//! su último arranque, el error, lo que encontró el doctor y lo último del P2P.
//! Va en español siempre: lo lee quien da soporte (Juan), como «📷 Imagen» en la base de datos.

use std::path::Path;

use crate::diagnostico::{self, CrashContext};
use crate::instances::Instance;
use crate::paths::Paths;

/// Lo más largo que puede ser el resumen: el mensaje entero es de 2000 letras y arriba va lo que
/// escribe el jugador (hasta 400).
pub const MAX: usize = 1500;

/// Corta un texto a `max` letras (sin partir un carácter).
fn cut(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(max - 1).collect::<String>())
    }
}

/// Las últimas líneas no vacías de un archivo (sin leerlo entero si es grande).
fn last_lines(path: &Path, count: usize) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else { return Vec::new() };
    let tail = &bytes[bytes.len().saturating_sub(16_000)..];
    let text = String::from_utf8_lossy(tail);
    let lines: Vec<String> = text.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect();
    lines[lines.len().saturating_sub(count)..].to_vec()
}

/// Hace cuánto, en palabras.
fn ago(ms: i64) -> String {
    let minutes = (chrono::Utc::now().timestamp_millis() - ms).max(0) / 60_000;
    match minutes {
        0 => "hace un momento".into(),
        1..=59 => format!("hace {minutes} min"),
        60..=1439 => format!("hace {} h", minutes / 60),
        _ => format!("hace {} días", minutes / 1440),
    }
}

/// El resumen. `instance`: la que falla (o la última que se jugó). Revisa los mods con el doctor, así
/// que puede tardar unos segundos con un modpack grande.
pub fn summary(paths: &Paths, instance: Option<&Instance>, ram_gb: u32, system_ram_gb: u32) -> String {
    let mut lines = vec![format!(
        "🛠 Parabellum {} · {} ({}) · {system_ram_gb} GB de RAM",
        env!("CARGO_PKG_VERSION"),
        match std::env::consts::OS {
            "windows" => "Windows",
            "macos" => "Mac",
            other => other,
        },
        std::env::consts::ARCH
    )];
    match instance {
        Some(instance) => {
            let loader = match (&instance.loader[..], &instance.loader_version) {
                ("Vanilla", _) => "Vanilla".to_string(),
                (loader, Some(version)) => format!("{loader} {version}"),
                (loader, None) => loader.to_string(),
            };
            lines.push(format!("Instancia: {} · Minecraft {} · {loader} · {ram_gb} GB", cut(&instance.name, 60), instance.minecraft_version));
            let since = instance.last_played.unwrap_or(0);
            if since > 0 {
                lines.push(format!("Se abrió {}", ago(since)));
            }
            // El último error del registro solo si el juego se cayó (si no, sale ruido de siempre, como el
            // de Realms en las cuentas no premium).
            if diagnostico::crash_report_since(instance, since) {
                if let Some(reason) = crate::launch::crash_reason(instance, since) {
                    let reason = reason.replace("<log4j:Throwable><![CDATA[", "").replace("]]>", "");
                    lines.push(format!("Error:\n{}", cut(&reason, 420)));
                }
            }
            let diagnosis = diagnostico::after_crash(instance, &CrashContext { since_ms: since, exit_code: None, ram_gb, system_ram_gb });
            for finding in diagnosis.findings.iter().filter(|f| f.severity != "info").take(2) {
                lines.push(format!("Doctor: {}", cut(&finding.title, 160)));
                if let Some(evidence) = &finding.evidence {
                    lines.push(format!("  {}", cut(evidence, 200)));
                }
            }
        }
        None => lines.push("Sin instancias jugadas todavía.".into()),
    }
    // Lo último del P2P (quién entró, directo o por relevo, rechazos, saltos), sin la fecha ni los «listo».
    let p2p: Vec<String> = last_lines(&paths.data.join("logs").join("p2p.log"), 40)
        .into_iter()
        .map(|l| l.get(11..).unwrap_or(&l).to_string())
        .filter(|l| !l.get(9..).unwrap_or_default().starts_with("listo"))
        .map(|l| cut(&l, 140))
        .collect();
    let p2p = &p2p[p2p.len().saturating_sub(3)..];
    if !p2p.is_empty() {
        lines.push(format!("P2P:\n{}", p2p.join("\n")));
    }
    cut(&redact(&lines.join("\n")), MAX)
}

/// Sin datos personales (docs/plan-diciembre.md, 7.1): la carpeta del usuario pasa a %USERPROFILE% (o ~),
/// y los correos, tokens y claves se tapan. Lo usa el resumen y lo que se manda al soporte.
pub fn redact(text: &str) -> String {
    use std::sync::OnceLock;
    static PATTERNS: OnceLock<Vec<(regex::Regex, &'static str)>> = OnceLock::new();
    let mut out = text.to_string();
    let patterns = PATTERNS.get_or_init(|| {
        vec![
            // X:\Users\<nombre> (cualquier unidad o usuario; el nombre puede tener espacios, no lo que Windows
            // prohíbe) y la carpeta de usuario de Mac.
            (regex::Regex::new(r#"(?i)\b[a-z]:[\\/]+users[\\/]+[^\\/:*?"<>|\r\n]+?([\\/])"#).unwrap(), "%USERPROFILE%${1}"),
            // Sin barra al final, la puntuación ya es de la frase.
            (regex::Regex::new(r#"(?i)\b[a-z]:[\\/]+users[\\/]+[^\\/:*?"<>|\s,;)\]]+"#).unwrap(), "%USERPROFILE%"),
            (regex::Regex::new(r"/Users/[^/\s]+").unwrap(), "~"),
            (regex::Regex::new(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}").unwrap(), "[correo]"),
            // Tokens (JWT, Google) y claves
            (regex::Regex::new(r"eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*").unwrap(), "[oculto]"),
            (regex::Regex::new(r"ya29\.[A-Za-z0-9_-]{10,}").unwrap(), "[oculto]"),
            (regex::Regex::new(r"AIza[0-9A-Za-z_-]{20,}").unwrap(), "[oculto]"),
            (regex::Regex::new(r"(?i)\b((?:access_|refresh_|id_)?token|auth|key|password|secret)=([^&\s]+)").unwrap(), "$1=[oculto]"),
        ]
    });
    for (pattern, replacement) in patterns {
        out = pattern.replace_all(&out, *replacement).into_owned();
    }
    // La carpeta del usuario si no está en X:\Users (otra unidad, Linux), como venga escrita.
    for (var, label) in [("USERPROFILE", "%USERPROFILE%"), ("HOME", "~")] {
        if let Some(home) = std::env::var_os(var).map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 3) {
            for form in [home.clone(), home.replace('\\', "/"), home.replace('\\', "\\\\")] {
                out = replace_ignore_case(&out, &form, label);
            }
        }
    }
    out
}

/// Reemplaza sin distinguir mayúsculas (las rutas de Windows vienen de cualquier forma).
fn replace_ignore_case(text: &str, needle: &str, replacement: &str) -> String {
    let (lower, needle_lower) = (text.to_lowercase(), needle.to_lowercase());
    // Si pasar a minúsculas cambia el largo (letras raras), uno exacto.
    if needle.is_empty() || lower.len() != text.len() || needle_lower.len() != needle.len() {
        return text.replace(needle, replacement);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = 0;
    while let Some(found) = lower[rest..].find(&needle_lower) {
        let end = rest + found + needle.len();
        // Solo si la carpeta termina ahí (C:\Users\Juan no tapa la mitad de C:\Users\Juana).
        let whole = !text[end..].starts_with(|c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ' '));
        out.push_str(&text[rest..rest + found]);
        out.push_str(if whole { replacement } else { &text[rest + found..end] });
        rest = end;
    }
    out.push_str(&text[rest..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_datos_personales() {
        let text = redact(r"Error en C:\Users\Juan Perez\AppData\x.jar y c:/users/otro/minecraft · juan@uabc.edu.mx · auth=eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTYifQ.abc · key=AIzaSyA1234567890abcdefghijk · /Users/diego/Library");
        assert!(!text.contains("Juan Perez"), "{text}");
        assert!(!text.contains("otro"), "{text}");
        assert!(!text.contains("juan@uabc"), "{text}");
        assert!(!text.contains("eyJ"), "{text}");
        assert!(!text.contains("AIza"), "{text}");
        assert!(!text.contains("diego"), "{text}");
        assert!(text.contains(r"%USERPROFILE%\AppData\x.jar"), "{text}");
        assert!(text.contains("[correo]"), "{text}");
        assert!(text.contains("%USERPROFILE%/minecraft"), "{text}");
        assert_eq!(redact(r"en C:\Users\Ana, ver"), "en %USERPROFILE%, ver");
        assert_eq!(replace_ignore_case(r"C:\Users\Juana y c:\users\juan\x", r"C:\Users\Juan", "~"), r"C:\Users\Juana y ~\x");
    }

    #[test]
    fn resumen_corto() {
        assert_eq!(cut("  hola  ", 10), "hola");
        assert_eq!(cut("abcdef", 4), "abc…");
        assert_eq!(cut("ñandúes", 3), "ña…");
        let root = std::env::temp_dir().join(format!("parabellum-reporte-{}", std::process::id()));
        let paths = Paths::new(root.clone());
        std::fs::create_dir_all(paths.data.join("logs")).unwrap();
        std::fs::write(paths.data.join("logs").join("p2p.log"), "2026-10-12 19:38:00 David se conectó: directo (36 ms)\n2026-10-12 19:40:00 listo (e07f4daf8a)\n").unwrap();
        let text = summary(&paths, None, 4, 16);
        let _ = std::fs::remove_dir_all(&root);
        assert!(text.starts_with("🛠 Parabellum "));
        assert!(text.contains("P2P:\n19:38:00 David se conectó: directo (36 ms)"), "{text}");
        assert!(text.chars().count() <= MAX);
    }
}
