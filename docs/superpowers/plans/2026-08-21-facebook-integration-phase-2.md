# Facebook Integration (Phase 2: Post Scheduler & Live Preview) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a robust, multi-type Facebook Post Scheduler supporting Status, Link, Photos, Albums, Resumable Videos, and Reels with dual-mode scheduling (Meta-side 10m–30d & Local-queued >30d), multi-page fan-out, and Svelte 5 Live Desktop/Mobile Feed Previews.

**Architecture:** Dual-mode scheduling engine in Rust where posts scheduled for 10 minutes to 30 days are pushed directly to Meta API with `published=false` and `scheduled_publish_time` for guaranteed unattended publishing. Posts >30 days stay in local SQLite queue and get pushed when within 29 days. Unpublished media is uploaded just-in-time during the push transaction. Includes a Svelte 5 Post Planner with Calendar/Kanban views and an authentic Facebook Feed Live Preview simulator.

**Tech Stack:** Tauri 2.0 (Rust), Svelte 5 (Runes), SQLite Local (`rusqlite`), `reqwest`, `graph-video.facebook.com` API, Tailwind CSS.

## Global Constraints

- Always use `bun` and `bunx` instead of `node`, `npm`, `npx`.
- Video uploads >= 50MB must use chunked resumable upload protocol via `graph-video.facebook.com`.
- Reels must be validated client-side: 9:16 aspect ratio, 540p+ resolution, 3–90s duration, and `description` field.
- Unpublished photos must only be uploaded at push time (not at draft creation) to prevent Meta CDN garbage collection.
- Multi-page posts must fan out into independent `facebook_scheduled_posts` records grouped by `draft_group_id`.
- TDD required: Write failing tests before implementation.

---

### Task 1: SQLite Schema & Data Types for Post Scheduler

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/facebook/post_types.rs`
- Modify: `src-tauri/src/conversation/facebook/mod.rs`
- Test: `src-tauri/src/conversation/facebook/post_types_test.rs`

**Interfaces:**
- Produces:
  - Table `facebook_scheduled_posts`
  - Struct `FacebookScheduledPostRecord`
  - Enum `FacebookPostType` (`Status`, `Link`, `Photo`, `MultiPhoto`, `Video`, `Reel`)
  - Enum `SchedulingMode` (`Meta`, `LocalQueued`)
  - Enum `PostStatus` (`Draft`, `Queued`, `Publishing`, `ScheduledOnMeta`, `Published`, `Failed`)

- [ ] **Step 1: Write test for Post Type serialization & validation**

```rust
// src-tauri/src/conversation/facebook/post_types_test.rs
#[cfg(test)]
mod tests {
    use super::super::post_types::*;

    #[test]
    fn test_scheduling_mode_selection() {
        let now = chrono::Utc::now();
        let target_in_5_days = now + chrono::Duration::days(5);
        let target_in_40_days = now + chrono::Duration::days(40);

        assert_eq!(determine_scheduling_mode(target_in_5_days, now).unwrap(), SchedulingMode::Meta);
        assert_eq!(determine_scheduling_mode(target_in_40_days, now).unwrap(), SchedulingMode::LocalQueued);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_scheduling_mode_selection`  
Expected: FAIL.

- [ ] **Step 3: Implement Post Types & Schema**

Update `src-tauri/src/conversation/schema.sql`:
```sql
CREATE TABLE IF NOT EXISTS facebook_scheduled_posts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    draft_group_id TEXT NOT NULL,
    channel_account_id INTEGER NOT NULL,
    page_id TEXT NOT NULL,
    post_type TEXT NOT NULL CHECK (post_type IN ('status', 'link', 'photo', 'multi_photo', 'video', 'reel')),
    message TEXT NOT NULL,
    link_url TEXT,
    media_paths_json TEXT NOT NULL DEFAULT '[]',
    scheduling_mode TEXT NOT NULL DEFAULT 'meta' CHECK (scheduling_mode IN ('meta', 'local_queued')),
    scheduled_publish_time TEXT,
    remote_post_id TEXT,
    upload_session_id TEXT,
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'queued', 'publishing', 'scheduled_on_meta', 'published', 'failed')),
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
```

Implement `src-tauri/src/conversation/facebook/post_types.rs`:
```rust
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

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_scheduling_mode_selection`  
Expected: PASS.

- [ ] **Step 5: Commit Task 1**

```bash
git add src-tauri/src/conversation/
git commit -m "feat(facebook): add post scheduler types, scheduling mode, and schema"
```

---

### Task 2: Multi-Type Media Pipeline & Resumable Video/Reel Uploader

**Files:**
- Create: `src-tauri/src/conversation/facebook/media_uploader.rs`
- Test: `src-tauri/src/conversation/facebook/media_uploader_test.rs`

**Interfaces:**
- Produces:
  - `upload_single_photo(client: &FacebookClient, page_id: &str, file_path: &str, caption: &str, schedule_ts: Option<i64>) -> Result<String>`
  - `upload_unpublished_photos(client: &FacebookClient, page_id: &str, file_paths: &[String]) -> Result<Vec<String>>`
  - `publish_multi_photo_post(client: &FacebookClient, page_id: &str, message: &str, photo_ids: &[String], schedule_ts: Option<i64>) -> Result<String>`
  - `upload_resumable_video(client: &FacebookClient, page_id: &str, file_path: &str, title: &str, description: &str, schedule_ts: Option<i64>) -> Result<String>`
  - `publish_reel(client: &FacebookClient, page_id: &str, file_path: &str, description: &str, schedule_ts: Option<i64>) -> Result<String>`

- [ ] **Step 1: Write test for Album Payload Builder**

```rust
// src-tauri/src/conversation/facebook/media_uploader_test.rs
#[cfg(test)]
mod tests {
    use super::super::media_uploader::*;
    use serde_json::json;

    #[test]
    fn test_build_album_attached_media_payload() {
        let photo_ids = vec!["111".to_string(), "222".to_string()];
        let payload = build_attached_media_json(&photo_ids);
        assert_eq!(payload, json!([{"media_fbid": "111"}, {"media_fbid": "222"}]));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_build_album_attached_media_payload`  
Expected: FAIL.

- [ ] **Step 3: Implement Media Uploader & Resumable Streamer**

```rust
// src-tauri/src/conversation/facebook/media_uploader.rs
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::{json, Value};

pub fn build_attached_media_json(photo_ids: &[String]) -> Value {
    let list: Vec<Value> = photo_ids
        .iter()
        .map(|id| json!({ "media_fbid": id }))
        .collect();
    json!(list)
}

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
            .ok_or_else(|| anyhow!("Không tìm thấy photo ID trong phản hồi: {:?}", body))
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_build_album_attached_media_payload`  
Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement multi-type media uploader and album payload builder"
```

---

### Task 3: Dual-Mode Scheduling Engine & Worker

**Files:**
- Create: `src-tauri/src/conversation/facebook/post_scheduler.rs`
- Test: `src-tauri/src/conversation/facebook/post_scheduler_test.rs`

**Interfaces:**
- Produces:
  - `PostSchedulerService::schedule_post(...) -> Result<Vec<FacebookScheduledPostRecord>>` (Fan-out)
  - `PostSchedulerService::push_to_meta(...) -> Result<String>` (With read-after-write verification)
  - `PostSchedulerWorker::scan_and_promote_queued_posts(...) -> Result<usize>`
  - `PostSchedulerService::handle_overdue_queued_post(...) -> Result<()>`

- [ ] **Step 1: Write test for Idempotency Hash Matcher**

```rust
// src-tauri/src/conversation/facebook/post_scheduler_test.rs
#[cfg(test)]
mod tests {
    use super::super::post_scheduler::*;

    #[test]
    fn test_generate_post_hash() {
        let msg = "Hello 2026";
        let hash1 = compute_post_content_hash(msg, "photo");
        let hash2 = compute_post_content_hash(msg, "photo");
        let hash3 = compute_post_content_hash("Different", "photo");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_generate_post_hash`  
Expected: FAIL.

- [ ] **Step 3: Implement Post Scheduler Service & Queue Worker**

```rust
// src-tauri/src/conversation/facebook/post_scheduler.rs
use sha2::{Digest, Sha256};

pub fn compute_post_content_hash(message: &str, post_type: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(message.as_bytes());
    hasher.update(post_type.as_bytes());
    format!("{:x}", hasher.finalize())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_generate_post_hash`  
Expected: PASS.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/src/conversation/facebook/
git commit -m "feat(facebook): implement post scheduler dual-mode engine and idempotency hash"
```

---

### Task 4: Tauri IPC Commands for Post Scheduler

**Files:**
- Modify: `src-tauri/src/conversation/facebook/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces Tauri IPC commands:
  - `create_facebook_post(draft: CreatePostDto) -> Result<Vec<PostRecordDto>>`
  - `list_scheduled_facebook_posts(page_id: Option<String>) -> Result<Vec<PostRecordDto>>`
  - `cancel_scheduled_facebook_post(post_id: i64) -> Result<()>`
  - `publish_facebook_post_now(post_id: i64) -> Result<()>`

- [ ] **Step 1: Implement commands and register in Tauri**
- [ ] **Step 2: Run cargo test to verify backend builds clean**
- [ ] **Step 3: Commit Task 4**

```bash
git add src-tauri/
git commit -m "feat(tauri): add post scheduler IPC commands to tauri bridge"
```

---

### Task 5: Svelte 5 Post Editor & Media Drag-and-Drop Queue

**Files:**
- Create: `src/lib/components/facebook/PostEditor.svelte`
- Create: `src/lib/components/facebook/MediaDropQueue.svelte`
- Test: `src/lib/components/facebook/__tests__/PostEditor.test.ts`

**Interfaces:**
- Produces:
  - Multi-page selection pills.
  - Character counter (with 63,206 threshold).
  - Media drag-and-drop queue (reordering, remove, client-side 9:16 validation for reels).
  - Date/time picker with 10m–30d validation and >30d local queue warning tooltip.

- [ ] **Step 1: Write test for Post Editor validation logic**
- [ ] **Step 2: Implement `PostEditor.svelte` and `MediaDropQueue.svelte`**
- [ ] **Step 3: Run `bun run check`**
- [ ] **Step 4: Commit Task 5**

```bash
git add src/lib/components/facebook/
git commit -m "feat(ui): implement PostEditor and MediaDropQueue components with Svelte 5 runes"
```

---

### Task 6: Svelte 5 Live Feed Preview Simulator (Desktop & Mobile)

**Files:**
- Create: `src/lib/components/facebook/LiveFeedPreview.svelte`
- Create: `src/lib/components/facebook/FeedPhotoGrid.svelte`

**Interfaces:**
- Produces:
  - Toggle between **Facebook Desktop Feed** (card width ~590px) and **Facebook Mobile Feed** (screen width ~375px).
  - Authentic Facebook typography, verified badge, simulated timestamp ("Vừa xong").
  - `FeedPhotoGrid`: Dynamic photo grid layouts (1 full photo, 2 horizontal/vertical splits, 3-photo grid, 4+ grid with `+N` count overlay).

- [ ] **Step 1: Implement `FeedPhotoGrid.svelte` and `LiveFeedPreview.svelte`**
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

**Interfaces:**
- Produces:
  - Calendar View (Month & Week grids showing scheduled post thumbnails and publish times).
  - Kanban / List View grouped by status (`Draft`, `Scheduled on Meta`, `Queued`, `Published`, `Failed`).
  - Action buttons: Edit, Publish Now, Cancel, View on Facebook.

- [ ] **Step 1: Implement Calendar and Kanban views**
- [ ] **Step 2: Assemble `PostPlanner.svelte` combining Editor, Preview, and Planner Views**
- [ ] **Step 3: Run `bun run check`**
- [ ] **Step 4: Commit Task 7**

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
- [ ] **Step 2: Run frontend build**  
  `bun run build`
- [ ] **Step 3: Final Commit for Phase 2**

```bash
git commit --allow-empty -m "chore: complete phase 2 facebook post scheduler implementation"
```
