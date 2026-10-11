//! Publicar en el Drive del catálogo desde Parabellum: los paquetes que se exportan «Para Drive o USB»
//! y las versiones nuevas de Parabellum se suben a la carpeta del catálogo con la misma estructura que
//! en la carpeta local (versión/modpack/…, _parabellum/). Lo que ya está igual en Drive no se vuelve a
//! subir; lo que cambió se reemplaza en el mismo archivo (su enlace no cambia).
//!
//! Escribir en una carpeta que no creó la app pide el permiso completo de Drive, así que va aparte del
//! inicio de sesión normal: solo quien publica lo da (Juan); la sesión de los demás no cambia. Queda
//! cifrado para el usuario de Windows (DPAPI) en datos/drive-publicar.bin.

use md5::{Digest, Md5};
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::Config;
use crate::google::{self, GoogleAccount};
use crate::paths::Paths;

const SCOPES: &str = "openid email https://www.googleapis.com/auth/drive";
const TOKEN_FILE: &str = "drive-publicar.bin";
const FILES_URL: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD_URL: &str = "https://www.googleapis.com/upload/drive/v3/files";
const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
/// Trozos de la subida (múltiplo de 256 KB, como pide Drive).
const CHUNK: u64 = 8 * 1024 * 1024;

fn http() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("Parabellum/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(180))
            .build()
            .expect("cliente HTTP")
    })
}

fn unavailable() -> String {
    t!("No se pudo conectar con Google Drive. Revisa tu conexión a internet.", "Could not connect to Google Drive. Check your internet connection.")
}

// ================= Permiso guardado

pub fn save(paths: &Paths, refresh_token: &str) -> Result<(), String> {
    let data = google::dpapi(refresh_token.as_bytes(), true).ok_or(t!("No se pudo cifrar el permiso de Drive.", "Could not encrypt the Drive permission."))?;
    std::fs::write(paths.data.join(TOKEN_FILE), data).map_err(|e| e.to_string())
}

pub fn load(paths: &Paths) -> Option<String> {
    let data = std::fs::read(paths.data.join(TOKEN_FILE)).ok()?;
    String::from_utf8(google::dpapi(&data, false)?).ok().filter(|t| !t.is_empty())
}

pub fn clear(paths: &Paths) {
    let _ = std::fs::remove_file(paths.data.join(TOKEN_FILE));
}

// ================= Publicar

pub struct Publisher {
    account: GoogleAccount,
    root: String,
}

/// Nombre para una consulta de Drive (entre comillas simples).
fn quoted(name: &str) -> String {
    format!("'{}'", name.replace('\\', "\\\\").replace('\'', "\\'"))
}

fn md5_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Md5::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hex::encode(hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

impl Publisher {
    /// Abre el navegador para dar el permiso de escribir en Drive (con la cuenta que se elija).
    pub async fn connect(config: &Config, hint: Option<&str>) -> Result<Self, String> {
        let account = GoogleAccount::sign_in_with(config, SCOPES, hint).await?;
        Ok(Self { account, root: config.drive_folder_id.clone() })
    }

    pub async fn resume(config: &Config, refresh_token: String) -> Result<Self, String> {
        let account = GoogleAccount::resume(config, refresh_token).await?;
        Ok(Self { account, root: config.drive_folder_id.clone() })
    }

    pub fn email(&self) -> &str {
        &self.account.email
    }

    pub fn refresh_token(&self) -> &str {
        &self.account.refresh_token
    }

    /// Una respuesta de Drive con error, explicada (sobre todo la falta de permiso en la carpeta).
    fn error(&self, status: reqwest::StatusCode, body: &Value) -> String {
        let reason = body.pointer("/error/errors/0/reason").and_then(|r| r.as_str()).unwrap_or_default();
        let message = body.pointer("/error/message").and_then(|m| m.as_str()).unwrap_or_default();
        match status.as_u16() {
            401 => t!("El permiso de Drive caducó o se quitó. Vuelve a conectar Drive.", "The Drive permission expired or was removed. Connect Drive again."),
            403 if reason.contains("storageQuota") || reason.contains("quotaExceeded") => {
                t!("El Drive de {} está lleno.", "{}'s Drive is full.", self.email())
            }
            403 | 404 => t!(
                "La cuenta {} no puede escribir en la carpeta del catálogo. En Drive, compártela con esa cuenta como Editor (o conecta Drive con la cuenta dueña de la carpeta).",
                "The account {} cannot write to the catalog folder. In Drive, share it with that account as Editor (or connect Drive with the account that owns the folder).",
                self.email()
            ),
            code => t!("Google Drive respondió {code}: {message}", "Google Drive answered {code}: {message}"),
        }
    }

    async fn checked(&self, response: reqwest::Response) -> Result<Value, String> {
        let status = response.status();
        let body: Value = response.json().await.unwrap_or_default();
        if status.is_success() {
            Ok(body)
        } else {
            Err(self.error(status, &body))
        }
    }

    /// El archivo o carpeta con ese nombre dentro de `parent` (el más nuevo si hay varios).
    async fn find(&self, parent: &str, name: &str, folder: bool) -> Result<Option<Value>, String> {
        let token = self.account.access_token().await?;
        let mime = if folder { format!(" and mimeType = '{FOLDER_MIME}'") } else { format!(" and mimeType != '{FOLDER_MIME}'") };
        let query = format!("{} in parents and name = {} and trashed = false{mime}", quoted(parent), quoted(name));
        let response = http()
            .get(FILES_URL)
            .bearer_auth(token)
            .query(&[
                ("q", query.as_str()),
                ("fields", "files(id,name,size,md5Checksum,modifiedTime)"),
                ("orderBy", "modifiedTime desc"),
                ("supportsAllDrives", "true"),
                ("includeItemsFromAllDrives", "true"),
            ])
            .send()
            .await
            .map_err(|_| unavailable())?;
        let body = self.checked(response).await?;
        Ok(body["files"].get(0).cloned())
    }

    async fn list(&self, parent: &str) -> Result<Vec<Value>, String> {
        let token = self.account.access_token().await?;
        let query = format!("{} in parents and trashed = false", quoted(parent));
        let response = http()
            .get(FILES_URL)
            .bearer_auth(token)
            .query(&[("q", query.as_str()), ("fields", "files(id,name)"), ("pageSize", "1000"), ("supportsAllDrives", "true"), ("includeItemsFromAllDrives", "true")])
            .send()
            .await
            .map_err(|_| unavailable())?;
        Ok(self.checked(response).await?["files"].as_array().cloned().unwrap_or_default())
    }

    async fn folder(&self, parent: &str, name: &str) -> Result<String, String> {
        if let Some(found) = self.find(parent, name, true).await? {
            return Ok(found["id"].as_str().unwrap_or_default().to_string());
        }
        let token = self.account.access_token().await?;
        let response = http()
            .post(FILES_URL)
            .bearer_auth(token)
            .query(&[("supportsAllDrives", "true"), ("fields", "id")])
            .json(&json!({ "name": name, "mimeType": FOLDER_MIME, "parents": [parent] }))
            .send()
            .await
            .map_err(|_| unavailable())?;
        Ok(self.checked(response).await?["id"].as_str().unwrap_or_default().to_string())
    }

    /// Manda a la papelera de Drive (se puede recuperar desde ahí).
    async fn trash(&self, id: &str) -> Result<(), String> {
        let token = self.account.access_token().await?;
        let response = http()
            .patch(format!("{FILES_URL}/{id}"))
            .bearer_auth(token)
            .query(&[("supportsAllDrives", "true")])
            .json(&json!({ "trashed": true }))
            .send()
            .await
            .map_err(|_| unavailable())?;
        self.checked(response).await.map(|_| ())
    }

    /// Sube un archivo en trozos (subida reanudable): si se corta la conexión, sigue donde iba.
    /// Si ya hay uno con ese nombre se reemplaza su contenido; si es idéntico, no se sube.
    /// Devuelve si lo subió.
    async fn upload(&self, parent: &str, path: &Path, progress: &(dyn Fn(u64) + Send + Sync)) -> Result<bool, String> {
        let name = path.file_name().map(|n| n.to_string_lossy().to_string()).ok_or(t!("Archivo no válido.", "Invalid file."))?;
        let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
        let existing = self.find(parent, &name, false).await?;
        if let Some(existing) = &existing {
            let same_size = existing["size"].as_str().and_then(|s| s.parse::<u64>().ok()) == Some(size);
            if same_size {
                let local = {
                    let path = path.to_path_buf();
                    tauri::async_runtime::spawn_blocking(move || md5_file(&path)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?
                };
                if existing["md5Checksum"].as_str() == Some(local.as_str()) {
                    progress(size);
                    return Ok(false);
                }
            }
        }
        let token = self.account.access_token().await?;
        let start = match existing.as_ref().and_then(|e| e["id"].as_str()) {
            Some(id) => http().patch(format!("{UPLOAD_URL}/{id}")).json(&json!({})),
            None => http().post(UPLOAD_URL).json(&json!({ "name": name, "parents": [parent] })),
        };
        let response = start
            .bearer_auth(token)
            .query(&[("uploadType", "resumable"), ("supportsAllDrives", "true")])
            .header("X-Upload-Content-Type", "application/octet-stream")
            .header("X-Upload-Content-Length", size.to_string())
            .send()
            .await
            .map_err(|_| unavailable())?;
        if !response.status().is_success() {
            let status = response.status();
            let body: Value = response.json().await.unwrap_or_default();
            return Err(self.error(status, &body));
        }
        let session = response.headers().get("location").and_then(|l| l.to_str().ok()).map(String::from).ok_or(t!("Google Drive no abrió la subida.", "Google Drive did not open the upload."))?;

        let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        let mut offset = 0u64;
        let mut failures = 0;
        loop {
            let length = (size - offset).min(CHUNK);
            let mut chunk = vec![0u8; length as usize];
            file.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
            file.read_exact(&mut chunk).map_err(|e| e.to_string())?;
            let range = if size == 0 { "bytes */0".to_string() } else { format!("bytes {offset}-{}/{size}", offset + length - 1) };
            let sent = http().put(&session).header("Content-Range", range).body(chunk).send().await;
            let response = match sent {
                Ok(response) => response,
                Err(_) if failures < 5 => {
                    // Se cortó: Drive dice hasta dónde llegó y se sigue desde ahí.
                    failures += 1;
                    tokio::time::sleep(Duration::from_secs(2 * failures)).await;
                    offset = self.received(&session, size).await.unwrap_or(offset);
                    continue;
                }
                Err(_) => return Err(unavailable()),
            };
            match response.status().as_u16() {
                200 | 201 => {
                    progress(size - offset);
                    return Ok(true);
                }
                308 => {
                    let next = response
                        .headers()
                        .get("range")
                        .and_then(|r| r.to_str().ok())
                        .and_then(|r| r.rsplit('-').next())
                        .and_then(|end| end.parse::<u64>().ok())
                        .map(|end| end + 1)
                        .unwrap_or(offset + length);
                    progress(next.saturating_sub(offset));
                    offset = next;
                    failures = 0;
                }
                code if code >= 500 && failures < 5 => {
                    failures += 1;
                    tokio::time::sleep(Duration::from_secs(2 * failures)).await;
                    offset = self.received(&session, size).await.unwrap_or(offset);
                }
                _ => {
                    let status = response.status();
                    let body: Value = response.json().await.unwrap_or_default();
                    return Err(self.error(status, &body));
                }
            }
        }
    }

    /// Hasta qué byte recibió Drive una subida reanudable.
    async fn received(&self, session: &str, size: u64) -> Option<u64> {
        let response = http().put(session).header("Content-Range", format!("bytes */{size}")).send().await.ok()?;
        if response.status().as_u16() != 308 {
            return None;
        }
        Some(
            response
                .headers()
                .get("range")
                .and_then(|r| r.to_str().ok())
                .and_then(|r| r.rsplit('-').next())
                .and_then(|end| end.parse::<u64>().ok())
                .map(|end| end + 1)
                .unwrap_or(0),
        )
    }

    /// Sube `files` a la carpeta del catálogo conservando su ruta relativa a `base` (las carpetas que
    /// falten se crean). Devuelve (subidos, ya estaban iguales).
    pub async fn publish(&self, base: &Path, files: &[PathBuf], progress: impl Fn(String, f64) + Send + Sync) -> Result<(usize, usize), String> {
        if self.root.is_empty() {
            return Err(t!("Parabellum no tiene configurada la carpeta de Drive del catálogo.", "Parabellum has no catalog Drive folder configured."));
        }
        let total: u64 = files.iter().filter_map(|f| std::fs::metadata(f).ok()).map(|m| m.len()).sum::<u64>().max(1);
        let done = std::sync::atomic::AtomicU64::new(0);
        let (mut uploaded, mut same) = (0, 0);
        for file in files {
            let relative = file.strip_prefix(base).map_err(|_| t!("El archivo no está en la carpeta de paquetes.", "The file is not in the packages folder."))?;
            let mut parent = self.root.clone();
            if let Some(dirs) = relative.parent() {
                for dir in dirs.components() {
                    parent = self.folder(&parent, &dir.as_os_str().to_string_lossy()).await?;
                }
            }
            let name = relative.display().to_string().replace('\\', "/");
            let report = |bytes: u64| {
                let now = done.fetch_add(bytes, std::sync::atomic::Ordering::Relaxed) + bytes;
                progress(
                    t!("Subiendo {name}… {:.0} de {:.0} MB", "Uploading {name}… {:.0} of {:.0} MB", now as f64 / 1048576.0, total as f64 / 1048576.0),
                    now as f64 / total as f64,
                );
            };
            if self.upload(&parent, file, &report).await? {
                uploaded += 1;
            } else {
                same += 1;
            }
        }
        Ok((uploaded, same))
    }

    /// Manda a la papelera de Drive los Parabellum-<versión>.zip/.json de la carpeta que no son los de
    /// `keep` (como en la carpeta local: solo queda la versión publicada).
    pub async fn remove_old_launchers(&self, folder: &str, keep: &[String]) -> Result<usize, String> {
        let mut parent = self.root.clone();
        parent = match self.find(&parent, folder, true).await? {
            Some(found) => found["id"].as_str().unwrap_or_default().to_string(),
            None => return Ok(0),
        };
        let mut removed = 0;
        for file in self.list(&parent).await? {
            let name = file["name"].as_str().unwrap_or_default();
            if name.starts_with("Parabellum-") && !keep.iter().any(|k| k == name) {
                if let Some(id) = file["id"].as_str() {
                    self.trash(id).await?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}

/// Los archivos de una carpeta local (sin subcarpetas).
pub fn files_in(folder: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(folder).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_en_consultas_de_drive() {
        assert_eq!(quoted("Better MC [FORGE] - BMC4"), "'Better MC [FORGE] - BMC4'");
        assert_eq!(quoted("Juan's pack"), "'Juan\\'s pack'");
        assert_eq!(quoted("a\\b"), "'a\\\\b'");
    }

    #[test]
    fn huella_md5_de_un_archivo() {
        let path = std::env::temp_dir().join(format!("parabellum-md5-{}", std::process::id()));
        std::fs::write(&path, b"hola").unwrap();
        assert_eq!(md5_file(&path).unwrap(), "4d186321c1a7f0f354b297e8914ab240");
        let _ = std::fs::remove_file(&path);
    }
}
