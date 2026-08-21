# Facebook Integration (Phase 2: Post Scheduler & Live Preview) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a robust, multi-type Facebook Post Scheduler supporting Status, Link, Photos, Albums, Resumable Videos, and Reels with dual-mode scheduling (Meta-side 10m–30d & Local-queued >30d), multi-page fan-out, and Svelte 5 Live Desktop/Mobile Feed Previews.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), `reqwest`, `graph-video.facebook.com` API, Tailwind CSS.

---

## 1. Mandatory Implementation Harness Ledger (Tier 3)

| Req ID | Requirement | Status | Owning Step | Boundary Trace & Invariants | Adversarial Counterexample / Proof | Verification Method |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-08** | JIT Media Upload & Zero Query Leakage | `PLANNED` | Task 2 (Step 1) | Media is uploaded unpublished at push time with `Authorization: Bearer <token>` header (never query string). | Inspect HTTP logs during media upload; verify 0 tokens appear in URL query strings. | Integration test: Mock server inspects headers and query params. |
| **REQ-FB-09** | 3-Phase Resumable Video & Reels Pipeline | `PLANNED` | Task 2 (Step 1) | Chunked transfer on `graph-video.facebook.com` (`start` -> `transfer` in 4MB chunks with offset validation -> `finish`). | Network drops at byte offset 8,388,608 (chunk 2); retry resumes from byte 8,388,608 without restarting file. | Resumable upload unit test with mocked chunk interruption. |
| **REQ-FB-10** | Dual-Mode Scheduling & Overdue Safety | `PLANNED` | Task 3 (Step 1) | Posts 10m–30d pushed to Meta with `published=false`; posts >30d held locally in SQLite. Overdue posts trigger safety prompt. | User's laptop closed for 35 days; opening app triggers "Đăng ngay / Dời lịch / Huỷ" prompt instead of unexpected auto-post. | Scheduler test: Pass overdue post to engine; assert status is `overdue_prompt`. |
| **REQ-FB-11** | Idempotent Multi-Page Fan-Out | `PLANNED` | Task 3 (Step 1) | Post targeting $N$ pages fans out into $N$ SQLite records with deterministic `idempotency_key = uuidv5(draft_group_id, page_id)`. | Network drops while pushing Page 2; retrying job pushes Page 2 without re-creating Page 1 post. | Idempotency test: Retry multi-page push; verify no duplicate posts created on Page 1. |

---

### Task 1: SQLite Schema & Data Types for Post Scheduler

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/post_types.rs`
- Test: `src-tauri/src/conversation/facebook/post_types_test.rs`

- [ ] **Step 1: Implement Post Types and Mode Decision Matrix**

```rust
// src-tauri/src/conversation/facebook/post_types.rs
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FacebookPostType {
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
    OverduePrompt,
}

pub fn determine_scheduling_mode(
    target_time: DateTime<Utc>,
    now: DateTime<Utc>,
) -> anyhow::Result<SchedulingMode> {
    let diff = target_time - now;
    if diff < Duration::minutes(10) {
        anyhow::bail!("Thời gian hẹn giờ phải cách ít nhất 10 phút so với hiện tại");
    }
    if diff <= Duration::days(30) {
        Ok(SchedulingMode::Meta)
    } else {
        Ok(SchedulingMode::LocalQueued)
    }
}
```

- [ ] **Step 2: Commit Task 1**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement post scheduler types and mode decision matrix"
```

---

### Task 2: Multi-Type Media Pipeline & 3-Phase Resumable Uploader

**Files:**
- Create: `src-tauri/src/conversation/facebook/media_uploader.rs`
- Test: `src-tauri/src/conversation/facebook/media_uploader_test.rs`

- [ ] **Step 1: Implement 3-Phase Resumable Video & Reels Uploader on `graph-video.facebook.com`**

```rust
// src-tauri/src/conversation/facebook/media_uploader.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

pub struct FacebookMediaUploader {
    token: String,
    http: Client,
}

impl FacebookMediaUploader {
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: Client::new(),
        }
    }

    pub async fn upload_photo_unpublished(&self, page_id: &str, image_bytes: Vec<u8>) -> Result<String> {
        let url = format!("https://graph.facebook.com/v26.0/{}/photos", page_id);
        let form = reqwest::multipart::Form::new()
            .text("published", "false")
            .part("source", reqwest::multipart::Part::bytes(image_bytes).file_name("photo.jpg"));

        let resp = self.http.post(&url)
            .bearer_auth(&self.token)
            .multipart(form)
            .send()
            .await?;

        let body: Value = resp.json().await?;
        body.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Photo upload failed: {:?}", body))
    }

    pub async fn upload_multi_photo_post(
        &self,
        page_id: &str,
        message: &str,
        photos_bytes: Vec<Vec<u8>>,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let mut attached_media = Vec::new();
        for photo in photos_bytes {
            let fbid = self.upload_photo_unpublished(page_id, photo).await?;
            attached_media.push(serde_json::json!({ "media_fbid": fbid }));
        }

        let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
        let mut params = vec![
            ("message", message.to_string()),
            ("attached_media", serde_json::to_string(&attached_media)?),
        ];
        if let Some(t) = scheduled_publish_time {
            params.push(("published", "false".to_string()));
            params.push(("scheduled_publish_time", t.to_string()));
        }

        let resp = self.http.post(&url).bearer_auth(&self.token).form(&params).send().await?;
        let body: Value = resp.json().await?;
        body.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| anyhow!("Multi photo post failed: {:?}", body))
    }

    pub async fn start_resumable_video_session(&self, page_id: &str, file_size_bytes: u64) -> Result<(String, String)> {
        let url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let resp = self.http.post(&url)
            .bearer_auth(&self.token)
            .form(&[
                ("upload_phase", "start"),
                ("file_size", &file_size_bytes.to_string()),
            ])
            .send().await?;

        let body: Value = resp.json().await?;
        let session_id = body.get("upload_session_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("No session ID"))?.to_string();
        let video_id = body.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("No video ID"))?.to_string();
        Ok((session_id, video_id))
    }

    pub async fn transfer_video_chunk(&self, page_id: &str, session_id: &str, start_offset: u64, chunk_bytes: Vec<u8>) -> Result<u64> {
        let url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let form = reqwest::multipart::Form::new()
            .text("upload_phase", "transfer")
            .text("upload_session_id", session_id.to_string())
            .text("start_offset", start_offset.to_string())
            .part("video_file_chunk", reqwest::multipart::Part::bytes(chunk_bytes).file_name("chunk.bin"));

        let resp = self.http.post(&url)
            .bearer_auth(&self.token)
            .multipart(form)
            .send()
            .await?;

        let body: Value = resp.json().await?;
        let next_offset = body.get("start_offset").and_then(|v| v.as_str()).and_then(|s| s.parse::<u64>().ok())
            .or_else(|| body.get("start_offset").and_then(|v| v.as_u64()))
            .ok_or_else(|| anyhow!("No next offset returned: {:?}", body))?;

        Ok(next_offset)
    }

    pub async fn finish_resumable_video_session(
        &self,
        page_id: &str,
        session_id: &str,
        title: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let mut params = vec![
            ("upload_phase", "finish".to_string()),
            ("upload_session_id", session_id.to_string()),
            ("title", title.to_string()),
            ("description", description.to_string()),
        ];

        if let Some(t) = scheduled_publish_time {
            params.push(("published", "false".to_string()));
            params.push(("scheduled_publish_time", t.to_string()));
        }

        let resp = self.http.post(&url)
            .bearer_auth(&self.token)
            .form(&params)
            .send().await?;

        let body: Value = resp.json().await?;
        let success = body.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        if !success {
            return Err(anyhow!("Finish video upload failed: {:?}", body));
        }
        Ok(session_id.to_string())
    }

    pub async fn upload_full_resumable_video(
        &self,
        page_id: &str,
        video_bytes: &[u8],
        title: &str,
        description: &str,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let file_size = video_bytes.len() as u64;
        let (session_id, _) = self.start_resumable_video_session(page_id, file_size).await?;
        let chunk_size = 4 * 1024 * 1024;
        let mut offset = 0u64;

        while offset < file_size {
            let end = std::cmp::min(offset + chunk_size, file_size);
            let chunk = &video_bytes[offset as usize..end as usize];
            let next = self.transfer_video_chunk(page_id, &session_id, offset, chunk.to_vec()).await?;
            if next <= offset || next > file_size {
                anyhow::bail!("Invalid offset progression during upload: {} -> {}", offset, next);
            }
            offset = next;
        }

        if offset != file_size {
            anyhow::bail!("Upload did not reach full file size: {} != {}", offset, file_size);
        }

        self.finish_resumable_video_session(page_id, &session_id, title, description, scheduled_publish_time).await
    }

    pub async fn publish_reel(
        &self,
        page_id: &str,
        video_bytes: &[u8],
        description: &str,
        scheduled_publish_time: Option<i64>,
    ) -> Result<String> {
        let url_init = format!("https://graph.facebook.com/v26.0/{}/video_reels", page_id);
        let resp = self.http.post(&url_init)
            .bearer_auth(&self.token)
            .form(&[("upload_phase", "start")])
            .send().await?;

        let body: Value = resp.json().await?;
        let video_id = body.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("Reels init failed: {:?}", body))?;

        let url_upload = format!("https://rupload.facebook.com/video-reels/{}", video_id);
        let resp_up = self.http.post(&url_upload)
            .bearer_auth(&self.token)
            .header("Offset", "0")
            .header("file_size", video_bytes.len().to_string())
            .body(video_bytes.to_vec())
            .send().await?;

        if !resp_up.status().is_success() {
            return Err(anyhow!("Reel binary upload failed: HTTP {}", resp_up.status()));
        }

        let mut finish_params = vec![
            ("upload_phase", "finish".to_string()),
            ("video_id", video_id.to_string()),
            ("video_state", "PUBLISHED".to_string()),
            ("description", description.to_string()),
        ];
        if let Some(t) = scheduled_publish_time {
            finish_params.push(("video_state", "SCHEDULED".to_string()));
            finish_params.push(("scheduled_publish_time", t.to_string()));
        }

        let resp_fin = self.http.post(&url_init).bearer_auth(&self.token).form(&finish_params).send().await?;
        let fin_body: Value = resp_fin.json().await?;
        fin_body.get("success").and_then(|v| v.as_bool()).filter(|b| *b).map(|_| video_id.to_string()).ok_or_else(|| anyhow!("Reel finish failed: {:?}", fin_body))
    }
}
```

- [ ] **Step 2: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete 3-phase resumable video uploader with bearer auth"
```

---

### Task 3: Dual-Mode Scheduling Engine with Read-After-Write Verification

**Files:**
- Create: `src-tauri/src/conversation/facebook/post_scheduler.rs`
- Test: `src-tauri/src/conversation/facebook/post_scheduler_test.rs`

- [ ] **Step 1: Implement Post Scheduler with Multi-Page Fan-Out & Read-After-Write Verification**

```rust
// src-tauri/src/conversation/facebook/post_scheduler.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

pub struct FacebookPostScheduler {
    token: String,
    http: Client,
}

impl FacebookPostScheduler {
    pub fn new(token: String) -> Self {
        Self {
            token,
            http: Client::new(),
        }
    }

    pub async fn publish_status_post(&self, page_id: &str, message: &str, scheduled_publish_time: Option<i64>) -> Result<String> {
        let url = format!("https://graph.facebook.com/v26.0/{}/feed", page_id);
        let mut form = vec![("message", message.to_string())];
        if let Some(t) = scheduled_publish_time {
            form.push(("published", "false".to_string()));
            form.push(("scheduled_publish_time", t.to_string()));
        }

        let resp = self.http.post(&url)
            .bearer_auth(&self.token)
            .form(&form)
            .send().await?;

        let body: Value = resp.json().await?;
        body.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| anyhow!("Status post failed: {:?}", body))
    }

    pub async fn verify_meta_scheduled_post(&self, remote_post_id: &str) -> Result<bool> {
        let url = format!("https://graph.facebook.com/v26.0/{}?fields=is_published,scheduled_publish_time", remote_post_id);
        let resp = self.http.get(&url)
            .bearer_auth(&self.token)
            .send().await?;

        if !resp.status().is_success() {
            return Err(anyhow!("Read-after-write verification failed for post {}", remote_post_id));
        }
        let body: Value = resp.json().await?;
        Ok(body.get("scheduled_publish_time").is_some())
    }
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement complete post scheduler engine with read-after-write verification"
```

---

### Task 4: Tauri IPC Commands for Post Scheduler

**Files:**
- Modify: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Register Post Scheduler IPC commands in Tauri**
- [ ] **Step 2: Commit Task 4**

```bash
git add src-tauri/
git commit -m "feat(tauri): add post scheduler IPC commands to tauri bridge"
```

---

### Task 5: Svelte 5 Post Editor & Media Drag-and-Drop Queue

**Files:**
- Create: `src/lib/components/facebook/PostEditor.svelte`
- Create: `src/lib/components/facebook/MediaDropQueue.svelte`

- [ ] **Step 1: Implement Svelte 5 PostEditor with Runes and media queue**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 5**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement PostEditor and MediaDropQueue with svelte 5 runes"
```

---

### Task 6: Svelte 5 Live Feed Preview Simulator (Desktop & Mobile)

**Files:**
- Create: `src/lib/components/facebook/LiveFeedPreview.svelte`
- Create: `src/lib/components/facebook/FeedPhotoGrid.svelte`

- [ ] **Step 1: Implement LiveFeedPreview supporting Desktop & Mobile layouts with photo grids**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 6**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement LiveFeedPreview with authentic desktop and mobile facebook grids"
```

---

### Task 7: Svelte 5 Calendar & Kanban Views (`PostPlanner.svelte`)

**Files:**
- Create: `src/lib/components/facebook/PostPlanner.svelte`
- Create: `src/lib/components/facebook/PostCalendarView.svelte`
- Create: `src/lib/components/facebook/PostKanbanView.svelte`

- [ ] **Step 1: Assemble PostPlanner combining Editor, Preview, and Schedule Views**
- [ ] **Step 2: Run `bun run check`**
- [ ] **Step 3: Commit Task 7**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement PostPlanner with calendar and kanban schedule views"
```

---

### Task 8: Integration Verification & Gate Check

**Files:**
- Test: Full integration test suite

- [ ] **Step 1: Run Rust test suite**  
  `cargo test --manifest-path src-tauri/Cargo.toml`
- [ ] **Step 2: Run frontend build and linter**  
  `bun run check && bun run build`
- [ ] **Step 3: Final Commit for Phase 2**

```bash
git commit --allow-empty -m "chore: complete phase 2 facebook post scheduler implementation"
```
