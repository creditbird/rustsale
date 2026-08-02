# Zalo Rich Chat Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver production-safe Zalo rich chat in the existing Conversation workspace: durable attachment staging, batch/part outbox delivery, emoji and live stickers, quote reply, rich rendering, provider recall, and read-only reactions.

**Architecture:** One composer action creates one `message_send_batches` row and one independently retryable `messages`/outbox part for text and each attachment. Rust owns a durable application-data spool, the existing Zalo session worker sends parts sequentially per account, exact provider message/client IDs reconcile echoes, and Svelte renders capability-driven rich content without adding provider checks to presentation components.

**Tech Stack:** Rust 2021, Tauri 2, SQLite/rusqlite, vendored `zca-rs`, Tokio, Svelte 5 runes, TypeScript, shadcn-svelte, Bun test runner, Testing Library, Tailwind CSS 4.

---

## Source of Truth

- Approved design: `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-1-design.md`
- Existing Conversation design: `docs/superpowers/specs/2026-07-31-unified-conversation-design.md`
- Existing Conversation master plan: `docs/superpowers/plans/2026-07-31-unified-conversation-master-plan.md`

This plan extends the current Conversation module. It must not replace canonical provider conversations, unified lead timeline queries, Tauri invalidation events, or the existing Zalo session supervisor.

## Execution Rules

- Use `bun` and `bunx` only. Never use `npm` or `npx`.
- Read Agent Memory before implementation starts.
- Use `superpowers:using-git-worktrees` if execution is isolated in a worktree.
- Run GitNexus impact before editing every existing function, class, method, trait, interface, or shared file listed by a task.
- Stop and warn the user before editing any symbol whose current impact is HIGH or CRITICAL.
- Run `mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})` at each backend/frontend checkpoint and before completion.
- Preserve unrelated working-tree changes.
- Do not commit unless the user explicitly authorizes commits. The usual Superpowers commit steps are replaced with review checkpoints.
- Use `apply_patch` for all manual file edits.

## Baseline GitNexus Risk Snapshot

The planning-time HEAD index reports:

| Symbol | Risk | Direct dependents | Notes |
|---|---:|---:|---|
| `migrate` | LOW | 0 | Re-run before editing migration flow |
| `apply_normalized_message` | LOW | 0 | Source inspection remains authoritative because sync behavior is critical |
| `process_next_outbox_job` | LOW | 0 | Existing tests call it directly even when graph edges are absent |
| `ZaloClient` | LOW | 1 | Direct implementer: `RealZaloClient`; test fakes also require updates |
| `ConversationMessage` | LOW | 1 | Direct indexed dependent: `timelineState.ts`; Svelte components also consume it |
| `run` | LOW | 0 | Tauri setup and command registration still require startup verification |

Re-run impact at execution time; do not rely only on this snapshot.

## Required File Map

### Backend files to create

```text
src-tauri/src/conversation/error.rs
src-tauri/src/conversation/staging.rs
src-tauri/src/conversation/staging_test.rs
src-tauri/src/conversation/retry.rs
src-tauri/src/conversation/retry_test.rs
src-tauri/src/conversation/projection.rs
src-tauri/src/conversation/zalo/sticker_cache.rs
src-tauri/src/conversation/zalo/sticker_cache_test.rs
```

### Backend files to modify

```text
src-tauri/Cargo.toml
src-tauri/capabilities/default.json
src-tauri/tauri.conf.json
src-tauri/src/lib.rs
src-tauri/src/conversation/mod.rs
src-tauri/src/conversation/schema.sql
src-tauri/src/conversation/migration.rs
src-tauri/src/conversation/migration_test.rs
src-tauri/src/conversation/types.rs
src-tauri/src/conversation/adapter.rs
src-tauri/src/conversation/repository.rs
src-tauri/src/conversation/repository_test.rs
src-tauri/src/conversation/service.rs
src-tauri/src/conversation/service_test.rs
src-tauri/src/conversation/commands.rs
src-tauri/src/conversation/commands_test.rs
src-tauri/src/conversation/zalo/mod.rs
src-tauri/src/conversation/zalo/client.rs
src-tauri/src/conversation/zalo/client_test.rs
src-tauri/src/conversation/zalo/normalizer.rs
src-tauri/src/conversation/zalo/normalizer_test.rs
src-tauri/src/conversation/zalo/supervisor.rs
src-tauri/src/conversation/zalo/supervisor_test.rs
src-tauri/vendor/zca-rs/src/models/message.rs
src-tauri/vendor/zca-rs/src/apis/message.rs
```

### Frontend files to create

```text
src/lib/services/conversationPlatform.ts
src/lib/services/__tests__/conversationPlatform.spec.ts
src/lib/components/conversation/composerDraftState.ts
src/lib/components/conversation/emojiData.ts
src/lib/components/conversation/messageMedia.ts
src/lib/components/conversation/ComposerToolbar.svelte
src/lib/components/conversation/EmojiPicker.svelte
src/lib/components/conversation/ZaloStickerPicker.svelte
src/lib/components/conversation/AttachmentPreviewQueue.svelte
src/lib/components/conversation/ComposerReplyBar.svelte
src/lib/components/conversation/AttachmentDropOverlay.svelte
src/lib/components/conversation/MessageBatchGroup.svelte
src/lib/components/conversation/MessageContentRenderer.svelte
src/lib/components/conversation/ImageMessageContent.svelte
src/lib/components/conversation/FileMessageContent.svelte
src/lib/components/conversation/VideoMessageContent.svelte
src/lib/components/conversation/AudioMessageContent.svelte
src/lib/components/conversation/StickerMessageContent.svelte
src/lib/components/conversation/LinkMessageContent.svelte
src/lib/components/conversation/MessageReplyPreview.svelte
src/lib/components/conversation/MessageReactionSummary.svelte
src/lib/components/conversation/MessageActionsMenu.svelte
src/lib/components/conversation/RecallMessageDialog.svelte
src/lib/components/conversation/__tests__/composerDraftState.spec.ts
src/lib/components/conversation/__tests__/ComposerToolbar.spec.ts
src/lib/components/conversation/__tests__/ZaloStickerPicker.spec.ts
src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts
src/lib/components/conversation/__tests__/MessageBatchGroup.spec.ts
src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts
src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts
src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts
```

### Frontend files to modify

```text
package.json
bun.lock
src/lib/services/conversationService.ts
src/lib/services/__tests__/conversationService.spec.ts
src/lib/components/conversation/MessageComposer.svelte
src/lib/components/conversation/MessageBubble.svelte
src/lib/components/conversation/ConversationTimeline.svelte
src/lib/components/conversation/ConversationWorkspace.svelte
src/lib/components/conversation/timelineState.ts
src/lib/components/conversation/__tests__/MessageComposer.spec.ts
src/lib/components/conversation/__tests__/MessageBubble.spec.ts
src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts
src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
src/lib/components/conversation/__tests__/timelineState.spec.ts
```

---

## Task 0: Establish a Clean Execution Baseline

**Files:** None.

- [ ] **Step 1: Read memory and inspect working-tree ownership**

Use Agent Memory:

```text
memory_recall("RustSale_v2 Zalo rich chat Phase 1 implementation decisions")
```

Run:

```bash
git status --short
git diff --stat
```

Expected: identify every pre-existing modified or untracked file before implementation. Do not overwrite or stage unrelated files.

- [ ] **Step 2: Check GitNexus freshness**

Run:

```bash
rg -n '"embeddings"|"indexed"|"commit"' .gitnexus/meta.json
git rev-parse HEAD
```

If GitNexus reports a stale index, preserve embeddings when present:

```bash
bunx gitnexus analyze --embeddings
```

Use `bunx gitnexus analyze` only when `.gitnexus/meta.json` confirms the embeddings count is zero.

- [ ] **Step 3: Run baseline targeted suites**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/components/conversation/__tests__
bun run check
```

Expected: PASS, or record exact unrelated baseline failures before changing code.

- [ ] **Step 4: Create an execution checkpoint**

Run:

```bash
git diff --check
```

Expected: no whitespace errors. Do not commit unless explicitly authorized.

---

## Task 1: Add Tauri Dialog, Rich-UI Primitives, and Staging Dependencies

**Files:**
- Modify: `package.json`
- Modify: `bun.lock`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/lib.rs:29`
- Create through CLI: `src/lib/components/ui/popover/`
- Create through CLI: `src/lib/components/ui/context-menu/`

- [ ] **Step 1: Run impact before editing Tauri startup**

Run through GitNexus:

```text
mcp__gitnexus.impact({target: "run", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Expected planning baseline: LOW. Stop and warn if the current result is HIGH or CRITICAL.

- [ ] **Step 2: Verify dependencies and primitives are absent**

Run:

```bash
test ! -d src/lib/components/ui/popover
test ! -d src/lib/components/ui/context-menu
! rg -n 'plugin-dialog|tauri-plugin-dialog|sha2|infer|image =' package.json src-tauri/Cargo.toml
```

Expected: PASS before installation.

- [ ] **Step 3: Install frontend packages with Bun**

Run:

```bash
bun add @tauri-apps/plugin-dialog@^2
bunx --bun shadcn-svelte@1.4.2 add popover context-menu
```

Expected: `package.json`, `bun.lock`, and both UI primitive directories update.

- [ ] **Step 4: Add exact Rust dependencies**

Add under `[dependencies]` in `src-tauri/Cargo.toml`:

```toml
tauri-plugin-dialog = "2"
sha2 = "0.10"
infer = "0.16"
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp", "gif"] }
```

- [ ] **Step 5: Register dialog and permissions**

Add to the Tauri builder immediately after `tauri::Builder::default()`:

```rust
.plugin(tauri_plugin_dialog::init())
```

Add `dialog:default` to `src-tauri/capabilities/default.json`:

```json
"permissions": [
  "core:default",
  "opener:default",
  "dialog:default"
]
```

Set the main window's drag-drop behavior explicitly in `src-tauri/tauri.conf.json`:

```json
{
  "title": "RustSale CRM & Outreach Pro",
  "width": 1280,
  "height": 800,
  "dragDropEnabled": true
}
```

- [ ] **Step 6: Verify dependency integration**

Run:

```bash
bun run check
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS.

- [ ] **Step 7: Review checkpoint**

Run:

```bash
git diff --check
git diff -- package.json src-tauri/Cargo.toml src-tauri/capabilities/default.json src-tauri/tauri.conf.json src-tauri/src/lib.rs
```

Expected: only dependency, permission, and plugin bootstrap changes.

---

## Task 2: Add Caller-Controlled Zalo Client IDs

**Files:**
- Modify: `src-tauri/vendor/zca-rs/src/models/message.rs:488`
- Modify: `src-tauri/vendor/zca-rs/src/apis/message.rs:82`

- [ ] **Step 1: Write failing vendored payload tests**

Add private pure-builder tests in `src-tauri/vendor/zca-rs/src/apis/message.rs`:

```rust
#[test]
fn explicit_text_client_id_is_preserved() {
    let params = build_text_params(
        "hello",
        "peer-1",
        ThreadType::User,
        1_785_491_200_123,
        None,
        None,
        None,
    );
    assert_eq!(params["clientId"], 1_785_491_200_123_u64);
}

#[test]
fn explicit_sticker_client_id_is_preserved() {
    let params = build_sticker_params(
        100,
        20,
        1,
        "group-1",
        ThreadType::Group,
        1_785_491_200_456,
        "imei-a",
    );
    assert_eq!(params["clientId"], 1_785_491_200_456_u64);
    assert_eq!(params["grid"], "group-1");
}
```

Also add a model test:

```rust
#[test]
fn send_message_result_defaults_missing_client_id() {
    let result: SendMessageResult = serde_json::from_value(serde_json::json!({
        "msgId": "101"
    }))
    .unwrap();
    assert_eq!(result.msg_id, 101);
    assert_eq!(result.cli_msg_id, 0);
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```bash
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml explicit_text_client_id_is_preserved
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml send_message_result_defaults_missing_client_id
```

Expected: FAIL because the builders and `cli_msg_id` do not exist.

- [ ] **Step 3: Extend the send result contract**

Change `SendMessageResult` to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageResult {
    #[serde(deserialize_with = "deserialize_string_or_number_u64")]
    pub msg_id: u64,
    #[serde(default, deserialize_with = "deserialize_string_or_number_u64")]
    pub cli_msg_id: u64,
}
```

- [ ] **Step 4: Add compatibility-preserving APIs**

Keep existing `send_message` and `send_sticker`. Add:

```rust
pub async fn send_message_with_client_id(
    &self,
    message: MessageContent,
    thread_id: &str,
    thread_type: ThreadType,
    client_id: u64,
) -> Result<SendMessageResponse, ZaloError>;

pub async fn send_sticker_with_client_id(
    &self,
    sticker_id: u32,
    cate_id: u32,
    sticker_type: u32,
    thread_id: &str,
    thread_type: ThreadType,
    client_id: u64,
) -> Result<SendMessageResult, ZaloError>;
```

Required behavior:

- Existing methods call the new internal path with `get_timestamp_ms()`.
- Explicit-client attachment sends accept at most one attachment. Return `PayloadFormat` for more than one because RustSale sends one attachment per part.
- Every returned `SendMessageResult` receives the exact client ID used for that provider request.
- Quote fields remain unchanged.

- [ ] **Step 5: Extract pure payload builders**

Use these signatures so tests do not require network calls:

```rust
fn build_text_params(
    message: &str,
    thread_id: &str,
    thread_type: ThreadType,
    client_id: u64,
    quote: Option<&SendMessageQuote>,
    mentions: Option<&[MessageMention]>,
    ttl: Option<u64>,
) -> serde_json::Value;

fn build_sticker_params(
    sticker_id: u32,
    cate_id: u32,
    sticker_type: u32,
    thread_id: &str,
    thread_type: ThreadType,
    client_id: u64,
    imei: &str,
) -> serde_json::Value;
```

Move the current direct/group field construction into these helpers without changing provider paths.

- [ ] **Step 6: Run vendored tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml explicit_text_client_id_is_preserved
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml explicit_sticker_client_id_is_preserved
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml send_message_result_defaults_missing_client_id
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
```

Expected: PASS.

- [ ] **Step 7: Review checkpoint**

Run:

```bash
git diff --check
git diff -- src-tauri/vendor/zca-rs/src/models/message.rs src-tauri/vendor/zca-rs/src/apis/message.rs
```

Expected: existing APIs remain available; only client-ID control and testable payload construction change.

---

## Task 3: Define Rich Conversation Contracts and Stable Errors

**Files:**
- Create: `src-tauri/src/conversation/error.rs`
- Modify: `src-tauri/src/conversation/mod.rs:1`
- Modify: `src-tauri/src/conversation/types.rs:189`
- Modify: `src-tauri/src/conversation/adapter.rs:1`

- [ ] **Step 1: Run impact for shared contracts**

Run:

```text
mcp__gitnexus.impact({target: "ConversationMessage", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "ProviderSendRequest", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Expected planning baseline: LOW. Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing serialization tests**

Add tests to `types.rs` for these exact JSON contracts:

```rust
#[test]
fn batch_request_uses_tagged_camel_case_parts() {
    let request = SendConversationBatchRequest {
        conversation_id: 31,
        client_batch_id: "batch-1".into(),
        reply_to_message_id: Some(9),
        parts: vec![ConversationSendPartRequest {
            client_part_id: "part-1".into(),
            content: ConversationSendPartContent::Attachment {
                staged_attachment_id: "stage-1".into(),
            },
        }],
    };
    let value = serde_json::to_value(request).unwrap();
    assert_eq!(value["conversationId"], 31);
    assert_eq!(value["parts"][0]["clientPartId"], "part-1");
    assert_eq!(value["parts"][0]["type"], "attachment");
    assert_eq!(value["parts"][0]["stagedAttachmentId"], "stage-1");
}
```

Add an error serialization test in `error.rs`:

```rust
#[test]
fn command_error_serializes_stable_code() {
    let error = ConversationError::new(
        ConversationErrorCode::StageExpired,
        "Attachment staging token expired",
    );
    let value = serde_json::to_value(error.command_error()).unwrap();
    assert_eq!(value["code"], "stage_expired");
}
```

- [ ] **Step 3: Run tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml batch_request_uses_tagged_camel_case_parts
cargo test --manifest-path src-tauri/Cargo.toml command_error_serializes_stable_code
```

Expected: FAIL because the contracts do not exist.

- [ ] **Step 4: Add stable errors**

Create `error.rs` with:

```rust
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationErrorCode {
    CapabilityDenied,
    AccountDisconnected,
    StageExpired,
    StageAlreadyClaimed,
    FileTooLarge,
    UnsupportedMedia,
    QuoteNotReady,
    ProviderAuth,
    ProviderRateLimited,
    DeliveryUnknown,
    RecallNotAllowed,
    InvalidRequest,
    ProviderFailure,
}

#[derive(Debug, Clone)]
pub struct ConversationError {
    pub code: ConversationErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationCommandError {
    pub code: ConversationErrorCode,
    pub message: String,
}
```

Implement `Display`, `std::error::Error`, `new()`, and `command_error()` without logging sensitive values.

- [ ] **Step 5: Add shared capability and projection contracts**

Add these domain types to `types.rs`:

```rust
pub enum MessageSendBatchStatus { Queued, Sending, Partial, Sent, Failed }

pub struct ConversationAttachmentCapabilities {
    pub image: bool,
    pub video: bool,
    pub audio: bool,
    pub file: bool,
    pub max_files_per_batch: u32,
    pub max_bytes_per_file: u64,
    pub max_bytes_per_batch: u64,
}

pub struct ConversationReactionCapabilities { pub read: bool, pub write: bool }

pub struct ConversationCapabilities {
    pub send_text: bool,
    pub send_sticker: bool,
    pub quote_reply: bool,
    pub recall_message: bool,
    pub reactions: ConversationReactionCapabilities,
    pub attachments: ConversationAttachmentCapabilities,
}

pub struct ConversationAttachment {
    pub id: i64,
    pub attachment_type: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub remote_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub metadata: serde_json::Value,
}

pub struct ConversationReplyPreview {
    pub message_id: Option<i64>,
    pub sender_display_name: Option<String>,
    pub content_type: String,
    pub text: Option<String>,
    pub thumbnail_url: Option<String>,
    pub available_in_timeline: bool,
}

pub struct ConversationReactionActor {
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub is_current_account: bool,
}

pub struct ConversationReactionSummary {
    pub reaction: String,
    pub count: u64,
    pub reacted_by_current_account: bool,
    pub actors: Vec<ConversationReactionActor>,
}

pub struct ConversationSendBatchSummary {
    pub id: String,
    pub status: MessageSendBatchStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
```

All serializable structs use `#[serde(rename_all = "camelCase")]`; status enums use `snake_case`.

- [ ] **Step 6: Add send, staging, sticker, and recall request types**

Use this exact shape:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendConversationBatchRequest {
    pub conversation_id: i64,
    pub client_batch_id: String,
    pub reply_to_message_id: Option<i64>,
    pub parts: Vec<ConversationSendPartRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSendPartRequest {
    pub client_part_id: String,
    #[serde(flatten)]
    pub content: ConversationSendPartContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversationSendPartContent {
    Text { text: String },
    Attachment {
        #[serde(rename = "stagedAttachmentId")]
        staged_attachment_id: String,
    },
    Sticker { sticker: ZaloStickerReference },
}
```

Also add:

- `SendConversationBatchResult { batch, messages }`
- `StageConversationAttachmentPathsRequest { conversation_id, paths }`
- `StageConversationClipboardImageRequest { conversation_id, file_name, mime_type, base64_data }`
- `StagedConversationAttachment { id, file_name, mime_type, size_bytes, attachment_type, preview_data_url, status }`
- `DiscardConversationStagedAttachmentRequest { staged_attachment_id }`
- `RecallConversationMessageRequest { message_id }`
- `SearchZaloStickersRequest { conversation_id, query, limit }`
- `ZaloStickerReference { sticker_id, category_id, sticker_type, text, preview_url, webp_url, total_frames, duration_ms }`

- [ ] **Step 7: Extend `ConversationMessage` and account targets**

Add required fields with empty/default values from repository mapping:

```rust
pub provider_client_message_id: Option<String>,
pub attachments: Vec<ConversationAttachment>,
pub reply_preview: Option<ConversationReplyPreview>,
pub reactions: Vec<ConversationReactionSummary>,
pub send_batch_id: Option<String>,
pub batch_index: Option<u32>,
pub send_batch: Option<ConversationSendBatchSummary>,
```

Add `capabilities: ConversationCapabilities` to both `ConversationReplyTarget` and `ConversationAccountRuntime`.

Implement these constructors so later tasks use one capability source:

```rust
impl ConversationCapabilities {
    pub fn text_only() -> Self {
        Self {
            send_text: true,
            send_sticker: false,
            quote_reply: false,
            recall_message: false,
            reactions: ConversationReactionCapabilities { read: false, write: false },
            attachments: ConversationAttachmentCapabilities {
                image: false,
                video: false,
                audio: false,
                file: false,
                max_files_per_batch: 0,
                max_bytes_per_file: 0,
                max_bytes_per_batch: 0,
            },
        }
    }

    pub fn zalo_phase_one() -> Self {
        Self {
            send_text: true,
            send_sticker: true,
            quote_reply: true,
            recall_message: true,
            reactions: ConversationReactionCapabilities { read: true, write: false },
            attachments: ConversationAttachmentCapabilities {
                image: true,
                video: true,
                audio: true,
                file: true,
                max_files_per_batch: 10,
                max_bytes_per_file: 50 * 1024 * 1024,
                max_bytes_per_batch: 100 * 1024 * 1024,
            },
        }
    }
}
```

- [ ] **Step 8: Replace adapter payload contracts**

Use `adapter.rs` for provider-neutral outbound requests:

```rust
pub enum ProviderOutboundContent {
    Text { text: String },
    Attachment {
        data: Vec<u8>,
        file_name: String,
        mime_type: String,
        size_bytes: u64,
    },
    Sticker {
        sticker_id: u32,
        category_id: u32,
        sticker_type: u32,
    },
}

pub struct ProviderQuoteContext {
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub sender_external_id: String,
    pub provider_timestamp_ms: String,
    pub message_type: String,
    pub content: serde_json::Value,
    pub property_ext: Option<serde_json::Value>,
    pub ttl: i32,
}

pub struct ProviderSendRequest {
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_client_message_id: String,
    pub content: ProviderOutboundContent,
    pub quote: Option<ProviderQuoteContext>,
}

pub struct ProviderSendResult {
    pub provider_message_id: String,
    pub provider_client_message_id: String,
}
```

Add provider error classification enums for `Retryable`, `RateLimited`, `Authentication`, and `Permanent`.

- [ ] **Step 9: Register new modules and run tests GREEN**

Add to `conversation/mod.rs`:

```rust
pub mod error;
pub mod projection;
pub mod retry;
pub mod staging;

#[cfg(test)]
mod retry_test;
#[cfg(test)]
mod staging_test;
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml batch_request_uses_tagged_camel_case_parts
cargo test --manifest-path src-tauri/Cargo.toml command_error_serializes_stable_code
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS after updating compile errors in existing constructors/tests with empty rich fields and text-only capabilities.

---

## Task 4: Add the Additive Rich-Chat Migration

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql:1`
- Modify: `src-tauri/src/conversation/migration.rs:3`
- Modify: `src-tauri/src/conversation/migration_test.rs:1`

- [ ] **Step 1: Run migration impact**

```text
mcp__gitnexus.impact({target: "migrate", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing migration tests**

Add tests asserting:

```rust
for expected in ["message_send_batches", "conversation_staged_attachments"] {
    assert!(tables.contains(&expected.to_string()), "missing {expected}");
}

assert!(column_names(&conn, "channel_accounts").contains(&"self_external_id".into()));
assert!(column_names(&conn, "messages").contains(&"send_batch_id".into()));
assert!(column_names(&conn, "messages").contains(&"batch_index".into()));
```

Add an upgrade test that creates the pre-rich schema, inserts an existing message, runs `migrate()` twice, and confirms the original message still exists.

Add uniqueness tests for:

```text
(channel_account_id, client_batch_id)
(send_batch_id, batch_index)
(conversation_id, provider_client_message_id) when non-null
```

- [ ] **Step 3: Run migration tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation_schema_is_idempotent
cargo test --manifest-path src-tauri/Cargo.toml rich_chat_migration_preserves_existing_messages
```

Expected: FAIL because tables/columns/indexes are absent.

- [ ] **Step 4: Add exact schema objects**

Append `message_send_batches` and `conversation_staged_attachments` from the approved design to `schema.sql`. Add only indexes whose referenced columns already exist in the original schema:

```sql
CREATE INDEX IF NOT EXISTS idx_send_batches_conversation_created
ON message_send_batches(conversation_id, created_at, id);

CREATE INDEX IF NOT EXISTS idx_staged_attachments_cleanup
ON conversation_staged_attachments(status, expires_at, created_at);
```

- [ ] **Step 5: Add columns idempotently**

Extend `migrate()`:

```rust
add_column_if_missing(conn, "channel_accounts", "self_external_id", "TEXT")?;
add_column_if_missing(conn, "messages", "send_batch_id", "TEXT")?;
add_column_if_missing(conn, "messages", "batch_index", "INTEGER")?;
```

After the columns exist, create the column-dependent indexes explicitly in `migration.rs`:

```rust
conn.execute_batch(
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_send_batch_part
       ON messages(send_batch_id, batch_index)
       WHERE send_batch_id IS NOT NULL;
     CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_provider_client_unique
       ON messages(conversation_id, provider_client_message_id)
       WHERE provider_client_message_id IS NOT NULL;",
)?;
```

Do not execute `schema.sql` a second time. This ordering prevents upgraded databases from evaluating an index before `send_batch_id` and `batch_index` have been added.

- [ ] **Step 6: Run migration tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::migration_test
```

Expected: PASS.

- [ ] **Step 7: Inspect an upgraded temporary database**

Run the migration test with output:

```bash
cargo test --manifest-path src-tauri/Cargo.toml rich_chat_migration_preserves_existing_messages -- --nocapture
```

Expected: existing row count remains unchanged and all new objects exist.

---

## Task 5: Implement Durable Attachment Staging

**Files:**
- Create: `src-tauri/src/conversation/staging.rs`
- Create: `src-tauri/src/conversation/staging_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs:86`
- Modify: `src-tauri/src/conversation/service.rs:25`
- Modify: `src-tauri/src/conversation/commands_test.rs:8`
- Modify: `src-tauri/src/conversation/service_test.rs:25`

- [ ] **Step 1: Run impact for the service constructor and repository**

```text
mcp__gitnexus.impact({target: "ConversationService", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "ConversationRepository", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing staging tests**

Cover these exact cases in `staging_test.rs`:

| Test | Setup | Required assertions |
|---|---|---|
| `stage_path_copies_bytes_into_spool_and_returns_opaque_id` | Write `hello.png` fixture bytes under a source temp directory and stage it | Returned ID is non-empty; DB path is below the spool root; DB path differs from source; size and SHA-256 match copied bytes; source remains present |
| `stage_path_rejects_file_larger_than_50_mib` | Create a sparse file of `MAX_ATTACHMENT_BYTES + 1` | Error code is `FileTooLarge`; no staging row and no spool file remain |
| `stage_clipboard_rejects_encoded_payload_over_10_mib` | Pass a base64 string whose encoded length exceeds the computed 10 MiB ceiling | Error code is `FileTooLarge`; decoding and DB insertion are not attempted |
| `discard_is_idempotent_when_file_is_already_missing` | Stage a file, remove its spool file manually, call discard twice | Both calls succeed and row status is `discarded` |
| `staging_rejects_symlink_source` | On Unix, create a symlink to a regular file | Error code is `UnsupportedMedia`; target contents are not copied |
| `staging_enforces_one_gib_spool_quota` | Insert active staging rows totaling `MAX_SPOOL_BYTES` | Next staging request fails with `FileTooLarge`; existing rows/files remain unchanged |

- [ ] **Step 3: Run staging tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::staging_test
```

Expected: FAIL because the stager does not exist.

- [ ] **Step 4: Implement staging constants and service**

Create `ConversationAttachmentStager`:

```rust
pub const MAX_ATTACHMENT_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_BATCH_BYTES: u64 = 100 * 1024 * 1024;
pub const MAX_CLIPBOARD_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_ATTACHMENTS_PER_BATCH: usize = 10;
pub const MAX_SPOOL_BYTES: u64 = 1024 * 1024 * 1024;
pub const STAGED_TTL_HOURS: i64 = 24;

#[derive(Clone)]
pub struct ConversationAttachmentStager {
    root: std::path::PathBuf,
    repository: std::sync::Arc<ConversationRepository>,
}
```

Required methods:

```rust
pub async fn stage_paths(
    &self,
    conversation_id: i64,
    paths: Vec<String>,
) -> ConversationResult<Vec<StagedConversationAttachment>>;

pub async fn stage_clipboard_image(
    &self,
    request: StageConversationClipboardImageRequest,
) -> ConversationResult<StagedConversationAttachment>;

pub async fn discard(&self, staged_attachment_id: &str) -> ConversationResult<()>;

pub async fn cleanup(&self, now: &str) -> ConversationResult<StagingCleanupReport>;
```

Use `tokio::task::spawn_blocking` for filesystem and image work. Generate physical names from random UUID-like IDs, never the source filename.

- [ ] **Step 5: Implement copied-byte validation**

Before copying, call `symlink_metadata` on the source and reject symlinks and non-regular files. The blocking validation function for the copied file must:

```rust
fn inspect_staged_file(path: &Path, display_name: &str) -> ConversationResult<InspectedFile> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(ConversationError::new(
            ConversationErrorCode::UnsupportedMedia,
            "Attachment must be a regular file",
        ));
    }
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Err(ConversationError::new(
            ConversationErrorCode::FileTooLarge,
            "Attachment exceeds the 50 MiB application limit",
        ));
    }
    // Read the staged copy, sniff MIME with infer, hash with SHA-256,
    // classify image/video/audio/file, and generate a bounded image thumbnail.
}
```

Reject executable MIME signatures. Treat PDF, Office ZIP containers, ZIP, text, and unknown non-executable application data as generic files.

Generate `preview_data_url` only from a resized image thumbnail, never from the full staged file.

- [ ] **Step 6: Add staging repository methods**

Add exact methods:

```rust
insert_staged_attachment(&StagedAttachmentRecord) -> Result<()>;
get_staged_attachment(&str) -> Result<Option<StagedAttachmentRecord>>;
mark_staged_attachment_discarded(&str) -> Result<()>;
list_staging_cleanup_candidates(&str) -> Result<Vec<StagedAttachmentRecord>>;
active_spool_bytes() -> Result<u64>;
```

All writes use canonical UTC timestamps. Repository rows never contain the original source path.

- [ ] **Step 7: Inject the spool root into `ConversationService`**

Change the constructor to:

```rust
pub fn new(
    repository: Arc<ConversationRepository>,
    events: Arc<ConversationEventPublisher>,
    spool_root: std::path::PathBuf,
) -> Self {
    let stager = ConversationAttachmentStager::new(spool_root, repository.clone());
    Self { repository, events, stager }
}
```

Update every test harness to use a unique temporary spool directory and remove it in `Drop`.

- [ ] **Step 8: Run staging tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::staging_test
cargo test --manifest-path src-tauri/Cargo.toml conversation::commands_test
cargo test --manifest-path src-tauri/Cargo.toml conversation::service_test
```

Expected: PASS.

- [ ] **Step 9: Review checkpoint**

Run:

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Expected: only Conversation staging, constructor, dependency, and test flows.

---

## Task 6: Enqueue Send Batches Atomically

**Files:**
- Modify: `src-tauri/src/conversation/repository.rs:1598`
- Modify: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/service.rs:369`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/commands.rs:95`
- Modify: `src-tauri/src/conversation/commands_test.rs`

- [x] **Step 1: Run impact for enqueue seams**

```text
mcp__gitnexus.impact({target: "enqueue_message", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "enqueue_outbound_message", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [x] **Step 2: Write failing repository transaction tests**

Add these exact tests:

| Test | Required assertions |
|---|---|
| `enqueue_batch_creates_batch_parts_attachments_and_jobs_in_one_transaction` | One batch, three ordered messages, two attachment rows, and three outbox jobs exist; all parts share the batch ID |
| `enqueue_batch_rolls_back_every_row_when_one_stage_is_invalid` | Batch/message/attachment/job counts remain zero and valid stage rows remain unclaimed |
| `repeated_client_batch_id_returns_existing_batch_without_duplicates` | Both calls return the same batch/message IDs and row counts remain unchanged |
| `stage_id_can_be_claimed_only_once` | First enqueue succeeds; second batch receives `StageAlreadyClaimed` and creates no rows |
| `quote_is_written_only_to_batch_and_first_part` | Batch and part zero reference the target; later parts have `reply_to_message_id IS NULL` |
| `sticker_must_be_the_only_part_in_phase_one` | Sticker plus text or attachment returns `InvalidRequest`; a one-sticker batch succeeds |

The first test must assert exact counts for batch, messages, attachments, and jobs and exact `batch_index` ordering.

- [x] **Step 3: Run tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml enqueue_batch_creates_batch_parts_attachments_and_jobs_in_one_transaction
cargo test --manifest-path src-tauri/Cargo.toml repeated_client_batch_id_returns_existing_batch_without_duplicates
```

Expected: FAIL because batch enqueue does not exist.

- [x] **Step 4: Add a monotonic provider client-ID generator**

Implement a process-monotonic decimal ID:

```rust
static LAST_PROVIDER_CLIENT_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

fn next_provider_client_message_id() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut observed = LAST_PROVIDER_CLIENT_ID.load(Ordering::Relaxed);
    loop {
        let candidate = now_ms.max(observed.saturating_add(1));
        match LAST_PROVIDER_CLIENT_ID.compare_exchange_weak(
            observed,
            candidate,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => return candidate.to_string(),
            Err(actual) => observed = actual,
        }
    }
}
```

Add a test generating 10,000 IDs and asserting uniqueness and numeric parsing.

- [x] **Step 5: Implement repository batch enqueue**

Add:

```rust
pub fn enqueue_outbound_batch(
    &self,
    request: &SendConversationBatchRequest,
    promoted_files: &[PromotedStagedAttachment],
    now: &str,
) -> ConversationResult<SendConversationBatchResult>;
```

Inside one transaction:

1. Load the conversation and authoritative channel account.
2. Parse account capabilities.
3. Return the existing batch for duplicate `(channel_account_id, client_batch_id)`.
4. Validate 1–10 parts, total staged bytes, text, sticker-only rule, and capability compatibility.
5. Validate quote target is in the same conversation and has both provider IDs.
6. Insert `message_send_batches` as `queued`.
7. Insert each message with `send_batch_id`, `batch_index`, deterministic `local_request_id`, and pre-generated `provider_client_message_id`.
8. Insert one attachment row for attachment/sticker parts.
9. Mark each staging row claimed with `claimed_message_id`.
10. Insert one outbox job per message.
11. Query and return every new message plus one batch summary.

Use:

```text
local_request_id = batch:{client_batch_id}:part:{client_part_id}
outbox job id   = outbox:{message_id}:{random_u128}
```

- [x] **Step 6: Add the service method and compatibility wrapper**

Add:

```rust
pub async fn enqueue_batch(
    &self,
    request: SendConversationBatchRequest,
) -> ConversationResult<SendConversationBatchResult>;
```

It promotes staged files before the DB transaction, compensates filesystem renames on transaction failure, publishes one invalidation per returned message, and clears only claimed stage tokens.

Change `enqueue_message` to build a text-only batch and return its first message:

```rust
let batch = self.enqueue_batch(SendConversationBatchRequest {
    conversation_id: request.conversation_id,
    client_batch_id: format!("legacy-{}", rand::random::<u128>()),
    reply_to_message_id: request.reply_to_message_id,
    parts: vec![ConversationSendPartRequest {
        client_part_id: format!("legacy-part-{}", rand::random::<u128>()),
        content: ConversationSendPartContent::Text {
            text: request.text.clone(),
        },
    }],
}).await?;
```

Keep validating the legacy `channel_account_id` against the authoritative conversation account before delegation.

- [x] **Step 7: Add the Tauri batch command**

Add:

```rust
#[tauri::command]
pub async fn send_conversation_batch(
    state: tauri::State<'_, ConversationState>,
    request: SendConversationBatchRequest,
) -> Result<SendConversationBatchResult, ConversationCommandError> {
    state.service.enqueue_batch(request).await.map_err(|error| error.command_error())
}
```

Convert the legacy command to async because it delegates to staging-aware batch enqueue.

- [x] **Step 8: Run batch tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml enqueue_batch_
cargo test --manifest-path src-tauri/Cargo.toml repeated_client_batch_id_returns_existing_batch_without_duplicates
cargo test --manifest-path src-tauri/Cargo.toml send_conversation_message
```

Expected: PASS.

- [x] **Step 9: Review checkpoint**

Run:

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Expected: batch enqueue affects Conversation commands, service, repository, and tests only.

---

## Task 7: Extend the Zalo Adapter for Parts, Quote, Sticker, and Recall

**Files:**
- Modify: `src-tauri/src/conversation/zalo/client.rs:67`
- Modify: `src-tauri/src/conversation/zalo/client_test.rs:1`
- Modify: `src-tauri/src/conversation/adapter.rs`

- [x] **Step 1: Run trait impact**

```text
mcp__gitnexus.impact({target: "ZaloClient", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Expected planning baseline: LOW. Update every fake implementation found by compilation even if GitNexus does not index test impls.

- [x] **Step 2: Write failing fake-client contract tests**

Replace `CapturedSendRequest` with a capture of `ProviderSendRequest`. Add tests asserting:

```rust
assert_eq!(captured.provider_client_message_id, "1785491200123");
assert!(matches!(captured.content, ProviderOutboundContent::Attachment { .. }));
assert_eq!(captured.quote.unwrap().provider_message_id, "provider-9");
```

Add tests that `search_stickers`, `recall`, and `own_external_id` are forwarded by the fake contract.

- [x] **Step 3: Run client tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::client_test
```

Expected: FAIL because the trait supports only `send_text`.

- [x] **Step 4: Replace the rich-write trait surface**

Add:

```rust
async fn send_part(&self, request: ProviderSendRequest) -> Result<ProviderSendResult, ProviderSendError>;
async fn search_stickers(&self, keyword: &str, limit: u32) -> Result<Vec<ZaloStickerReference>, ProviderSendError>;
async fn recall(&self, request: ProviderRecallRequest) -> Result<(), ProviderSendError>;
fn own_external_id(&self) -> Option<String>;
```

Keep the history, profile, group, subscription, start, and stop methods unchanged.

- [x] **Step 5: Implement `RealZaloClient::send_part`**

Parse `provider_client_message_id` as `u64`; return `Permanent` on invalid numeric format.

Map content:

```rust
ProviderOutboundContent::Text { text } => MessageContent {
    msg: text,
    styles: None,
    urgency: None,
    quote: request.quote.map(to_zalo_quote),
    mentions: None,
    attachments: None,
    ttl: None,
}

ProviderOutboundContent::Attachment { data, file_name, size_bytes, .. } => MessageContent {
    msg: String::new(),
    styles: None,
    urgency: None,
    quote: request.quote.map(to_zalo_quote),
    mentions: None,
    attachments: Some(vec![AttachmentSource::Buffer {
        data,
        filename: file_name,
        metadata: AttachmentMetadata { total_size: size_bytes, width: None, height: None },
    }]),
    ttl: None,
}
```

Sticker calls `send_sticker_with_client_id`. Return `ProviderSendResult` with both IDs.

- [x] **Step 6: Implement quote conversion**

Use:

```rust
fn to_zalo_quote(value: ProviderQuoteContext) -> SendMessageQuote {
    SendMessageQuote {
        content: value.content,
        msg_type: value.message_type,
        property_ext: value.property_ext,
        uid_from: value.sender_external_id,
        msg_id: value.provider_message_id,
        cli_msg_id: value.provider_client_message_id,
        ts: value.provider_timestamp_ms,
        ttl: value.ttl,
    }
}
```

The repository/service must supply provider timestamp milliseconds, not RFC 3339, for Zalo quote payloads.

- [x] **Step 7: Implement sticker search and recall**

Sticker search:

```rust
let basics = self.inner.search_sticker(keyword, Some(limit.clamp(1, 50))).await?;
let ids = basics.iter().map(|item| item.sticker_id).collect::<Vec<_>>();
let details = self.inner.get_stickers_detail(&ids).await?;
```

Normalize details by original ID order into `ZaloStickerReference`.

Recall calls:

```rust
self.inner
    .undo(
        &request.provider_message_id,
        &request.provider_client_message_id,
        &request.external_thread_id,
        request.thread_type,
    )
    .await?;
```

`own_external_id()` returns `self.inner.get_own_id()`.

- [x] **Step 8: Classify `ZaloError` structurally**

Required mapping:

- `Network` and `WebSocket`: retryable.
- API code `429` or message containing `rate limit`: rate-limited.
- API code `>= 500`: retryable.
- API/login/session/cookie/auth failures: authentication.
- `PayloadFormat`, `Io`, invalid client ID, and unsupported content: permanent.
- Other API errors: permanent unless explicitly classified retryable.

- [x] **Step 9: Run client tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::client_test
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
```

Expected: PASS.

---

## Task 8: Upgrade the Outbox Worker, Retry Policy, and Batch Status

**Files:**
- Create: `src-tauri/src/conversation/retry.rs`
- Create: `src-tauri/src/conversation/retry_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs:66`
- Modify: `src-tauri/src/conversation/repository.rs:2115`
- Modify: `src-tauri/src/conversation/service.rs:438`
- Modify: `src-tauri/src/conversation/service_test.rs`

- [x] **Step 1: Run worker impacts**

```text
mcp__gitnexus.impact({target: "claim_next_outbox_job", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "process_next_outbox_job", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "complete_outbox_job", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [x] **Step 2: Write failing pure retry tests**

```rust
#[test]
fn retry_policy_uses_five_bounded_backoff_steps() {
    let policy = RetryPolicy::default();
    assert_eq!(policy.delay(1, 0.5), Duration::from_secs(5));
    assert_eq!(policy.delay(2, 0.5), Duration::from_secs(15));
    assert_eq!(policy.delay(3, 0.5), Duration::from_secs(45));
    assert_eq!(policy.delay(4, 0.5), Duration::from_secs(120));
    assert_eq!(policy.delay(5, 0.5), Duration::from_secs(300));
    assert!(policy.should_stop(5));
}
```

Use `jitter_unit = 0.5` as the neutral value. Clamp production jitter to ±20%.

- [x] **Step 3: Write failing worker behavior tests**

Add tests:

```rust
successful_attachment_part_reads_claimed_spool_and_completes_job
successful_part_is_not_requeued_when_sibling_part_retries
retryable_provider_error_sets_retry_wait_and_partial_batch
permanent_file_error_fails_without_retry
authentication_error_invalidates_account_and_stops_following_jobs
startup_running_job_becomes_delivery_unknown_without_requeue
```

- [x] **Step 4: Run tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml retry_policy_uses_five_bounded_backoff_steps
cargo test --manifest-path src-tauri/Cargo.toml successful_attachment_part_reads_claimed_spool_and_completes_job
```

Expected: FAIL.

- [x] **Step 5: Expand `ClaimedOutboxJob`**

It must include:

```rust
pub content_type: String,
pub text: Option<String>,
pub content: serde_json::Value,
pub provider_client_message_id: String,
pub attachment: Option<ClaimedAttachment>,
pub quote: Option<ProviderQuoteContext>,
pub send_batch_id: Option<String>,
pub batch_index: Option<u32>,
pub attempt_count: u32,
```

The claim query requires `channel_accounts.connection_status = 'connected'`, orders by batch creation time, message `batch_index`, job creation time, and message ID. It changes only the selected part to running and recalculates its batch. This filter prevents the supervisor's next ticker from claiming more work after an authentication failure marks the account invalid.

- [x] **Step 6: Build provider requests safely**

In `process_next_outbox_job`:

1. Resolve phone recipients using the existing direct-phone flow.
2. Read one claimed attachment only after canonicalizing it under the spool root.
3. Verify size and checksum against the DB row.
4. Build `ProviderSendRequest`.
5. Call `client.send_part()`.
6. Persist both provider IDs on success.
7. Delete neither claimed file nor attachment row before provider echo supplies remote metadata.

- [x] **Step 7: Implement retry/error transitions**

Change repository methods to accept stable error code and recalculate batch in the same transaction:

```rust
complete_outbox_job(job_id, provider_message_id, provider_client_message_id, now)
schedule_outbox_retry(job_id, retry_at, code, message, now)
fail_outbox_job(job_id, code, message, now)
mark_running_jobs_delivery_unknown(now)
```

`mark_running_jobs_delivery_unknown` keeps message status `failed`, sets `error_code = 'delivery_unknown'`, and never changes the job back to pending.

- [x] **Step 8: Implement exact batch recalculation**

Add one transaction helper:

```rust
fn recalculate_send_batch_status_tx(
    tx: &rusqlite::Transaction<'_>,
    send_batch_id: &str,
    now: &str,
) -> Result<MessageSendBatchStatus>;
```

Use counts of sent, active, and terminal parts to apply the approved `queued`, `sending`, `partial`, `sent`, and `failed` rules. Set `completed_at` only for `sent` or `failed`.

- [x] **Step 9: Run worker tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::retry_test
cargo test --manifest-path src-tauri/Cargo.toml successful_attachment_part_reads_claimed_spool_and_completes_job
cargo test --manifest-path src-tauri/Cargo.toml successful_part_is_not_requeued_when_sibling_part_retries
cargo test --manifest-path src-tauri/Cargo.toml startup_running_job_becomes_delivery_unknown_without_requeue
```

Expected: PASS.

- [x] **Step 10: Backend checkpoint**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

Then:

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Expected: only approved Conversation and Zalo delivery flows.

---

## Task 9: Normalize Rich Payloads and Reconcile Provider Echoes

**Files:**
- Create: `src-tauri/src/conversation/projection.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs:85`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs:26`
- Modify: `src-tauri/src/conversation/repository.rs:93`
- Modify: `src-tauri/src/conversation/repository.rs:880`
- Modify: `src-tauri/src/conversation/repository_test.rs`

- [ ] **Step 1: Run reconciliation impact**

```text
mcp__gitnexus.impact({target: "apply_normalized_message", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "normalize_listener_event", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing normalizer tests**

Add provider fixtures and assert normalized fields for:

```text
chat.photo       -> image attachment with remote and thumbnail URLs
chat.video.msg   -> video attachment
chat.voice       -> audio attachment
share.file       -> file attachment with filename and size
chat.sticker     -> sticker attachment metadata
chat.link        -> link content metadata, no frontend scraping
message quote    -> normalized provider IDs plus fallback preview
```

Use actual `TMessage` JSON shapes captured by current model fields. Assert that unknown fields remain in `raw_payload`.

- [ ] **Step 3: Write failing reconciliation tests**

Add these exact tests:

| Test | Required assertions |
|---|---|
| `outbound_echo_matches_provider_client_id_and_updates_local_part` | Message row count stays one; provider message ID/timestamp/raw payload update; job becomes completed; batch becomes sent |
| `outbound_echo_completes_delivery_unknown_job_without_duplicate_row` | Failed delivery-unknown part becomes sent and row count stays one |
| `repeated_equal_text_without_matching_provider_ids_inserts_a_new_message` | Two equal-text events with different provider IDs produce two rows |
| `incoming_quote_resolves_local_reply_target_when_present` | `reply_to_message_id` points to the local target and projection uses current target content |
| `incoming_quote_keeps_snapshot_when_target_is_missing` | `reply_to_message_id` is null and projection uses normalized fallback sender/text/type |
| `normalized_attachment_upserts_existing_local_attachment_metadata` | Existing local attachment row gains provider ID/remote URL/thumbnail without creating a second row |

- [ ] **Step 4: Run tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml photo_message_normalizes_attachment
cargo test --manifest-path src-tauri/Cargo.toml outbound_echo_matches_provider_client_id_and_updates_local_part
```

Expected: FAIL.

- [ ] **Step 5: Extend `NormalizedMessage`**

Add:

```rust
pub attachments: Vec<NormalizedAttachment>,
pub reply: Option<NormalizedReplyReference>,
```

`NormalizedAttachment` contains provider attachment ID, attachment type, filename, MIME, size, remote URL, thumbnail URL, and metadata JSON.

`NormalizedReplyReference` contains provider message/client IDs, owner ID, timestamp, text, attachment snapshot, and raw quote JSON.

- [ ] **Step 6: Normalize without destructive assumptions**

Replace `content_parts()` with a helper returning:

```rust
struct NormalizedContentParts {
    content_type: String,
    text_content: Option<String>,
    content: serde_json::Value,
    attachments: Vec<NormalizedAttachment>,
}
```

Read known URL/name/size fields through ordered fallback lists and keep the original content JSON. Do not fetch remote content during normalization.

- [ ] **Step 7: Reconcile existing rows exactly**

In `apply_normalized_message`, match in this order:

1. `(conversation_id, provider_message_id)`.
2. `(conversation_id, provider_client_message_id)`.
3. `local_request_id` only when explicitly supplied.

When matched, update the existing row rather than returning immediately. Merge provider IDs, normalized content, timestamps, raw payload, status, reply target, and attachments. Complete any open outbox job and recalculate its batch.

Never add a content/time/filename heuristic.

- [ ] **Step 8: Add bulk rich projection hydration**

Create `projection.rs`:

```rust
pub fn hydrate_messages(
    conn: &rusqlite::Connection,
    messages: &mut [ConversationMessage],
) -> rusqlite::Result<()>;
```

Use bounded bulk queries by message IDs to populate attachments, reaction groups/actors, reply previews, and send-batch summaries. Do not issue one query per message.

Initialize rich fields to empty values in `map_conversation_message`, then call `hydrate_messages` from contact, conversation, identity, and enqueue result paths.

- [ ] **Step 9: Run normalization and repository tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::normalizer_test
cargo test --manifest-path src-tauri/Cargo.toml outbound_echo_
cargo test --manifest-path src-tauri/Cargo.toml incoming_quote_
cargo test --manifest-path src-tauri/Cargo.toml normalized_attachment_upserts_existing_local_attachment_metadata
```

Expected: PASS.

---

## Task 10: Add Self Identity, Sticker Cache, Recall, and Rich Commands

**Files:**
- Create: `src-tauri/src/conversation/zalo/sticker_cache.rs`
- Create: `src-tauri/src/conversation/zalo/sticker_cache_test.rs`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`
- Modify: `src-tauri/src/conversation/zalo/supervisor.rs:217`
- Modify: `src-tauri/src/conversation/zalo/supervisor_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`

- [ ] **Step 1: Run supervisor and command impacts**

```text
mcp__gitnexus.impact({target: "ZaloSessionSupervisor", direction: "upstream", repo: "RustSale_v2", includeTests: true})
mcp__gitnexus.impact({target: "send_conversation_message", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing sticker cache tests**

Cover:

```rust
same_account_query_and_limit_hits_cache_within_five_minutes
different_accounts_do_not_share_sticker_cache
expired_entry_calls_provider_again
cache_evicts_oldest_entry_after_one_hundred_keys
```

Use an injected clock so no test sleeps.

- [ ] **Step 3: Write failing self-identity and recall tests**

Add tests:

```rust
connected_supervisor_persists_client_own_external_id
reaction_projection_marks_current_account_actor
prepare_recall_rejects_inbound_pending_removed_or_missing_provider_ids
successful_recall_marks_tombstone_after_provider_success
failed_recall_leaves_message_unchanged
already_recalled_provider_response_is_idempotent_success
```

- [ ] **Step 4: Run tests RED**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml same_account_query_and_limit_hits_cache_within_five_minutes
cargo test --manifest-path src-tauri/Cargo.toml successful_recall_marks_tombstone_after_provider_success
```

Expected: FAIL.

- [ ] **Step 5: Track connected clients in the supervisor**

Add:

```rust
clients: tokio::sync::Mutex<HashMap<String, Arc<dyn ZaloClient>>>,
sticker_cache: ZaloStickerCache,
```

Insert the client only after listener readiness and remove it on reconnect/stop. Expose:

```rust
pub async fn connected_client(
    &self,
    provider_account_key: &str,
) -> Option<Arc<dyn ZaloClient>>;

pub async fn search_stickers(
    &self,
    provider_account_key: &str,
    query: &str,
    limit: u32,
) -> ConversationResult<Vec<ZaloStickerReference>>;
```

- [ ] **Step 6: Persist own identity and capabilities on readiness**

After account status becomes connected:

```rust
service.repository.update_account_identity_and_capabilities(
    account_id,
    client.own_external_id().as_deref(),
    &ConversationCapabilities::zalo_phase_one(),
    &utc_now(),
)?;
```

Text-only fallback capabilities remain available for non-Zalo accounts.

- [ ] **Step 7: Add recall preparation and completion**

Service methods:

```rust
pub fn prepare_recall(&self, message_id: i64) -> ConversationResult<PreparedRecall>;
pub fn complete_recall(&self, message_id: i64, now: &str) -> ConversationResult<ConversationMessage>;
```

`PreparedRecall` contains account key, thread, group flag, and provider IDs. Validation happens before the provider call. Completion sets `status='removed'` and `removed_at` transactionally and publishes invalidation.

- [ ] **Step 8: Add all rich commands**

Commands and return types:

```rust
stage_conversation_attachment_paths(...) -> Result<Vec<StagedConversationAttachment>, ConversationCommandError>
stage_conversation_clipboard_image(...) -> Result<StagedConversationAttachment, ConversationCommandError>
discard_conversation_staged_attachment(...) -> Result<(), ConversationCommandError>
send_conversation_batch(...) -> Result<SendConversationBatchResult, ConversationCommandError>
search_zalo_stickers(...) -> Result<Vec<ZaloStickerReference>, ConversationCommandError>
recall_conversation_message(...) -> Result<ConversationMessage, ConversationCommandError>
```

The search command resolves the authoritative account from `conversation_id`. The recall command prepares, fetches the connected client, calls provider undo, then completes the tombstone.

- [ ] **Step 9: Run cache, supervisor, and command tests GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::sticker_cache_test
cargo test --manifest-path src-tauri/Cargo.toml connected_supervisor_persists_client_own_external_id
cargo test --manifest-path src-tauri/Cargo.toml reaction_projection_marks_current_account_actor
cargo test --manifest-path src-tauri/Cargo.toml recall_
cargo test --manifest-path src-tauri/Cargo.toml conversation::commands_test
```

Expected: PASS.

---

## Task 11: Register Rich Commands and Startup Cleanup

**Files:**
- Modify: `src-tauri/src/lib.rs:120`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Run startup impact again**

```text
mcp__gitnexus.impact({target: "run", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Use application-local data for the spool**

Keep the existing database location unchanged. Add:

```rust
let conversation_spool_dir = app
    .path()
    .app_local_data_dir()
    .expect("Failed to get app local data directory")
    .join("conversation-spool");
```

Pass this path into `ConversationService::new`.

- [ ] **Step 3: Register rich commands**

Add all six rich commands to `tauri::generate_handler!` immediately after the existing Conversation commands.

- [ ] **Step 4: Run startup and periodic cleanup**

Extend the startup task:

```rust
if let Err(error) = startup_service.cleanup_staging().await {
    tracing::error!(target: "conversation", %error, "staging cleanup failed");
}
```

Spawn one 15-minute periodic loop using a cloned service. The loop exits when the Tauri runtime shuts down; each pass is idempotent.

- [ ] **Step 5: Verify app registration**

Run:

```bash
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml conversation::commands_test
```

Expected: PASS with no duplicate command names or missing managed state.

- [ ] **Step 6: Backend completion checkpoint**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

Then:

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Expected: approved backend scope only.

---

## Task 12: Add Frontend Rich Contracts and Platform Bridges

**Files:**
- Modify: `src/lib/services/conversationService.ts:1`
- Modify: `src/lib/services/__tests__/conversationService.spec.ts`
- Create: `src/lib/services/conversationPlatform.ts`
- Create: `src/lib/services/__tests__/conversationPlatform.spec.ts`

- [ ] **Step 1: Run interface impact**

```text
mcp__gitnexus.impact({target: "ConversationMessage", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing IPC tests**

Assert exact invocations:

```ts
await sendConversationBatch({
  conversationId: 31,
  clientBatchId: 'batch-1',
  replyToMessageId: 9,
  parts: [
    { clientPartId: 'part-1', type: 'text', text: 'Xin chào' },
    { clientPartId: 'part-2', type: 'attachment', stagedAttachmentId: 'stage-1' }
  ]
});

expect(invokedCommands).toContainEqual({
  command: 'send_conversation_batch',
  args: { request: expect.any(Object) }
});
```

Add equivalent tests for stage paths, clipboard image, discard, sticker search, and recall.

- [ ] **Step 3: Write failing platform tests**

Mock `@tauri-apps/plugin-dialog` and `@tauri-apps/api/webview`. Verify:

- Multi-file dialog returns `string[]` and normalizes cancel to `[]`.
- `onDragDropEvent` forwards only `drop` paths and toggles enter/leave state.
- Clipboard image converts to base64 without including a data-URL prefix.
- Clipboard helper rejects files over 10 MiB before invoking Rust.

- [ ] **Step 4: Run tests RED**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Add exact TypeScript contracts**

Mirror every Rust contract from Task 3. Extend `ConversationMessage` with required rich fields and extend reply targets/runtime accounts with `capabilities`.

Define the UI-owned reply selection separately from the backend reply preview:

```ts
export type ConversationReplySelection = {
  messageId: number;
  conversationId: number;
  channelAccountId: number;
  preview: ConversationReplyPreview;
};
```

Add wrappers:

```ts
export const stageConversationAttachmentPaths = (request: StageConversationAttachmentPathsRequest) =>
  invoke<StagedConversationAttachment[]>('stage_conversation_attachment_paths', { request });

export const stageConversationClipboardImage = (request: StageConversationClipboardImageRequest) =>
  invoke<StagedConversationAttachment>('stage_conversation_clipboard_image', { request });

export const discardConversationStagedAttachment = (stagedAttachmentId: string) =>
  invoke<void>('discard_conversation_staged_attachment', {
    request: { stagedAttachmentId }
  });

export const sendConversationBatch = (request: SendConversationBatchRequest) =>
  invoke<SendConversationBatchResult>('send_conversation_batch', { request });

export const searchZaloStickers = (conversationId: number, query: string, limit = 50) =>
  invoke<ZaloStickerReference[]>('search_zalo_stickers', {
    request: { conversationId, query, limit }
  });

export const recallConversationMessage = (messageId: number) =>
  invoke<ConversationMessage>('recall_conversation_message', {
    request: { messageId }
  });
```

Add `normalizeConversationError(error)` that accepts serialized `{code,message}` or legacy strings.

- [ ] **Step 6: Implement platform bridge**

Use official Tauri 2 APIs:

```ts
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';

export async function pickConversationAttachmentPaths(): Promise<string[]> {
  const result = await open({ multiple: true, directory: false });
  return Array.isArray(result) ? result : result ? [result] : [];
}

export async function listenConversationDragDrop(
  handler: (state: { active: boolean; paths?: string[] }) => void
) {
  return getCurrentWebview().onDragDropEvent(({ payload }) => {
    if (payload.type === 'enter') handler({ active: true });
    if (payload.type === 'leave') handler({ active: false });
    if (payload.type === 'drop') handler({ active: false, paths: payload.paths });
  });
}
```

Implement clipboard base64 with chunked string conversion to avoid call-stack overflow.

- [ ] **Step 7: Update all test fixtures with rich defaults**

Every `ConversationMessage` fixture receives:

```ts
providerClientMessageId: 'client-1',
attachments: [],
replyPreview: null,
reactions: [],
sendBatchId: null,
batchIndex: null,
sendBatch: null
```

Every reply target/runtime account receives explicit text-only or Zalo Phase 1 capabilities.

- [ ] **Step 8: Run service/platform tests GREEN**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts
bun run check
```

Expected: PASS.

---

## Task 13: Add Composer Draft State and Attachment Queue

**Files:**
- Create: `src/lib/components/conversation/composerDraftState.ts`
- Create: `src/lib/components/conversation/__tests__/composerDraftState.spec.ts`
- Create: `src/lib/components/conversation/AttachmentPreviewQueue.svelte`
- Create: `src/lib/components/conversation/AttachmentDropOverlay.svelte`
- Create: `src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts`
- Modify: `src/lib/components/conversation/MessageComposer.svelte:1`
- Modify: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`

- [ ] **Step 1: Run composer file impact**

```text
mcp__gitnexus.impact({target: "src/lib/components/conversation/MessageComposer.svelte", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing pure draft tests**

Test:

```ts
expect(composerDraftKey(31, 4)).toBe('31:4');

const cache = new ComposerDraftCache();
cache.update('31:4', { text: 'A' });
cache.update('32:4', { text: 'B' });
expect(cache.get('31:4').text).toBe('A');
expect(cache.get('32:4').text).toBe('B');

expect(insertTextAtSelection('hello', '😀', 2, 4)).toEqual({
  value: 'he😀o',
  caret: 4
});
```

Also test snapshot clearing: content typed after a send snapshot remains.

- [ ] **Step 3: Write failing queue/component tests**

Verify:

- Staging, ready, error, and removing states render.
- Image preview uses only backend `previewDataUrl`.
- Remove callback receives the opaque stage ID.
- Send button stays disabled while any item is staging.
- Picker, drag-drop, and clipboard paths call the correct staging wrapper.

- [ ] **Step 4: Run tests RED**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/composerDraftState.spec.ts src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement draft cache**

Use:

```ts
export type ComposerAttachmentItem = {
  localId: string;
  stagedAttachmentId: string | null;
  fileName: string;
  mimeType: string;
  sizeBytes: number;
  attachmentType: string;
  previewDataUrl: string | null;
  status: 'staging' | 'ready' | 'error' | 'removing';
  errorMessage: string | null;
};

export type ComposerDraftSnapshot = {
  text: string;
  stagedAttachmentIds: string[];
};

export type ComposerDraft = {
  text: string;
  stagedAttachments: ComposerAttachmentItem[];
  activePopover: 'emoji' | 'sticker' | null;
};

export class ComposerDraftCache {
  private drafts = new Map<string, ComposerDraft>();
  get(key: string): ComposerDraft;
  set(key: string, value: ComposerDraft): void;
  update(key: string, patch: Partial<ComposerDraft>): ComposerDraft;
  clearSubmittedSnapshot(key: string, snapshot: ComposerDraftSnapshot): ComposerDraft;
}
```

Keep drafts in memory only. TTL cleanup handles files after app restart.

- [ ] **Step 6: Implement attachment UI**

`AttachmentPreviewQueue.svelte` receives:

```ts
items: ComposerAttachmentItem[];
onRemove: (id: string) => void | Promise<void>;
```

`AttachmentDropOverlay.svelte` receives `active` and displays the bounded overlay only over the Conversation workspace.

- [ ] **Step 7: Integrate staging sources into the composer**

In `MessageComposer.svelte`:

- Install and clean up the Tauri drag-drop listener.
- Use native dialog for multi-file selection.
- Handle clipboard image paste from the textarea.
- Insert temporary queue entries as `staging` before calling Rust.
- Replace them with returned `ready` metadata or an `error` state.
- Discard ready stage IDs on explicit remove.
- Enforce capability counts and bytes before send.

- [ ] **Step 8: Run draft and attachment tests GREEN**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/composerDraftState.spec.ts src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
bun run check
```

Expected: PASS.

---

## Task 14: Add Emoji, Live Sticker Picker, Quote Bar, and Batch Send

**Files:**
- Create: `src/lib/components/conversation/emojiData.ts`
- Create: `src/lib/components/conversation/ComposerToolbar.svelte`
- Create: `src/lib/components/conversation/EmojiPicker.svelte`
- Create: `src/lib/components/conversation/ZaloStickerPicker.svelte`
- Create: `src/lib/components/conversation/ComposerReplyBar.svelte`
- Create: `src/lib/components/conversation/__tests__/ComposerToolbar.spec.ts`
- Create: `src/lib/components/conversation/__tests__/ZaloStickerPicker.spec.ts`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`
- Modify: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`

- [ ] **Step 1: Write failing emoji and sticker tests**

Verify:

- Emoji click inserts at the textarea selection and restores focus.
- Toolbar hides sticker/attachment actions when capabilities deny them.
- Sticker search waits 300 ms, trims the query, and calls once.
- Stale sticker responses do not replace newer results.
- Selecting a sticker sends one sticker-only batch immediately.
- Sticker send failure keeps the picker open and exposes the normalized error.

- [ ] **Step 2: Write failing quote and batch tests**

Verify exact request order:

```ts
expect(sendConversationBatch).toHaveBeenCalledWith({
  conversationId: 31,
  clientBatchId: expect.any(String),
  replyToMessageId: 9,
  parts: [
    { clientPartId: expect.any(String), type: 'text', text: 'Xin chào' },
    { clientPartId: expect.any(String), type: 'attachment', stagedAttachmentId: 'stage-a' },
    { clientPartId: expect.any(String), type: 'attachment', stagedAttachmentId: 'stage-b' }
  ]
});
```

Also verify a failed enqueue preserves text, attachments, and quote.

- [ ] **Step 3: Run tests RED**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/ComposerToolbar.spec.ts src/lib/components/conversation/__tests__/ZaloStickerPicker.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
```

Expected: FAIL.

- [ ] **Step 4: Add a fixed Unicode emoji dataset**

`emojiData.ts` exports six exact categories: smileys, gestures, hearts, people, objects, and symbols. Use a local Unicode list; do not add a provider emoji dependency or Zalo assets.

Example contract:

```ts
export type EmojiCategory = { id: string; label: string; emojis: string[] };
export const EMOJI_CATEGORIES: EmojiCategory[] = [
  { id: 'smileys', label: 'Cảm xúc', emojis: ['😀','😃','😄','😁','😆','😅','😂','🤣','😊','😍','🥰','😘','😎','🤔','😭','😡'] },
  { id: 'gestures', label: 'Cử chỉ', emojis: ['👍','👎','👌','✌️','🤞','👏','🙏','💪','👋','🤝','🫶','🤟'] },
  { id: 'hearts', label: 'Trái tim', emojis: ['❤️','🧡','💛','💚','💙','💜','🖤','🤍','🤎','💔','💕','💖'] },
  { id: 'people', label: 'Con người', emojis: ['👨','👩','👦','👧','👶','👴','👵','👮','👷','💁','🙋','🙆'] },
  { id: 'objects', label: 'Đồ vật', emojis: ['🎉','🎁','🔥','⭐','✨','💡','📌','📞','📅','✅','❌','⚠️'] },
  { id: 'symbols', label: 'Ký hiệu', emojis: ['✔️','☑️','➡️','⬅️','⬆️','⬇️','➕','➖','❓','❗','💯','♻️'] }
];
```

- [ ] **Step 5: Implement toolbar and popovers**

Use shadcn Popover for Emoji and Sticker. `ComposerToolbar.svelte` emits `emoji`, `sticker`, and `attachment` actions and receives capabilities and busy state.

`ZaloStickerPicker.svelte` owns debounce and request cancellation tokens but receives `conversationId`, `disabled`, `onSelect`, and `search` as props for testability.

- [ ] **Step 6: Implement quote bar and target pinning input**

`ComposerReplyBar.svelte` renders sender, content summary, thumbnail, and cancel button.

Extend composer props:

```ts
replyTarget: ConversationReplySelection | null;
onCancelReply: () => void;
onTargetResolved?: (target: ConversationReplyTarget) => void;
onBatchQueued?: (result: SendConversationBatchResult) => void;
```

When `replyTarget` exists, select and lock its exact account/conversation.

- [ ] **Step 7: Replace text send with snapshot-based batch send**

Generate IDs with `crypto.randomUUID()` and a deterministic fallback for tests. Build text first, then ready attachments. Clear only the submitted snapshot after success.

Keep `sendConversationMessage` exported for compatibility, but `MessageComposer.svelte` must call only `sendConversationBatch`.

- [ ] **Step 8: Run composer tests GREEN**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/ComposerToolbar.spec.ts src/lib/components/conversation/__tests__/ZaloStickerPicker.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
bun run check
```

Expected: PASS.

---

## Task 15: Build Safe Rich Message Renderers

**Files:**
- Create: `src/lib/components/conversation/messageMedia.ts`
- Create: `src/lib/components/conversation/MessageContentRenderer.svelte`
- Create: `src/lib/components/conversation/ImageMessageContent.svelte`
- Create: `src/lib/components/conversation/FileMessageContent.svelte`
- Create: `src/lib/components/conversation/VideoMessageContent.svelte`
- Create: `src/lib/components/conversation/AudioMessageContent.svelte`
- Create: `src/lib/components/conversation/StickerMessageContent.svelte`
- Create: `src/lib/components/conversation/LinkMessageContent.svelte`
- Create: `src/lib/components/conversation/MessageReplyPreview.svelte`
- Create: `src/lib/components/conversation/__tests__/messageMedia.spec.ts`
- Create: `src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts`
- Modify: `src/lib/components/conversation/AttachmentPreviewQueue.svelte`
- Modify: `src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts`
- Modify: `src/lib/components/conversation/MessageBubble.svelte:1`
- Modify: `src/lib/components/conversation/__tests__/MessageBubble.spec.ts`

- [ ] **Step 1: Run bubble impact**

```text
mcp__gitnexus.impact({target: "src/lib/components/conversation/MessageBubble.svelte", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing media-safety tests**

Test the pure URL helper:

```ts
expect(safeConversationUrl('https://cdn.example/a.jpg')).toBe('https://cdn.example/a.jpg');
expect(safeConversationUrl('http://cdn.example/a.jpg')).toBe('http://cdn.example/a.jpg');
expect(safeConversationUrl('javascript:alert(1)')).toBeNull();
expect(safeConversationUrl('file:///etc/passwd')).toBeNull();
expect(safeConversationUrl('../secret')).toBeNull();
expect(safeConversationUrl('data:text/html;base64,PHNjcmlwdD4=')).toBeNull();

expect(safeImagePreviewDataUrl('data:image/png;base64,iVBORw0KGgo=')).toBe(
  'data:image/png;base64,iVBORw0KGgo='
);
expect(safeImagePreviewDataUrl('data:image/jpeg;base64,/9j/4AAQ')).toBe(
  'data:image/jpeg;base64,/9j/4AAQ'
);
expect(safeImagePreviewDataUrl('data:image/webp;base64,UklGRg==')).toBe(
  'data:image/webp;base64,UklGRg=='
);
expect(safeImagePreviewDataUrl('data:image/gif;base64,R0lGODlh')).toBe(
  'data:image/gif;base64,R0lGODlh'
);
expect(safeImagePreviewDataUrl('data:image/svg+xml;base64,PHN2Zz4=')).toBeNull();
expect(safeImagePreviewDataUrl('data:text/html;base64,PHNjcmlwdD4=')).toBeNull();
expect(safeImagePreviewDataUrl('file:///tmp/preview.png')).toBeNull();
expect(safeImagePreviewDataUrl('../preview.png')).toBeNull();
expect(safeImagePreviewDataUrl('javascript:alert(1)')).toBeNull();
```

This replaces the current permissive media URL logic. `safeImagePreviewDataUrl()` accepts only backend-generated `data:image/png|jpeg|webp|gif;base64,...` values; it rejects every other `data:` MIME type, `file:`, relative paths, and executable schemes.

- [ ] **Step 3: Write failing renderer tests**

Verify each `contentType` selects the correct renderer and:

- Image alt/caption.
- File name/size/link.
- Video has controls and safe source.
- Audio has controls and safe source.
- Sticker uses WebP fallback then provider URL.
- Link uses only provider-normalized metadata.
- Quote preview renders linked or fallback data.
- Removed message renders only the tombstone.

- [ ] **Step 4: Run tests RED**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/messageMedia.spec.ts src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts src/lib/components/conversation/__tests__/MessageBubble.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement safe URL and formatting helpers**

`safeConversationUrl()` accepts only absolute HTTP/HTTPS URLs. `safeImagePreviewDataUrl()` accepts only the four approved base64 image MIME types and returns `null` for all other schemes or formats. Add `formatBytes()` and `attachmentLabel()` as pure functions with tests.

Route composer thumbnails through `safeImagePreviewDataUrl()` before assigning `src`. Invalid preview data renders the existing non-image attachment fallback rather than a broken or unsafe image.

- [ ] **Step 6: Implement focused renderers**

Each renderer accepts normalized typed props and contains no channel checks. Unsupported or unsafe remote content renders a bounded fallback with filename/content type, never a raw path.

`LinkMessageContent.svelte` uses:

```html
<a href={safeUrl} target="_blank" rel="noreferrer noopener">...</a>
```

It does not fetch metadata.

- [ ] **Step 7: Reduce `MessageBubble` to shell behavior**

Keep direction, sender/account header, timestamp, tombstone, and status shell. Delegate normal content and quote preview to `MessageContentRenderer`.

- [ ] **Step 8: Run renderer tests GREEN**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/messageMedia.spec.ts src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts src/lib/components/conversation/__tests__/AttachmentPreviewQueue.spec.ts src/lib/components/conversation/__tests__/MessageBubble.spec.ts
bun run check
```

Expected: PASS.

---

## Task 16: Add Message Actions, Quote Flow, Recall Dialog, and Read-Only Reactions

**Files:**
- Create: `src/lib/components/conversation/MessageReactionSummary.svelte`
- Create: `src/lib/components/conversation/MessageActionsMenu.svelte`
- Create: `src/lib/components/conversation/RecallMessageDialog.svelte`
- Create: `src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts`
- Create: `src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts`
- Modify: `src/lib/components/conversation/MessageBubble.svelte`
- Modify: `src/lib/components/conversation/ConversationTimeline.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Write failing action eligibility tests**

Verify:

- Reply appears only when quote capability is true and both provider IDs exist.
- Copy appears for text or a safe URL.
- Recall appears only for sent outbound non-tombstone messages with both provider IDs and recall capability.
- Pending, failed, inbound, and removed messages cannot recall.
- Right-click and explicit `...` trigger the same action callbacks.
- Shift+F10 opens the context menu.

- [ ] **Step 2: Write failing reaction tests**

Verify:

- Emoji/count chips render.
- A current-account chip receives accent styling.
- Clicking a chip opens actor names/avatars/external-ID fallback.
- There is no add/remove reaction button.
- Tombstones do not show reactions.

- [ ] **Step 3: Write failing workspace quote/recall tests**

Verify:

- Reply pins the composer to the source message's exact conversation/account.
- Selecting another target is blocked until quote cancellation.
- Recall confirmation calls `recallConversationMessage(message.id)` once.
- Successful recall replaces the cached message with the returned tombstone.
- Failed recall leaves the cached message and displays the normalized error.

- [ ] **Step 4: Run tests RED**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement actions and reaction popover**

`MessageActionsMenu.svelte` receives explicit booleans and callbacks. It does not calculate capabilities from channel names.

Use ContextMenu for right-click and DropdownMenu for the explicit button. Use the same action descriptor list for both menus to prevent drift.

`MessageReactionSummary.svelte` uses Popover and renders read-only actor data.

- [ ] **Step 6: Implement workspace quote state**

Add:

```ts
let replySelection = $state<ConversationReplySelection | null>(null);
let recallCandidate = $state<ConversationMessage | null>(null);
let recallError = $state<string | null>(null);
```

`ConversationTimeline` forwards `onReply` and `onRecall`. `MessageComposer` receives reply selection and cancel callback.

Clear reply selection when the selected inbox item changes or after successful batch enqueue.

- [ ] **Step 7: Implement recall confirmation**

`RecallMessageDialog.svelte` uses the existing Dialog primitive and emits confirm/cancel. Disable confirm while the request is running.

- [ ] **Step 8: Run action/reaction tests GREEN**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts src/lib/components/conversation/__tests__/MessageActionsMenu.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

---

## Task 17: Group Batch Parts and Preserve Timeline Behavior

**Files:**
- Create: `src/lib/components/conversation/MessageBatchGroup.svelte`
- Create: `src/lib/components/conversation/__tests__/MessageBatchGroup.spec.ts`
- Modify: `src/lib/components/conversation/timelineState.ts:1`
- Modify: `src/lib/components/conversation/__tests__/timelineState.spec.ts`
- Modify: `src/lib/components/conversation/ConversationTimeline.svelte:1`
- Modify: `src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte:429`

- [ ] **Step 1: Write failing grouping tests**

Add a pure helper test:

```ts
const grouped = groupTimelineItems([
  makeMessage(1, { sendBatchId: 'batch-a', batchIndex: 0 }),
  makeMessage(2, { sendBatchId: 'batch-a', batchIndex: 1 }),
  makeMessage(3, { sendBatchId: null, batchIndex: null })
]);

expect(grouped).toEqual([
  { kind: 'batch', batchId: 'batch-a', messages: expect.any(Array) },
  { kind: 'message', message: expect.objectContaining({ id: 3 }) }
]);
```

Also verify non-consecutive equal batch IDs are not moved across intervening messages.

Add boundary-expansion tests using an end-exclusive window:

```ts
const items = [
  makeMessage(1, { sendBatchId: 'batch-a', batchIndex: 0 }),
  makeMessage(2, { sendBatchId: 'batch-a', batchIndex: 1 }),
  makeMessage(3, { sendBatchId: 'batch-a', batchIndex: 2 }),
  makeMessage(4, { sendBatchId: null, batchIndex: null }),
  makeMessage(5, { sendBatchId: 'batch-b', batchIndex: 0 }),
  makeMessage(6, { sendBatchId: 'batch-b', batchIndex: 1 }),
  makeMessage(7, { sendBatchId: 'batch-b', batchIndex: 2 })
];

expect(expandWindowForBatchBoundaries(items, 1, 6)).toEqual({ start: 0, end: 7 });
expect(expandWindowForBatchBoundaries(items, 3, 4)).toEqual({ start: 3, end: 4 });
```

Also verify the helper expands only contiguous parts at each edge and never bridges an intervening message that happens to reuse the same batch ID.

- [ ] **Step 2: Write failing batch status tests**

Verify `MessageBatchGroup` renders one footer:

```text
queued  -> Đang chờ gửi
sending -> Đang gửi
partial -> Đã gửi một phần
sent    -> Đã gửi
failed  -> Gửi thất bại
```

Failed parts show a warning without duplicating the aggregate footer.

- [ ] **Step 3: Write failing optimistic merge tests**

Verify `handleBatchQueued` merges all returned messages, updates inbox preview from the last part with text/file fallback, and scrolls once.

Verify backend refresh replaces matching message IDs without losing unsynced queued parts.

- [ ] **Step 4: Run tests RED**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/timelineState.spec.ts src/lib/components/conversation/__tests__/MessageBatchGroup.spec.ts src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement adjacent batch grouping**

Add:

```ts
export type TimelineRenderItem =
  | { kind: 'message'; message: ConversationMessage }
  | { kind: 'batch'; batchId: string; messages: ConversationMessage[] };

export function expandWindowForBatchBoundaries(
  items: ConversationMessage[],
  startInclusive: number,
  endExclusive: number
): { start: number; end: number };

export function groupTimelineItems(items: ConversationMessage[]): TimelineRenderItem[];
```

Sort parts inside each adjacent group by `batchIndex`, then message ID. Do not mutate the input list.

- [ ] **Step 6: Preserve virtualization and anchors**

Calculate the existing bounded virtual window first, pass its start/end indexes through `expandWindowForBatchBoundaries()`, then slice and group. Expand the start backward through all contiguous parts sharing the first visible item's batch ID and expand the end forward through all contiguous parts sharing the last visible item's batch ID.

The configured batch limit is 10 parts, so each boundary adds at most the remainder of one 10-part batch. Every nested bubble retains its own `data-message-id` so existing anchor capture/restore remains valid.

- [ ] **Step 7: Replace single-message optimistic callback**

Change:

```ts
onQueued?: (message: ConversationMessage) => void;
```

to:

```ts
onBatchQueued?: (result: SendConversationBatchResult) => void;
```

Workspace merges `result.messages` in one update and scrolls once.

- [ ] **Step 8: Run timeline tests GREEN**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/timelineState.spec.ts src/lib/components/conversation/__tests__/MessageBatchGroup.spec.ts src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

- [ ] **Step 9: Frontend checkpoint**

Run:

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Expected: approved Conversation service/component flows only.

---

## Task 18: Full Verification, Security Audit, and Live Zalo Gate

**Files:**
- Modify only if validation exposes defects in files already listed by this plan.
- Do not expand Phase 1 scope.

- [ ] **Step 1: Run formatting and targeted Rust suites**

Run:

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

Expected: PASS.

- [ ] **Step 2: Run full Rust validation**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: PASS. If unrelated pre-existing failures exist, record their exact command, file, and error without modifying unrelated modules.

- [ ] **Step 3: Run frontend targeted and full validation**

Run:

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts src/lib/components/conversation/__tests__
bun run check
bun test --conditions=browser --conditions=svelte
bun run build
```

Expected: PASS.

- [ ] **Step 4: Audit security invariants**

Run repository searches:

```bash
rg -n "file://|javascript:|local_path|original.*path|cookies_json" src/lib/components/conversation src/lib/services src-tauri/src/conversation
rg -n "channel === ['\"]zalo|channel == ['\"]zalo" src/lib/components/conversation
rg -n "add_reaction|remove_reaction|forward|voice-recorder|quick-template|mention" src/lib/components/conversation src-tauri/src/conversation
```

Expected:

- No local path or auth data reaches frontend contracts/errors.
- No presentation component gates behavior through direct Zalo channel checks.
- No excluded Phase 1 write features were added.

- [ ] **Step 5: Run GitNexus final scope audit**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Review every changed symbol and affected process. Confirm all d=1 dependents from impact runs were updated.

- [ ] **Step 6: Execute the non-production live Zalo smoke gate**

Use a dedicated test account, consenting recipient, and non-sensitive files. Verify in order:

1. Text-only batch.
2. Image send.
3. PDF/Office/ZIP file send.
4. Video and audio file send.
5. Text plus three attachments.
6. Live sticker search and send.
7. Quote an incoming message.
8. Quote an outbound message after echo reconciliation.
9. Recall a sent message and confirm tombstone.
10. Render reaction counts, actors, and current-account marker.
11. Interrupt network during a multi-part batch and confirm `partial` without resending successful parts.
12. Restart with staged and pending files and confirm safe recovery.
13. Confirm realtime/history echo creates no duplicate bubbles.

- [ ] **Step 7: Verify rollback behavior**

Disable rich capabilities for the test account and restart the UI.

Expected:

- Rich controls disappear or disable.
- Existing text send compatibility remains functional.
- Additive schema remains readable.
- No migration rollback or column removal is required.

- [ ] **Step 8: Final diff review**

Run:

```bash
git diff --check
git status --short
git diff --stat
```

Expected: only files listed in this plan plus generated shadcn primitives and lockfile changes. Do not commit without explicit authorization.

---

## Acceptance Checklist

- [ ] One composer action creates one logical batch.
- [ ] Text and each attachment have independent durable outbox jobs.
- [ ] A successful part is never resent because a sibling failed.
- [ ] Staged files survive restart and never depend on original source paths.
- [ ] Client and provider IDs reconcile echoes without content/time heuristics.
- [ ] Quote Reply uses exact provider message/client IDs and never silently degrades.
- [ ] Live Zalo sticker search is cached briefly and not hard-coded.
- [ ] Recall updates a tombstone only after provider success or confirmed idempotent completion.
- [ ] Reaction display is read-only and marks the current account correctly.
- [ ] Rich rendering rejects unsafe local, data, and script URLs.
- [ ] UI behavior is capability-driven rather than channel-name-driven.
- [ ] Non-Zalo channels retain existing text behavior.
- [ ] All targeted and full validation gates pass or unrelated baseline failures are documented.
- [ ] GitNexus final change detection matches the approved Conversation/Zalo/UI scope.
