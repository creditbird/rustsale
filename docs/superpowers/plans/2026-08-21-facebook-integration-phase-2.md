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
use reqwest::Client;
use serde_json::{json, Value};
use std::path::Path;
use crate::facebook::uploader::FacebookUploader;

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

    pub async fn publish_post(
        &self,
        conn: &rusqlite::Connection,
        page_id: &str,
        page_token: &str,
        post_type: &str,
        message: &str,
        link_url: Option<&str>,
        media_paths: &[String],
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        match post_type {
            "status" => {
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![("access_token", page_token), ("message", message)];
                let sched_str;
                if let Some(st) = scheduled_publish_time {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }
                let resp = self.client.post(&url).form(&form).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Status post failed: {}", resp.text().await?));
                }
                let json: Value = resp.json().await?;
                let post_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id.is_empty() {
                    return Err(anyhow!("Empty post ID returned from Meta"));
                }
                Ok(post_id)
            }
            "link" => {
                let link = link_url.ok_or_else(|| anyhow!("Link URL required for link post"))?;
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![("access_token", page_token), ("message", message), ("link", link)];
                let sched_str;
                if let Some(st) = scheduled_publish_time {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }
                let resp = self.client.post(&url).form(&form).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Link post failed: {}", resp.text().await?));
                }
                let json: Value = resp.json().await?;
                let post_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id.is_empty() {
                    return Err(anyhow!("Empty post ID returned from Meta"));
                }
                Ok(post_id)
            }
            "photo" => {
                let photo_path = media_paths.first().ok_or_else(|| anyhow!("At least 1 photo path required"))?;
                let file_bytes = tokio::fs::read(photo_path).await?;
                let file_name = Path::new(photo_path).file_name().and_then(|n| n.to_str()).unwrap_or("photo.jpg").to_string();
                let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

                let mut multipart = reqwest::multipart::Form::new()
                    .text("access_token", page_token.to_string())
                    .text("caption", message.to_string())
                    .part("source", part);

                if let Some(st) = scheduled_publish_time {
                    multipart = multipart.text("published", "false").text("scheduled_publish_time", st.to_string());
                }

                let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                let resp = self.client.post(&url).multipart(multipart).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Photo post failed: {}", resp.text().await?));
                }
                let json: Value = resp.json().await?;
                let post_id = json.get("post_id").or_else(|| json.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id.is_empty() {
                    return Err(anyhow!("Empty post ID returned from Meta"));
                }
                Ok(post_id)
            }
            "multi_photo" => {
                if media_paths.len() < 2 {
                    return Err(anyhow!("Multi-photo post requires at least 2 photos"));
                }
                let mut media_fbids = Vec::new();
                for path_str in media_paths {
                    let file_bytes = tokio::fs::read(path_str).await?;
                    let file_name = Path::new(path_str).file_name().and_then(|n| n.to_str()).unwrap_or("photo.jpg").to_string();
                    let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name);

                    let multipart = reqwest::multipart::Form::new()
                        .text("access_token", page_token.to_string())
                        .text("published", "false")
                        .part("source", part);

                    let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                    let resp = self.client.post(&url).multipart(multipart).send().await?;
                    if !resp.status().is_success() {
                        return Err(anyhow!("Multi-photo item upload failed: {}", resp.text().await?));
                    }
                    let json: Value = resp.json().await?;
                    let fbid = json.get("id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing photo id"))?;
                    media_fbids.push(fbid.to_string());
                }

                let attached_media: Vec<Value> = media_fbids.iter().map(|id| json!({ "media_fbid": id })).collect();
                let attached_str = serde_json::to_string(&attached_media)?;

                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut form = vec![
                    ("access_token", page_token),
                    ("message", message),
                    ("attached_media", &attached_str),
                ];
                let sched_str;
                if let Some(st) = scheduled_publish_time {
                    form.push(("published", "false"));
                    sched_str = st.to_string();
                    form.push(("scheduled_publish_time", &sched_str));
                }

                let resp = self.client.post(&url).form(&form).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Multi-photo feed post failed: {}", resp.text().await?));
                }
                let json: Value = resp.json().await?;
                let post_id = json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if post_id.is_empty() {
                    return Err(anyhow!("Empty post ID returned from Meta"));
                }
                Ok(post_id)
            }
            "video" => {
                let video_path = media_paths.first().ok_or_else(|| anyhow!("Video path required"))?;
                self.uploader.upload_video_resumable(conn, page_id, page_token, video_path, message, scheduled_publish_time).await
            }
            "reel" => {
                let reel_path = media_paths.first().ok_or_else(|| anyhow!("Reel path required"))?;
                self.uploader.upload_reel_resumable(conn, page_id, page_token, reel_path, message, scheduled_publish_time).await
            }
            _ => Err(anyhow!("Unsupported post type: {}", post_type)),
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
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

pub struct FacebookUploader {
    client: Client,
}

impl FacebookUploader {
    pub fn new() -> Self {
        Self { client: Client::new() }
    }

    pub async fn upload_video_resumable(
        &self,
        conn: &Connection,
        page_id: &str,
        page_token: &str,
        file_path: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let file_meta = tokio::fs::metadata(file_path).await?;
        let file_size = file_meta.len();

        // 1. Check existing in-progress session in facebook_upload_sessions
        let existing_session: Option<(String, String, u64)> = conn.query_row(
            "SELECT upload_session_id, video_id, start_offset 
             FROM facebook_upload_sessions 
             WHERE page_id = ?1 AND upload_type = 'video' AND file_path = ?2 AND file_size_bytes = ?3 AND status = 'in_progress'",
            params![page_id, file_path, file_size as i64],
            |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? as u64)),
        ).ok();

        let (upload_session_id, video_id, mut offset) = if let Some((sess_id, vid_id, _saved_offset)) = existing_session {
            // Query Graph video upload session for authoritative server offset
            let status_url = format!("https://graph-video.facebook.com/v26.0/{}?fields=start_offset,end_offset&access_token={}", sess_id, page_token);
            let status_resp = self.client.get(&status_url).send().await?;
            if !status_resp.status().is_success() {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![sess_id],
                );
                return Err(anyhow!("Failed to query authoritative server offset on video resume: {}", status_resp.text().await?));
            }
            let status_json: Value = status_resp.json().await?;
            let server_offset = status_json.get("start_offset")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| anyhow!("Missing start_offset in Meta status response on video resume"))?;

            if server_offset > file_size {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![sess_id],
                );
                return Err(anyhow!("Server offset {} exceeds file size {}", server_offset, file_size));
            }
            (sess_id, vid_id, server_offset)
        } else {
            // Start Phase
            let start_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
            let start_resp = self.client.post(&start_url)
                .form(&[
                    ("access_token", page_token),
                    ("upload_phase", "start"),
                    ("file_size", &file_size.to_string()),
                ])
                .send().await?;

            if !start_resp.status().is_success() {
                return Err(anyhow!("Video start phase failed: {}", start_resp.text().await?));
            }

            let start_json: Value = start_resp.json().await?;
            let sess_id = start_json.get("upload_session_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing upload_session_id"))?.to_string();
            let vid_id = start_json.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing video_id"))?.to_string();

            conn.execute(
                "INSERT INTO facebook_upload_sessions (page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, created_at, updated_at)
                 VALUES (?1, 'video', ?2, ?3, ?4, ?4, ?5, 0, 'in_progress', datetime('now'), datetime('now'))",
                params![page_id, sess_id, vid_id, file_path, file_size as i64],
            )?;

            (sess_id, vid_id, 0u64)
        };

        // 2. Transfer Phase
        let start_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let mut file = File::open(file_path).await?;
        let chunk_size: usize = 4 * 1024 * 1024; // 4MB

        while offset < file_size {
            let bytes_to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
            let mut buffer = vec![0u8; bytes_to_read];
            file.seek(SeekFrom::Start(offset)).await?;
            file.read_exact(&mut buffer).await?;

            let part = reqwest::multipart::Part::bytes(buffer).file_name("chunk.mp4");
            let form = reqwest::multipart::Form::new()
                .text("access_token", page_token.to_string())
                .text("upload_phase", "transfer")
                .text("upload_session_id", upload_session_id.clone())
                .text("start_offset", offset.to_string())
                .part("video_file_chunk", part);

            let transfer_resp = self.client.post(&start_url).multipart(form).send().await?;
            if !transfer_resp.status().is_success() {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![upload_session_id],
                );
                return Err(anyhow!("Video chunk upload failed: {}", transfer_resp.text().await?));
            }

            let transfer_json: Value = transfer_resp.json().await?;
            let next_offset = transfer_json.get("start_offset")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| anyhow!("Missing start_offset in Meta video transfer response"))?;

            if next_offset <= offset || next_offset > file_size {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![upload_session_id],
                );
                return Err(anyhow!("Invalid server offset progression: prev={}, next={}, file_size={}", offset, next_offset, file_size));
            }

            offset = next_offset;
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2",
                params![offset as i64, upload_session_id],
            );
        }

        // 3. Finish Phase
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

        let finish_resp = self.client.post(&start_url).json(&finish_body).send().await?;
        if !finish_resp.status().is_success() {
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                params![upload_session_id],
            );
            return Err(anyhow!("Video finish phase failed: {}", finish_resp.text().await?));
        }

        let _ = conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
            params![upload_session_id],
        );

        Ok(video_id)
    }

    pub async fn upload_reel_resumable(
        &self,
        conn: &Connection,
        page_id: &str,
        page_token: &str,
        file_path: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let file_meta = tokio::fs::metadata(file_path).await?;
        let file_size = file_meta.len();

        let existing_session: Option<(String, u64)> = conn.query_row(
            "SELECT video_id, start_offset 
             FROM facebook_upload_sessions 
             WHERE page_id = ?1 AND upload_type = 'reel' AND file_path = ?2 AND file_size_bytes = ?3 AND status = 'in_progress'",
            params![page_id, file_path, file_size as i64],
            |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)),
        ).ok();

        let (video_id, mut offset) = if let Some((vid_id, _saved_offset)) = existing_session {
            let check_url = format!("https://rupload.facebook.com/video-reels/{}", vid_id);
            let check_resp = self.client.get(&check_url)
                .header("Authorization", format!("OAuth {}", page_token))
                .send().await?;

            if !check_resp.status().is_success() {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![vid_id],
                );
                return Err(anyhow!("Failed to query Reel upload offset on resume: {}", check_resp.text().await?));
            }

            let check_json: Value = check_resp.json().await?;
            let server_offset = check_json.get("offset")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow!("Missing offset in Reel status response on resume"))?;

            if server_offset > file_size {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![vid_id],
                );
                return Err(anyhow!("Reel server offset {} exceeds file size {}", server_offset, file_size));
            }
            (vid_id, server_offset)
        } else {
            let init_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
            let init_resp = self.client.post(&init_url)
                .form(&[
                    ("access_token", page_token),
                    ("upload_phase", "start"),
                ])
                .send().await?;

            if !init_resp.status().is_success() {
                return Err(anyhow!("Reel init failed: {}", init_resp.text().await?));
            }

            let init_json: Value = init_resp.json().await?;
            let vid_id = init_json.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing video_id"))?.to_string();

            let _ = conn.execute(
                "INSERT INTO facebook_upload_sessions (page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, created_at, updated_at)
                 VALUES (?1, 'reel', ?2, ?2, ?3, ?3, ?4, 0, 'in_progress', datetime('now'), datetime('now'))",
                params![page_id, vid_id, file_path, file_size as i64],
            );
            (vid_id, 0u64)
        };

        let rupload_url = format!("https://rupload.facebook.com/video-reels/{}", video_id);
        let mut file = File::open(file_path).await?;
        let chunk_size: usize = 4 * 1024 * 1024;

        while offset < file_size {
            let bytes_to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
            let mut buffer = vec![0u8; bytes_to_read];
            file.seek(SeekFrom::Start(offset)).await?;
            file.read_exact(&mut buffer).await?;

            let resp = self.client.post(&rupload_url)
                .header("Authorization", format!("OAuth {}", page_token))
                .header("Offset", offset.to_string())
                .header("file_size", file_size.to_string())
                .body(buffer)
                .send().await?;

            if !resp.status().is_success() {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![video_id],
                );
                return Err(anyhow!("Reel chunk upload failed at offset {}: {}", offset, resp.text().await?));
            }

            let resp_json: Value = resp.json().await?;
            let confirmed_offset = resp_json.get("offset")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| anyhow!("Missing offset in Meta Reel chunk response"))?;

            if confirmed_offset != offset + bytes_to_read as u64 {
                let _ = conn.execute(
                    "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                    params![video_id],
                );
                return Err(anyhow!("Reel chunk offset mismatch: expected {}, got {}", offset + bytes_to_read as u64, confirmed_offset));
            }

            offset = confirmed_offset;
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2",
                params![offset as i64, video_id],
            );
        }

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
        let finish_resp = self.client.post(&finish_url).json(&finish_body).send().await?;
        if !finish_resp.status().is_success() {
            let _ = conn.execute(
                "UPDATE facebook_upload_sessions SET status = 'failed', updated_at = datetime('now') WHERE upload_session_id = ?1",
                params![video_id],
            );
            return Err(anyhow!("Reel finish failed: {}", finish_resp.text().await?));
        }

        let _ = conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
            params![video_id],
        );

        Ok(video_id)
    }
}
```

---

### Task 3: Fail-Closed Promotion Worker & Multi-Endpoint Reconciliation (`src-tauri/src/facebook/scheduler.rs`)

- **Promotion Worker:** Claims rows with `status='queued'`, queries `facebook_page_tokens` for the specific claimed row's `page_id`, strictly parses RFC3339 timestamp (failing closed with `error_code='invalid_timestamp'` on parse failure), and sets `delivery_unknown` on transport timeout or 5xx.
- **Reconciliation Engine:** Periodically claims `delivery_unknown` rows. Queries Graph API `/scheduled_posts` and `/feed` with pagination. Matches post type against Graph `status_type`, computes message SHA-256 hash, and verifies timestamp within 60s window. Sets `status='published'` for `/feed` matches and `status='scheduled_on_meta'` for `/scheduled_posts`.
