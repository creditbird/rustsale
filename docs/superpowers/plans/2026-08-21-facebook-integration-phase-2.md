# Facebook Integration (Phase 2: Post Scheduler & Live Preview) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a robust, multi-type Facebook Post Scheduler supporting Status, Link, Photos, Albums, Resumable Videos, and Reels with dual-mode scheduling (Meta-side 10m–30d & Local-queued >30d), multi-page fan-out, and Svelte 5 Live Desktop/Mobile Feed Previews.

**Architecture:** Dual-mode scheduling engine in Rust where posts scheduled for 10 minutes to 30 days are pushed directly to Meta API with `published=false` and `scheduled_publish_time` for guaranteed unattended publishing. Posts >30 days stay in local SQLite queue and get pushed when within 29 days. Unpublished media is uploaded just-in-time during the push transaction. Includes a Svelte 5 Post Planner with Calendar/Kanban views and an authentic Facebook Feed Live Preview simulator.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), `reqwest`, `graph-video.facebook.com` API, Tailwind CSS.

---

## Mandatory Implementation Harness & Requirement Ledger (Tier 3)

| Req ID | Requirement | Success Criteria | Adversarial Counterexample / Failure Mode | Verification Method |
| :--- | :--- | :--- | :--- | :--- |
| **REQ-FB-08** | Multi-Type JIT Media Upload | Photos for albums uploaded unpublished just-in-time when pushing to Meta. | Uploading photos at draft creation causes Meta CDN cleanup before scheduled time. | Test JIT pipeline: Verify `media_paths_json` kept local until push transaction executes. |
| **REQ-FB-09** | Resumable Video & Reels Pipeline | Videos >= 50MB use chunked protocol on `graph-video.facebook.com`; Reels validated client-side. | Network drops during 200MB video upload; retry resumes from chunk offset without restarting. | Integration test: Simulate network interruption at chunk 2; verify resumption from `upload_session_id`. |
| **REQ-FB-10** | Dual-Mode Scheduling Verification | Meta-side posts verified via read-after-write; overdue queued posts prompt user safely. | PC offline across overdue queued post window; app auto-posts stale content without user consent. | Test overdue handler: Simulate expired queued post; verify prompt returns "Đăng ngay / Dời lịch / Huỷ". |
| **REQ-FB-11** | Idempotent Post Creation & Fan-Out | Posts to $N$ pages fan out into independent records with matching `(timestamp_sec, hash(message))`. | Retrying interrupted multi-page post creates duplicate post on Page 1 while retrying Page 2. | Idempotency test: Run duplicate push simulation; verify `remote_post_id` populated without re-creating post. |

---

### Task 1: SQLite Schema & Data Types for Post Scheduler

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/post_types.rs`
- Test: `src-tauri/src/conversation/facebook/post_types_test.rs`

- [ ] **Step 1: Implement Post Types with validation**

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
git commit -m "feat(facebook): implement post scheduler types and mode selector"
```

---

### Task 2: Multi-Type Media Pipeline & Resumable Uploader

**Files:**
- Create: `src-tauri/src/conversation/facebook/media_uploader.rs`
- Test: `src-tauri/src/conversation/facebook/media_uploader_test.rs`

- [ ] **Step 1: Implement complete Resumable Video & JIT Photo Uploader with correct endpoints**

```rust
// src-tauri/src/conversation/facebook/media_uploader.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

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
            .text("access_token", self.token.clone())
            .part("source", reqwest::multipart::Part::bytes(image_bytes).file_name("photo.jpg"));

        let resp = self.http.post(&url).multipart(form).send().await?;
        let body: Value = resp.json().await?;
        body.get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("Photo upload failed: {:?}", body))
    }

    pub async fn start_video_resumable_session(&self, page_id: &str, file_size_bytes: u64) -> Result<(String, String)> {
        let url = format!("https://graph-video.facebook.com/v26.0/{}/videos", page_id);
        let resp = self.http.post(&url)
            .form(&[
                ("upload_phase", "start"),
                ("access_token", &self.token),
                ("file_size", &file_size_bytes.to_string()),
            ])
            .send().await?;
        let body: Value = resp.json().await?;
        let session_id = body.get("upload_session_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("No session ID"))?.to_string();
        let video_id = body.get("video_id").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("No video ID"))?.to_string();
        Ok((session_id, video_id))
    }
}
```

- [ ] **Step 2: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement multi-type media uploader and resumable video protocol"
```

---

### Task 3: Dual-Mode Scheduling Engine with Read-After-Write Verification

**Files:**
- Create: `src-tauri/src/conversation/facebook/post_scheduler.rs`
- Test: `src-tauri/src/conversation/facebook/post_scheduler_test.rs`

- [ ] **Step 1: Implement Post Scheduler with Read-After-Write Verification**

```rust
// src-tauri/src/conversation/facebook/post_scheduler.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;

pub async fn verify_meta_scheduled_post(token: &str, remote_post_id: &str) -> Result<bool> {
    let client = Client::new();
    let url = format!("https://graph.facebook.com/v26.0/{}?fields=is_published,scheduled_publish_time&access_token={}", remote_post_id, token);
    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(anyhow!("Read-after-write verification failed for post {}", remote_post_id));
    }
    let body: Value = resp.json().await?;
    Ok(body.get("scheduled_publish_time").is_some())
}
```

- [ ] **Step 2: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement post scheduler dual-mode engine with read-after-write verification"
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
