use anyhow::Result;
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::{json, Value};
use sha2::Digest;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

use crate::facebook::publisher::{FacebookPublisher, PublishApiResult};

pub struct FacebookUploader {
    client: Client,
}

impl Default for FacebookUploader {
    fn default() -> Self {
        Self::new()
    }
}

impl FacebookUploader {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    async fn verify_feed_publication(
        client: &Client,
        page_id: &str,
        page_token: &str,
        remote_id: &str,
    ) -> bool {
        let direct_url = format!(
            "https://graph.facebook.com/v26.0/{}_{}?fields=id,is_published&access_token={}",
            page_id, remote_id, page_token
        );
        if let Ok(resp) = client.get(&direct_url).send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<Value>().await {
                    let is_pub = json
                        .get("is_published")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if is_pub {
                        return true;
                    }
                }
            }
        }

        let mut next_url: Option<String> = Some(format!("https://graph.facebook.com/v26.0/{}/feed?fields=id,is_published&limit=50&access_token={}", page_id, page_token));
        let mut page_count = 0;
        while let Some(url) = next_url {
            if page_count >= 5 {
                break;
            }
            page_count += 1;
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<Value>().await {
                        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            for item in data {
                                if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                                    let is_pub = item
                                        .get("is_published")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false);
                                    if (id == remote_id
                                        || id == format!("{}_{}", page_id, remote_id))
                                        && is_pub
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                        next_url = json
                            .get("paging")
                            .and_then(|p| p.get("next"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                        continue;
                    }
                }
            }
            break;
        }
        false
    }

    async fn verify_scheduled_publication(
        client: &Client,
        page_id: &str,
        page_token: &str,
        remote_id: &str,
    ) -> bool {
        // Direct object lookup does not prove scheduled status; verify strictly against /scheduled_posts endpoint
        let mut next_url: Option<String> = Some(format!("https://graph.facebook.com/v26.0/{}/scheduled_posts?fields=id&limit=50&access_token={}", page_id, page_token));
        let mut page_count = 0;
        while let Some(url) = next_url {
            if page_count >= 5 {
                break;
            }
            page_count += 1;
            if let Ok(resp) = client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(json) = resp.json::<Value>().await {
                        if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                            for item in data {
                                if let Some(id) = item.get("id").and_then(|i| i.as_str()) {
                                    if id == remote_id || id == format!("{}_{}", page_id, remote_id)
                                    {
                                        return true;
                                    }
                                }
                            }
                        }
                        next_url = json
                            .get("paging")
                            .and_then(|p| p.get("next"))
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string());
                        continue;
                    }
                }
            }
            break;
        }
        false
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

    #[allow(clippy::too_many_arguments)]
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
            Err(e) => {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_metadata_error".to_string(),
                    error_message: format!(
                        "Failed to read file metadata for '{}': {}",
                        file_path, e
                    ),
                }
            }
        };
        let file_size = file_meta.len();
        let file_hash = match Self::compute_file_hash(file_path).await {
            Ok(h) => h,
            Err(e) => {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_hash_error".to_string(),
                    error_message: format!(
                        "Failed to compute file hash for '{}': {}",
                        file_path, e
                    ),
                }
            }
        };

        // 1. Check existing in-progress, finishing, or init_pending session in SQLite scoped to this post
        let existing_session: Option<(i64, String, Option<String>, u64, String, i64)> = match conn.query_row(
            "SELECT id, upload_session_id, video_id, start_offset, status, retry_count
             FROM facebook_upload_sessions
             WHERE post_id = ?1 AND page_id = ?2 AND upload_type = 'video' AND file_path = ?3 AND file_size_bytes = ?4 AND file_hash = ?5 AND status IN ('init_pending', 'in_progress', 'finishing')",
            params![post_id, page_id, file_path, file_size as i64, file_hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? as u64, r.get(4)?, r.get(5)?)),
        ) {
            Ok(sess) => Some(sess),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => return PublishApiResult::AmbiguousTransport {
                error_code: "db_error".to_string(),
                error_message: format!("Database query error looking up video upload session: {}", err),
            },
        };

        let (upload_session_id, video_id, mut offset) = if let Some((
            sess_row_id,
            sess_id,
            vid_id_opt,
            _saved_offset,
            session_status,
            retry_count,
        )) = existing_session
        {
            // Atomically claim single-writer session lease
            let rows_claimed = match conn.execute(
                "UPDATE facebook_upload_sessions
                 SET owner_id = ?1, lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE id = ?2 AND (owner_id = ?1 OR owner_id IS NULL OR lease_deadline < datetime('now'))",
                params![owner_id, sess_row_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_error".to_string(),
                    error_message: format!("Failed to claim video upload session lease: {}", e),
                },
            };
            if rows_claimed != 1 {
                return PublishApiResult::StillProcessing {
                    upload_session_id: sess_id,
                };
            }

            // Enforce max retry attempts fail-closed
            if retry_count >= 3 {
                let fail_rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2",
                    params![sess_row_id, owner_id],
                ).unwrap_or(0);
                if fail_rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while marking upload session failed after exceeding max retries".to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: "max_retries_exceeded".to_string(),
                    error_message: format!(
                        "Video upload session {} exceeded maximum retry limit ({})",
                        sess_id, retry_count
                    ),
                };
            }

            if session_status == "init_pending" {
                // Execute Meta START phase for uncommitted inert container
                let start_url =
                    format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
                let file_size_str = file_size.to_string();
                let start_resp = match self
                    .client
                    .post(&start_url)
                    .form(&[
                        ("access_token", page_token),
                        ("upload_phase", "start"),
                        ("file_size", file_size_str.as_str()),
                    ])
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!(
                                "Network error starting video upload session: {}",
                                e
                            ),
                        }
                    }
                };

                let status_code = start_resp.status();
                if !status_code.is_success() {
                    let retry_after = start_resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60);
                    let err_text = start_resp.text().await.unwrap_or_default();
                    if status_code.as_u16() == 429 {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                            params![sess_row_id, owner_id, retry_after as i64],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message:
                                    "Lost lease while recording 429 backoff for video START"
                                        .to_string(),
                            };
                        }
                        return PublishApiResult::RateLimited {
                            retry_after_secs: retry_after,
                            error_message: format!(
                                "HTTP 429 Rate limited on video start: {}",
                                err_text
                            ),
                        };
                    }
                    if status_code.is_client_error() {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message: "Lost lease while marking video start failed"
                                    .to_string(),
                            };
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!(
                                "Meta rejected video start phase (HTTP {}): {}",
                                status_code.as_u16(),
                                err_text
                            ),
                        };
                    }
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on video start phase (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }

                let start_json: Value = match start_resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!(
                                "Invalid JSON in video start phase response: {}",
                                e
                            ),
                        }
                    }
                };
                let new_sess_id = match start_json.get("upload_session_id").and_then(|v| v.as_str())
                {
                    Some(s) => s.to_string(),
                    None => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "missing_upload_session_id".to_string(),
                            error_message: "Missing upload_session_id in Meta video start response"
                                .to_string(),
                        }
                    }
                };
                let new_vid_id = match start_json.get("video_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "missing_video_id".to_string(),
                            error_message: "Missing video_id in Meta video start response"
                                .to_string(),
                        }
                    }
                };

                let update_rows = match conn.execute(
                    "UPDATE facebook_upload_sessions
                     SET upload_session_id = ?1, video_id = ?2, status = 'in_progress', updated_at = datetime('now')
                     WHERE id = ?3 AND owner_id = ?4 AND status = 'init_pending'",
                    params![new_sess_id, new_vid_id, sess_row_id, owner_id],
                ) {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "db_update_error".to_string(),
                        error_message: format!("Failed to update upload session in SQLite: {}", e),
                    },
                };
                if update_rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message:
                            "Lost lease on pre-START intent row while transitioning to in_progress"
                                .to_string(),
                    };
                }

                let assoc_rows = match conn.execute(
                    "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                    params![new_sess_id, post_id, owner_id],
                ) {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "db_update_error".to_string(),
                        error_message: format!("Failed to associate upload session with scheduled post: {}", e),
                    },
                };
                if assoc_rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "post_lease_loss".to_string(),
                        error_message:
                            "Lost lease on scheduled post while associating upload session"
                                .to_string(),
                    };
                }

                (new_sess_id, new_vid_id, 0u64)
            } else {
                let vid_id = match vid_id_opt {
                    Some(v) => v,
                    None => return PublishApiResult::DeterministicRejection {
                        error_code: "corrupted_session".to_string(),
                        error_message: "Upload session in progress/finishing state is missing authoritative video_id".to_string(),
                    },
                };

                if session_status == "finishing" {
                    // Check authoritative video status and feed/scheduled publication proof on Graph API
                    let check_url = format!("https://graph.facebook.com/v26.0/{}?fields=id,status,published&access_token={}", vid_id, page_token);
                    match self.client.get(&check_url).send().await {
                        Ok(check_resp) if check_resp.status().is_success() => {
                            let check_json: Value = check_resp.json().await.unwrap_or_default();
                            let video_status = check_json
                                .get("status")
                                .and_then(|s| s.get("video_status"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            let is_published = check_json
                                .get("published")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false);

                            if is_published {
                                let rows = conn.execute(
                                    "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                                    params![sess_row_id, owner_id],
                                ).unwrap_or(0);
                                if rows == 1 {
                                    return PublishApiResult::Success {
                                        remote_post_id: vid_id,
                                    };
                                }
                                return PublishApiResult::StillProcessing {
                                    upload_session_id: sess_id,
                                };
                            } else if video_status == "ready" {
                                let is_future = scheduled_publish_time
                                    .map(|t| t > chrono::Utc::now().timestamp())
                                    .unwrap_or(false);
                                let is_live = if is_future {
                                    Self::verify_scheduled_publication(
                                        &self.client,
                                        page_id,
                                        page_token,
                                        &vid_id,
                                    )
                                    .await
                                } else {
                                    Self::verify_feed_publication(
                                        &self.client,
                                        page_id,
                                        page_token,
                                        &vid_id,
                                    )
                                    .await
                                };
                                if is_live {
                                    let rows = conn.execute(
                                        "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                                        params![sess_row_id, owner_id],
                                    ).unwrap_or(0);
                                    if rows == 1 {
                                        return PublishApiResult::Success {
                                            remote_post_id: vid_id,
                                        };
                                    }
                                }
                                return PublishApiResult::StillProcessing {
                                    upload_session_id: sess_id,
                                };
                            } else if video_status == "error" {
                                let rows = conn.execute(
                                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                                    params![sess_row_id, owner_id],
                                ).unwrap_or(0);
                                if rows != 1 {
                                    return PublishApiResult::DeterministicRejection {
                                        error_code: "session_lease_loss".to_string(),
                                        error_message:
                                            "Lost lease while marking video session failed"
                                                .to_string(),
                                    };
                                }
                                return PublishApiResult::DeterministicRejection {
                                    error_code: "meta_video_error".to_string(),
                                    error_message: "Meta video processing failed with error state"
                                        .to_string(),
                                };
                            } else {
                                return PublishApiResult::StillProcessing {
                                    upload_session_id: sess_id,
                                };
                            }
                        }
                        Ok(check_resp) => {
                            let retry_after = check_resp
                                .headers()
                                .get("retry-after")
                                .and_then(|v| v.to_str().ok())
                                .and_then(|s| s.parse::<u64>().ok())
                                .unwrap_or(60);
                            let status_code = check_resp.status();
                            let err_text = check_resp.text().await.unwrap_or_default();
                            if status_code.as_u16() == 429 {
                                let rows = conn.execute(
                                    "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                                    params![sess_row_id, owner_id, retry_after as i64],
                                ).unwrap_or(0);
                                if rows != 1 {
                                    return PublishApiResult::DeterministicRejection {
                                        error_code: "session_lease_loss".to_string(),
                                        error_message: "Lost lease while recording 429 backoff for video status check".to_string(),
                                    };
                                }
                                return PublishApiResult::RateLimited {
                                    retry_after_secs: retry_after,
                                    error_message: format!(
                                        "HTTP 429 Rate limited during video check: {}",
                                        err_text
                                    ),
                                };
                            }
                            if status_code.is_client_error() {
                                return PublishApiResult::DeterministicRejection {
                                    error_code: format!("http_{}", status_code.as_u16()),
                                    error_message: format!(
                                        "Meta rejected video status check (HTTP {}): {}",
                                        status_code.as_u16(),
                                        err_text
                                    ),
                                };
                            }
                            return PublishApiResult::AmbiguousTransport {
                                error_code: format!("http_{}", status_code.as_u16()),
                                error_message: format!(
                                    "Meta server error on video status check (HTTP {}): {}",
                                    status_code.as_u16(),
                                    err_text
                                ),
                            };
                        }
                        Err(err) => {
                            return PublishApiResult::AmbiguousTransport {
                                error_code: "network_transport_error".to_string(),
                                error_message: format!(
                                    "Network error checking finishing video session: {}",
                                    err
                                ),
                            };
                        }
                    }
                } else {
                    // in_progress: query authoritative server offset
                    let status_url = format!("https://graph-video.facebook.com/v26.0/{}?fields=start_offset,end_offset&access_token={}", sess_id, page_token);
                    let status_resp = match self.client.get(&status_url).send().await {
                        Ok(r) => r,
                        Err(e) => {
                            return PublishApiResult::AmbiguousTransport {
                                error_code: "network_transport_error".to_string(),
                                error_message: format!(
                                    "Network error querying video upload session offset: {}",
                                    e
                                ),
                            }
                        }
                    };
                    let status_code = status_resp.status();
                    if !status_code.is_success() {
                        let retry_after = status_resp
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .and_then(|s| s.parse::<u64>().ok())
                            .unwrap_or(60);
                        let err_text = status_resp.text().await.unwrap_or_default();
                        if status_code.as_u16() == 429 {
                            let rows = conn.execute(
                                "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                                params![sess_row_id, owner_id, retry_after as i64],
                            ).unwrap_or(0);
                            if rows != 1 {
                                return PublishApiResult::DeterministicRejection {
                                    error_code: "session_lease_loss".to_string(),
                                    error_message: "Lost lease while recording 429 backoff for video resume query".to_string(),
                                };
                            }
                            return PublishApiResult::RateLimited {
                                retry_after_secs: retry_after,
                                error_message: format!(
                                    "HTTP 429 Rate limited on video resume query: {}",
                                    err_text
                                ),
                            };
                        }
                        if status_code.is_client_error() {
                            let rows = conn.execute(
                                "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                                params![sess_row_id, owner_id],
                            ).unwrap_or(0);
                            if rows != 1 {
                                return PublishApiResult::DeterministicRejection {
                                    error_code: "session_lease_loss".to_string(),
                                    error_message: "Lost lease while marking video resume failed"
                                        .to_string(),
                                };
                            }
                            return PublishApiResult::DeterministicRejection {
                                error_code: format!("http_{}", status_code.as_u16()),
                                error_message: format!(
                                    "Meta rejected video resume query (HTTP {}): {}",
                                    status_code.as_u16(),
                                    err_text
                                ),
                            };
                        }
                        return PublishApiResult::AmbiguousTransport {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!(
                                "Meta server error on video resume query (HTTP {}): {}",
                                status_code.as_u16(),
                                err_text
                            ),
                        };
                    }
                    let status_json: Value = match status_resp.json().await {
                        Ok(j) => j,
                        Err(e) => {
                            return PublishApiResult::AmbiguousTransport {
                                error_code: "malformed_json_response".to_string(),
                                error_message: format!(
                                    "Invalid JSON in video resume query response: {}",
                                    e
                                ),
                            }
                        }
                    };
                    let server_offset =
                        match status_json
                            .get("start_offset")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<u64>().ok())
                        {
                            Some(o) => o,
                            None => return PublishApiResult::AmbiguousTransport {
                                error_code: "missing_start_offset".to_string(),
                                error_message:
                                    "Missing start_offset in Meta status response on video resume"
                                        .to_string(),
                            },
                        };

                    if server_offset > file_size {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message:
                                    "Lost lease while handling out of bounds server offset"
                                        .to_string(),
                            };
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: "server_offset_out_of_bounds".to_string(),
                            error_message: format!(
                                "Server offset {} exceeds file size {}",
                                server_offset, file_size
                            ),
                        };
                    }
                    (sess_id, vid_id, server_offset)
                }
            }
        } else {
            // Pre-START intent persistence
            let intent_id = format!("intent_{}_{}", post_id, uuid::Uuid::new_v4());
            if let Err(e) = conn.execute(
                "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
                 VALUES (?1, ?2, 'video', ?3, NULL, ?4, ?5, ?6, 0, 'init_pending', 0, ?7, datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
                params![post_id, page_id, intent_id, file_path, file_hash, file_size as i64, owner_id],
            ) {
                return PublishApiResult::AmbiguousTransport {
                    error_code: "db_insert_error".to_string(),
                    error_message: format!("Failed to record pre-START upload intent: {}", e),
                };
            }

            let start_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
            let file_size_str = file_size.to_string();
            let start_resp = match self
                .client
                .post(&start_url)
                .form(&[
                    ("access_token", page_token),
                    ("upload_phase", "start"),
                    ("file_size", file_size_str.as_str()),
                ])
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!(
                            "Network error starting video upload session: {}",
                            e
                        ),
                    }
                }
            };

            let status_code = start_resp.status();
            if !status_code.is_success() {
                let retry_after = start_resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let err_text = start_resp.text().await.unwrap_or_default();
                if status_code.as_u16() == 429 {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                        params![intent_id, owner_id, retry_after as i64],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while recording 429 backoff on initial video start"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!(
                            "HTTP 429 Rate limited on video start: {}",
                            err_text
                        ),
                    };
                }
                if status_code.is_client_error() {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                        params![intent_id, owner_id],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message: "Lost lease while marking video start failed"
                                .to_string(),
                        };
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected video start phase (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta server error on video start phase (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }

            let start_json: Value = match start_resp.json().await {
                Ok(j) => j,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("Invalid JSON in video start phase response: {}", e),
                    }
                }
            };
            let sess_id = match start_json.get("upload_session_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "missing_upload_session_id".to_string(),
                        error_message: "Missing upload_session_id in Meta video start response"
                            .to_string(),
                    }
                }
            };
            let vid_id = match start_json.get("video_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "missing_video_id".to_string(),
                        error_message: "Missing video_id in Meta video start response".to_string(),
                    }
                }
            };

            let update_rows = match conn.execute(
                "UPDATE facebook_upload_sessions
                 SET upload_session_id = ?1, video_id = ?2, status = 'in_progress', updated_at = datetime('now')
                 WHERE upload_session_id = ?3 AND owner_id = ?4 AND status = 'init_pending'",
                params![sess_id, vid_id, intent_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_update_error".to_string(),
                    error_message: format!("Failed to update upload session with START IDs: {}", e),
                },
            };
            if update_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "session_lease_loss".to_string(),
                    error_message:
                        "Lost lease on pre-START intent row while transitioning to in_progress"
                            .to_string(),
                };
            }

            let assoc_rows = match conn.execute(
                "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                params![sess_id, post_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_update_error".to_string(),
                    error_message: format!("Failed to associate upload session with scheduled post: {}", e),
                },
            };
            if assoc_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "post_lease_loss".to_string(),
                    error_message: "Lost lease on scheduled post while associating upload session"
                        .to_string(),
                };
            }

            (sess_id, vid_id, 0u64)
        };

        // 2. Transfer Phase
        let start_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let mut file = match File::open(file_path).await {
            Ok(f) => f,
            Err(e) => {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_open_error".to_string(),
                    error_message: format!("Failed to open video file '{}': {}", file_path, e),
                }
            }
        };
        let chunk_size: usize = 4 * 1024 * 1024; // 4MB

        while offset < file_size {
            // Heartbeat lease renewal for BOTH post claim AND upload session lease before reading and uploading chunk
            if let Err(e) = FacebookPublisher::renew_post_claim(conn, post_id, owner_id) {
                return PublishApiResult::DeterministicRejection {
                    error_code: "lease_loss".to_string(),
                    error_message: format!("Aborted video upload due to lease loss: {}", e),
                };
            }
            let sess_lease_rows = match conn.execute(
                "UPDATE facebook_upload_sessions
                 SET lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                params![upload_session_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "db_error".to_string(),
                        error_message: format!("Failed to heartbeat upload session lease: {}", e),
                    }
                }
            };
            if sess_lease_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "session_lease_loss".to_string(),
                    error_message: "Aborted video upload due to upload session lease loss"
                        .to_string(),
                };
            }

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
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!(
                            "Transport error uploading video chunk at offset {}: {}",
                            offset, e
                        ),
                    }
                }
            };
            let status_code = transfer_resp.status();
            if !status_code.is_success() {
                let retry_after = transfer_resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let err_text = transfer_resp.text().await.unwrap_or_default();
                if status_code.as_u16() == 429 {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                        params![upload_session_id, owner_id, retry_after as i64],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while recording 429 backoff on video transfer"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!(
                            "HTTP 429 Rate limited during video transfer at offset {}: {}",
                            offset, err_text
                        ),
                    };
                }
                if status_code.is_client_error() {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                        params![upload_session_id, owner_id],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message: "Lost lease while marking video chunk failed"
                                .to_string(),
                        };
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected video chunk at offset {} (HTTP {}): {}",
                            offset,
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta server error on video chunk at offset {} (HTTP {}): {}",
                        offset,
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }

            let transfer_json: Value = match transfer_resp.json().await {
                Ok(j) => j,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("Invalid JSON in video chunk response: {}", e),
                    }
                }
            };
            let next_offset = match transfer_json
                .get("start_offset")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
            {
                Some(o) => o,
                None => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "missing_start_offset".to_string(),
                        error_message: "Missing start_offset in Meta video transfer response"
                            .to_string(),
                    }
                }
            };

            if next_offset <= offset || next_offset > file_size {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                    params![upload_session_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while handling invalid video offset progression"
                            .to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: "invalid_offset_progression".to_string(),
                    error_message: format!(
                        "Invalid server offset progression: prev={}, next={}, file_size={}",
                        offset, next_offset, file_size
                    ),
                };
            }

            offset = next_offset;
            let offset_rows = match conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
                params![offset as i64, upload_session_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_update_error".to_string(),
                    error_message: format!("Failed to update upload session offset in SQLite: {}", e),
                },
            };
            if offset_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "session_lease_loss".to_string(),
                    error_message: "Lost lease on upload session while updating offset".to_string(),
                };
            }
        }

        // 3. Finish Phase
        let finish_rows = match conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'finishing', updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
            params![upload_session_id, owner_id],
        ) {
            Ok(r) => r,
            Err(e) => return PublishApiResult::AmbiguousTransport {
                error_code: "db_update_error".to_string(),
                error_message: format!("Failed to mark session finishing in SQLite: {}", e),
            },
        };
        if finish_rows != 1 {
            return PublishApiResult::DeterministicRejection {
                error_code: "session_lease_loss".to_string(),
                error_message: "Lost lease on upload session while transitioning to finishing"
                    .to_string(),
            };
        }

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
            Err(e) => {
                return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Transport error finishing video upload: {}", e),
                }
            }
        };
        let status_code = finish_resp.status();
        if !status_code.is_success() {
            let retry_after = finish_resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            let err_text = finish_resp.text().await.unwrap_or_default();
            if status_code.as_u16() == 429 {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![upload_session_id, owner_id, retry_after as i64],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message:
                            "Lost lease while recording 429 backoff finishing video upload"
                                .to_string(),
                    };
                }
                return PublishApiResult::RateLimited {
                    retry_after_secs: retry_after,
                    error_message: format!(
                        "HTTP 429 Rate limited finishing video upload: {}",
                        err_text
                    ),
                };
            }
            if status_code.is_client_error() {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![upload_session_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while marking video finish failed".to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta rejected video finish phase (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }
            return PublishApiResult::AmbiguousTransport {
                error_code: format!("http_{}", status_code.as_u16()),
                error_message: format!(
                    "Meta server error on video finish phase (HTTP {}): {}",
                    status_code.as_u16(),
                    err_text
                ),
            };
        }

        let check_url = format!(
            "https://graph.facebook.com/v26.0/{}?fields=id,status,published&access_token={}",
            video_id, page_token
        );
        for attempt in 0..5 {
            if let Err(e) = FacebookPublisher::renew_post_claim(conn, post_id, owner_id) {
                return PublishApiResult::DeterministicRejection {
                    error_code: "lease_loss".to_string(),
                    error_message: format!("Aborted video status polling due to lease loss: {}", e),
                };
            }
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
            }

            let check_resp = match self.client.get(&check_url).send().await {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Network error during video status polling: {}", e),
                    }
                }
            };
            let status_code = check_resp.status();
            if !status_code.is_success() {
                let retry_after = check_resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let err_text = check_resp.text().await.unwrap_or_default();
                if status_code.as_u16() == 429 {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                        params![upload_session_id, owner_id, retry_after as i64],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while recording 429 backoff during video status polling"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!(
                            "HTTP 429 Rate limited during video status polling: {}",
                            err_text
                        ),
                    };
                }
                if status_code.is_client_error() {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                        params![upload_session_id, owner_id],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message: "Lost lease while marking video session failed during status polling".to_string(),
                        };
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected video status polling (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta server error on video status polling (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }

            let check_json: Value = check_resp.json().await.unwrap_or_default();
            let video_status = check_json
                .get("status")
                .and_then(|s| s.get("video_status"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let is_published = check_json
                .get("published")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_published {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![upload_session_id, owner_id],
                ).unwrap_or(0);
                if rows == 1 {
                    return PublishApiResult::Success {
                        remote_post_id: video_id,
                    };
                }
                return PublishApiResult::StillProcessing { upload_session_id };
            } else if video_status == "ready" {
                let is_future = scheduled_publish_time
                    .map(|t| t > chrono::Utc::now().timestamp())
                    .unwrap_or(false);
                let is_live = if is_future {
                    Self::verify_scheduled_publication(&self.client, page_id, page_token, &video_id)
                        .await
                } else {
                    Self::verify_feed_publication(&self.client, page_id, page_token, &video_id)
                        .await
                };
                if is_live {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                        params![upload_session_id, owner_id],
                    ).unwrap_or(0);
                    if rows == 1 {
                        return PublishApiResult::Success {
                            remote_post_id: video_id,
                        };
                    }
                }
                return PublishApiResult::StillProcessing { upload_session_id };
            } else if video_status == "error" {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![upload_session_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while marking video session failed".to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: "meta_video_error".to_string(),
                    error_message: "Meta video processing failed with error state".to_string(),
                };
            }
        }

        // Release lease on still-processing session so next cycle can claim cleanly via checked CAS
        let rows = conn.execute(
            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
            params![upload_session_id, owner_id],
        ).unwrap_or(0);
        if rows != 1 {
            return PublishApiResult::DeterministicRejection {
                error_code: "session_lease_loss".to_string(),
                error_message: "Lost lease while releasing still-processing video session"
                    .to_string(),
            };
        }

        PublishApiResult::StillProcessing { upload_session_id }
    }

    #[allow(clippy::too_many_arguments)]
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
        // Read file metadata and verify file exists
        let file_meta = match tokio::fs::metadata(file_path).await {
            Ok(m) => m,
            Err(e) => {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_metadata_error".to_string(),
                    error_message: format!(
                        "Failed to read file metadata for '{}': {}",
                        file_path, e
                    ),
                }
            }
        };
        let file_size = file_meta.len();
        if file_size == 0 {
            return PublishApiResult::DeterministicRejection {
                error_code: "empty_file".to_string(),
                error_message: format!("Reel file '{}' is empty (0 bytes)", file_path),
            };
        }

        let file_hash = match Self::compute_file_hash(file_path).await {
            Ok(h) => h,
            Err(e) => {
                return PublishApiResult::DeterministicRejection {
                    error_code: "file_hash_error".to_string(),
                    error_message: format!(
                        "Failed to compute file hash for '{}': {}",
                        file_path, e
                    ),
                }
            }
        };

        let existing_session: Option<(i64, String, Option<String>, u64, String, i64)> = match conn.query_row(
            "SELECT id, upload_session_id, video_id, start_offset, status, retry_count
             FROM facebook_upload_sessions
             WHERE post_id = ?1 AND page_id = ?2 AND upload_type = 'reel' AND file_path = ?3 AND file_size_bytes = ?4 AND file_hash = ?5 AND status IN ('init_pending', 'in_progress', 'finishing')",
            params![post_id, page_id, file_path, file_size as i64, file_hash],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get::<_, i64>(3)? as u64, r.get(4)?, r.get(5)?)),
        ) {
            Ok(sess) => Some(sess),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => return PublishApiResult::AmbiguousTransport {
                error_code: "db_error".to_string(),
                error_message: format!("Database query error looking up reel upload session: {}", err),
            },
        };

        let (video_id, already_uploaded) = if let Some((
            sess_row_id,
            sess_id,
            vid_id_opt,
            _saved_offset,
            session_status,
            retry_count,
        )) = existing_session
        {
            // Atomically claim single-writer session lease
            let rows_claimed = match conn.execute(
                "UPDATE facebook_upload_sessions
                 SET owner_id = ?1, lease_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE id = ?2 AND (owner_id = ?1 OR owner_id IS NULL OR lease_deadline < datetime('now'))",
                params![owner_id, sess_row_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_error".to_string(),
                    error_message: format!("Failed to claim Reel upload session lease: {}", e),
                },
            };
            if rows_claimed != 1 {
                return PublishApiResult::StillProcessing {
                    upload_session_id: sess_id,
                };
            }

            if retry_count >= 3 {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = ?3",
                    params![sess_row_id, owner_id, session_status],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while marking max-retried Reel session failed"
                            .to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: "max_retries_exceeded".to_string(),
                    error_message: format!(
                        "Reel upload session {} exceeded max retry attempts ({})",
                        sess_id, retry_count
                    ),
                };
            }

            if session_status == "init_pending" {
                // Recover from interrupted START phase
                let init_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
                let init_resp = match self
                    .client
                    .post(&init_url)
                    .form(&[("access_token", page_token), ("upload_phase", "start")])
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!(
                                "Network error re-initializing Reel upload: {}",
                                e
                            ),
                        }
                    }
                };
                let status_code = init_resp.status();
                if !status_code.is_success() {
                    let retry_after = init_resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60);
                    let err_text = init_resp.text().await.unwrap_or_default();
                    if status_code.as_u16() == 429 {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                            params![sess_row_id, owner_id, retry_after as i64],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message:
                                    "Lost lease while recording 429 backoff for Reel start retry"
                                        .to_string(),
                            };
                        }
                        return PublishApiResult::RateLimited {
                            retry_after_secs: retry_after,
                            error_message: format!(
                                "HTTP 429 Rate limited during Reel start retry: {}",
                                err_text
                            ),
                        };
                    }
                    if status_code.is_client_error() {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message:
                                    "Lost lease while marking Reel session failed on client error"
                                        .to_string(),
                            };
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!(
                                "Meta rejected Reel start retry (HTTP {}): {}",
                                status_code.as_u16(),
                                err_text
                            ),
                        };
                    }
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on Reel start retry (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }

                let init_json: Value = match init_resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!(
                                "Invalid JSON in Reel start retry response: {}",
                                e
                            ),
                        }
                    }
                };
                let new_vid_id = match init_json.get("video_id").and_then(|v| v.as_str()) {
                    Some(id) => id.to_string(),
                    None => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "missing_video_id".to_string(),
                            error_message: "Missing video_id in Reel start retry response"
                                .to_string(),
                        }
                    }
                };

                let rows = match conn.execute(
                    "UPDATE facebook_upload_sessions
                     SET upload_session_id = ?1, video_id = ?1, status = 'in_progress', updated_at = datetime('now')
                     WHERE id = ?2 AND owner_id = ?3 AND status = 'init_pending'",
                    params![new_vid_id, sess_row_id, owner_id],
                ) {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "db_error".to_string(),
                        error_message: format!("Failed to update Reel session after start retry: {}", e),
                    },
                };
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while transitioning Reel session to in_progress"
                            .to_string(),
                    };
                }

                let assoc_rows = match conn.execute(
                    "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                    params![new_vid_id, post_id, owner_id],
                ) {
                    Ok(r) => r,
                    Err(e) => return PublishApiResult::AmbiguousTransport {
                        error_code: "db_error".to_string(),
                        error_message: format!("Failed to associate Reel upload session on post: {}", e),
                    },
                };
                if assoc_rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "post_lease_loss".to_string(),
                        error_message:
                            "Lost lease on scheduled post while associating Reel upload session"
                                .to_string(),
                    };
                }

                (new_vid_id, false)
            } else {
                let vid_id = match vid_id_opt {
                    Some(v) => v,
                    None => return PublishApiResult::DeterministicRejection {
                        error_code: "corrupted_session".to_string(),
                        error_message: "Reel upload session in progress/finishing state is missing authoritative video_id".to_string(),
                    },
                };

                // Authoritative Graph API status verification on resume
                let check_url = format!(
                    "https://graph.facebook.com/v26.0/{}?fields=id,status&access_token={}",
                    vid_id, page_token
                );
                let check_resp = match self.client.get(&check_url).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "network_transport_error".to_string(),
                            error_message: format!(
                                "Network error querying Reel status on resume: {}",
                                e
                            ),
                        }
                    }
                };
                let status_code = check_resp.status();
                if !status_code.is_success() {
                    let retry_after = check_resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(60);
                    let err_text = check_resp.text().await.unwrap_or_default();
                    if status_code.as_u16() == 429 {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?4 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = ?3",
                            params![sess_row_id, owner_id, session_status, retry_after as i64],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message: "Lost lease while recording 429 backoff during Reel status check".to_string(),
                            };
                        }
                        return PublishApiResult::RateLimited {
                            retry_after_secs: retry_after,
                            error_message: format!(
                                "HTTP 429 Rate limited during Reel check: {}",
                                err_text
                            ),
                        };
                    }
                    if status_code.is_client_error() {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = ?3",
                            params![sess_row_id, owner_id, session_status],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message:
                                    "Lost lease while marking Reel session failed on client error"
                                        .to_string(),
                            };
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: format!("http_{}", status_code.as_u16()),
                            error_message: format!(
                                "Meta rejected reel status check (HTTP {}): {}",
                                status_code.as_u16(),
                                err_text
                            ),
                        };
                    }
                    return PublishApiResult::AmbiguousTransport {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta server error on reel status check (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }

                let check_json: Value = match check_resp.json().await {
                    Ok(j) => j,
                    Err(e) => {
                        return PublishApiResult::AmbiguousTransport {
                            error_code: "malformed_json_response".to_string(),
                            error_message: format!(
                                "Invalid JSON in Reel status check response: {}",
                                e
                            ),
                        }
                    }
                };
                let status_obj =
                    match check_json.get("status").and_then(|s| s.as_object()) {
                        Some(o) => o,
                        None => return PublishApiResult::AmbiguousTransport {
                            error_code: "missing_status_object".to_string(),
                            error_message:
                                "Missing or invalid 'status' object in Reel status check response"
                                    .to_string(),
                        },
                    };
                let video_status = status_obj
                    .get("video_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let uploading_status = status_obj
                    .get("uploading_phase")
                    .and_then(|p| p.get("status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let publishing_status = status_obj
                    .get("publishing_phase")
                    .and_then(|p| p.get("status"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Fail-closed authoritative-status gate: Meta's documented status payload must
                // expose at least one classification field (video_status, uploading_phase.status,
                // publishing_phase.status). An empty/partial status object cannot establish the
                // upload state, so the session must NOT be blindly re-streamed; it is marked
                // delivery_unknown via AmbiguousTransport and resolved by reconciliation instead.
                if video_status.is_empty()
                    && uploading_status.is_empty()
                    && publishing_status.is_empty()
                {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "missing_authoritative_status_fields".to_string(),
                        error_message: "Meta Reel status response exposes no authoritative video_status/uploading_phase.status/publishing_phase.status; upload state unknown".to_string(),
                    };
                }

                if session_status == "finishing" {
                    if publishing_status == "complete" || video_status == "ready" {
                        let is_future = scheduled_publish_time
                            .map(|t| t > chrono::Utc::now().timestamp())
                            .unwrap_or(false);
                        let is_live = if is_future {
                            Self::verify_scheduled_publication(
                                &self.client,
                                page_id,
                                page_token,
                                &vid_id,
                            )
                            .await
                        } else {
                            Self::verify_feed_publication(
                                &self.client,
                                page_id,
                                page_token,
                                &vid_id,
                            )
                            .await
                        };
                        if is_live {
                            let rows = conn.execute(
                                "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                                params![sess_row_id, owner_id],
                            ).unwrap_or(0);
                            if rows == 1 {
                                return PublishApiResult::Success {
                                    remote_post_id: vid_id,
                                };
                            }
                        }
                        // Release lease on still-processing finishing session so next cycle can claim cleanly via checked CAS
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message: "Lost lease while releasing still-processing Reel finishing session".to_string(),
                            };
                        }
                        return PublishApiResult::StillProcessing {
                            upload_session_id: sess_id,
                        };
                    } else if video_status == "error"
                        || uploading_status == "error"
                        || publishing_status == "error"
                    {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message: "Lost lease while marking Reel session failed"
                                    .to_string(),
                            };
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: "meta_reel_error".to_string(),
                            error_message: "Meta Reel processing failed with error state"
                                .to_string(),
                        };
                    } else {
                        // Release lease on still-processing finishing session so next cycle can claim cleanly via checked CAS
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message: "Lost lease while releasing still-processing Reel finishing session".to_string(),
                            };
                        }
                        return PublishApiResult::StillProcessing {
                            upload_session_id: sess_id,
                        };
                    }
                } else {
                    // session_status == "in_progress"
                    if uploading_status == "complete" || video_status == "ready" {
                        (vid_id, true)
                    } else if video_status == "error" || uploading_status == "error" {
                        let rows = conn.execute(
                            "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                            params![sess_row_id, owner_id],
                        ).unwrap_or(0);
                        if rows != 1 {
                            return PublishApiResult::DeterministicRejection {
                                error_code: "session_lease_loss".to_string(),
                                error_message: "Lost lease while marking Reel session failed"
                                    .to_string(),
                            };
                        }
                        return PublishApiResult::DeterministicRejection {
                            error_code: "meta_reel_error".to_string(),
                            error_message: "Meta Reel processing failed with error state"
                                .to_string(),
                        };
                    } else {
                        (vid_id, false)
                    }
                }
            }
        } else {
            // Pre-START intent persistence
            let intent_id = format!("intent_{}_{}", post_id, uuid::Uuid::new_v4());
            if let Err(e) = conn.execute(
                "INSERT INTO facebook_upload_sessions (post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, retry_count, owner_id, lease_deadline, created_at, updated_at)
                 VALUES (?1, ?2, 'reel', ?3, NULL, ?4, ?5, ?6, 0, 'init_pending', 0, ?7, datetime('now', '+5 minutes'), datetime('now'), datetime('now'))",
                params![post_id, page_id, intent_id, file_path, file_hash, file_size as i64, owner_id],
            ) {
                return PublishApiResult::AmbiguousTransport {
                    error_code: "db_insert_error".to_string(),
                    error_message: format!("Failed to record pre-START Reel intent: {}", e),
                };
            }

            let init_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
            let init_resp = match self
                .client
                .post(&init_url)
                .form(&[("access_token", page_token), ("upload_phase", "start")])
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Network error initializing Reel upload: {}", e),
                    }
                }
            };
            let status_code = init_resp.status();
            if !status_code.is_success() {
                let retry_after = init_resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let err_text = init_resp.text().await.unwrap_or_default();
                if status_code.as_u16() == 429 {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                        params![intent_id, owner_id, retry_after as i64],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while recording 429 backoff for initial Reel start"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!(
                            "HTTP 429 Rate limited on initial Reel start: {}",
                            err_text
                        ),
                    };
                }
                if status_code.is_client_error() {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'init_pending'",
                        params![intent_id, owner_id],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message: "Lost lease while marking initial Reel session failed"
                                .to_string(),
                        };
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected Reel start (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta server error on Reel start (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }

            let init_json: Value = match init_resp.json().await {
                Ok(j) => j,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!("Invalid JSON in Reel start response: {}", e),
                    }
                }
            };
            let vid_id = match init_json.get("video_id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "missing_video_id".to_string(),
                        error_message: "Missing video_id in Reel start response".to_string(),
                    }
                }
            };

            let rows = match conn.execute(
                "UPDATE facebook_upload_sessions
                 SET upload_session_id = ?1, video_id = ?1, status = 'in_progress', updated_at = datetime('now')
                 WHERE upload_session_id = ?2 AND owner_id = ?3 AND status = 'init_pending'",
                params![vid_id, intent_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_error".to_string(),
                    error_message: format!("Failed to update Reel session after start: {}", e),
                },
            };
            if rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "session_lease_loss".to_string(),
                    error_message: "Lost lease while transitioning Reel session to in_progress"
                        .to_string(),
                };
            }

            let assoc_rows = match conn.execute(
                "UPDATE facebook_scheduled_posts SET upload_session_id = ?1 WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                params![vid_id, post_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_error".to_string(),
                    error_message: format!("Failed to associate Reel upload session: {}", e),
                },
            };
            if assoc_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "post_lease_loss".to_string(),
                    error_message:
                        "Lost lease on scheduled post while associating Reel upload session"
                            .to_string(),
                };
            }

            (vid_id, false)
        };

        // 2. Binary Upload Phase with Bounded-Memory File Streaming (if not already uploaded)
        if !already_uploaded {
            let stream_lease_mins = std::cmp::max(30, ((file_size / (1024 * 1024)) * 2) as i64);
            let claim_deadline_str = format!("+{} minutes", stream_lease_mins);
            let post_lease_rows = match conn.execute(
                "UPDATE facebook_scheduled_posts
                 SET claim_deadline = datetime('now', ?1), updated_at = datetime('now')
                 WHERE id = ?2 AND owner_id = ?3 AND status = 'publishing'",
                params![&claim_deadline_str, post_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "db_error".to_string(),
                        error_message: format!(
                            "Failed to extend scheduled post claim deadline for Reel stream: {}",
                            e
                        ),
                    }
                }
            };
            if post_lease_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "post_lease_loss".to_string(),
                    error_message: "Aborted Reel upload due to post lease loss".to_string(),
                };
            }

            let sess_lease_rows = match conn.execute(
                "UPDATE facebook_upload_sessions
                 SET lease_deadline = datetime('now', ?1), updated_at = datetime('now')
                 WHERE upload_session_id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
                params![&claim_deadline_str, video_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "db_error".to_string(),
                        error_message: format!("Failed to extend Reel upload session lease: {}", e),
                    }
                }
            };
            if sess_lease_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "session_lease_loss".to_string(),
                    error_message: "Aborted Reel upload due to Reel upload session lease loss"
                        .to_string(),
                };
            }

            let file = match tokio::fs::File::open(file_path).await {
                Ok(f) => f,
                Err(e) => {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "file_open_error".to_string(),
                        error_message: format!("Failed to open Reel file '{}': {}", file_path, e),
                    }
                }
            };
            let stream = tokio_util::io::ReaderStream::new(file);
            let body = reqwest::Body::wrap_stream(stream);

            let rupload_url = format!(
                "https://rupload.facebook.com/video-upload/v26.0/{}",
                video_id
            );
            let resp = match self
                .client
                .post(&rupload_url)
                .header("Authorization", format!("OAuth {}", page_token))
                .header("offset", "0")
                .header("file_size", file_size.to_string())
                .header("Content-Type", "application/octet-stream")
                .body(body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!(
                            "Network error uploading Reel binary payload: {}",
                            e
                        ),
                    }
                }
            };

            let status_code = resp.status();
            if !status_code.is_success() {
                let retry_after = resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let err_text = resp.text().await.unwrap_or_default();
                if status_code.as_u16() == 429 {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                        params![video_id, owner_id, retry_after as i64],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while recording 429 backoff for Reel binary upload"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!(
                            "HTTP 429 Rate limited uploading Reel binary payload: {}",
                            err_text
                        ),
                    };
                }
                if status_code.is_client_error() {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                        params![video_id, owner_id],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message: "Lost lease while marking Reel upload failed"
                                .to_string(),
                        };
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected Reel binary upload (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta server error on Reel binary upload (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }

            let resp_json: Value = match resp.json().await {
                Ok(j) => j,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!(
                            "Invalid JSON in Reel binary upload response: {}",
                            e
                        ),
                    }
                }
            };
            let is_success = resp_json.get("success").and_then(|v| v.as_bool()) == Some(true);
            if !is_success {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
                    params![video_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while handling failed Reel binary upload"
                            .to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: "upload_rejected".to_string(),
                    error_message: format!(
                        "Meta Reel upload rejected with response: {}",
                        resp_json
                    ),
                };
            }

            let offset_rows = match conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2 AND owner_id = ?3 AND status = 'in_progress'",
                params![file_size as i64, video_id, owner_id],
            ) {
                Ok(r) => r,
                Err(e) => return PublishApiResult::AmbiguousTransport {
                    error_code: "db_error".to_string(),
                    error_message: format!("Failed to update Reel upload session offset: {}", e),
                },
            };
            if offset_rows != 1 {
                return PublishApiResult::DeterministicRejection {
                    error_code: "session_lease_loss".to_string(),
                    error_message: "Lost lease on Reel session while updating completed offset"
                        .to_string(),
                };
            }
        }

        // 3. Finish Phase
        if let Err(e) = FacebookPublisher::renew_post_claim(conn, post_id, owner_id) {
            return PublishApiResult::DeterministicRejection {
                error_code: "lease_loss".to_string(),
                error_message: format!("Aborted Reel finish due to lease loss: {}", e),
            };
        }

        let finishing_rows = match conn.execute(
            "UPDATE facebook_upload_sessions
             SET status = 'finishing', updated_at = datetime('now')
             WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'in_progress'",
            params![video_id, owner_id],
        ) {
            Ok(r) => r,
            Err(e) => {
                return PublishApiResult::AmbiguousTransport {
                    error_code: "db_error".to_string(),
                    error_message: format!("Failed to advance Reel session to finishing: {}", e),
                }
            }
        };
        if finishing_rows != 1 {
            return PublishApiResult::DeterministicRejection {
                error_code: "session_lease_loss".to_string(),
                error_message: "Lost lease on Reel session while transitioning to finishing"
                    .to_string(),
            };
        }

        let finish_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
        let mut finish_body = serde_json::json!({
            "access_token": page_token,
            "upload_phase": "finish",
            "video_id": video_id,
            "video_state": "PUBLISHED",
            "description": description,
        });
        if let Some(scheduled_epoch) = scheduled_publish_time {
            if scheduled_epoch > chrono::Utc::now().timestamp() {
                finish_body["video_state"] = serde_json::json!("SCHEDULED");
                finish_body["scheduled_publish_time"] = serde_json::json!(scheduled_epoch);
            }
        }

        let finish_resp = match self
            .client
            .post(&finish_url)
            .json(&finish_body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return PublishApiResult::AmbiguousTransport {
                    error_code: "network_transport_error".to_string(),
                    error_message: format!("Network error finalizing Reel upload: {}", e),
                }
            }
        };
        let status_code = finish_resp.status();
        if !status_code.is_success() {
            let retry_after = finish_resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(60);
            let err_text = finish_resp.text().await.unwrap_or_default();
            if status_code.as_u16() == 429 {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![video_id, owner_id, retry_after as i64],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while recording 429 backoff for Reel finish"
                            .to_string(),
                    };
                }
                return PublishApiResult::RateLimited {
                    retry_after_secs: retry_after,
                    error_message: format!(
                        "HTTP 429 Rate limited during Reel finish: {}",
                        err_text
                    ),
                };
            }
            if status_code.is_client_error() {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![video_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message:
                            "Lost lease while marking Reel session failed on finish error"
                                .to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta rejected Reel finish (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }
            return PublishApiResult::AmbiguousTransport {
                error_code: format!("http_{}", status_code.as_u16()),
                error_message: format!(
                    "Meta server error on Reel finish (HTTP {}): {}",
                    status_code.as_u16(),
                    err_text
                ),
            };
        }

        // Authoritative terminal status verification with bounded polling and checked error handling
        let check_url = format!(
            "https://graph.facebook.com/v26.0/{}?fields=id,status&access_token={}",
            video_id, page_token
        );
        for attempt in 0..5 {
            if let Err(e) = FacebookPublisher::renew_post_claim(conn, post_id, owner_id) {
                return PublishApiResult::DeterministicRejection {
                    error_code: "lease_loss".to_string(),
                    error_message: format!("Aborted Reel status polling due to lease loss: {}", e),
                };
            }
            if attempt > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(1000 * attempt as u64)).await;
            }

            let check_resp = match self.client.get(&check_url).send().await {
                Ok(r) => r,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "network_transport_error".to_string(),
                        error_message: format!("Transport error polling Reel status: {}", e),
                    }
                }
            };
            let status_code = check_resp.status();
            if !status_code.is_success() {
                let retry_after = check_resp
                    .headers()
                    .get("retry-after")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(60);
                let err_text = check_resp.text().await.unwrap_or_default();
                if status_code.as_u16() == 429 {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = datetime('now', '+' || ?3 || ' seconds'), retry_count = retry_count + 1, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                        params![video_id, owner_id, retry_after as i64],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while recording 429 backoff during Reel status polling"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::RateLimited {
                        retry_after_secs: retry_after,
                        error_message: format!(
                            "HTTP 429 Rate limited during Reel status polling: {}",
                            err_text
                        ),
                    };
                }
                if status_code.is_client_error() {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                        params![video_id, owner_id],
                    ).unwrap_or(0);
                    if rows != 1 {
                        return PublishApiResult::DeterministicRejection {
                            error_code: "session_lease_loss".to_string(),
                            error_message:
                                "Lost lease while marking Reel session failed during status polling"
                                    .to_string(),
                        };
                    }
                    return PublishApiResult::DeterministicRejection {
                        error_code: format!("http_{}", status_code.as_u16()),
                        error_message: format!(
                            "Meta rejected Reel status polling (HTTP {}): {}",
                            status_code.as_u16(),
                            err_text
                        ),
                    };
                }
                return PublishApiResult::AmbiguousTransport {
                    error_code: format!("http_{}", status_code.as_u16()),
                    error_message: format!(
                        "Meta server error on Reel status polling (HTTP {}): {}",
                        status_code.as_u16(),
                        err_text
                    ),
                };
            }

            let check_json: Value = match check_resp.json().await {
                Ok(j) => j,
                Err(e) => {
                    return PublishApiResult::AmbiguousTransport {
                        error_code: "malformed_json_response".to_string(),
                        error_message: format!(
                            "Invalid JSON in Reel status polling response: {}",
                            e
                        ),
                    }
                }
            };
            let video_status = check_json
                .get("status")
                .and_then(|s| s.get("video_status"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let publishing_status = check_json
                .get("status")
                .and_then(|s| s.get("publishing_phase"))
                .and_then(|p| p.get("status"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if publishing_status == "complete" {
                let is_future = scheduled_publish_time
                    .map(|t| t > chrono::Utc::now().timestamp())
                    .unwrap_or(false);
                let is_live = if is_future {
                    Self::verify_scheduled_publication(&self.client, page_id, page_token, &video_id)
                        .await
                } else {
                    Self::verify_feed_publication(&self.client, page_id, page_token, &video_id)
                        .await
                };
                if is_live {
                    let rows = conn.execute(
                        "UPDATE facebook_upload_sessions SET status = 'completed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                        params![video_id, owner_id],
                    ).unwrap_or(0);
                    if rows == 1 {
                        return PublishApiResult::Success {
                            remote_post_id: video_id,
                        };
                    }
                }
                // Release the possibly stream-extended lease via checked CAS before yielding,
                // so reconciliation can re-claim on the next cycle instead of waiting out a
                // long binary-upload lease deadline for a publication that Meta already completed.
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![video_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while releasing not-yet-live completed Reel"
                            .to_string(),
                    };
                }
                return PublishApiResult::StillProcessing {
                    upload_session_id: video_id,
                };
            } else if video_status == "error" || publishing_status == "error" {
                let rows = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
                    params![video_id, owner_id],
                ).unwrap_or(0);
                if rows != 1 {
                    return PublishApiResult::DeterministicRejection {
                        error_code: "session_lease_loss".to_string(),
                        error_message: "Lost lease while marking Reel session failed".to_string(),
                    };
                }
                return PublishApiResult::DeterministicRejection {
                    error_code: "meta_reel_error".to_string(),
                    error_message: "Meta Reel processing failed with error state".to_string(),
                };
            }
        }

        // Release lease on still-processing session so next cycle can claim cleanly via checked CAS
        let rows = conn.execute(
            "UPDATE facebook_upload_sessions SET owner_id = NULL, lease_deadline = NULL, updated_at = datetime('now') WHERE upload_session_id = ?1 AND owner_id = ?2 AND status = 'finishing'",
            params![video_id, owner_id],
        ).unwrap_or(0);
        if rows != 1 {
            return PublishApiResult::DeterministicRejection {
                error_code: "session_lease_loss".to_string(),
                error_message: "Lost lease while releasing still-processing Reel session"
                    .to_string(),
            };
        }

        PublishApiResult::StillProcessing {
            upload_session_id: video_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn compute_file_hash_matches_direct_sha256_of_content() {
        let path = std::env::temp_dir().join(format!(
            "rustsale_uploader_hash_test_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let payload = b"rustsale-facebook-uploader-streaming-hash-payload";
        std::fs::write(&path, payload).expect("failed to write temp fixture");

        let streamed = FacebookUploader::compute_file_hash(path.to_str().unwrap())
            .await
            .expect("streaming hash failed");

        let mut direct = sha2::Sha256::new();
        direct.update(payload);
        let direct_hex = format!("{:x}", direct.finalize());

        assert_eq!(streamed, direct_hex);
        std::fs::remove_file(&path).ok();
    }

    #[tokio::test]
    async fn compute_file_hash_is_stable_and_content_sensitive() {
        let base = std::env::temp_dir().join(format!(
            "rustsale_uploader_hash_stability_{}_{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&base, b"content-a").unwrap();

        let first = FacebookUploader::compute_file_hash(base.to_str().unwrap())
            .await
            .unwrap();
        let second = FacebookUploader::compute_file_hash(base.to_str().unwrap())
            .await
            .unwrap();
        assert_eq!(
            first, second,
            "hash must be deterministic for identical content"
        );

        let other = base.with_extension("bin");
        std::fs::write(&other, b"content-b").unwrap();
        let third = FacebookUploader::compute_file_hash(other.to_str().unwrap())
            .await
            .unwrap();
        assert_ne!(
            first, third,
            "different content must produce different hashes"
        );

        std::fs::remove_file(&base).ok();
        std::fs::remove_file(&other).ok();
    }

    #[tokio::test]
    async fn compute_file_hash_errors_for_missing_file() {
        let missing = std::env::temp_dir().join("definitely-missing-rustsale-file.bin");
        assert!(
            FacebookUploader::compute_file_hash(missing.to_str().unwrap())
                .await
                .is_err()
        );
    }
}
