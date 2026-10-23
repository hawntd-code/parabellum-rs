//! Drive privado para mundos: permiso independiente, propiedades de la app y subidas persistentes.
use crate::{
    config::Config,
    google::{self, GoogleAccount},
    mundos_archivo::{self as archive, Revision},
    paths::Paths,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

const SCOPES: &str = "openid email https://www.googleapis.com/auth/drive.file";
const TOKEN_FILE: &str = "drive-mundos.bin";
const FILES: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
const CHUNK: u64 = 8 * 1024 * 1024;
const MIME: &str = "application/vnd.google-apps.folder";
#[derive(Clone, Serialize, Deserialize)]
pub struct Saved {
    pub refresh: String,
    pub owner: String,
    pub email: String,
}
pub fn save(paths: &Paths, saved: &Saved) -> Result<(), String> {
    let bytes = serde_json::to_vec(saved).map_err(|e| e.to_string())?;
    let encrypted = google::dpapi(&bytes, true).ok_or_else(|| {
        t!(
            "No se pudo guardar el permiso de mundos.",
            "Could not save the worlds permission."
        )
    })?;
    std::fs::write(paths.data.join(TOKEN_FILE), encrypted).map_err(|e| e.to_string())
}
pub fn load(paths: &Paths) -> Option<Saved> {
    serde_json::from_slice(&google::dpapi(
        &std::fs::read(paths.data.join(TOKEN_FILE)).ok()?,
        false,
    )?)
    .ok()
}
pub fn clear(paths: &Paths) -> Result<(), String> {
    match std::fs::remove_file(paths.data.join(TOKEN_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
pub struct Client {
    account: Option<GoogleAccount>,
    pub owner: String,
    pub email: String,
    http: reqwest::Client,
    files: String,
    upload: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub file: String,
    pub revision: Revision,
    pub bytes: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub revision: Revision,
    pub file: Option<String>,
    pub session: Option<String>,
    pub paused: bool,
}
fn quoted(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
fn safe_file(id: &str) -> Result<(), String> {
    if id.is_empty()
        || id.len() > 180
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        Err(t!(
            "Identificador de Drive no válido.",
            "Invalid Drive identifier."
        ))
    } else {
        Ok(())
    }
}
fn protected_url(url: &str) -> Result<String, String> {
    google::dpapi(url.as_bytes(), true)
        .map(|b| base64::engine::general_purpose::STANDARD.encode(b))
        .ok_or_else(|| {
            t!(
                "No se pudo guardar la subida pendiente.",
                "Could not save the pending upload."
            )
        })
}
fn unprotected_url(value: &str) -> Option<String> {
    String::from_utf8(google::dpapi(
        &base64::engine::general_purpose::STANDARD
            .decode(value)
            .ok()?,
        false,
    )?)
    .ok()
}
fn error(status: reqwest::StatusCode) -> String {
    match status.as_u16() {
        401=>t!("Vuelve a conectar el Drive de mundos. ⟦accion:conectar-mundos⟧","Reconnect your worlds Drive. ⟦accion:conectar-mundos⟧"),
        403=>t!("Drive no permite esta operación. Revisa el espacio o vuelve a conectar los mundos. ⟦accion:recursos⟧","Drive does not allow this operation. Check storage or reconnect worlds. ⟦accion:recursos⟧"),
        404=>t!("La copia ya no está disponible en Drive. ⟦accion:reintentar⟧","The backup is no longer available in Drive. ⟦accion:reintentar⟧"),
        _=>t!("No se pudo completar la operación con Drive. Puedes reintentar sin perder la copia pendiente. ⟦accion:reintentar⟧","Could not complete the Drive operation. You can retry without losing the pending backup. ⟦accion:reintentar⟧"),
    }
}
fn network(_: reqwest::Error) -> String {
    t!(
        "No se pudo conectar con Drive. La copia pendiente se conserva. ⟦accion:reintentar⟧",
        "Could not connect to Drive. The pending backup is preserved. ⟦accion:reintentar⟧"
    )
}
impl Client {
    fn new(
        account: Option<GoogleAccount>,
        owner: String,
        email: String,
        files: String,
        upload: String,
    ) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(180))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let url = attempt.url();
                let host = url.host_str().unwrap_or("");
                if attempt.previous().len() < 5
                    && url.scheme() == "https"
                    && (host == "www.googleapis.com" || host.ends_with(".googleusercontent.com"))
                {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .build()
            .expect("cliente de Drive de mundos");
        Self {
            account,
            owner,
            email,
            http,
            files,
            upload,
        }
    }
    pub async fn connect(
        config: &Config,
        hint: Option<&str>,
    ) -> Result<(Arc<Self>, Saved), String> {
        let account = GoogleAccount::sign_in_with(config, SCOPES, hint).await?;
        let email = account.email.clone();
        let refresh = account.refresh_token.clone();
        let mut client = Self::new(
            Some(account),
            String::new(),
            email.clone(),
            FILES.into(),
            UPLOAD.into(),
        );
        client.owner = client.permission_id().await?;
        let saved = Saved {
            refresh,
            owner: client.owner.clone(),
            email,
        };
        Ok((Arc::new(client), saved))
    }
    pub async fn resume(config: &Config, saved: &Saved) -> Result<Arc<Self>, String> {
        let account = GoogleAccount::resume(config, saved.refresh.clone()).await?;
        if !account.email.eq_ignore_ascii_case(&saved.email) {
            return Err(t!(
                "El permiso guardado corresponde a otra cuenta de Drive.",
                "The saved permission belongs to another Drive account."
            ));
        }
        let client = Self::new(
            Some(account),
            saved.owner.clone(),
            saved.email.clone(),
            FILES.into(),
            UPLOAD.into(),
        );
        if client.permission_id().await? != saved.owner {
            return Err(t!(
                "La cuenta de Drive cambió. Vuelve a conectarla.",
                "The Drive account changed. Reconnect it."
            ));
        }
        Ok(Arc::new(client))
    }
    async fn token(&self) -> Result<String, String> {
        match &self.account {
            Some(a) => a.access_token().await,
            None => Ok("test".into()),
        }
    }
    async fn checked(&self, response: reqwest::Response) -> Result<Value, String> {
        let status = response.status();
        if !status.is_success() {
            return Err(error(status));
        }
        response.json().await.map_err(network)
    }
    async fn permission_id(&self) -> Result<String, String> {
        let body = self
            .checked(
                self.http
                    .get("https://www.googleapis.com/drive/v3/about")
                    .bearer_auth(self.token().await?)
                    .query(&[("fields", "user(permissionId)")])
                    .send()
                    .await
                    .map_err(network)?,
            )
            .await?;
        body["user"]["permissionId"]
            .as_str()
            .filter(|id| !id.is_empty())
            .map(String::from)
            .ok_or_else(|| {
                t!(
                    "Drive no devolvió la identidad de la cuenta.",
                    "Drive did not return the account identity."
                )
            })
    }
    async fn list(&self, query: &str) -> Result<Vec<Value>, String> {
        let mut result = Vec::new();
        let mut page = String::new();
        loop {
            let body=self.checked(self.http.get(&self.files).bearer_auth(self.token().await?).query(&[
                ("q",query),("fields","nextPageToken,files(id,name,size,md5Checksum,description,appProperties,mimeType)"),
                ("pageSize","1000"),("pageToken",&page)]).send().await.map_err(network)?).await?;
            result.extend(body["files"].as_array().cloned().unwrap_or_default());
            page = body["nextPageToken"].as_str().unwrap_or("").into();
            if page.is_empty() {
                break;
            }
        }
        Ok(result)
    }
    async fn metadata(&self, id: &str) -> Result<Value, String> {
        safe_file(id)?;
        self.checked(
            self.http
                .get(format!("{}/{id}", self.files))
                .bearer_auth(self.token().await?)
                .query(&[("fields", "id,size,md5Checksum,description,appProperties")])
                .send()
                .await
                .map_err(network)?,
        )
        .await
    }
    async fn create(&self, metadata: &Value) -> Result<String, String> {
        let body = self
            .checked(
                self.http
                    .post(&self.files)
                    .bearer_auth(self.token().await?)
                    .query(&[("fields", "id")])
                    .json(metadata)
                    .send()
                    .await
                    .map_err(network)?,
            )
            .await?;
        let id = body["id"].as_str().ok_or_else(|| {
            t!(
                "Drive no devolvió el identificador del archivo.",
                "Drive did not return the file identifier."
            )
        })?;
        safe_file(id)?;
        Ok(id.into())
    }
    async fn folder(
        &self,
        name: &str,
        tag: &str,
        parent: Option<&str>,
        world: Option<&str>,
    ) -> Result<String, String> {
        let mut query=format!("trashed = false and 'me' in owners and mimeType = '{MIME}' and appProperties has {{ key='pbl' and value={} }}",quoted(tag));
        if let Some(parent) = parent {
            query += &format!(" and {} in parents", quoted(parent));
        }
        if let Some(world) = world {
            query += &format!(
                " and appProperties has {{ key='world' and value={} }}",
                quoted(world)
            );
        }
        if let Some(id) = self
            .list(&query)
            .await?
            .first()
            .and_then(|f| f["id"].as_str())
        {
            safe_file(id)?;
            return Ok(id.into());
        }
        let mut props = json!({"pbl":tag});
        if let Some(world) = world {
            props["world"] = json!(world);
        }
        let mut body = json!({"name":name,"mimeType":MIME,"appProperties":props});
        if let Some(parent) = parent {
            body["parents"] = json!([parent]);
        }
        self.create(&body).await
    }
    async fn world_folder(&self, revision: &Revision) -> Result<String, String> {
        let root = self
            .folder("Parabellum", "worlds-root-v1", None, None)
            .await?;
        let worlds = self
            .folder("Mundos", "worlds-v1", Some(&root), None)
            .await?;
        self.folder(
            &revision.name,
            "world-v1",
            Some(&worlds),
            Some(&revision.world_id),
        )
        .await
    }
    fn parse(&self, file: &Value, confirmed: bool) -> Option<Entry> {
        if file["appProperties"]["pbl"] != "revision-v1"
            || (confirmed && file["appProperties"]["state"] != "confirmed")
        {
            return None;
        }
        let revision: Revision = serde_json::from_str(file["description"].as_str()?).ok()?;
        if revision.owner != self.owner
            || !archive::valid_id(&revision.id)
            || !archive::valid_id(&revision.world_id)
            || revision
                .parents
                .iter()
                .any(|p| !archive::valid_id(p) || *p == revision.id)
            || file["appProperties"]["world"] != revision.world_id
            || file["appProperties"]["revision"] != revision.id
        {
            return None;
        }
        Some(Entry {
            file: file["id"].as_str()?.into(),
            bytes: file["size"]
                .as_str()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0),
            revision,
        })
    }
    pub async fn revisions(&self) -> Result<Vec<Entry>, String> {
        let files = self
            .list("trashed = false and appProperties has { key='pbl' and value='revision-v1' }")
            .await?;
        Ok(files.iter().filter_map(|f| self.parse(f, true)).collect())
    }
    pub async fn upload(
        &self,
        path: &Path,
        pending: &mut Pending,
        state_file: &Path,
        progress: impl Fn(f64),
    ) -> Result<Entry, String> {
        if pending.revision.owner != self.owner {
            return Err(t!(
                "Esta copia pendiente pertenece a otra cuenta.",
                "This pending backup belongs to another account."
            ));
        }
        let (sha, md5) = archive::hashes(path)?;
        if sha != pending.revision.sha256 || md5 != pending.revision.md5 {
            return Err(t!(
                "La copia pendiente cambió. Crea una copia nueva.",
                "The pending backup changed. Create a new backup."
            ));
        }
        let total = pending.revision.size;
        if total == 0 || total > archive::MAX_CLOUD {
            return Err(t!(
                "La copia supera el límite de 5 GB.",
                "The backup exceeds the 5 GB limit."
            ));
        }
        let file = if let Some(file) = &pending.file {
            file.clone()
        } else {
            let found = self
                .list(&format!(
                    "trashed = false and appProperties has {{ key='revision' and value={} }}",
                    quoted(&pending.revision.id)
                ))
                .await?;
            let file = if let Some(existing) = found.iter().find(|f| self.parse(f, false).is_some())
            {
                existing["id"].as_str().unwrap().into()
            } else {
                let parent = self.world_folder(&pending.revision).await?;
                self.create(&json!({"name":format!("{}.zip",pending.revision.id),"mimeType":"application/zip","parents":[parent],
                    "description":serde_json::to_string(&pending.revision).unwrap(),"appProperties":{"pbl":"revision-v1","world":pending.revision.world_id,"revision":pending.revision.id,"state":"pending"}})).await?
            };
            pending.file = Some(file.clone());
            archive::atomic(state_file, pending)?;
            file
        };
        let metadata = self.metadata(&file).await?;
        if self.parse(&metadata, false).is_none() {
            return Err(t!(
                "El archivo pendiente no pertenece a los mundos de Parabellum.",
                "The pending file does not belong to Parabellum worlds."
            ));
        }
        if metadata["md5Checksum"] != md5 {
            let mut offset = 0u64;
            let mut session = pending.session.as_deref().and_then(unprotected_url);
            if let Some(url) = session.as_ref() {
                self.validate_session(url)?;
                let response = self
                    .http
                    .put(url)
                    .bearer_auth(self.token().await?)
                    .header("Content-Length", "0")
                    .header("Content-Range", format!("bytes */{total}"))
                    .send()
                    .await
                    .map_err(network)?;
                if response.status().is_success() {
                    offset = total
                } else if response.status().as_u16() == 308 {
                    offset = received(response.headers(), total)?;
                } else if matches!(response.status().as_u16(), 404 | 410) {
                    session = None;
                } else {
                    return Err(error(response.status()));
                }
            }
            if session.is_none() {
                let response = self
                    .http
                    .patch(format!("{}/{file}", self.upload))
                    .bearer_auth(self.token().await?)
                    .query(&[("uploadType", "resumable")])
                    .header("X-Upload-Content-Type", "application/zip")
                    .header("X-Upload-Content-Length", total)
                    .json(&json!({}))
                    .send()
                    .await
                    .map_err(network)?;
                if !response.status().is_success() {
                    return Err(error(response.status()));
                }
                let url = response
                    .headers()
                    .get("Location")
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| {
                        t!(
                            "Drive no devolvió la sesión de subida.",
                            "Drive did not return the upload session."
                        )
                    })?
                    .to_string();
                self.validate_session(&url)?;
                pending.session = Some(protected_url(&url)?);
                archive::atomic(state_file, pending)?;
                session = Some(url);
                offset = 0;
            }
            let url = session.unwrap();
            let mut source = tokio::fs::File::open(path)
                .await
                .map_err(|e| e.to_string())?;
            while offset < total {
                crate::descargas::check()?;
                source
                    .seek(std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| e.to_string())?;
                let count = (total - offset).min(CHUNK) as usize;
                let mut buffer = vec![0u8; count];
                source
                    .read_exact(&mut buffer)
                    .await
                    .map_err(|e| e.to_string())?;
                let response = self
                    .http
                    .put(&url)
                    .bearer_auth(self.token().await?)
                    .header("Content-Length", count)
                    .header(
                        "Content-Range",
                        format!("bytes {}-{}/{total}", offset, offset + count as u64 - 1),
                    )
                    .body(buffer)
                    .send()
                    .await
                    .map_err(network)?;
                let next = if response.status().is_success() {
                    total
                } else if response.status().as_u16() == 308 {
                    received(response.headers(), total)?
                } else {
                    return Err(error(response.status()));
                };
                if next <= offset || next > offset + count as u64 {
                    return Err(t!(
                        "Drive confirmó un intervalo de subida no válido.",
                        "Drive acknowledged an invalid upload range."
                    ));
                }
                offset = next;
                crate::descargas::received(count).await;
                progress(offset as f64 / total as f64);
                archive::atomic(state_file, pending)?;
            }
        }
        let verified = self.metadata(&file).await?;
        if verified["md5Checksum"] != md5
            || verified["size"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                != Some(total)
        {
            return Err(t!(
                "Drive no confirmó la huella de la copia. La subida se conserva para reintentar.",
                "Drive did not confirm the backup checksum. The upload is preserved for retry."
            ));
        }
        self.checked(
            self.http
                .patch(format!("{}/{file}", self.files))
                .bearer_auth(self.token().await?)
                .json(&json!({"appProperties":{"state":"confirmed"}}))
                .send()
                .await
                .map_err(network)?,
        )
        .await?;
        Ok(Entry {
            file,
            revision: pending.revision.clone(),
            bytes: total,
        })
    }
    fn validate_session(&self, url: &str) -> Result<(), String> {
        let parsed =
            reqwest::Url::parse(url).map_err(|_| "Sesión de subida no válida".to_string())?;
        let valid = parsed.scheme() == "https"
            && parsed.host_str() == Some("www.googleapis.com")
            && parsed.path().starts_with("/upload/drive/");
        #[cfg(test)]
        let valid = valid
            || (parsed.scheme() == "http"
                && parsed.host_str() == Some("127.0.0.1")
                && url.starts_with(&self.upload));
        if valid {
            Ok(())
        } else {
            Err(t!(
                "Dirección de subida no válida.",
                "Invalid upload address."
            ))
        }
    }
    pub async fn download(
        &self,
        entry: &Entry,
        path: &Path,
        progress: impl Fn(f64),
    ) -> Result<(), String> {
        let id = entry
            .revision
            .archive_file
            .as_deref()
            .unwrap_or(&entry.file);
        safe_file(id)?;
        let metadata = self.metadata(id).await?;
        if self.parse(&metadata, true).is_none_or(|r| {
            r.revision.world_id != entry.revision.world_id
                || r.revision.sha256 != entry.revision.sha256
        }) {
            return Err(t!(
                "La revisión no referencia una copia confirmada.",
                "The revision does not reference a confirmed backup."
            ));
        }
        let response = self
            .http
            .get(format!("{}/{id}", self.files))
            .bearer_auth(self.token().await?)
            .query(&[("alt", "media")])
            .send()
            .await
            .map_err(network)?;
        if !response.status().is_success() {
            return Err(error(response.status()));
        }
        let mut response = response;
        let mut file = tokio::fs::File::create(path)
            .await
            .map_err(|e| e.to_string())?;
        let mut done = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            crate::descargas::check()?;
            done += chunk.len() as u64;
            if done > entry.revision.size || done > archive::MAX_CLOUD {
                return Err(t!(
                    "La descarga supera el tamaño esperado.",
                    "The download exceeds its expected size."
                ));
            }
            file.write_all(&chunk).await.map_err(|e| e.to_string())?;
            crate::descargas::received(chunk.len()).await;
            progress(done as f64 / entry.revision.size.max(1) as f64);
        }
        file.sync_all().await.map_err(|e| e.to_string())?;
        if done != entry.revision.size {
            return Err(t!(
                "La descarga quedó incompleta.",
                "The download is incomplete."
            ));
        }
        Ok(())
    }
    pub async fn resolve(
        &self,
        chosen: &Entry,
        parents: Vec<String>,
        device: String,
    ) -> Result<Entry, String> {
        let mut revision = chosen.revision.clone();
        revision.content_revision = Some(chosen.revision.data_revision().into());
        revision.archive_file = Some(
            chosen
                .revision
                .archive_file
                .clone()
                .unwrap_or_else(|| chosen.file.clone()),
        );
        revision.id = archive::id();
        revision.parents = parents;
        revision.at = chrono::Utc::now().timestamp_millis();
        revision.device = device;
        let parent = self.world_folder(&revision).await?;
        let file=self.create(&json!({"name":format!("{}.json",revision.id),"mimeType":"application/json","parents":[parent],
            "description":serde_json::to_string(&revision).unwrap(),"appProperties":{"pbl":"revision-v1","world":revision.world_id,"revision":revision.id,"state":"confirmed"}})).await?;
        Ok(Entry {
            file,
            revision,
            bytes: 0,
        })
    }
    pub async fn prune(&self, reserve: u64) -> Result<(), String> {
        let mut entries = self.revisions().await?;
        entries.sort_by_key(|e| e.revision.at);
        let retained = archive::retained(
            &entries
                .iter()
                .map(|e| (e.file.clone(), e.revision.clone()))
                .collect::<Vec<_>>(),
        );
        let mut total = entries.iter().map(|e| e.bytes).sum::<u64>();
        let mut removed = std::collections::HashSet::new();
        for entry in entries.iter().filter(|e| !retained.contains(&e.file)) {
            self.trash(entry).await?;
            removed.insert(entry.file.clone());
            total = total.saturating_sub(entry.bytes);
        }
        if total.saturating_add(reserve) > archive::MAX_CLOUD {
            let heads: std::collections::HashSet<String> = entries
                .iter()
                .filter(|e| !removed.contains(&e.file))
                .filter(|e| {
                    !entries
                        .iter()
                        .any(|other| other.revision.parents.contains(&e.revision.id))
                })
                .flat_map(|e| {
                    [
                        e.file.clone(),
                        e.revision
                            .archive_file
                            .clone()
                            .unwrap_or_else(|| e.file.clone()),
                    ]
                })
                .collect();
            let referenced: std::collections::HashSet<_> = entries
                .iter()
                .filter_map(|e| e.revision.archive_file.clone())
                .collect();
            for entry in entries.iter().filter(|e| {
                !removed.contains(&e.file)
                    && !heads.contains(&e.file)
                    && !referenced.contains(&e.file)
            }) {
                if total.saturating_add(reserve) <= archive::MAX_CLOUD {
                    break;
                }
                self.trash(entry).await?;
                total = total.saturating_sub(entry.bytes);
            }
        }
        if total.saturating_add(reserve) > archive::MAX_CLOUD {
            return Err(t!("Los mundos protegidos y la copia nueva superan 5 GB. Conserva las ramas de conflicto antes de liberar espacio. ⟦accion:recursos⟧","Protected worlds and the new backup exceed 5 GB. Preserve conflicting branches before freeing space. ⟦accion:recursos⟧"));
        }
        Ok(())
    }
    async fn trash(&self, entry: &Entry) -> Result<(), String> {
        let body = self.metadata(&entry.file).await?;
        if self.parse(&body, true).is_none() {
            return Err(t!(
                "Solo se limpian copias confirmadas de Parabellum.",
                "Only confirmed Parabellum backups can be cleaned up."
            ));
        }
        self.checked(
            self.http
                .patch(format!("{}/{}", self.files, entry.file))
                .bearer_auth(self.token().await?)
                .json(&json!({"trashed":true}))
                .send()
                .await
                .map_err(network)?,
        )
        .await
        .map(|_| ())
    }
}
fn received(headers: &reqwest::header::HeaderMap, total: u64) -> Result<u64, String> {
    let Some(range) = headers.get("Range") else {
        return Ok(0);
    };
    range
        .to_str()
        .ok()
        .and_then(|r| r.strip_prefix("bytes=0-"))
        .and_then(|r| r.parse::<u64>().ok())
        .and_then(|n| n.checked_add(1))
        .filter(|n| *n <= total)
        .ok_or_else(|| {
            t!(
                "Drive devolvió un intervalo no válido.",
                "Drive returned an invalid range."
            )
        })
}
pub fn pending_path(paths: &Paths, id: &str) -> PathBuf {
    paths
        .data
        .join("mundos")
        .join("pendientes")
        .join(format!("{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn intervalos_no_saltan_bytes_ni_desbordan() {
        let mut h = reqwest::header::HeaderMap::new();
        assert_eq!(received(&h, 10).unwrap(), 0);
        for (range, expected) in [
            ("bytes=0-0", Some(1)),
            ("bytes=0-9", Some(10)),
            ("bytes=1-9", None),
            ("bytes=0-10", None),
            ("bytes=0-18446744073709551615", None),
        ] {
            h.insert("Range", range.parse().unwrap());
            assert_eq!(received(&h, 10).ok(), expected);
        }
    }
    #[test]
    fn solo_acepta_revisiones_de_la_cuenta_y_con_propiedades_coincidentes() {
        let c = Client::new(
            None,
            "cuenta".into(),
            String::new(),
            FILES.into(),
            UPLOAD.into(),
        );
        let r = Revision {
            id: archive::id(),
            world_id: archive::id(),
            owner: "cuenta".into(),
            ..Default::default()
        };
        let mut f = json!({"id":"f","size":"7","description":serde_json::to_string(&r).unwrap(),"appProperties":{"pbl":"revision-v1","state":"confirmed","revision":r.id,"world":r.world_id}});
        assert!(c.parse(&f, true).is_some());
        f["appProperties"]["world"] = json!(archive::id());
        assert!(c.parse(&f, true).is_none());
        f["appProperties"]["world"] = json!(r.world_id);
        f["appProperties"]["state"] = json!("pending");
        assert!(c.parse(&f, true).is_none());
        let other = Client::new(
            None,
            "otra".into(),
            String::new(),
            FILES.into(),
            UPLOAD.into(),
        );
        assert!(other.parse(&f, false).is_none());
        assert!(c
            .validate_session("https://malicioso.test/upload/drive/v3/files")
            .is_err());
        assert!(c
            .validate_session("http://www.googleapis.com/upload/drive/v3/files")
            .is_err());
    }
}

// Windows prueba además la persistencia cifrada real de la sesión (DPAPI), sin credenciales de Google.
#[cfg(all(test, windows))]
mod upload_tests {
    use super::*;
    use std::io::{Read, Write};
    struct Step {
        method: &'static str,
        path: &'static str,
        range: Option<String>,
        status: u16,
        headers: String,
        body: Value,
    }
    fn step(method: &'static str, path: &'static str, status: u16, body: Value) -> Step {
        Step {
            method,
            path,
            status,
            body,
            headers: String::new(),
            range: None,
        }
    }
    struct Server {
        base: String,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Server {
        fn start(build: impl FnOnce(&str) -> Vec<Step>) -> Self {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let steps = build(&base);
            let thread = std::thread::spawn(move || {
                for expected in steps {
                    let deadline = std::time::Instant::now() + Duration::from_secs(15);
                    let mut stream = loop {
                        match listener.accept() {
                            Ok((s, _)) => break s,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                assert!(
                                    std::time::Instant::now() < deadline,
                                    "faltó {} {}",
                                    expected.method,
                                    expected.path
                                );
                                std::thread::sleep(Duration::from_millis(5));
                            }
                            Err(e) => panic!("{e}"),
                        }
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(10)))
                        .unwrap();
                    let mut header = Vec::new();
                    let mut byte = [0u8; 1];
                    while !header.ends_with(b"\r\n\r\n") {
                        stream.read_exact(&mut byte).unwrap();
                        header.push(byte[0]);
                        assert!(header.len() < 64 * 1024);
                    }
                    let header = String::from_utf8(header).unwrap();
                    let first = header.lines().next().unwrap();
                    assert!(
                        first.starts_with(&format!("{} {}", expected.method, expected.path)),
                        "{first}"
                    );
                    let lowered = header.to_ascii_lowercase();
                    let count = lowered
                        .lines()
                        .find_map(|l| {
                            l.strip_prefix("content-length:")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if let Some(range) = &expected.range {
                        assert!(
                            lowered.contains(&format!("content-range: {}", range.to_lowercase())),
                            "{header}"
                        );
                    }
                    let mut data = vec![0; count];
                    stream.read_exact(&mut data).unwrap();
                    let body = serde_json::to_vec(&expected.body).unwrap();
                    write!(stream,"HTTP/1.1 {} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n",expected.status,body.len(),expected.headers).unwrap();
                    stream.write_all(&body).unwrap();
                }
            });
            Self {
                base,
                thread: Some(thread),
            }
        }
        fn done(mut self) {
            self.thread.take().unwrap().join().unwrap();
        }
    }
    fn fixture(bytes: usize) -> (PathBuf, PathBuf, Revision, Value) {
        let root = std::env::temp_dir().join(format!("pbl-subida39-{}", archive::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("copia.zip");
        std::fs::write(&path, vec![71u8; bytes]).unwrap();
        let (sha, md5) = archive::hashes(&path).unwrap();
        let r = Revision {
            id: archive::id(),
            world_id: archive::id(),
            owner: "cuenta".into(),
            size: bytes as u64,
            sha256: sha,
            md5,
            ..Default::default()
        };
        let f = json!({"id":"archivo","size":r.size.to_string(),"md5Checksum":r.md5,"description":serde_json::to_string(&r).unwrap(),"appProperties":{"pbl":"revision-v1","revision":r.id,"world":r.world_id,"state":"pending"}});
        (root, path, r, f)
    }
    #[tokio::test]
    async fn corte_y_reinicio_reanudan_el_mismo_archivo_desde_el_byte_confirmado() {
        let (root, path, r, metadata) = fixture(CHUNK as usize + 262144);
        let mut empty = metadata.clone();
        empty.as_object_mut().unwrap().remove("md5Checksum");
        let server = Server::start(|base| {
            let mut start = step("PATCH", "/upload/archivo", 200, json!({}));
            start.headers = format!("Location: {base}/upload/sesion\r\n");
            let mut first = step("PUT", "/upload/sesion", 308, json!({}));
            first.range = Some(format!("bytes 0-{}/{total}", CHUNK - 1, total = r.size));
            first.headers = format!("Range: bytes=0-{}\r\n", CHUNK - 1);
            let mut query = step("PUT", "/upload/sesion", 308, json!({}));
            query.range = Some(format!("bytes */{}", r.size));
            query.headers = first.headers.clone();
            let mut last = step("PUT", "/upload/sesion", 200, json!({}));
            last.range = Some(format!("bytes {}-{}/{}", CHUNK, r.size - 1, r.size));
            vec![
                step("GET", "/files/archivo", 200, empty.clone()),
                start,
                first,
                step("PUT", "/upload/sesion", 500, json!({})),
                step("GET", "/files/archivo", 200, empty),
                query,
                last,
                step("GET", "/files/archivo", 200, metadata),
                step("PATCH", "/files/archivo", 200, json!({"id":"archivo"})),
            ]
        });
        let client = Client::new(
            None,
            "cuenta".into(),
            String::new(),
            format!("{}/files", server.base),
            format!("{}/upload", server.base),
        );
        let state = root.join("pendiente.json");
        let mut pending = Pending {
            revision: r,
            file: Some("archivo".into()),
            session: None,
            paused: false,
        };
        assert!(client
            .upload(&path, &mut pending, &state, |_| {})
            .await
            .is_err());
        let saved = std::fs::read_to_string(&state).unwrap();
        assert!(!saved.contains("127.0.0.1"));
        let mut restarted: Pending = serde_json::from_str(&saved).unwrap();
        let result = client
            .upload(&path, &mut restarted, &state, |_| {})
            .await
            .unwrap();
        assert_eq!(result.file, "archivo");
        server.done();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn sesion_expirada_se_renueva_sin_crear_otro_archivo() {
        let (root, path, r, metadata) = fixture(300000);
        let mut empty = metadata.clone();
        empty.as_object_mut().unwrap().remove("md5Checksum");
        let server = Server::start(|base| {
            let mut start = step("PATCH", "/upload/archivo", 200, json!({}));
            start.headers = format!("Location: {base}/upload/nueva\r\n");
            vec![
                step("GET", "/files/archivo", 200, empty),
                step("PUT", "/upload/expirada", 404, json!({})),
                start,
                step("PUT", "/upload/nueva", 200, json!({})),
                step("GET", "/files/archivo", 200, metadata),
                step("PATCH", "/files/archivo", 200, json!({})),
            ]
        });
        let client = Client::new(
            None,
            "cuenta".into(),
            String::new(),
            format!("{}/files", server.base),
            format!("{}/upload", server.base),
        );
        let mut pending = Pending {
            revision: r,
            file: Some("archivo".into()),
            session: Some(protected_url(&format!("{}/upload/expirada", server.base)).unwrap()),
            paused: false,
        };
        client
            .upload(&path, &mut pending, &root.join("estado.json"), |_| {})
            .await
            .unwrap();
        server.done();
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn huella_incorrecta_nunca_se_confirma_y_listado_recorre_paginas() {
        let (root, path, r, metadata) = fixture(300);
        let mut bad = metadata.clone();
        bad["md5Checksum"] = json!("incorrecto");
        let server = Server::start(|_| {
            vec![
                step("GET", "/files/archivo", 200, metadata),
                step("GET", "/files/archivo", 200, bad),
                step(
                    "GET",
                    "/files?",
                    200,
                    json!({"files":[],"nextPageToken":"segunda"}),
                ),
                step("GET", "/files?", 200, json!({"files":[]})),
            ]
        });
        let client = Client::new(
            None,
            "cuenta".into(),
            String::new(),
            format!("{}/files", server.base),
            format!("{}/upload", server.base),
        );
        let mut pending = Pending {
            revision: r,
            file: Some("archivo".into()),
            session: None,
            paused: false,
        };
        assert!(client
            .upload(&path, &mut pending, &root.join("estado.json"), |_| {})
            .await
            .is_err());
        assert!(client.revisions().await.unwrap().is_empty());
        server.done();
        std::fs::remove_dir_all(root).unwrap();
    }
}
