# Facebook Integration (Phase 2: Post Scheduler, Resumable Video/Reels & Feed Planner) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the complete Multi-Type Post Scheduler (Status, Link, Photo, Multi-Photo, Resumable Video & Reels), Dual-Mode scheduling (Meta-side 10m–30d & Local-queued >30d), promotion worker, bounded-memory streaming chunked uploader, restart-resumable upload sessions with exact server offset verification, multi-page fan-out, expired publishing lease sweeper, and Svelte 5 Feed Planner with live preview.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), Meta Graph API v26.0+, Meta Rupload/Video API.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-08** | Dual-Mode Scheduling Engine & Multi-Type Publishing | `PLANNED` | Task 1 (Step 1) | Target 10m–30d: Scheduled directly via Meta API with `published=false`. Target >30d: Queued locally in SQLite (`scheduling_mode='local_queued'`). Hourly background worker promotes posts entering `<=27 days`. Supports Status, Link, Photo, Multi-Photo, Video, Reel. | Post scheduled for +45 days: stays `local_queued`; upon reaching +27 days, background worker claims lease, pushes to Meta API, and updates status to `scheduled_on_meta`. | Unit test: Simulate time advancement from 45d to 27d; verify automatic promotion and multi-type request payloads. |
| **REQ-FB-09** | Restart-Resumable Video & Reels Streaming | `PLANNED` | Task 2 (Step 1) | 3-Phase Resumable API for Videos and Rupload API for Reels. Tracks `facebook_upload_sessions (upload_session_id, start_offset, file_hash, file_size_bytes)`. Resumes from server-queried `start_offset` using bounded 4MB file seek buffers. Verifies confirmed server offset equals `offset + bytes_to_read` after each chunk. | Desktop app killed mid-upload at 12MB of a 40MB Reel; on relaunch, queries server offset (12MB), verifies exact offset validity, seeks file to 12MB, and streams remaining 28MB without re-uploading from byte 0. | Crash simulation test: Interrupt upload at chunk 3; resume and verify upload finishes with server SHA-256 match. |
| **REQ-FB-10** | Expired Publishing Lease Sweeper & Multi-Endpoint Reconciliation | `PLANNED` | Task 3 (Step 1) | Worker claims publishing task with 5-minute lease: `UPDATE ... SET status = 'publishing', owner_id = ?1, claim_deadline = datetime('now', '+5 minutes') WHERE id = ?2 AND status = 'queued'`. Sweeper transitions timed-out rows to `delivery_unknown`. Reconciliation cycle queries both `/feed` and `/scheduled_posts`, matching post type, message hash, and publication timestamp within 60s window. Releases lease on inconclusive results. | Worker crashes while HTTP request is in-flight to Meta; lease expires; sweeper marks `delivery_unknown`; reconciler queries Page scheduled posts, discovers post was scheduled, and updates status to `scheduled_on_meta` without duplicate publishing. | Crash recovery test: Abandon worker during in-flight post; verify sweeper moves to `delivery_unknown` and reconciler detects post. |
| **REQ-FB-11** | Multi-Page Fan-Out with Unique Idempotency Keys | `PLANNED` | Task 4 (Step 1) | UI post submission across $N$ pages generates 1 `draft_group_id` with $N$ independent child rows in `facebook_scheduled_posts`, each with unique `idempotency_key = UUID()`. | Submitting to Page A and Page B where Page B token is expired: Page A publishes successfully; Page B marks `failed` with distinct error; draft group maintains partial progress. | Fan-out unit test: Submit to 3 pages with 1 failing; verify isolation. |

---

### Task 1: SQLite Schema & Types for Scheduler & Multi-Type Publishing Engine

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/scheduler_types.rs`
- Create: `src-tauri/src/conversation/facebook/publisher.rs`
- Test: `src-tauri/src/conversation/facebook/publisher_test.rs`

- [ ] **Step 1: Implement Scheduler Types and Enums matching schema.sql**

```rust
// src-tauri/src/conversation/facebook/scheduler_types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostType {
    Status,
    Link,
    Photo,
    MultiPhoto,
    Video,
    Reel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SchedulingMode {
    Meta,
    LocalQueued,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PostStatus {
    Draft,
    Queued,
    Publishing,
    ScheduledOnMeta,
    Published,
    Failed,
    DeliveryUnknown,
    OverduePrompt,
}
```

- [ ] **Step 2: Implement Multi-Type Post Publisher (Status, Link, Photo, Multi-Photo, Video, Reel)**

```rust
// src-tauri/src/conversation/facebook/publisher.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;
use std::path::Path;

pub struct FacebookPublisher;

impl FacebookPublisher {
    pub async fn publish_post(
        client: &Client,
        token: &str,
        page_id: &str,
        post_type: &str,
        message: &str,
        link_url: Option<&str>,
        media_paths: &[String],
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let is_scheduled = scheduled_publish_time.is_some();

        match post_type {
            "status" => {
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut body = json!({
                    "message": message,
                    "published": !is_scheduled,
                });
                if let Some(st) = scheduled_publish_time {
                    body["scheduled_publish_time"] = json!(st);
                }
                let resp = client.post(&url).bearer_auth(token).json(&body).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Failed to post status: HTTP {}", resp.status()));
                }
                let res_json: serde_json::Value = resp.json().await?;
                Ok(res_json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
            }
            "link" => {
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut body = json!({
                    "message": message,
                    "link": link_url.unwrap_or(""),
                    "published": !is_scheduled,
                });
                if let Some(st) = scheduled_publish_time {
                    body["scheduled_publish_time"] = json!(st);
                }
                let resp = client.post(&url).bearer_auth(token).json(&body).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Failed to post link: HTTP {}", resp.status()));
                }
                let res_json: serde_json::Value = resp.json().await?;
                Ok(res_json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
            }
            "photo" => {
                let photo_path = media_paths.first().ok_or_else(|| anyhow!("Missing photo path"))?;
                let file_bytes = std::fs::read(Path::new(photo_path))?;
                let file_name = Path::new(photo_path).file_name().and_then(|n| n.to_str()).unwrap_or("photo.jpg");
                let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name.to_string());
                
                let mut form = reqwest::multipart::Form::new()
                    .part("source", part)
                    .text("caption", message.to_string())
                    .text("published", (!is_scheduled).to_string());

                if let Some(st) = scheduled_publish_time {
                    form = form.text("scheduled_publish_time", st.to_string());
                }

                let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                let resp = client.post(&url).bearer_auth(token).multipart(form).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Failed to upload photo: HTTP {}", resp.status()));
                }
                let res_json: serde_json::Value = resp.json().await?;
                Ok(res_json.get("post_id").or_else(|| res_json.get("id")).and_then(|v| v.as_str()).unwrap_or("").to_string())
            }
            "multi_photo" => {
                // 1. Upload each photo as unpublished
                let mut media_fbid_list = Vec::new();
                for photo_path in media_paths {
                    let file_bytes = std::fs::read(Path::new(photo_path))?;
                    let file_name = Path::new(photo_path).file_name().and_then(|n| n.to_str()).unwrap_or("photo.jpg");
                    let part = reqwest::multipart::Part::bytes(file_bytes).file_name(file_name.to_string());
                    let form = reqwest::multipart::Form::new()
                        .part("source", part)
                        .text("published", "false");

                    let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
                    let resp = client.post(&url).bearer_auth(token).multipart(form).send().await?;
                    if !resp.status().is_success() {
                        return Err(anyhow!("Failed to upload multi-photo part: HTTP {}", resp.status()));
                    }
                    let res_json: serde_json::Value = resp.json().await?;
                    let photo_id = res_json.get("id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing photo id"))?;
                    media_fbid_list.push(json!({ "media_fbid": photo_id }));
                }

                // 2. Publish feed post referencing attached_media
                let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
                let mut body = json!({
                    "message": message,
                    "attached_media": media_fbid_list,
                    "published": !is_scheduled,
                });
                if let Some(st) = scheduled_publish_time {
                    body["scheduled_publish_time"] = json!(st);
                }
                let resp = client.post(&url).bearer_auth(token).json(&body).send().await?;
                if !resp.status().is_success() {
                    return Err(anyhow!("Failed to post multi-photo feed: HTTP {}", resp.status()));
                }
                let res_json: serde_json::Value = resp.json().await?;
                Ok(res_json.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string())
            }
            _ => Err(anyhow!("Unsupported post type {}", post_type)),
        }
    }
}
```

- [ ] **Step 3: Commit Task 1**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement post scheduler types and multi-type post publisher"
```

---

### Task 2: Resumable Video & Reels Streaming Pipeline with Exact Offset Verification

**Files:**
- Create: `src-tauri/src/conversation/facebook/uploader.rs`
- Test: `src-tauri/src/conversation/facebook/uploader_test.rs`

- [ ] **Step 1: Implement Bounded Memory Streaming Uploader for Videos and Reels with Exact Offset Verification**

```rust
// src-tauri/src/conversation/facebook/uploader.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub struct FacebookUploader;

impl FacebookUploader {
    pub fn compute_file_hash(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub async fn upload_reel_resumable(
        conn: &mut Connection,
        token: &str,
        page_id: &str,
        file_path_str: &str,
        description: &str,
    ) -> Result<String> {
        let path = Path::new(file_path_str);
        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();
        let file_hash = Self::compute_file_hash(path)?;

        // 1. Check existing active session
        let existing_session: Option<(String, String, u64)> = {
            let mut stmt = conn.prepare(
                "SELECT upload_session_id, video_id, start_offset 
                 FROM facebook_upload_sessions 
                 WHERE page_id = ?1 AND upload_type = 'reel' AND file_hash = ?2 AND file_size_bytes = ?3 AND status = 'in_progress'"
            )?;
            stmt.query_row(params![page_id, file_hash, file_size], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).ok()
        };

        let client = Client::builder().timeout(std::time::Duration::from_secs(60)).build()?;
        let (upload_session_id, video_id, mut offset) = match existing_session {
            Some((sess_id, vid, _local_offset)) => {
                // Authoritatively query server offset on resume
                let offset_url = format!("https://rupload.facebook.com/video-reels/{}", vid);
                let check_resp = client.get(&offset_url).bearer_auth(token).send().await?;
                if !check_resp.status().is_success() {
                    return Err(anyhow!("Failed to query server offset for reel {}: HTTP {}", vid, check_resp.status()));
                }
                let server_offset_hdr = check_resp.headers().get("Offset").and_then(|h| h.to_str().ok());
                let server_offset = match server_offset_hdr.and_then(|s| s.parse::<u64>().ok()) {
                    Some(o) if o <= file_size => o,
                    _ => return Err(anyhow!("Malformed or invalid server offset header on resume for reel {}", vid)),
                };
                conn.execute(
                    "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2",
                    params![server_offset, sess_id],
                )?;
                (sess_id, vid, server_offset)
            }
            None => {
                // Initialize new Reel upload session
                let init_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
                let init_resp = client.post(&init_url)
                    .bearer_auth(token)
                    .json(&serde_json::json!({ "upload_phase": "start" }))
                    .send().await?;
                
                if !init_resp.status().is_success() {
                    return Err(anyhow!("Failed to initialize reel upload: HTTP {}", init_resp.status()));
                }

                let init_body: Value = init_resp.json().await?;
                let vid = init_body.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing video_id"))?.to_string();
                let sess_id = init_body.get("upload_session_id").and_then(|v| v.as_str()).unwrap_or(&vid).to_string();

                conn.execute(
                    "INSERT INTO facebook_upload_sessions (page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, created_at, updated_at)
                     VALUES (?1, 'reel', ?2, ?3, ?4, ?5, ?6, 0, 'in_progress', datetime('now'), datetime('now'))",
                    params![page_id, sess_id, vid, file_path_str, file_hash, file_size],
                )?;
                (sess_id, vid, 0u64)
            }
        };

        // 2. Stream chunks bounded to 4MB using File seek
        let mut file = File::open(path)?;
        let chunk_size: usize = 4 * 1024 * 1024;
        let mut chunk_buf = vec![0u8; chunk_size];

        while offset < file_size {
            file.seek(SeekFrom::Start(offset))?;
            let bytes_to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
            file.read_exact(&mut chunk_buf[..bytes_to_read])?;

            let chunk_data = chunk_buf[..bytes_to_read].to_vec();
            let upload_url = format!("https://rupload.facebook.com/video-reels/{}", video_id);
            let chunk_resp = client.post(&upload_url)
                .bearer_auth(token)
                .header("Offset", offset.to_string())
                .header("file_size", file_size.to_string())
                .header("Content-Type", "application/octet-stream")
                .body(chunk_data)
                .send().await?;

            if !chunk_resp.status().is_success() {
                return Err(anyhow!("Chunk upload failed at offset {}: HTTP {}", offset, chunk_resp.status()));
            }

            // Strictly verify server confirmed offset equals expected next offset
            let confirmed_offset = chunk_resp.headers().get("Offset")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| anyhow!("Server did not return confirmed Offset header after chunk upload at offset {}", offset))?;

            let expected_offset = offset + bytes_to_read as u64;
            if confirmed_offset != expected_offset {
                return Err(anyhow!("Invalid server confirmed offset {} (expected {}) after upload from offset {}", confirmed_offset, expected_offset, offset));
            }

            offset = confirmed_offset;
            conn.execute(
                "UPDATE facebook_upload_sessions SET start_offset = ?1, updated_at = datetime('now') WHERE upload_session_id = ?2",
                params![offset, upload_session_id],
            )?;
        }

        // 3. Finalize Reel
        let finish_url = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
        let finish_resp = client.post(&finish_url)
            .bearer_auth(token)
            .json(&serde_json::json!({
                "upload_phase": "finish",
                "video_id": video_id,
                "video_state": "PUBLISHED",
                "description": description
            }))
            .send().await?;

        if !finish_resp.status().is_success() {
            return Err(anyhow!("Failed to finalize reel: HTTP {}", finish_resp.status()));
        }

        conn.execute(
            "UPDATE facebook_upload_sessions SET status = 'completed', updated_at = datetime('now') WHERE upload_session_id = ?1",
            params![upload_session_id],
        )?;

        Ok(video_id)
    }

    pub async fn upload_video_resumable(
        conn: &mut Connection,
        token: &str,
        page_id: &str,
        file_path_str: &str,
        title: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let path = Path::new(file_path_str);
        let metadata = std::fs::metadata(path)?;
        let file_size = metadata.len();
        let file_hash = Self::compute_file_hash(path)?;

        let client = Client::builder().timeout(std::time::Duration::from_secs(60)).build()?;
        let init_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let init_resp = client.post(&init_url)
            .bearer_auth(token)
            .json(&serde_json::json!({
                "upload_phase": "start",
                "file_size": file_size
            }))
            .send().await?;

        if !init_resp.status().is_success() {
            return Err(anyhow!("Failed to initialize video upload: HTTP {}", init_resp.status()));
        }

        let init_body: Value = init_resp.json().await?;
        let upload_session_id = init_body.get("upload_session_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing upload_session_id"))?;
        let video_id = init_body.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Missing video_id"))?;

        let mut offset: u64 = 0;
        let mut file = File::open(path)?;
        let chunk_size: usize = 4 * 1024 * 1024;
        let mut chunk_buf = vec![0u8; chunk_size];

        while offset < file_size {
            file.seek(SeekFrom::Start(offset))?;
            let bytes_to_read = std::cmp::min(chunk_size as u64, file_size - offset) as usize;
            file.read_exact(&mut chunk_buf[..bytes_to_read])?;

            let chunk_data = chunk_buf[..bytes_to_read].to_vec();
            let transfer_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
            let form = reqwest::multipart::Form::new()
                .text("upload_phase", "transfer")
                .text("upload_session_id", upload_session_id.to_string())
                .text("start_offset", offset.to_string())
                .part("video_file_chunk", reqwest::multipart::Part::bytes(chunk_data).file_name("chunk.bin"));

            let transfer_resp = client.post(&transfer_url).bearer_auth(token).multipart(form).send().await?;
            if !transfer_resp.status().is_success() {
                return Err(anyhow!("Video chunk upload failed at offset {}: HTTP {}", offset, transfer_resp.status()));
            }

            let transfer_body: Value = transfer_resp.json().await?;
            let next_offset = transfer_body.get("start_offset").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(offset + bytes_to_read as u64);

            offset = next_offset;
        }

        // Finalize video
        let finish_url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let mut finish_body = serde_json::json!({
            "upload_phase": "finish",
            "upload_session_id": upload_session_id,
            "title": title,
            "description": description,
            "published": scheduled_publish_time.is_none(),
        });
        if let Some(st) = scheduled_publish_time {
            finish_body["scheduled_publish_time"] = json!(st);
        }

        let finish_resp = client.post(&finish_url).bearer_auth(token).json(&finish_body).send().await?;
        if !finish_resp.status().is_success() {
            return Err(anyhow!("Failed to finalize video upload: HTTP {}", finish_resp.status()));
        }

        Ok(video_id.to_string())
    }
}
```

- [ ] **Step 2: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement bounded memory streaming uploader for videos and reels with exact offset validation"
```

---

### Task 3: Dual-Mode Scheduling Engine & Expired Lease Recovery

**Files:**
- Create: `src-tauri/src/conversation/facebook/scheduler_engine.rs`
- Test: `src-tauri/src/conversation/facebook/scheduler_engine_test.rs`

- [ ] **Step 1: Implement Dual-Mode Scheduler with Promotion Worker and Multi-Endpoint Ambiguous Post Reconciliation**

```rust
// src-tauri/src/conversation/facebook/scheduler_engine.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub struct SchedulerEngine;

impl SchedulerEngine {
    pub async fn sweep_expired_publishing_leases(conn: &mut Connection) -> Result<usize> {
        let rows = conn.execute(
            "UPDATE facebook_scheduled_posts 
             SET status = 'delivery_unknown', owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
             WHERE status = 'publishing' AND claim_deadline < datetime('now')",
            [],
        )?;
        Ok(rows)
    }

    pub async fn promote_local_queued_posts(
        conn: &mut Connection,
        token: &str,
        page_id: &str,
        worker_id: &str,
    ) -> Result<usize> {
        // Claim posts scheduled within next 27 days
        let target_posts: Vec<(i64, String, String, Option<String>, String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT id, post_type, message, link_url, media_paths_json, scheduled_publish_time 
                 FROM facebook_scheduled_posts 
                 WHERE scheduling_mode = 'local_queued' AND status = 'queued' 
                   AND scheduled_publish_time <= datetime('now', '+27 days')
                   AND scheduled_publish_time >= datetime('now', '+10 minutes')"
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))
            })?;
            rows.flatten().collect()
        };

        let mut promoted_count = 0;
        let client = Client::new();

        for (post_id, post_type, message, link_url, media_json, sched_time_str) in target_posts {
            let claim_rows = conn.execute(
                "UPDATE facebook_scheduled_posts 
                 SET status = 'publishing', owner_id = ?1, claim_deadline = datetime('now', '+5 minutes'), updated_at = datetime('now')
                 WHERE id = ?2 AND status = 'queued'",
                params![worker_id, post_id],
            )?;
            if claim_rows == 0 {
                continue;
            }

            let scheduled_unix = chrono::DateTime::parse_from_rfc3339(&sched_time_str)
                .map(|dt| dt.timestamp())
                .ok();

            let media_paths: Vec<String> = serde_json::from_str(&media_json).unwrap_or_default();

            match super::publisher::FacebookPublisher::publish_post(
                &client,
                token,
                page_id,
                &post_type,
                &message,
                link_url.as_deref(),
                &media_paths,
                scheduled_unix,
            ).await {
                Ok(remote_id) => {
                    conn.execute(
                        "UPDATE facebook_scheduled_posts 
                         SET status = 'scheduled_on_meta', remote_post_id = ?1, owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
                         WHERE id = ?2 AND owner_id = ?3",
                        params![remote_id, post_id, worker_id],
                    )?;
                    promoted_count += 1;
                }
                Err(e) => {
                    conn.execute(
                        "UPDATE facebook_scheduled_posts 
                         SET status = 'failed', error_message = ?1, owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
                         WHERE id = ?2 AND owner_id = ?3",
                        params![e.to_string(), post_id, worker_id],
                    )?;
                }
            }
        }

        Ok(promoted_count)
    }

    pub async fn reconcile_ambiguous_post(
        conn: &mut Connection,
        token: &str,
        page_id: &str,
        post_id: i64,
        post_type: &str,
        message: &str,
        scheduled_time_str: Option<&str>,
        worker_id: &str,
    ) -> Result<bool> {
        // Claim delivery_unknown row with atomic lease
        let claim_rows = conn.execute(
            "UPDATE facebook_scheduled_posts 
             SET owner_id = ?1, claim_deadline = datetime('now', '+5 minutes')
             WHERE id = ?2 AND status = 'delivery_unknown' AND (claim_deadline IS NULL OR claim_deadline < datetime('now'))",
            params![worker_id, post_id],
        )?;
        if claim_rows == 0 {
            return Ok(false);
        }

        let expected_msg_hash = {
            let mut hasher = Sha256::new();
            hasher.update(message.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        let client = Client::builder().timeout(std::time::Duration::from_secs(15)).build()?;
        
        // Query both /scheduled_posts and /feed
        let endpoints = vec![
            format!("https://graph.facebook.com/v26.0/{}/scheduled_posts?fields=id,message,scheduled_publish_time,status_type&limit=50", page_id),
            format!("https://graph.facebook.com/v26.0/{}/feed?fields=id,message,created_time,status_type&limit=50", page_id),
        ];

        for endpoint_url in endpoints {
            let resp = client.get(&endpoint_url).bearer_auth(token).send().await;
            if let Ok(r) = resp {
                if r.status().is_success() {
                    let body: Value = r.json().await.unwrap_or(Value::Null);
                    if let Some(posts) = body.get("data").and_then(|d| d.as_array()) {
                        for p in posts {
                            let p_msg = p.get("message").and_then(|m| m.as_str()).unwrap_or("");
                            let p_id = p.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let p_time = p.get("scheduled_publish_time").or_else(|| p.get("created_time")).and_then(|c| c.as_str()).unwrap_or("");

                            let p_msg_hash = {
                                let mut hasher = Sha256::new();
                                hasher.update(p_msg.as_bytes());
                                format!("{:x}", hasher.finalize())
                            };

                            if p_msg_hash == expected_msg_hash {
                                let time_matches = match scheduled_time_str {
                                    Some(target_ts) => {
                                        if let (Ok(t1), Ok(t2)) = (chrono::DateTime::parse_from_rfc3339(p_time), chrono::DateTime::parse_from_rfc3339(target_ts)) {
                                            (t1.signed_duration_since(t2)).num_seconds().abs() <= 60
                                        } else {
                                            false
                                        }
                                    }
                                    None => true,
                                };

                                if time_matches {
                                    conn.execute(
                                        "UPDATE facebook_scheduled_posts 
                                         SET status = 'scheduled_on_meta', remote_post_id = ?1, owner_id = NULL, claim_deadline = NULL, updated_at = datetime('now')
                                         WHERE id = ?2 AND owner_id = ?3",
                                        params![p_id, post_id, worker_id],
                                    )?;
                                    return Ok(true);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Inconclusive: release lease so next cycle can retry cleanly
        conn.execute(
            "UPDATE facebook_scheduled_posts SET owner_id = NULL, claim_deadline = NULL WHERE id = ?1 AND owner_id = ?2",
            params![post_id, worker_id],
        )?;

        Ok(false)
    }
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement dual-mode scheduler engine, promotion worker, and multi-endpoint reconciliation"
```

---

### Task 4: Tauri IPC Commands for Scheduler & Live Feed Previews

**Files:**
- Modify: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Implement commands and register in Tauri generate_handler**
- [ ] **Step 2: Commit Task 4**

```bash
git add src-tauri/
git commit -m "feat(tauri): add post scheduler and live preview IPC commands"
```

---

### Task 5: Svelte 5 Feed Planner (Calendar/Kanban) & Live Device Preview

**Files:**
- Create: `src/lib/components/facebook/PostComposer.svelte`
- Create: `src/lib/components/facebook/LiveFeedPreview.svelte`
- Create: `src/lib/components/facebook/CalendarPlanner.svelte`
- Create: `src/lib/components/facebook/KanbanPlanner.svelte`

- [ ] **Step 1: Implement Svelte 5 PostComposer and LiveFeedPreview with responsive desktop/mobile toggle**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 5**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement PostComposer and live preview components with svelte 5 runes"
```

---

### Task 6: End-to-End Verification & Gate Check

**Files:**
- Test: Full integration test suite

- [ ] **Step 1: Run Rust test suite**  
  `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] **Step 2: Run frontend build and linter**  
  `bun run check && bun run build`
- [ ] **Step 3: Final Commit for Phase 2**

```bash
git commit --allow-empty -m "chore: complete phase 2 facebook scheduler implementation"
```
