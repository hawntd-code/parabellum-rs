//! Firma de las versiones de Parabellum que se publican (catalog::publish_launcher) y comprobación antes de
//! actualizarse (catalog::apply_launcher_update). El zip nuevo y su .json viven en el mismo Drive: si alguien
//! entrara a esa cuenta, sin firma podría repartir un programa suyo a todos los que tienen Parabellum. Con
//! firma necesitaría además la clave privada de quien publica, que vive en su PC y nunca en el repo:
//! `%USERPROFILE%\.parabellum\firma-actualizaciones.clave` (32 bytes en base64). Se acepta una versión
//! solo si la firmó una de las claves públicas de `TRUSTED`.
//!
//! Lo firmado es «parabellum-lanzador:<versión>:<sha256 del zip>»: no se puede hacer pasar el zip de una
//! versión por otra.

use base64::Engine;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use std::path::PathBuf;

/// Las claves públicas de quien publica Parabellum. Juan: la de su PC (2026-11-09, creada con `--crear-firma`). La
/// del 2026-11-02 (`kKSSerol…`) se perdió y ya no se acepta: las 0.42 y anteriores no se actualizan solas a la 0.43.
const TRUSTED: &[&str] = &["ym5RlHzKfKgBTFFaj43OHyndub4TAk1vlpK0HfdnzJ0="];

fn message(version: &str, sha256: &str) -> Vec<u8> {
    format!("parabellum-lanzador:{version}:{}", sha256.to_ascii_lowercase()).into_bytes()
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

/// Dónde está la clave privada de quien publica (en su carpeta de usuario).
pub fn key_path() -> Option<PathBuf> {
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })?;
    Some(PathBuf::from(home).join(".parabellum").join("firma-actualizaciones.clave"))
}

fn signing_key() -> Option<SigningKey> {
    signing_key_at(&key_path()?)
}

fn signing_key_at(path: &std::path::Path) -> Option<SigningKey> {
    let text = std::fs::read_to_string(path).ok()?;
    let seed: [u8; 32] = b64().decode(text.trim()).ok()?.try_into().ok()?;
    Some(SigningKey::from_bytes(&seed))
}

/// Crea la clave privada de este PC (`--crear-firma`) y devuelve la pública, para `TRUSTED`. Nunca pisa una que
/// ya esté: perderla obliga a que los ya instalados se actualicen a mano una vez (pasó el 2026-11-09).
pub fn create_key() -> Result<String, String> {
    create_key_at(&key_path().ok_or("No se sabe cuál es la carpeta de usuario.")?)
}

fn create_key_at(path: &std::path::Path) -> Result<String, String> {
    let public = |key: SigningKey| b64().encode(key.verifying_key().to_bytes());
    if path.exists() {
        return signing_key_at(path).map(public).ok_or_else(|| format!("{} ya existe pero no es una clave válida: no se toca.", path.display()));
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(path.parent().ok_or("Ruta de la clave no válida.")?).map_err(|e| e.to_string())?;
    // create_new: si apareció una entre medio, tampoco se pisa.
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut file, b64().encode(seed).as_bytes()).map_err(|e| e.to_string())?;
    drop(file);
    signing_key_at(path).map(public).ok_or_else(|| "La clave nueva no se pudo leer.".into())
}

/// La firma de esa versión, o None si en este PC no está la clave privada.
pub fn sign(version: &str, sha256: &str) -> Option<String> {
    let key = signing_key()?;
    Some(b64().encode(key.sign(&message(version, sha256)).to_bytes()))
}

/// La clave pública de este PC, si tiene la privada (para agregarla a TRUSTED).
pub fn public_key() -> Option<String> {
    signing_key().map(|key| b64().encode(key.verifying_key().to_bytes()))
}

fn verify_with(keys: &[&str], version: &str, sha256: &str, signature: Option<&str>) -> Result<(), String> {
    let unsigned = || t!(
        "La actualización {} no viene firmada por Parabellum: no se instala. Descárgala desde la página oficial.",
        "Update {} is not signed by Parabellum: it is not installed. Download it from the official page.",
        version
    );
    let signature = signature.and_then(|s| b64().decode(s.trim()).ok()).and_then(|bytes| <[u8; 64]>::try_from(bytes).ok()).map(|b| Signature::from_bytes(&b)).ok_or_else(unsigned)?;
    let message = message(version, sha256);
    let trusted = keys.iter().filter_map(|k| b64().decode(k).ok()).filter_map(|b| <[u8; 32]>::try_from(b).ok()).filter_map(|b| VerifyingKey::from_bytes(&b).ok());
    for key in trusted {
        if key.verify(&message, &signature).is_ok() {
            return Ok(());
        }
    }
    Err(unsigned())
}

/// Que esa versión la haya firmado quien publica Parabellum.
pub fn verify(version: &str, sha256: &str, signature: Option<&str>) -> Result<(), String> {
    verify_with(TRUSTED, version, sha256, signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solo_vale_la_firma_de_esa_version_y_ese_zip() {
        let key = SigningKey::from_bytes(&[7u8; 32]);
        let public = b64().encode(key.verifying_key().to_bytes());
        let keys = [public.as_str()];
        let signature = b64().encode(key.sign(&message("0.42.0", "ABCD")).to_bytes());
        assert!(verify_with(&keys, "0.42.0", "abcd", Some(&signature)).is_ok(), "la huella sin importar mayúsculas");
        assert!(verify_with(&keys, "0.43.0", "abcd", Some(&signature)).is_err(), "otra versión no");
        assert!(verify_with(&keys, "0.42.0", "abce", Some(&signature)).is_err(), "otro zip no");
        assert!(verify_with(&keys, "0.42.0", "abcd", None).is_err(), "sin firma no");
        assert!(verify_with(&keys, "0.42.0", "abcd", Some("no es base64!")).is_err());
        let other = b64().encode(SigningKey::from_bytes(&[8u8; 32]).sign(&message("0.42.0", "abcd")).to_bytes());
        assert!(verify_with(&keys, "0.42.0", "abcd", Some(&other)).is_err(), "otra clave no");
        assert!(TRUSTED.iter().all(|k| b64().decode(k).is_ok_and(|b| b.len() == 32)));
    }

    #[test]
    fn crear_la_clave_nunca_pisa_una_que_ya_esta() {
        let root = std::env::temp_dir().join(format!("parabellum-firma-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let path = root.join(".parabellum").join("firma-actualizaciones.clave");
        let public = create_key_at(&path).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert_eq!(b64().decode(saved.trim()).unwrap().len(), 32);
        // La segunda vez devuelve la misma pública y el archivo queda igual.
        assert_eq!(create_key_at(&path).unwrap(), public);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), saved);
        // Firma y se verifica con esa pública.
        let key = signing_key_at(&path).unwrap();
        let signature = b64().encode(key.sign(&message("0.43.0", "abcd")).to_bytes());
        assert!(verify_with(&[public.as_str()], "0.43.0", "abcd", Some(&signature)).is_ok());
        // Un archivo que no es una clave tampoco se toca.
        std::fs::write(&path, "otra cosa").unwrap();
        assert!(create_key_at(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "otra cosa");
        let _ = std::fs::remove_dir_all(&root);
    }
}
