use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::path::Path;

use crate::facebook::uploader::FacebookUploader;
use crate::facebook::vault::FacebookTokenVault;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishOutcome {
    Published { remote_post_id: String },
    ScheduledOnMeta { remote_post_id: String },
    StillProcessing { upload_session_id: String },
}

#[derive(Debug)]
pub enum PublishApiResult {
    Success {
        remote_post_id: String,
    },
    StillProcessing {
        upload_session_id: String,
    },
    RateLimited {
        retry_after_secs: u64,
        error_message: String,
    },
    DeterministicRejection {
        error_code: String,
        error_message: String,
    },
    AmbiguousTransport {
        error_code: String,
        error_message: String,
    },
}

pub struct FacebookPublisher {
    client: Client,
    uploader: FacebookUploader,
}

impl Default for FacebookPublisher {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_retry_after(headers: &reqwest::header::HeaderMap) -> u64 {
    headers
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60)
}

impl FacebookPublisher {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
            uploader: FacebookUploader::new(),
        }
    }

    pub fn renew_post_claim(conn: &Connection, post_id: i64, owner_id: &str) -> Result<()> {
        let rows = conn.execute(
            "UPDATE facebook_scheduled_posts
             SET claim_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
             WHERE id = ?1 AND owner_id = ?2 AND status = 'publishing'",
            params![post_id, owner_id],
        )?;
        if rows != 1 {
            return Err(anyhow!(
                "Lost lease ownership on post {}: cannot continue",
                post_id
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn publish_post(
        &self,
        conn: &rusqlite::Connection,
        post_id: i64,
    ) -> Result<PublishOutcome> {
        // 1. Authoritatively fetch and validate scheduled post row from database
        let (channel_account_id, page_id, post_type, message, link_url, media_paths_json, scheduled_publish_time_str, current_status): (
            i64, String, String, String, Option<String>, String, Option<String>, String
        ) = conn.query_row(
            "SELECT channel_account_id, page_id, post_type, message, link_url, media_paths_json, scheduled_publish_time, status
             FROM facebook_scheduled_posts
             WHERE id = ?1",
            [post_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)),
        ).map_err(|e| anyhow!("Scheduled post {} not found: {}", post_id, e))?;

        // Validate RFC3339 timestamp and convert to unix epoch seconds for Meta Graph API
        let scheduled_publish_time_epoch: Option<i64> = match scheduled_publish_time_str.as_deref()
        {
            Some(s) => {
                let dt = chrono::DateTime::parse_from_rfc3339(s).map_err(|e| {
                    anyhow!(
                        "Invalid RFC3339 scheduled_publish_time '{}' for post {}: {}",
                        s,
                        post_id,
                        e
                    )
                })?;
                Some(dt.timestamp())
            }
            None => None,
        };

        // 2. Atomic single-writer claim: Transition strictly from 'queued' to 'publishing' with lease deadline
        if current_status == "delivery_unknown" {
            return Err(anyhow!("Cannot publish post {}: status is 'delivery_unknown' and must be resolved by reconciler only", post_id));
        }

        let worker_owner = format!("worker_{}_{}", std::process::id(), uuid::Uuid::new_v4());
        let rows_claimed = conn.execute(
            "UPDATE facebook_scheduled_posts
             SET status = 'publishing',
                 owner_id = ?1,
                 claim_deadline = datetime('now', '+5 minutes'),
                 updated_at = datetime('now')
             WHERE id = ?2 AND status = 'queued'
               AND (next_retry_at IS NULL OR datetime(next_retry_at) <= datetime('now'))",
            params![worker_owner, post_id],
        )?;

        if rows_claimed != 1 {
            let active_status: Option<String> = conn
                .query_row(
                    "SELECT status FROM facebook_scheduled_posts WHERE id = ?1",
                    [post_id],
                    |r| r.get(0),
                )
                .optional()?;

            match active_status.as_deref() {
                Some("publishing") => return Err(anyhow!("Cannot publish post {}: already being processed by another worker", post_id)),
                Some("delivery_unknown") => return Err(anyhow!("Cannot publish post {}: status transitioned to 'delivery_unknown' and must be resolved by reconciler only", post_id)),
                Some(st) => return Err(anyhow!("Cannot publish post {}: invalid status '{}'", post_id, st)),
                None => return Err(anyhow!("Scheduled post {} not found", post_id)),
            }
        }

        // Authoritatively fetch and decrypt active Page token via local AES-256-GCM TokenVault helper joining account + page + valid
        let page_token =
            match FacebookTokenVault::get_valid_page_token(conn, channel_account_id, &page_id) {
                Ok(t) => t,
                Err(e) => {
                    let err_msg =
                        format!("Failed to resolve valid token for post {}: {}", post_id, e);
                    let rows = conn.execute(
                        "UPDATE facebook_scheduled_posts
                     SET status = 'failed',
                         error_code = 'token_vault_error',
                         error_message = ?1,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                        params![err_msg, post_id, worker_owner],
                    )?;
                    if rows != 1 {
                        return Err(anyhow!(
                            "Failed to transition post {} to failed: lease ownership lost ({})",
                            post_id,
                            err_msg
                        ));
                    }
                    return Err(anyhow!(err_msg));
                }
            };

        let publish_result = self
            .execute_publish_network_flow(
                conn,
                post_id,
                &worker_owner,
                &page_id,
                &page_token,
                &post_type,
                &message,
                link_url.as_deref(),
                &media_paths_json,
                scheduled_publish_time_epoch,
            )
            .await;

        match publish_result {
            PublishApiResult::Success { remote_post_id } => {
                let is_future_scheduled = scheduled_publish_time_epoch
                    .map(|t| t > chrono::Utc::now().timestamp())
                    .unwrap_or(false);
                let target_status = if is_future_scheduled {
                    "scheduled_on_meta"
                } else {
                    "published"
                };
                let rows = conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = ?1,
                         remote_post_id = ?2,
                         next_retry_at = NULL,
                         error_code = NULL,
                         error_message = NULL,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?3 AND owner_id = ?4 AND status = 'publishing'",
                    params![target_status, remote_post_id, post_id, worker_owner],
                )?;
                if rows != 1 {
                    return Err(anyhow!(
                        "Failed to transition post {}: lease ownership lost or expired",
                        post_id
                    ));
                }
                if is_future_scheduled {
                    Ok(PublishOutcome::ScheduledOnMeta { remote_post_id })
                } else {
                    Ok(PublishOutcome::Published { remote_post_id })
                }
            }
            PublishApiResult::StillProcessing { upload_session_id } => {
                let rows = conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'publishing',
                         upload_session_id = ?1,
                         error_code = NULL,
                         error_message = NULL,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                    params![upload_session_id, post_id, worker_owner],
                )?;
                if rows != 1 {
                    return Err(anyhow!(
                        "Failed to transition post {}: lease ownership lost or expired",
                        post_id
                    ));
                }
                Ok(PublishOutcome::StillProcessing { upload_session_id })
            }
            PublishApiResult::RateLimited {
                retry_after_secs,
                error_message,
            } => {
                let backoff_secs = std::cmp::max(retry_after_secs, 60);
                let rows = conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'queued',
                         error_code = 'rate_limited',
                         error_message = ?1,
                         next_retry_at = datetime('now', '+' || ?2 || ' seconds'),
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?3 AND owner_id = ?4 AND status = 'publishing'",
                    params![error_message, backoff_secs as i64, post_id, worker_owner],
                )?;
                if rows != 1 {
                    return Err(anyhow!(
                        "Failed to transition rate-limited post {}: lease ownership lost",
                        post_id
                    ));
                }
                Err(anyhow!(
                    "Meta rate limit encountered (retry in {}s): {}",
                    backoff_secs,
                    error_message
                ))
            }
            PublishApiResult::AmbiguousTransport {
                error_code,
                error_message,
            } => {
                let rows = conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'delivery_unknown',
                         error_code = ?1,
                         error_message = ?2,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?3 AND owner_id = ?4 AND status = 'publishing'",
                    params![error_code, error_message, post_id, worker_owner],
                )?;
                if rows != 1 {
                    return Err(anyhow!(
                        "Failed to transition post {} to delivery_unknown: lease ownership lost",
                        post_id
                    ));
                }
                Err(anyhow!("Ambiguous transport outcome: {}", error_message))
            }
            PublishApiResult::DeterministicRejection {
                error_code,
                error_message,
            } => {
                let rows = conn.execute(
                    "UPDATE facebook_scheduled_posts
                     SET status = 'failed',
                         error_code = ?1,
                         error_message = ?2,
                         owner_id = NULL,
                         claim_deadline = NULL,
                         updated_at = datetime('now')
                     WHERE id = ?3 AND owner_id = ?4 AND status = 'publishing'",
                    params![error_code, error_message, post_id, worker_owner],
                )?;
                if rows != 1 {
                    return Err(anyhow!(
                        "Failed to transition post {} to failed: lease ownership lost",
                        post_id
                    ));
                }
                Err(anyhow!("Meta deterministic rejection: {}", error_message))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
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
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!("Transport error sending status post: {}", e),
                        }
                    }
                };
                let status_code = resp.status();
                let retry_after = parse_retry_after(resp.headers());
                if status_code.as_u16() == 429 {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!("HTTP 429 Rate limited: {}", err_body),
                    };
                }
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    if let Ok(err_json) = serde_json::from_str::<Value>(&err_body) {
                        let code = err_json
                            .get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(|c| c.as_i64())
                            .unwrap_or(0);
                        let is_transient = err_json
                            .get("error")
                            .and_then(|e| e.get("is_transient"))
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        if code == 4
                            || code == 17
                            || code == 32
                            || code == 341
                            || code == 613
                            || is_transient
                        {
                            return PublishApiResult::RateLimited {
                                retry_after_secs: retry_after,
                                error_message: format!(
                                    "Meta rate limit error (code {}): {}",
                                    code, err_body
                                ),
                            };
                        }
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected status post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on status post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!("2xx response with invalid JSON body: {}", e),
                        }
                    }
                };
                let post_id_val = json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success {
                    remote_post_id: post_id_val,
                }
            }
            "link" => {
                let link = match link_url {
                    Some(l) => l,
                    None => {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "missing_link_url".to_string(),
                            error_message: "Link URL required for link post".to_string(),
                        }
                    }
                };
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![
                    ("access_token", page_token),
                    ("message", message),
                    ("link", link),
                ];
                let sched_str;
                if let Some(st) = scheduled_publish_time_epoch {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }
                let resp = match self.client.post(&url).form(&form).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!("Transport error sending link post: {}", e),
                        }
                    }
                };
                let status_code = resp.status();
                let retry_after = parse_retry_after(resp.headers());
                if status_code.as_u16() == 429 {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!("HTTP 429 Rate limited: {}", err_body),
                    };
                }
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    if let Ok(err_json) = serde_json::from_str::<Value>(&err_body) {
                        let code = err_json
                            .get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(|c| c.as_i64())
                            .unwrap_or(0);
                        let is_transient = err_json
                            .get("error")
                            .and_then(|e| e.get("is_transient"))
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        if code == 4
                            || code == 17
                            || code == 32
                            || code == 341
                            || code == 613
                            || is_transient
                        {
                            return PublishApiResult::RateLimited {
                                retry_after_secs: retry_after,
                                error_message: format!(
                                    "Meta rate limit error (code {}): {}",
                                    code, err_body
                                ),
                            };
                        }
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected link post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on link post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!("2xx response with invalid JSON body: {}", e),
                        }
                    }
                };
                let post_id_val = json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success {
                    remote_post_id: post_id_val,
                }
            }
            "photo" => {
                let photo_path = match media_paths.first() {
                    Some(p) => p,
                    None => {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "missing_photo_path".to_string(),
                            error_message: "At least 1 photo path required".to_string(),
                        }
                    }
                };
                let file_bytes = match tokio::fs::read(photo_path).await {
                    Ok(b) => b,
                    Err(e) => {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "file_read_error".to_string(),
                            error_message: format!(
                                "Failed to read photo file '{}': {}",
                                photo_path, e
                            ),
                        }
                    }
                };
                let file_name = Path::new(photo_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("photo.jpg")
                    .to_string();
                let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

                let mut multipart = reqwest::multipart::Form::new()
                    .text("access_token", page_token.to_string())
                    .text("caption", message.to_string())
                    .part("source", part);

                if let Some(st) = scheduled_publish_time_epoch {
                    multipart = multipart
                        .text("published", "false")
                        .text("scheduled_publish_time", st.to_string());
                }

                let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                let resp = match self.client.post(&url).multipart(multipart).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!("Transport error uploading photo: {}", e),
                        }
                    }
                };
                let status_code = resp.status();
                let retry_after = parse_retry_after(resp.headers());
                if status_code.as_u16() == 429 {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!("HTTP 429 Rate limited: {}", err_body),
                    };
                }
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    if let Ok(err_json) = serde_json::from_str::<Value>(&err_body) {
                        let code = err_json
                            .get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(|c| c.as_i64())
                            .unwrap_or(0);
                        let is_transient = err_json
                            .get("error")
                            .and_then(|e| e.get("is_transient"))
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        if code == 4
                            || code == 17
                            || code == 32
                            || code == 341
                            || code == 613
                            || is_transient
                        {
                            return PublishApiResult::RateLimited {
                                retry_after_secs: retry_after,
                                error_message: format!(
                                    "Meta rate limit error (code {}): {}",
                                    code, err_body
                                ),
                            };
                        }
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected photo post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on photo post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!("2xx response with invalid JSON body: {}", e),
                        }
                    }
                };
                let post_id_val = json
                    .get("post_id")
                    .or_else(|| json.get("id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success {
                    remote_post_id: post_id_val,
                }
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
                        Err(e) => {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "file_read_error".to_string(),
                                error_message: format!(
                                    "Failed to read photo file '{}': {}",
                                    path_str, e
                                ),
                            }
                        }
                    };
                    let file_name = Path::new(path_str)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("photo.jpg")
                        .to_string();
                    let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

                    let multipart = reqwest::multipart::Form::new()
                        .text("access_token", page_token.to_string())
                        .text("published", "false")
                        .part("source", part);

                    let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                    let resp = match self.client.post(&url).multipart(multipart).send().await {
                        Ok(r) => r,
                        Err(e) => {
                            return PublishApiResult::AmbiguousTransport {
                                error_code: "network_transport_error".to_string(),
                                error_message: format!(
                                    "Transport error staging photo for multi-photo: {}",
                                    e
                                ),
                            }
                        }
                    };
                    let status_code = resp.status();
                    let retry_after = parse_retry_after(resp.headers());
                    if status_code.as_u16() == 429 {
                        let err_body = resp.text().await.unwrap_or_default();
                        return PublishApiResult::RateLimited {
                            retry_after_secs: retry_after,
                            error_message: format!("HTTP 429 Rate limited: {}", err_body),
                        };
                    }
                    if status_code.is_client_error() {
                        let err_body = resp.text().await.unwrap_or_default();
                        if let Ok(err_json) = serde_json::from_str::<Value>(&err_body) {
                            let code = err_json
                                .get("error")
                                .and_then(|e| e.get("code"))
                                .and_then(|c| c.as_i64())
                                .unwrap_or(0);
                            let is_transient = err_json
                                .get("error")
                                .and_then(|e| e.get("is_transient"))
                                .and_then(|t| t.as_bool())
                                .unwrap_or(false);
                            if code == 4
                                || code == 17
                                || code == 32
                                || code == 341
                                || code == 613
                                || is_transient
                            {
                                return PublishApiResult::RateLimited {
                                    retry_after_secs: retry_after,
                                    error_message: format!(
                                        "Meta rate limit error (code {}): {}",
                                        code, err_body
                                    ),
                                };
                            }
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!(
                                "Meta rejected multi-photo stage upload (HTTP {}): {}",
                                status_code.as_u16(),
                                err_body
                            ),
                        };
                    }
                    if status_code.is_server_error() {
                        let err_body = resp.text().await.unwrap_or_default();
                        return PublishApiResult::AmbiguousTransport {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!(
                                "Meta server error on multi-photo stage upload (HTTP {}): {}",
                                status_code.as_u16(),
                                err_body
                            ),
                        };
                    }
                    let json: Value = match resp.json().await {
                        Ok(j) => j,
                        Err(e) => {
                            return PublishApiResult::AmbiguousTransport {
                                error_code: "malformed_json_response".to_string(),
                                error_message: format!(
                                    "2xx stage upload response with invalid JSON: {}",
                                    e
                                ),
                            }
                        }
                    };
                    let fbid = match json.get("id").and_then(|v| v.as_str()) {
                        Some(id) => id.to_string(),
                        None => {
                            return PublishApiResult::AmbiguousTransport {
                                error_code: "missing_media_id".to_string(),
                                error_message: "Missing media ID in staged photo response"
                                    .to_string(),
                            }
                        }
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
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!(
                                "Transport error sending multi-photo feed post: {}",
                                e
                            ),
                        }
                    }
                };
                let status_code = resp.status();
                let retry_after = parse_retry_after(resp.headers());
                if status_code.as_u16() == 429 {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!("HTTP 429 Rate limited: {}", err_body),
                    };
                }
                if status_code.is_client_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    if let Ok(err_json) = serde_json::from_str::<Value>(&err_body) {
                        let code = err_json
                            .get("error")
                            .and_then(|e| e.get("code"))
                            .and_then(|c| c.as_i64())
                            .unwrap_or(0);
                        let is_transient = err_json
                            .get("error")
                            .and_then(|e| e.get("is_transient"))
                            .and_then(|t| t.as_bool())
                            .unwrap_or(false);
                        if code == 4
                            || code == 17
                            || code == 32
                            || code == 341
                            || code == 613
                            || is_transient
                        {
                            return PublishApiResult::RateLimited {
                                retry_after_secs: retry_after,
                                error_message: format!(
                                    "Meta rate limit error (code {}): {}",
                                    code, err_body
                                ),
                            };
                        }
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected multi-photo post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                if status_code.is_server_error() {
                    let err_body = resp.text().await.unwrap_or_default();
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on multi-photo post (HTTP {}): {}",
                            status_code.as_u16(),
                            err_body
                        ),
                    };
                }
                let json: Value = match resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!("2xx response with invalid JSON body: {}", e),
                        }
                    }
                };
                let post_id_val = json
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if post_id_val.is_empty() {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "empty_remote_id".to_string(),
                        error_message: "2xx response missing id field".to_string(),
                    };
                }
                PublishApiResult::Success {
                    remote_post_id: post_id_val,
                }
            }
            "video" => {
                let video_path = match media_paths.first() {
                    Some(p) => p,
                    None => {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "missing_video_path".to_string(),
                            error_message: "Video path required".to_string(),
                        }
                    }
                };
                self.uploader
                    .upload_video_resumable(
                        conn,
                        page_id,
                        page_token,
                        video_path,
                        message,
                        scheduled_publish_time_epoch,
                        post_id,
                        owner_id,
                    )
                    .await
            }
            "reel" => {
                let reel_path = match media_paths.first() {
                    Some(p) => p,
                    None => {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "missing_reel_path".to_string(),
                            error_message: "Reel path required".to_string(),
                        }
                    }
                };
                self.uploader
                    .upload_reel_resumable(
                        conn,
                        page_id,
                        page_token,
                        reel_path,
                        message,
                        scheduled_publish_time_epoch,
                        post_id,
                        owner_id,
                    )
                    .await
            }
            _ => PublishApiResult::DeterministicRejection {
                error_code: "unsupported_post_type".to_string(),
                error_message: format!(
                    "Unsupported post_type '{}' for scheduled post {}",
                    post_type, post_id
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE facebook_scheduled_posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                draft_group_id TEXT NOT NULL,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL,
                post_type TEXT NOT NULL,
                message TEXT NOT NULL,
                link_url TEXT,
                media_paths_json TEXT NOT NULL DEFAULT '[]',
                scheduling_mode TEXT NOT NULL DEFAULT 'meta',
                scheduled_publish_time TEXT,
                next_retry_at TEXT,
                remote_post_id TEXT,
                upload_session_id TEXT,
                idempotency_key TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'draft',
                retry_count INTEGER NOT NULL DEFAULT 0,
                owner_id TEXT,
                claim_deadline TEXT,
                error_code TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    fn insert_post(
        conn: &Connection,
        status: &str,
        next_retry_at: Option<&str>,
        scheduled_publish_time: Option<&str>,
    ) -> i64 {
        conn.execute(
            "INSERT INTO facebook_scheduled_posts
             (draft_group_id, channel_account_id, page_id, post_type, message, media_paths_json, scheduled_publish_time, next_retry_at, idempotency_key, status, created_at, updated_at)
             VALUES ('grp-test', 1, 'page-1', 'status', 'hello world', '[]', ?1, ?2, ?3, ?4, datetime('now'), datetime('now'))",
            params![scheduled_publish_time, next_retry_at, uuid::Uuid::new_v4().to_string(), status],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn query_cell<T: rusqlite::types::FromSql>(conn: &Connection, sql: &str, post_id: i64) -> T {
        conn.query_row(sql, [post_id], |r| r.get(0)).unwrap()
    }

    #[tokio::test]
    async fn renew_post_claim_updates_deadline_for_owner_and_rejects_foreign_owner() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "queued", None, None);
        conn.execute(
            "UPDATE facebook_scheduled_posts SET status = 'publishing', owner_id = 'worker-a', claim_deadline = '2000-01-01 00:00:00' WHERE id = ?1",
            [post_id],
        )
        .unwrap();

        FacebookPublisher::renew_post_claim(&conn, post_id, "worker-a")
            .expect("owner must renew its own lease");
        let renewed: String = query_cell(
            &conn,
            "SELECT claim_deadline FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(
            renewed.as_str() > "2000-01-01",
            "deadline must be pushed into the future"
        );

        let err = FacebookPublisher::renew_post_claim(&conn, post_id, "worker-b").unwrap_err();
        assert!(
            err.to_string().contains("Lost lease ownership"),
            "foreign owner must be rejected: {}",
            err
        );

        let owner: String = query_cell(
            &conn,
            "SELECT owner_id FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(
            owner, "worker-a",
            "ownership must survive a foreign renewal attempt"
        );
    }

    #[tokio::test]
    async fn renew_post_claim_rejects_non_publishing_status() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "queued", None, None);
        assert!(FacebookPublisher::renew_post_claim(&conn, post_id, "worker-a").is_err());
    }

    #[tokio::test]
    async fn publish_missing_post_errors() {
        let conn = setup_conn();
        let err = FacebookPublisher::new()
            .publish_post(&conn, 99_999)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "unexpected error: {}",
            err
        );
    }

    #[tokio::test]
    async fn publish_refuses_delivery_unknown_without_state_change() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "delivery_unknown", None, None);
        conn.execute(
            "UPDATE facebook_scheduled_posts SET owner_id = NULL WHERE id = ?1",
            [post_id],
        )
        .unwrap();

        let err = FacebookPublisher::new()
            .publish_post(&conn, post_id)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("delivery_unknown"),
            "unexpected error: {}",
            err
        );

        let status: String = query_cell(
            &conn,
            "SELECT status FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(
            status, "delivery_unknown",
            "delivery_unknown rows belong to the reconciler only"
        );
        let owner: Option<String> = query_cell(
            &conn,
            "SELECT owner_id FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(
            owner.is_none(),
            "no worker may take ownership of a delivery_unknown row"
        );
    }

    #[tokio::test]
    async fn publish_refuses_terminal_status_without_claim() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "published", None, None);
        let err = FacebookPublisher::new()
            .publish_post(&conn, post_id)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid status"),
            "unexpected error: {}",
            err
        );

        let status: String = query_cell(
            &conn,
            "SELECT status FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(status, "published");
        let owner: Option<String> = query_cell(
            &conn,
            "SELECT owner_id FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(owner.is_none());
    }

    #[tokio::test]
    async fn publish_respects_future_next_retry_backoff() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "queued", Some("2999-01-01 00:00:00"), None);

        let err = FacebookPublisher::new()
            .publish_post(&conn, post_id)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("invalid status 'queued'"),
            "backoff row must not be claimable yet: {}",
            err
        );

        let status: String = query_cell(
            &conn,
            "SELECT status FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(
            status, "queued",
            "post must remain queued while backoff is active"
        );
        let owner: Option<String> = query_cell(
            &conn,
            "SELECT owner_id FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(owner.is_none());
    }

    #[tokio::test]
    async fn publish_invalid_rfc3339_fails_before_claim() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "queued", None, Some("not-a-timestamp"));

        let err = FacebookPublisher::new()
            .publish_post(&conn, post_id)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Invalid RFC3339"),
            "unexpected error: {}",
            err
        );

        let status: String = query_cell(
            &conn,
            "SELECT status FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(
            status, "queued",
            "validation failure must happen before any mutation"
        );
        let owner: Option<String> = query_cell(
            &conn,
            "SELECT owner_id FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(owner.is_none());
    }

    #[tokio::test]
    async fn publish_claims_then_fails_closed_when_no_page_token_exists() {
        let conn = setup_conn();
        let post_id = insert_post(&conn, "queued", None, None);

        let err = FacebookPublisher::new()
            .publish_post(&conn, post_id)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("Failed to resolve valid token"),
            "missing token must abort before any network call: {}",
            err
        );

        let status: String = query_cell(
            &conn,
            "SELECT status FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(
            status, "failed",
            "token resolution failure must fail the post"
        );
        let error_code: String = query_cell(
            &conn,
            "SELECT error_code FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert_eq!(error_code, "token_vault_error");
        let error_message: String = query_cell(
            &conn,
            "SELECT error_message FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(!error_message.is_empty());

        let owner: Option<String> = query_cell(
            &conn,
            "SELECT owner_id FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(owner.is_none(), "lease must be released after failure");
        let deadline: Option<String> = query_cell(
            &conn,
            "SELECT claim_deadline FROM facebook_scheduled_posts WHERE id = ?",
            post_id,
        );
        assert!(
            deadline.is_none(),
            "claim deadline must be cleared after failure"
        );
    }
}
