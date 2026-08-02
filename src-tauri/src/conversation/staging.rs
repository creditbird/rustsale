use super::error::{ConversationError, ConversationErrorCode};
use super::repository::{ConversationRepository, StagedAttachmentRecord};
use super::types::{utc_now, StageConversationClipboardImageRequest, StagedAttachmentResponse};
use chrono::{Duration, Utc};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_BATCH_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_CLIPBOARD_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_ATTACHMENTS_PER_BATCH: usize = 10;
pub const MAX_SPOOL_BYTES: u64 = 1024 * 1024 * 1024;
pub const STAGED_TTL_HOURS: i64 = 24;

pub type ConversationResult<T> = Result<T, ConversationError>;

#[derive(Debug, Clone)]
pub struct StagingCleanupReport {
    pub cleaned_count: usize,
    pub cleaned_bytes: u64,
}

#[derive(Clone)]
pub struct ConversationAttachmentStager {
    root: PathBuf,
    repository: Arc<ConversationRepository>,
}

impl ConversationAttachmentStager {
    pub fn new(root: PathBuf, repository: Arc<ConversationRepository>) -> Self {
        if let Err(e) = fs::create_dir_all(&root) {
            eprintln!("Failed to create staging root directory {:?}: {}", root, e);
        }
        Self { root, repository }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub async fn stage_paths(
        &self,
        conversation_id: i64,
        paths: Vec<String>,
    ) -> ConversationResult<Vec<StagedAttachmentResponse>> {
        if paths.len() > MAX_ATTACHMENTS_PER_BATCH {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!(
                    "Maximum {} attachments per batch allowed",
                    MAX_ATTACHMENTS_PER_BATCH
                ),
            ));
        }

        let stager = self.clone();
        tokio::task::spawn_blocking(move || stager.stage_paths_sync(conversation_id, paths))
            .await
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?
    }

    fn stage_paths_sync(
        &self,
        conversation_id: i64,
        paths: Vec<String>,
    ) -> ConversationResult<Vec<StagedAttachmentResponse>> {
        let mut sources = Vec::with_capacity(paths.len());
        let mut batch_bytes = 0_u64;

        for source_path_str in paths {
            let source_path = PathBuf::from(&source_path_str);
            let metadata = fs::symlink_metadata(&source_path).map_err(|_| {
                ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    "Failed to access attachment source",
                )
            })?;

            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(ConversationError::new(
                    ConversationErrorCode::UnsupportedMedia,
                    "Attachment must be a regular file",
                ));
            }

            let file_size = metadata.len();
            if file_size > MAX_ATTACHMENT_BYTES {
                return Err(ConversationError::new(
                    ConversationErrorCode::FileTooLarge,
                    "Attachment exceeds the 50 MiB application limit",
                ));
            }

            batch_bytes = batch_bytes.checked_add(file_size).ok_or_else(|| {
                ConversationError::new(
                    ConversationErrorCode::FileTooLarge,
                    "Attachment batch exceeds the 100 MiB application limit",
                )
            })?;
            if batch_bytes > MAX_BATCH_BYTES {
                return Err(ConversationError::new(
                    ConversationErrorCode::FileTooLarge,
                    "Attachment batch exceeds the 100 MiB application limit",
                ));
            }

            let file_name = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("attachment")
                .to_string();
            sources.push((source_path, file_name, file_size));
        }

        let current_spool_bytes = self.repository.active_spool_bytes().map_err(|error| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, error.to_string())
        })?;
        if current_spool_bytes.saturating_add(batch_bytes) > MAX_SPOOL_BYTES {
            return Err(ConversationError::new(
                ConversationErrorCode::FileTooLarge,
                "Staging spool quota of 1 GiB exceeded",
            ));
        }

        let mut results: Vec<StagedAttachmentResponse> = Vec::new();
        for (source_path, file_name, file_size) in sources {
            let staged_id = format!(
                "stage-{:016x}{:016x}",
                rand::random::<u64>(),
                rand::random::<u64>()
            );
            let spool_filename = format!("{}.dat", staged_id);
            let target_path = self.root.join(&spool_filename);

            if let Err(error) = fs::copy(&source_path, &target_path) {
                for staged in &results {
                    let _ = self.discard_sync(&staged.staged_attachment_id);
                }
                return Err(ConversationError::new(
                    ConversationErrorCode::ProviderFailure,
                    format!("Failed to copy attachment into the staging spool: {error}"),
                ));
            }

            let staged = match self.inspect_and_register(
                conversation_id,
                &staged_id,
                &file_name,
                &target_path,
                file_size,
            ) {
                Ok(s) => s,
                Err(e) => {
                    let _ = fs::remove_file(&target_path);
                    for staged in &results {
                        let _ = self.discard_sync(&staged.staged_attachment_id);
                    }
                    return Err(e);
                }
            };

            results.push(staged);
        }

        Ok(results)
    }

    pub async fn stage_clipboard_image(
        &self,
        request: StageConversationClipboardImageRequest,
    ) -> ConversationResult<StagedAttachmentResponse> {
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            request.base64_data.trim(),
        )
        .map_err(|e| {
            ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!("Invalid base64 payload: {}", e),
            )
        })?;

        let file_size = decoded.len() as u64;
        if file_size > MAX_CLIPBOARD_IMAGE_BYTES {
            return Err(ConversationError::new(
                ConversationErrorCode::FileTooLarge,
                "Clipboard image exceeds 10 MiB limit",
            ));
        }

        let stager = self.clone();
        tokio::task::spawn_blocking(move || {
            stager.stage_clipboard_image_sync(request.conversation_id, &request.file_name, decoded)
        })
        .await
        .map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
        })?
    }

    fn stage_clipboard_image_sync(
        &self,
        conversation_id: i64,
        file_name: &str,
        data: Vec<u8>,
    ) -> ConversationResult<StagedAttachmentResponse> {
        let file_size = data.len() as u64;
        let current_spool_bytes = self.repository.active_spool_bytes().map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
        })?;

        if current_spool_bytes + file_size > MAX_SPOOL_BYTES {
            return Err(ConversationError::new(
                ConversationErrorCode::FileTooLarge,
                "Staging spool quota of 1 GiB exceeded",
            ));
        }

        let staged_id = format!(
            "stage-{:016x}{:016x}",
            rand::random::<u64>(),
            rand::random::<u64>()
        );
        let spool_filename = format!("{}.dat", staged_id);
        let target_path = self.root.join(&spool_filename);

        fs::write(&target_path, &data).map_err(|e| {
            ConversationError::new(
                ConversationErrorCode::ProviderFailure,
                format!("Failed to write clipboard image: {}", e),
            )
        })?;

        match self.inspect_and_register(
            conversation_id,
            &staged_id,
            file_name,
            &target_path,
            file_size,
        ) {
            Ok(s) => Ok(s),
            Err(e) => {
                let _ = fs::remove_file(&target_path);
                Err(e)
            }
        }
    }

    fn inspect_and_register(
        &self,
        conversation_id: i64,
        staged_id: &str,
        file_name: &str,
        target_path: &Path,
        file_size: u64,
    ) -> ConversationResult<StagedAttachmentResponse> {
        let inferred_mime = infer::get_from_path(target_path)
            .ok()
            .flatten()
            .map(|kind| kind.mime_type().to_string())
            .unwrap_or_else(|| {
                let ext = Path::new(file_name)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                match ext.as_str() {
                    "png" => "image/png".to_string(),
                    "jpg" | "jpeg" => "image/jpeg".to_string(),
                    "webp" => "image/webp".to_string(),
                    "gif" => "image/gif".to_string(),
                    "mp4" => "video/mp4".to_string(),
                    "mp3" => "audio/mpeg".to_string(),
                    "pdf" => "application/pdf".to_string(),
                    "txt" => "text/plain".to_string(),
                    _ => "application/octet-stream".to_string(),
                }
            });

        // Executable signature check
        if inferred_mime == "application/x-executable"
            || inferred_mime == "application/x-dosexec"
            || inferred_mime == "application/x-sharedlib"
        {
            return Err(ConversationError::new(
                ConversationErrorCode::UnsupportedMedia,
                "Executable files are not supported as attachments",
            ));
        }

        let media_type = if inferred_mime.starts_with("image/") {
            "image"
        } else if inferred_mime.starts_with("video/") {
            "video"
        } else if inferred_mime.starts_with("audio/") {
            "audio"
        } else {
            "file"
        };

        let mut width = None;
        let mut height = None;

        if media_type == "image" {
            if let Ok(img) = image::open(target_path) {
                width = Some(img.width());
                height = Some(img.height());
            }
        }

        let created_at = utc_now();
        let expires_at = (Utc::now() + Duration::hours(STAGED_TTL_HOURS))
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();

        let record = StagedAttachmentRecord {
            id: format!(
                "record-{:016x}{:016x}",
                rand::random::<u64>(),
                rand::random::<u64>()
            ),
            conversation_id,
            staged_attachment_id: staged_id.to_string(),
            media_type: media_type.to_string(),
            mime_type: inferred_mime.clone(),
            file_name: file_name.to_string(),
            file_size_bytes: file_size,
            local_path: target_path.to_string_lossy().to_string(),
            thumbnail_path: None,
            width,
            height,
            duration_seconds: None,
            status: "staged".to_string(),
            expires_at: expires_at.clone(),
            created_at: created_at.clone(),
            claimed_at: None,
        };

        self.repository
            .insert_staged_attachment(&record)
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?;

        Ok(StagedAttachmentResponse {
            staged_attachment_id: staged_id.to_string(),
            media_type: media_type.to_string(),
            mime_type: inferred_mime,
            file_name: file_name.to_string(),
            file_size_bytes: file_size,
            width,
            height,
            duration_seconds: None,
            expires_at,
        })
    }

    pub async fn discard(&self, staged_attachment_id: &str) -> ConversationResult<()> {
        let stager = self.clone();
        let id_owned = staged_attachment_id.to_string();
        tokio::task::spawn_blocking(move || stager.discard_sync(&id_owned))
            .await
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?
    }

    fn discard_sync(&self, staged_attachment_id: &str) -> ConversationResult<()> {
        if let Ok(Some(record)) = self.repository.get_staged_attachment(staged_attachment_id) {
            let p = Path::new(&record.local_path);
            if p.exists() {
                let _ = fs::remove_file(p);
            }
        }
        self.repository
            .mark_staged_attachment_discarded(staged_attachment_id)
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?;
        Ok(())
    }

    pub async fn cleanup(&self, now: &str) -> ConversationResult<StagingCleanupReport> {
        let stager = self.clone();
        let now_owned = now.to_string();
        tokio::task::spawn_blocking(move || stager.cleanup_sync(&now_owned))
            .await
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?
    }

    fn cleanup_sync(&self, now: &str) -> ConversationResult<StagingCleanupReport> {
        let candidates = self
            .repository
            .list_staging_cleanup_candidates(now)
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?;

        let mut cleaned_count = 0;
        let mut cleaned_bytes = 0;

        for record in candidates {
            let p = Path::new(&record.local_path);
            if p.exists() {
                let _ = fs::remove_file(p);
            }
            cleaned_count += 1;
            cleaned_bytes += record.file_size_bytes;
        }

        Ok(StagingCleanupReport {
            cleaned_count,
            cleaned_bytes,
        })
    }
}
