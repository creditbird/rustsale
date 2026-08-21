# Facebook Integration Phase 2 Implementation Plan: Post Scheduler, Resumable Video & Reels, Live Feed Previews & Planner

- **Goal:** Deliver full Facebook post publishing and scheduling capabilities across all 6 post types (`status`, `link`, `photo`, `multi_photo`, `video`, `reel`), restart-resumable chunked video and Reel uploaders with exact server offset verification, dual-mode scheduler (Meta-side 10m–30d & local-queued >30d) with per-row token resolution, fail-closed atomic reconciliation with Graph status-type matching, and Svelte 5 Live Feed Preview.
- **Architecture & Invariants:**
  - **Six-Type Multi-Publisher:** `FacebookPublisher::publish_post` supports status, link, photo, multi-photo, video, and reel.
  - **Restart-Resumable Video & Reel Uploader:** `facebook_upload_sessions` persists active uploads. On resume, queries Meta's authoritative server offset and strictly validates bounds (`server_offset > offset && server_offset <= file_size`). Reels verifies `confirmed_offset == offset + bytes_to_read as u64`.
  - **Fail-Closed Promotion Worker:** `promote_local_queued_posts` claims rows with 5-minute atomic leases, resolves token and page ID per claimed row, strictly fails closed on corrupt timestamps (`error_code='invalid_timestamp'`), and transitions transport/5xx/empty-id errors to `delivery_unknown`.
  - **Multi-Endpoint Reconciliation:** Scans both `/scheduled_posts` and `/feed`, matches `post_type` to Graph `status_type`, normalizes numeric and ISO timestamps, sets `published` for `/feed` and `scheduled_on_meta` for `/scheduled_posts`.

---
### Task 1: Complete 6-Type Post Publisher (`src-tauri/src/facebook/publisher.rs`)

```rust
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::prelude::*;
use keyring::Entry;
use crate::facebook::uploader::FacebookUploader;

pub struct FacebookTokenVault;

impl FacebookTokenVault {
    fn get_master_key() -> Result<[u8; 32]> {
        let entry = Entry::new("rustsale", "facebook_vault_key")
            .map_err(|e| anyhow!("Failed to access OS keyring: {}", e))?;
        match entry.get_password() {
            Ok(secret) => {
                let mut key = [0u8; 32];
                let decoded = BASE64_STANDARD.decode(secret.trim())
                    .map_err(|e| anyhow!("Invalid base64 in keyring secret: {}", e))?;
                if decoded.len() != 32 {
                    return Err(anyhow!("Keyring master key length is not 32 bytes"));
                }
                key.copy_from_slice(&decoded);
                Ok(key)
            }
            Err(keyring::Error::NoEntry) => {
                let mut key = [0u8; 32];
                rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
                let encoded = BASE64_STANDARD.encode(key);
                entry.set_password(&encoded)
                    .map_err(|e| anyhow!("Failed to persist master key to OS keyring: {}", e))?;
                Ok(key)
            }
            Err(e) => Err(anyhow!("Keyring access failed: {}", e)),
        }
    }

    pub fn encrypt_token(plain_token: &str) -> Result<String> {
        let key = Self::get_master_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("AES-256-GCM init error: {}", e))?;
        let mut nonce_bytes = [0u8; 12];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher.encrypt(nonce, plain_token.as_bytes())
            .map_err(|e| anyhow!("Token encryption error: {}", e))?;
        let mut combined = Vec::with_capacity(12 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64_STANDARD.encode(combined))
    }

    pub fn decrypt_token(ciphertext_b64: &str) -> Result<String> {
        let raw = BASE64_STANDARD.decode(ciphertext_b64.trim())
            .map_err(|e| anyhow!("Base64 decode failed: {}", e))?;
        if raw.len() < 12 {
            return Err(anyhow!("Ciphertext payload too short for nonce"));
        }
        let (nonce_bytes, encrypted_payload) = raw.split_at(12);
        let key = Self::get_master_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|e| anyhow!("AES-256-GCM cipher init error: {}", e))?;
        let nonce = Nonce::from_slice(nonce_bytes);
        let decrypted_bytes = cipher.decrypt(nonce, encrypted_payload)
            .map_err(|e| anyhow!("AES-256-GCM decryption failed: {}", e))?;
        String::from_utf8(decrypted_bytes)
            .map_err(|e| anyhow!("Decrypted token is not valid UTF-8: {}", e))
    }

    pub fn encrypt_and_upsert_page_token(
        conn: &Connection,
        channel_account_id: &str,
        page_id: &str,
        page_name: &str,
        plain_token: &str,
        token_expires_at: Option<&str>,
    ) -> Result<()> {
        let encrypted_ciphertext = Self::encrypt_token(plain_token)?;
        conn.execute(
            "INSERT INTO facebook_page_tokens (channel_account_id, page_id, page_name, access_token_encrypted, token_status, token_expires_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'valid', ?5, datetime('now'))
             ON CONFLICT(page_id) DO UPDATE SET
                 channel_account_id = EXCLUDED.channel_account_id,
                 page_name = EXCLUDED.page_name,
                 access_token_encrypted = EXCLUDED.access_token_encrypted,
                 token_status = 'valid',
                 token_expires_at = EXCLUDED.token_expires_at,
                 updated_at = datetime('now')",
            params![channel_account_id, page_id, page_name, encrypted_ciphertext, token_expires_at],
        ).map_err(|e| anyhow!("Failed to upsert page token in vault: {}", e))?;
        Ok(())
    }

    pub fn get_valid_page_token(
        conn: &Connection,
        channel_account_id: &str,
        page_id: &str,
    ) -> Result<String> {
        let encrypted_token: Option<String> = conn.query_row(
            "SELECT access_token_encrypted FROM facebook_page_tokens
             WHERE channel_account_id = ?1 AND page_id = ?2 AND token_status = 'valid'",
            params![channel_account_id, page_id],
            |row| row.get(0),
        ).optional().map_err(|e| anyhow!("DB query failed for channel_account_id {} page_id {}: {}", channel_account_id, page_id, e))?;

        let ciphertext = match encrypted_token {
            Some(t) if !t.is_empty() => t,
            _ => return Err(anyhow!("No valid active token found for channel_account_id: {} page_id: {}", channel_account_id, page_id)),
        };

        Self::decrypt_token(&ciphertext)
    }
}

#[derive(Debug)]
pub enum PublishApiResult {
    Success { remote_post_id: String },
    StillProcessing { upload_session_id: String },
    DeterministicRejection { error_code: String, error_message: String },
    AmbiguousTransport { error_code: String, error_message: String },
}

pub struct FacebookPublisher {
    client: Client,
    uploader: FacebookUploader,
}

impl FacebookPublisher {
    pub fn new() -> Self {
        Self {
            client: Client::builder().timeout(std::time::Duration::from_secs(60)).build().unwrap_or_default(),
            uploader: FacebookUploader::new(),
        }
    }

    pub fn renew_post_claim(conn: &Connection, post_id: i64, owner_id: &str) -> Result<()> {
        conn.execute(
            "UPDATE facebook_scheduled_posts
             SET claim_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = 'publishing'",
            params![post_id, owner_id],
        )?;
        Ok(())
    }

    pub async fn publish_post(
        &self,
        conn: &rusqlite::Connection,
        post_id: i64,
    ) -> Result<String> {
        // 1. Authoritatively fetch and validate scheduled post row from database
        let (channel_account_id, page_id, post_type, message, link_url, media_paths_json, scheduled_publish_time_str, current_status): (
            String, String, String, String, Option<String>, String, Option<String>, String
        ) = conn.query_row(
            "SELECT channel_account_id, page_id, post_type, message, link_url, media_paths_json, scheduled_publish_time, status
             FROM facebook_scheduled_posts
             WHERE id = ?1",
            [post_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        ).map_err(|e| anyhow!("Scheduled post {} not found: {}", post_id, e))?;

        // Validate RFC3339 timestamp and convert to unix epoch seconds for Meta Graph API
        let scheduled_publish_time_epoch: Option<i64> = match scheduled_publish_time_str.as_deref() {
            Some(s) => {
                let dt = chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|e| anyhow!("Invalid RFC3339 scheduled_publish_time '{}' for post {}: {}", s, post_id, e))?;
                Some(dt.timestamp())
            }
            None => None,
        };

        // 2. Atomic single-writer claim: Transition strictly from 'queued' to 'publishing' with lease deadline
        if current_status == "delivery_unknown" {
            return Err(anyhow!("Cannot publish post {}: status is 'delivery_unknown' and must be resolved by reconciler only", post_id));
        }

        let worker_owner = format!("worker_{}", std::process::id());
        let rows_claimed = conn.execute(
            "UPDATE facebook_scheduled_posts
             SET status = 'publishing',
                 owner_id = ?1,
                 claim_deadline = datetime('now', '+5 minutes'),
                 updated_at = datetime('now')
             WHERE id = ?2 AND status = 'queued'",
            params![worker_owner, post_id],
        )?;

        if rows_claimed != 1 {
            let active_status: Option<String> = conn.query_row(
                "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| r.get(0),
            ).optional()?;

            match active_status.as_deref() {
                Some("publishing") => return Err(anyhow!("Cannot publish post {}: already being processed by another worker", post_id)),
                Some("delivery_unknown") => return Err(anyhow!("Cannot publish post {}: status transitioned to 'delivery_unknown' and must be resolved by reconciler only", post_id)),
                Some(st) => return Err(anyhow!("Cannot publish post {}: invalid status '{}'", post_id, st)),
                None => return Err(anyhow!("Scheduled post {} not found", post_id)),
            }
        }

        // Authoritatively fetch and decrypt active Page token via local AES-256-GCM TokenVault helper joining account + page + valid
        let page_token = match FacebookTokenVault::get_valid_page_token(conn, &channel_account_id, &page_id) {
            Ok(t) => t,
            Err(e) => {
                let err_msg = format!("Failed to resolve valid token for post {}: {}", post_id, e);
                conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'failed',
                         error_code = 'token_vault_error',
                         error_message = ?1,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?2 AND status = 'publishing'",
                    params![err_msg, post_id],
                )?;
                return Err(anyhow!(err_msg));
            }
        };

        let publish_result = self.execute_publish_network_flow(
            conn, post_id, &worker_owner, &page_id, &page_token, &post_type, &message,
            link_url.as_deref(), &media_paths_json, scheduled_publish_time_epoch
        ).await;

        match publish_result {
            PublishApiResult::Success { remote_post_id } => {
                conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = CASE
                             WHEN scheduled_publish_time IS NOT NULL AND datetime(scheduled_publish_time) > datetime('now')
                             THEN 'scheduled_on_meta'
                             ELSE 'published'
                         END,
                         remote_post_id = ?1,
                         error_code = NULL,
                         error_message = NULL,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?2 AND status = 'publishing'",
                    params![remote_post_id, post_id],
                )?;
                Ok(remote_post_id)
            }
            PublishApiResult::StillProcessing { upload_session_id } => {
                conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'publishing',
                         upload_session_id = ?1,
                         error_code = NULL,
                         error_message = NULL,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?2 AND status = 'publishing'",
                    params![upload_session_id, post_id],
                )?;
                Ok(upload_session_id)
            }
            PublishApiResult::AmbiguousTransport { error_code, error_message } => {
                conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'delivery_unknown',
                         error_code = ?1,
                         error_message = ?2,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?3 AND status = 'publishing'",
                    params![error_code, error_message, post_id],
                )?;
                Err(anyhow!("Ambiguous transport outcome: {}", error_message))
            }
            PublishApiResult::DeterministicRejection { error_code, error_message } => {
                conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'failed',
                         error_code = ?1,
                         error_message = ?2,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?3 AND status = 'publishing'",
                    params![error_code, error_message, post_id],
                )?;
                Err(anyhow!("Meta deterministic rejection: {}", error_message))
            }
        }
    }

    async fn execute_publish_network_flow(
        &self,
        conn: &rusqlite::Connection,
        post_id: i64,
        owner_id: &str,
        page_id: &str,
        page_token: &str,
        post_type: &str,
        message: &str,
        link_url: Option<&str>,
        media_paths_json: &str,
        scheduled_publish_time_epoch: Option<i64>,
    ) -> PublishApiResult {
        let media_paths: Vec<String> = serde_json::from_str(media_paths_json).unwrap_or_default();

        match post_type {
            "status" => {
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![("access_token", page_token), ("message", message)];
                let sched_str;
                if let Some(st) = scheduled_publish_time_epoch {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }
                let resp = match self.client.post(&url).form(&form).send().await {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Transport error sending status post: {}", e),
                    },
                };
                let status_code = resp.status();
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected status post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta server error on status post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("2xx response with invalid JSON body: {}", e),
                    },
                };
                let post_id_val = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success { remote_post_id: post_id_val }
            }
            "link" => {
                let link = match link_url {
                    Some(l) => l,
                    None => return PublishApiResult::DeterministicRejection {
                        error_code: "missing_link_url".to_string(),
                        error_message: "Link URL required for link post".to_string(),
                    },
                };
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![("access_token", page_token), ("message", message), ("link", link)];
                let sched_str;
                if let Some(st) = scheduled_publish_time_epoch {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }
                let resp = match self.client.post(&url).form(&form).send().await {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Transport error sending link post: {}", e),
                    },
                };
                let status_code = resp.status();
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected link post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta server error on link post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("2xx response with invalid JSON body: {}", e),
                    },
                };
                let post_id_val = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success { remote_post_id: post_id_val }
            }
            "photo" => {
                let photo_path = match media_paths.first() {
                    Some(p) => p,
                    None => return PublishApiResult::DeterministicRejection {
                        error_code: "missing_photo_path".to_string(),
                        error_message: "At least 1 photo path required".to_string(),
                    },
                };
                let file_bytes = match tokio::fs::read(photo_path).await {
                    Ok(b) => b,
                    Err(e) => return PublishApiResult::DeterministicRejection {
                        error_code: "file_read_error".to_string(),
                        error_message: format!("Failed to read photo file '{}': {}", photo_path, e),
                    },
                };
                let file_name = Path::new(photo_path).file_name().and_then(|n| n.to_str()).unwrap_or("photo.jpg").to_string();
                let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

                let mut multipart = reqwest::multipart::Form::new()
                    .text("access_token", page_token.to_string())
                    .text("caption", message.to_string())
                    .part("source", part);

                if let Some(st) = scheduled_publish_time_epoch {
                    multipart = multipart.text("published", "false").text("scheduled_publish_time", st.to_string());
                }

                let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                let resp = match self.client.post(&url).multipart(multipart).send().await {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Transport error uploading photo: {}", e),
                    },
                };
                let status_code = resp.status();
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected photo post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta server error on photo post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("2xx response with invalid JSON body: {}", e),
                    },
                };
                let post_id_val = json.get("post_id").or_else(|| json.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success { remote_post_id: post_id_val }
            }
            "multi_photo" => {
                if media_paths.len() < 2 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "insufficient_photos".to_string(),
                        error_message: "Multi-photo post requires at least 2 photos".to_string(),
                    };
                }
                let mut media_fbids = Vec::new();
                for path_str in &media_paths {
                    let file_bytes = match tokio::fs::read(path_str).await {
                        Ok(b) => b,
                        Err(e) => return PublishApiResult::DeterministicRejection {
                            error_code: "file_read_error".to_string(),
                            error_message: format!("Failed to read photo file '{}': {}", path_str, e),
                        },
                    };
                    let file_name = Path::new(path_str).file_name().and_then(|n| n.to_str()).unwrap_or("photo.jpg").to_string();
                    let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

                    let multipart = reqwest::multipart::Form::new()
                        .text("access_token", page_token.to_string())
                        .text("published", "false")
                        .part("source", part);

                    let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                    let resp = match self.client.post(&url).multipart(multipart).send().await {
                        Ok(r) => r,
                        Err(e) => return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!("Transport error staging photo for multi-photo: {}", e),
                        },
                    };
                    let status_code = resp.status();
                    if status_code.is_client_error() {
                        let err_body = resp.text().await.unwrap_or_default();
                        return PublishApiResult::DeterministicRejection {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!("Meta rejected multi-photo stage upload (HTTP {}): {}", status_code.as_u16(), err_body),
                        };
                    }
                    if status_code.is_server_error() {
                        let err_body = resp.text().await.unwrap_or_default();
                        return PublishApiResult::AmbiguousTransport {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!("Meta server error on multi-photo stage upload (HTTP {}): {}", status_code.as_u16(), err_body),
                        };
                    }
                    let json: Value = match resp.json().await {
                        Ok(j) => j,
                        Err(e) => return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!("2xx stage upload response with invalid JSON: {}", e),
                        },
                    };
                    let fbid = match json.get("id").and_then(|v| v.as_str()) {
                        Some(id) => id.to_string(),
                        None => return PublishApiResult::AmbiguousTransport {
                            error_code: "missing_media_id".to_string(),
                            error_message: "Missing media ID in staged photo response".to_string(),
                        },
                    };
                    media_fbids.push(fbid);
                }

                let attached_media: Vec<Value> = media_fbids
                    .into_iter()
                    .map(|id| json!({ "media_fbid": id }))
                    .collect();
                let attached_str = serde_json::to_string(&attached_media).unwrap_or_default();

                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![
                    ("access_token", page_token),
                    ("message", message),
                    ("attached_media", &attached_str),
                ];
                let sched_str;
                if let Some(st) = scheduled_publish_time_epoch {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }

                let resp = match self.client.post(&url).form(&form).send().await {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Transport error sending multi-photo feed post: {}", e),
                    },
                };
                let status_code = resp.status();
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected multi-photo post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta server error on multi-photo post (HTTP {}): {}", status_code.as_u16(), err_body),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("2xx response with invalid JSON body: {}", e),
                    },
                };
                let post_id_val = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success { remote_post_id: post_id_val }
            }
            "video" => {
                let video_path = match media_paths.first() {
                    Some(p) => p,
                    None => return PublishApiResult::DeterministicRejection {
                        error_code: "missing_video_path".to_string(),
                        error_message: "Video path required".to_string(),
                    },
                };
                self.uploader.upload_video_resumable(conn, page_id, page_token, video_path, message, scheduled_publish_time_epoch, post_id, owner_id).await
            }
            "reel" => {
                let reel_path = match media_paths.first() {
                    Some(p) => p,
                    None => return PublishApiResult::DeterministicRejection {
                        error_code: "missing_reel_path".to_string(),
                        error_message: "Reel path required".to_string(),
                    },
                };
                self.uploader.upload_reel_resumable(conn, page_id, page_token, reel_path, message, scheduled_publish_time_epoch, post_id, owner_id).await
            }
            _ => PublishApiResult::DeterministicRejection {
                error_code: "unsupported_post_type".to_string(),
                error_message: format!("Unsupported post_type '{}' for scheduled post {}", post_type, post_id),
            },
        }
    }
}
```

---

### Task 2: Restart-Resumable Chunked Video & Reel Uploader (`src-tauri/src/facebook/uploader.rs`)

```rust
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::Digest;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
use crate::facebook::publisher::{FacebookPublisher, PublishApiResult};

pub struct FacebookUploader {
    client: Client,
}

impl FacebookUploader {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }

    async fn compute_file_hash(file_path: &str) -> Result<String> {
        let mut hasher = sha2::Sha256::new();
        let mut file = File::open(file_path).await?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub async fn upload_video_resumable(
        &self,
        conn: &Connection,
        page_id: &str,
        page_token: &str,
        file_path: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
        post_id: i64,
        owner_id: &str,
    ) -> PublishApiResult {
        let file_meta = match tokio::fs::metadata(file_path).await {
            Ok(m) => m,
            Err(e) => return PublishApiResult::DeterministicRejection {
                error_code: "file_metadata_error".to_string(),
                error_message: format!("Failed to read file metadata for '{}': {}", file_path, e),
            },
        };
        let file_size = file_meta.len();
        let file_hash = match Self::compute_file_hash(file_path).await {
            Ok(h) => h,
            Err(e) => return PublishApiResult::DeterministicRejection {
                error_code: "file_hash_error".to_string(),
                error_message: format!("Failed to compute file hash for '{}': {}", file_path, e),
            },
        };

        // 1. Check existing in-progress or finishing session in facebook_upload_sessions with content hash verification scoped to this exact post
        let existing_session: Option<(String, String, u64, String)> = match conn.query_row(
            "SELECT upload_session_id, video_id, start_offset, status
             FROM facebook_upload_sessions
             WHERE post_id = ?1 AND page_id = ?2 AND upload_type = 'video' AND file_path = ?3 AND file_size_bytes = ?4 AND file_hash = ?5 AND status IN ('in_progress', 'finishing')",
            params![post_id, page_id, file_path, file_size as i64, file_hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64, r.get(3)?)),
        ) {
            Ok(sess) => Some(sess),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => return PublishApiResult::AmbiguousTransport {
                error_code: "db_error".to_string(),
                error_message: format!("Database query error looking up video upload session: {}", err),
            },
        };

        let (upload_session_id, video_id, mut offset) = if let Some((sess_id, vid_id, _saved_offset, session_status)) = existing_session {
            // If in finishing state, reconcile by checking authoritative video status and publishing phase on Graph API
            if session_status == "finishing" {
                let check_url = format!("https://graph.facebook.com/v26.0/{}?fields=id,status,published&access_token={}", vid_id, page_token);
                match self.client.get(&check_url).send().await {
                    Ok(check_resp) if check_resp.status().is_success() => {
                        let check_json: Value = check_resp.json().await.unwrap_or_default();
                        let video_status = check_json.get("status")
                            .and_then(|s| s.get("video_status"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let is_published = check_json.get("published").and_then(|v| v.as_bool()).unwrap_or(false);

                        if video_status == "ready" || is_published {
                            let _ = conn.execute(
                                "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                                params![sess_id],
                            );
                            return PublishApiResult::Success { remote_post_id: vid_id };
                        } else if video_status == "error" {
                            let _ = conn.execute(
                                "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                                params![sess_id],
                            );
                            return PublishApiResult::DeterministicRejection {
                                error_code: "meta_video_error".to_string(),
                                error_message: "Meta video processing failed with error state".to_string(),
                            };
                        } else {
                            // Video is still processing on Meta; return non-terminal StillProcessing so upload reconciler continues
                            return PublishApiResult::StillProcessing { upload_session_id: sess_id };
                        }
                    }
                    Ok(check_resp) => {
                        let status_code = check_resp.status();
                        let err_text = check_resp.text().await.unwrap_or_default();
                        if status_code.is_client_error() {
                            return PublishApiResult::DeterministicRejection {
                                error_code: format!("http_{}", status_code.as_u16()),
                                error_message: format!("Meta rejected video status check (HTTP {}): {}", status_code.as_u16(), err_text),
                            };
                        }
                        return PublishApiResult::AmbiguousTransport {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!("Meta server error on video status check (HTTP {}): {}", status_code.as_u16(), err_text),
                        };
                    }
                    Err(err) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!("Network error checking finishing video session: {}", err),
                        };
                    }
                }
            }

            // Query Graph video upload session for authoritative server offset
            let status_url = format!("https://graph-video.facebook.com/v26.0/{}?fields=start_offset,end_offset&access_token={}", sess_id, page_token);
            let status_resp = match self.client.get(&status_url).send().await {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Network error querying video upload session offset: {}", e),
                },
            };
            let status_code = status_resp.status();
            if !status_code.is_success() {
                let err_text = status_resp.text().await.unwrap_or_default();
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![sess_id],
                );
                if status_code.is_client_error() {
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected video resume query (HTTP {}): {}", status_code.as_u16(), err_text),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta server error on video resume query (HTTP {}): {}", status_code.as_u16(), err_text),
                };
            }
            let status_json: Value = match status_resp.json().await {
                Ok(j) => j,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "malformed_json_response".to_string(),
                    error_message: format!("Invalid JSON in video resume query response: {}", e),
                },
            };
            let server_offset = match status_json.get("start_offset").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok()) {
                Some(o) => o,
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_start_offset".to_string(),
                    error_message: "Missing start_offset in Meta status response on video resume".to_string(),
                },
            };

            if server_offset > file_size {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![sess_id],
                );
                return PublishApiResult::DeterministicRejection {
                    error_code: "server_offset_out_of_bounds".to_string(),
                    error_message: format!("Server offset {} exceeds file size {}", server_offset, file_size),
                };
            }
            (sess_id, vid_id, server_offset)
        } else {
            // Start Phase
            let start_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
            let start_resp = match self.client.post(&start_url)
                .form(&[
                    ("access_token", page_token),
                    ("upload_phase", "start"),
                    ("file_size", &file_size.to_string()),
                ])
                .send().await {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Network error starting video upload session: {}", e),
                },
            };

            let status_code = start_resp.status();
            if !status_code.is_success() {
                let err_text = start_resp.text().await.unwrap_or_default();
                if status_code.is_client_error() {
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected video start phase (HTTP {}): {}", status_code.as_u16(), err_text),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta server error on video start phase (HTTP {}): {}", status_code.as_u16(), err_text),
                };
            }

            let start_json: Value = match start_resp.json().await {
                Ok(j) => j,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "malformed_json_response".to_string(),
                    error_message: format!("Invalid JSON in video start phase response: {}", e),
                },
            };
            let sess_id = match start_json.get("upload_session_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_upload_session_id".to_string(),
                    error_message: "Missing upload_session_id in Meta video start response".to_string(),
                },
            };
            let vid_id = match start_json.get("video_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_video_id".to_string(),
                    error_message: "Missing video_id in Meta video start response".to_string(),
                },
            };

            let _ = conn.execute(
                "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, created_at, updated_at)
                 VALUES (?1, ?2, 'video', ?3, ?4, ?5, ?6, ?7, 0, 'in_progress', datetime('now'), datetime('now'))",
                params![post_id, page_id, sess_id, vid_id, file_path, file_hash, file_size as i64],
            );

            let _ = conn.execute(
                "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2",
                params![sess_id, post_id],
            );

            (sess_id, vid_id, 0u64)
        };

        // 2. Transfer Phase
        let start_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let mut file = match File::open(file_path).await {
            Ok(f) => f,
            Err(e) => return PublishApiResult::DeterministicRejection {
                error_code: "file_open_error".to_string(),
                error_message: format!("Failed to open video file '{}': {}", file_path, e),
            },
        };
        let chunk_size: usize = 4 * 1024 * 1024; // 4MB

        while offset < file_size {
            // Heartbeat lease renewal before reading and uploading each chunk
            let _ = FacebookPublisher::renew_post_claim(conn, post_id, owner_id);

            let bytes_to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
            let mut buffer = vec![0u8; bytes_to_read];
            if let Err(e) = file.seek(SeekFrom::Start(offset)).await {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_seek_error".to_string(),
                    error_message: format!("Failed to seek video file: {}", e),
                };
            }
            if let Err(e) = file.read_exact(&mut buffer).await {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_read_error".to_string(),
                    error_message: format!("Failed to read video chunk: {}", e),
                };
            }

            let part = reqwest::multipart::Part::bytes(buffer).file_name("chunk.mp4");
            let form = reqwest::multipart::Form::new()
                .text("access_token", page_token.to_string())
                .text("upload_phase", "transfer")
                .text("upload_session_id", upload_session_id.clone())
                .text("start_offset", offset.to_string())
                .part("video_file_chunk", part);

            let transfer_resp = match self.client.post(&start_url).multipart(form).send().await {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Transport error uploading video chunk at offset {}: {}", offset, e),
                },
            };
            let status_code = transfer_resp.status();
            if !status_code.is_success() {
                let err_text = transfer_resp.text().await.unwrap_or_default();
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![upload_session_id],
                );
                if status_code.is_client_error() {
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected video chunk at offset {} (HTTP {}): {}", offset, status_code.as_u16(), err_text),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta server error on video chunk at offset {} (HTTP {}): {}", offset, status_code.as_u16(), err_text),
                };
            }

            let transfer_json: Value = match transfer_resp.json().await {
                Ok(j) => j,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "malformed_json_response".to_string(),
                    error_message: format!("Invalid JSON in video chunk response: {}", e),
                },
            };
            let next_offset = match transfer_json.get("start_offset").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok()) {
                Some(o) => o,
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_start_offset".to_string(),
                    error_message: "Missing start_offset in Meta video transfer response".to_string(),
                },
            };

            if next_offset <= offset || next_offset > file_size {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![upload_session_id],
                );
                return PublishApiResult::DeterministicRejection {
                    error_code: "invalid_offset_progression".to_string(),
                    error_message: format!("Invalid server offset progression: prev={}, next={}, file_size={}", offset, next_offset, file_size),
                };
            }

            offset = next_offset;
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2",
                params![offset as i64, upload_session_id],
            );
        }

        // 3. Finish Phase
        let _ = conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now') WHERE upload_session_id = ?1",
            params![upload_session_id],
        );

        let mut finish_body = json!({
            "access_token": page_token,
            "upload_phase": "finish",
            "upload_session_id": upload_session_id,
            "description": description
        });

        if let Some(st) = scheduled_publish_time {
            finish_body["published"] = json!(false);
            finish_body["scheduled_publish_time"] = json!(st);
        }

        let finish_resp = match self.client.post(&start_url).json(&finish_body).send().await {
            Ok(r) => r,
            Err(e) => return PublishApiResult::AmbiguousTransport {
                error_code: "network_transport_error".to_string(),
                error_message: format!("Transport error finishing video upload: {}", e),
            },
        };
        let status_code = finish_resp.status();
        if !status_code.is_success() {
            let err_text = finish_resp.text().await.unwrap_or_default();
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                params![upload_session_id],
            );
            if status_code.is_client_error() {
                return PublishApiResult::DeterministicRejection {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta rejected video finish phase (HTTP {}): {}", status_code.as_u16(), err_text),
                };
            }
            return PublishApiResult::AmbiguousTransport {
                error_code: format!("http_{}", status_code.as_u16()),
                error_message: format!("Meta server error on video finish phase (HTTP {}): {}", status_code.as_u16(), err_text),
            };
        }

        // Authoritative terminal status verification with bounded polling
        let check_url = format!("https://graph.facebook.com/v26.0/{}?fields=id,status,published&access_token={}", video_id, page_token);
        for attempt in 0..5 {
            let _ = FacebookPublisher::renew_post_claim(conn, post_id, owner_id);
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
            }
            if let Ok(check_resp) = self.client.get(&check_url).send().await {
                if check_resp.status().is_success() {
                    let check_json: Value = check_resp.json().await.unwrap_or_default();
                    let video_status = check_json.get("status")
                        .and_then(|s| s.get("video_status"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let is_published = check_json.get("published").and_then(|v| v.as_bool()).unwrap_or(false);

                    if video_status == "ready" || is_published {
                        let _ = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                            params![upload_session_id],
                        );
                        return PublishApiResult::Success { remote_post_id: video_id };
                    } else if video_status == "error" {
                        let _ = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                            params![upload_session_id],
                        );
                        return PublishApiResult::DeterministicRejection {
                            error_code: "meta_video_error".to_string(),
                            error_message: "Meta video processing failed with error state".to_string(),
                        };
                    }
                }
            }
        }

        // Processing pending; session remains in finishing state for background reconciliation
        PublishApiResult::StillProcessing { upload_session_id }
    }

    pub async fn upload_reel_resumable(
        &self,
        conn: &Connection,
        page_id: &str,
        page_token: &str,
        file_path: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
        post_id: i64,
        owner_id: &str,
    ) -> PublishApiResult {
        let file_meta = match tokio::fs::metadata(file_path).await {
            Ok(m) => m,
            Err(e) => return PublishApiResult::DeterministicRejection {
                error_code: "file_metadata_error".to_string(),
                error_message: format!("Failed to read file metadata for '{}': {}", file_path, e),
            },
        };
        let file_size = file_meta.len();
        let file_hash = match Self::compute_file_hash(file_path).await {
            Ok(h) => h,
            Err(e) => return PublishApiResult::DeterministicRejection {
                error_code: "file_hash_error".to_string(),
                error_message: format!("Failed to compute file hash for '{}': {}", file_path, e),
            },
        };

        let existing_session: Option<(String, u64, String)> = match conn.query_row(
            "SELECT video_id, start_offset, status
             FROM facebook_upload_sessions
             WHERE post_id = ?1 AND page_id = ?2 AND upload_type = 'reel' AND file_path = ?3 AND file_size_bytes = ?4 AND file_hash = ?5 AND status IN ('in_progress', 'finishing')",
            params![post_id, page_id, file_path, file_size as i64, file_hash],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64, r.get(2)?)),
        ) {
            Ok(sess) => Some(sess),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => return PublishApiResult::AmbiguousTransport {
                error_code: "db_error".to_string(),
                error_message: format!("Database query error looking up reel upload session: {}", err),
            },
        };

        let (video_id, mut offset) = if let Some((vid_id, _saved_offset, session_status)) = existing_session {
            // If in finishing state, reconcile by checking authoritative reel publishing status on Graph API
            if session_status == "finishing" {
                let check_url = format!("https://graph.facebook.com/v26.0/{}?fields=id,status&access_token={}", vid_id, page_token);
                match self.client.get(&check_url).send().await {
                    Ok(check_resp) if check_resp.status().is_success() => {
                        let check_json: Value = check_resp.json().await.unwrap_or_default();
                        let video_status = check_json.get("status")
                            .and_then(|s| s.get("video_status"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        let publishing_status = check_json.get("status")
                            .and_then(|s| s.get("publishing_phase"))
                            .and_then(|p| p.get("status"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");

                        if publishing_status == "complete" {
                            let _ = conn.execute(
                                "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                                params![vid_id],
                            );
                            return PublishApiResult::Success { remote_post_id: vid_id };
                        } else if video_status == "error" || publishing_status == "error" {
                            let _ = conn.execute(
                                "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                                params![vid_id],
                            );
                            return PublishApiResult::DeterministicRejection {
                                error_code: "meta_reel_error".to_string(),
                                error_message: "Meta Reel processing failed with error state".to_string(),
                            };
                        } else {
                            // Reel publishing is still in progress; return StillProcessing so upload reconciler continues
                            return PublishApiResult::StillProcessing { upload_session_id: vid_id };
                        }
                    }
                    Ok(check_resp) => {
                        let status_code = check_resp.status();
                        let err_text = check_resp.text().await.unwrap_or_default();
                        if status_code.is_client_error() {
                            return PublishApiResult::DeterministicRejection {
                                error_code: format!("http_{}", status_code.as_u16()),
                                error_message: format!("Meta rejected reel status check (HTTP {}): {}", status_code.as_u16(), err_text),
                            };
                        }
                        return PublishApiResult::AmbiguousTransport {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!("Meta server error on reel status check (HTTP {}): {}", status_code.as_u16(), err_text),
                        };
                    }
                    Err(err) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!("Network error checking finishing reel session: {}", err),
                        };
                    }
                }
            }

            let check_url = format!("https://rupload.facebook.com/video-reels/{}", vid_id);
            let check_resp = match self.client.get(&check_url)
                .header("Authorization", format!("OAuth {}", page_token))
                .send().await {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Network error querying Reel offset on resume: {}", e),
                },
            };
            let status_code = check_resp.status();
            if !status_code.is_success() {
                let err_text = check_resp.text().await.unwrap_or_default();
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![vid_id],
                );
                if status_code.is_client_error() {
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected Reel resume query (HTTP {}): {}", status_code.as_u16(), err_text),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta server error on Reel resume query (HTTP {}): {}", status_code.as_u16(), err_text),
                };
            }

            let check_json: Value = match check_resp.json().await {
                Ok(j) => j,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "malformed_json_response".to_string(),
                    error_message: format!("Invalid JSON in Reel resume query response: {}", e),
                },
            };
            let server_offset = match check_json.get("offset").and_then(|v| v.as_u64()) {
                Some(o) => o,
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_offset".to_string(),
                    error_message: "Missing offset in Reel status response on resume".to_string(),
                },
            };

            if server_offset > file_size {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![vid_id],
                );
                return PublishApiResult::DeterministicRejection {
                    error_code: "server_offset_out_of_bounds".to_string(),
                    error_message: format!("Reel server offset {} exceeds file size {}", server_offset, file_size),
                };
            }
            (vid_id, server_offset)
        } else {
            let init_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
            let init_resp = match self.client.post(&init_url)
                .form(&[
                    ("access_token", page_token),
                    ("upload_phase", "start"),
                ])
                .send().await {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Network error initializing Reel upload: {}", e),
                },
            };
            let status_code = init_resp.status();
            if !status_code.is_success() {
                let err_text = init_resp.text().await.unwrap_or_default();
                if status_code.is_client_error() {
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected Reel init (HTTP {}): {}", status_code.as_u16(), err_text),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta server error on Reel init (HTTP {}): {}", status_code.as_u16(), err_text),
                };
            }

            let init_json: Value = match init_resp.json().await {
                Ok(j) => j,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "malformed_json_response".to_string(),
                    error_message: format!("Invalid JSON in Reel init response: {}", e),
                },
            };
            let vid_id = match init_json.get("video_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_video_id".to_string(),
                    error_message: "Missing video_id in Reel init response".to_string(),
                },
            };

            let _ = conn.execute(
                "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, created_at, updated_at)
                 VALUES (?1, ?2, 'reel', ?3, ?3, ?4, ?5, ?6, 0, 'in_progress', datetime('now'), datetime('now'))",
                params![post_id, page_id, vid_id, file_path, file_hash, file_size as i64],
            );

            let _ = conn.execute(
                "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2",
                params![vid_id, post_id],
            );

            (vid_id, 0u64)
        };

        let rupload_url = format!("https://rupload.facebook.com/video-reels/{}", video_id);
        let mut file = match File::open(file_path).await {
            Ok(f) => f,
            Err(e) => return PublishApiResult::DeterministicRejection {
                error_code: "file_open_error".to_string(),
                error_message: format!("Failed to open Reel file '{}': {}", file_path, e),
            },
        };
        let chunk_size: usize = 4 * 1024 * 1024;

        while offset < file_size {
            // Heartbeat lease renewal before reading and uploading each Reel chunk
            let _ = FacebookPublisher::renew_post_claim(conn, post_id, owner_id);

            let bytes_to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
            let mut buffer = vec![0u8; bytes_to_read];
            if let Err(e) = file.seek(SeekFrom::Start(offset)).await {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_seek_error".to_string(),
                    error_message: format!("Failed to seek Reel file: {}", e),
                };
            }
            if let Err(e) = file.read_exact(&mut buffer).await {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_read_error".to_string(),
                    error_message: format!("Failed to read Reel chunk: {}", e),
                };
            }

            let resp = match self.client.post(&rupload_url)
                .header("Authorization", format!("OAuth {}", page_token))
                .header("Offset", offset.to_string())
                .header("file_size", file_size.to_string())
                .body(buffer)
                .send().await {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Transport error uploading Reel chunk at offset {}: {}", offset, e),
                },
            };
            let status_code = resp.status();
            if !status_code.is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![video_id],
                );
                if status_code.is_client_error() {
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!("Meta rejected Reel chunk at offset {} (HTTP {}): {}", offset, status_code.as_u16(), err_text),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta server error on Reel chunk at offset {} (HTTP {}): {}", offset, status_code.as_u16(), err_text),
                };
            }

            let resp_json: Value = match resp.json().await {
                Ok(j) => j,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "malformed_json_response".to_string(),
                    error_message: format!("Invalid JSON in Reel chunk response: {}", e),
                },
            };
            let confirmed_offset = match resp_json.get("offset").and_then(|v| v.as_u64()) {
                Some(o) => o,
                None => return PublishApiResult::AmbiguousTransport {
                    error_code: "missing_offset".to_string(),
                    error_message: "Missing offset in Meta Reel chunk response".to_string(),
                },
            };

            if confirmed_offset != offset + bytes_to_read as u64 {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![video_id],
                );
                return PublishApiResult::DeterministicRejection {
                    error_code: "offset_mismatch".to_string(),
                    error_message: format!("Reel chunk offset mismatch: expected {}, got {}", offset + bytes_to_read as u64, confirmed_offset),
                };
            }

            offset = confirmed_offset;
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2",
                params![offset as i64, video_id],
            );
        }

        // 3. Finish Phase
        let _ = conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now') WHERE upload_session_id = ?1",
            params![video_id],
        );

        let mut finish_body = json!({
            "access_token": page_token,
            "upload_phase": "finish",
            "video_state": if scheduled_publish_time.is_some() { "SCHEDULED" } else { "PUBLISHED" },
            "description": description
        });

        if let Some(st) = scheduled_publish_time {
            finish_body["scheduled_publish_time"] = json!(st);
        }

        let finish_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
        let finish_resp = match self.client.post(&finish_url).json(&finish_body).send().await {
            Ok(r) => r,
            Err(e) => return PublishApiResult::AmbiguousTransport {
                error_code: "network_transport_error".to_string(),
                error_message: format!("Transport error finishing Reel upload: {}", e),
            },
        };
        let status_code = finish_resp.status();
        if !status_code.is_success() {
            let err_text = finish_resp.text().await.unwrap_or_default();
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                params![video_id],
            );
            if status_code.is_client_error() {
                return PublishApiResult::DeterministicRejection {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!("Meta rejected Reel finish phase (HTTP {}): {}", status_code.as_u16(), err_text),
                };
            }
            return PublishApiResult::AmbiguousTransport {
                error_code: format!("http_{}", status_code.as_u16()),
                error_message: format!("Meta server error on Reel finish phase (HTTP {}): {}", status_code.as_u16(), err_text),
            };
        }

        // Authoritative terminal status verification with bounded polling
        let check_url = format!("https://graph.facebook.com/v26.0/{}?fields=id,status&access_token={}", video_id, page_token);
        for attempt in 0..5 {
            let _ = FacebookPublisher::renew_post_claim(conn, post_id, owner_id);
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
            }
            if let Ok(check_resp) = self.client.get(&check_url).send().await {
                if check_resp.status().is_success() {
                    let check_json: Value = check_resp.json().await.unwrap_or_default();
                    let video_status = check_json.get("status")
                        .and_then(|s| s.get("video_status"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let publishing_status = check_json.get("status")
                        .and_then(|s| s.get("publishing_phase"))
                        .and_then(|p| p.get("status"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if publishing_status == "complete" {
                        let _ = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                            params![video_id],
                        );
                        return PublishApiResult::Success { remote_post_id: video_id };
                    } else if video_status == "error" || publishing_status == "error" {
                        let _ = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                            params![video_id],
                        );
                        return PublishApiResult::DeterministicRejection {
                            error_code: "meta_reel_error".to_string(),
                            error_message: "Meta Reel processing failed with error state".to_string(),
                        };
                    }
                }
            }
        }

        // Publishing pending; session remains in finishing state for background reconciliation
        PublishApiResult::StillProcessing { upload_session_id: video_id }
    }
}
```

---

### Task 3: Fail-Closed Promotion Worker & Multi-Endpoint Reconciliation (`src-tauri/src/facebook/scheduler.rs`)

- **Promotion Worker:** Claims rows with `status='queued'`, queries `facebook_page_tokens` for the specific claimed row's `page_id`, strictly parses RFC3339 timestamp (failing closed with `error_code='invalid_timestamp'` on parse failure), and sets `delivery_unknown` on transport timeout or 5xx.
- **Reconciliation Engine:** Periodically claims `delivery_unknown` rows. Queries Graph API `/scheduled_posts` and `/feed` with pagination. Matches post type against Graph `status_type`, computes message SHA-256 hash, and verifies timestamp within 60s window. Sets `status='published'` for `/feed` matches and `status='scheduled_on_meta'` for `/scheduled_posts`.
- **Upload Session Background Reconciler:** Concurrently claims `facebook_upload_sessions` via atomic lease token claim:
  ```sql
  UPDATE facebook_upload_sessions
  SET owner_id = ?1,
      lease_deadline = datetime('now', '+60 seconds'),
      updated_at = datetime('now')
  WHERE id = (
      SELECT id FROM facebook_upload_sessions
      WHERE status = 'finishing'
        AND (lease_deadline IS NULL OR lease_deadline < datetime('now'))
      LIMIT 1
  ) RETURNING id, post_id, upload_session_id, upload_type, page_id, video_id;
  ```
  Queries Meta Graph API `/video_id` status (`status.video_status` for videos, `status.publishing_phase.status` for reels).
  - On authoritative terminal success (`video_status == 'ready' || published == true` for Video; `publishing_phase.status == 'complete'` for Reel):
    ```rust
    // Execute atomic transaction: session CAS completion + scheduled post propagation
    let tx = conn.transaction()?;
    let session_rows = tx.execute(
        "UPDATE facebook_upload_sessions
         SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
         WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
        params![session_id, worker_id],
    )?;

    // Only if this worker won the CAS race (affected exactly 1 row), propagate terminal state to scheduled post within the same transaction
    if session_rows == 1 {
        let post_rows = tx.execute(
            "UPDATE facebook_scheduled_posts
             SET status = CASE
                     WHEN scheduled_publish_time IS NOT NULL AND datetime(scheduled_publish_time) > datetime('now')
                     THEN 'scheduled_on_meta'
                     ELSE 'published'
                 END,
                 remote_post_id = ?1,
                 error_code = NULL,
                 error_message = NULL,
                 updated_at = datetime('now')
             WHERE id = ?2 AND status IN ('publishing', 'delivery_unknown')",
            params![remote_video_id, post_id],
        )?;

        // Enforce exactly 1 updated row or verify that the post is already in an equivalent terminal success state
        if post_rows != 1 {
            let post_status: Option<String> = tx.query_row(
                "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                params![post_id],
                |r| r.get(0),
            ).optional()?;

            match post_status.as_deref() {
                Some("published") | Some("scheduled_on_meta") => {
                    // Already terminal and consistent
                }
                _ => {
                    return Err(anyhow!("Reconciler invariant violation: expected exactly 1 updated post row for id {} (affected {}, current: {:?}); rolling back", post_id, post_rows, post_status));
                }
            }
        }
    }
    tx.commit()?;
    ```
  - On authoritative terminal failure:
    ```rust
    // Execute atomic transaction: session CAS failure + scheduled post failure propagation
    let tx = conn.transaction()?;
    let session_rows = tx.execute(
        "UPDATE facebook_upload_sessions
         SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
         WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
        params![session_id, worker_id],
    )?;

    // Only if this worker won the CAS race, propagate failure to scheduled post within the same transaction
    if session_rows == 1 {
        let post_rows = tx.execute(
            "UPDATE facebook_scheduled_posts
             SET status = 'failed', error_code = 'upload_failed', error_message = ?1, updated_at = datetime('now')
             WHERE id = ?2 AND status IN ('publishing', 'delivery_unknown')",
            params![err_msg, post_id],
        )?;

        if post_rows != 1 {
            let post_status: Option<String> = tx.query_row(
                "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                params![post_id],
                |r| r.get(0),
            ).optional()?;

            if post_status.as_deref() != Some("failed") {
                return Err(anyhow!("Reconciler invariant violation: expected exactly 1 updated post row for id {} (affected {}, current: {:?}); rolling back", post_id, post_rows, post_status));
            }
        }
    }
    tx.commit()?;
    ```
  - On non-terminal processing state:
    ```rust
    // Atomic CAS lease release: clears lease if still held by this worker
    conn.execute(
        "UPDATE facebook_upload_sessions
         SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now')
         WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
        params![session_id, worker_id],
    )?;
    ```
