//! Descargas en paralelo con verificación SHA-1, reintentos y reanudación
//! (un archivo que ya existe con el tamaño correcto no se vuelve a bajar).

use futures::{stream, StreamExt};
use sha1::{Digest, Sha1};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug)]
pub struct FileDownload {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
}

impl FileDownload {
    pub fn from_json(node: &serde_json::Value, path: PathBuf) -> Option<Self> {
        Some(Self {
            url: node.get("url")?.as_str()?.to_string(),
            path,
            sha1: node.get("sha1").and_then(|v| v.as_str()).map(String::from),
            size: node.get("size").and_then(|v| v.as_u64()),
        })
    }

    fn already_present(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(meta) => self.size.is_none_or(|size| meta.len() == size),
            Err(_) => false,
        }
    }
}

pub fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent(concat!("Parabellum/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(300))
            .pool_max_idle_per_host(16)
            .build()
            .expect("cliente HTTP")
    })
}

/// Progreso: (archivos terminados, archivos totales, bytes terminados, bytes totales).
pub async fn download_all(
    files: Vec<FileDownload>,
    on_progress: impl Fn(usize, usize, u64, u64) + Send + Sync + 'static,
) -> Result<(), String> {
    let mut seen = std::collections::HashSet::new();
    let pending: Vec<FileDownload> = files
        .into_iter()
        .filter(|f| seen.insert(f.path.to_string_lossy().to_lowercase()))
        .filter(|f| !f.already_present())
        .collect();
    let total_files = pending.len();
    let total_bytes: u64 = pending.iter().filter_map(|f| f.size).sum::<u64>().max(1);
    let done_files = Arc::new(AtomicUsize::new(0));
    let done_bytes = Arc::new(AtomicU64::new(0));
    let on_progress = Arc::new(on_progress);
    on_progress(0, total_files, 0, total_bytes);

    let errors: Vec<String> = stream::iter(pending)
        .map(|file| {
            let (done_files, done_bytes, on_progress) = (done_files.clone(), done_bytes.clone(), on_progress.clone());
            async move {
                let mut last_error = String::new();
                for attempt in 1..=3u64 {
                    match download_file(&file).await {
                        Ok(()) => {
                            let bytes = done_bytes.fetch_add(file.size.unwrap_or(0), Ordering::Relaxed) + file.size.unwrap_or(0);
                            let count = done_files.fetch_add(1, Ordering::Relaxed) + 1;
                            on_progress(count, total_files, bytes, total_bytes);
                            return None;
                        }
                        Err(error) => {
                            last_error = error;
                            tokio::time::sleep(Duration::from_millis(800 * attempt)).await;
                        }
                    }
                }
                Some(format!("{}: {last_error}", file.path.file_name().unwrap_or_default().to_string_lossy()))
            }
        })
        .buffer_unordered(8)
        .filter_map(|r| async move { r })
        .collect()
        .await;

    if errors.is_empty() {
        Ok(())
    } else {
        Err(t!(
            "No se pudieron descargar {} archivos. Vuelve a intentarlo: se continúa donde quedó.\n{}", "{} files could not be downloaded. Try again: it continues where it stopped.\n{}",
            errors.len(),
            errors.iter().take(3).cloned().collect::<Vec<_>>().join("\n")
        ))
    }
}

async fn download_file(file: &FileDownload) -> Result<(), String> {
    if let Some(parent) = file.path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| e.to_string())?;
    }
    let mut response = client().get(&file.url).send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        return Err(t!("el servidor respondió {}", "the server answered {}", response.status()));
    }
    // java.exe y java.dll comparten nombre base: el temporal conserva la extensión original.
    let mut temp = file.path.clone().into_os_string();
    temp.push(".parte");
    let temp = PathBuf::from(temp);
    let mut output = tokio::fs::File::create(&temp).await.map_err(|e| e.to_string())?;
    let mut hasher = Sha1::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        hasher.update(&chunk);
        output.write_all(&chunk).await.map_err(|e| e.to_string())?;
    }
    output.flush().await.map_err(|e| e.to_string())?;
    drop(output);
    if let Some(expected) = &file.sha1 {
        if !hex::encode(hasher.finalize()).eq_ignore_ascii_case(expected) {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(t!("archivo dañado (SHA-1 distinto)", "damaged file (different SHA-1)"));
        }
    }
    tokio::fs::rename(&temp, &file.path).await.map_err(|e| e.to_string())
}

pub async fn get_text(url: &str) -> Result<String, reqwest::Error> {
    client().get(url).send().await?.error_for_status()?.text().await
}
