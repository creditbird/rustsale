# Zalo Rich Chat Phase 2B Voice and Conversation Media Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add safe native Zalo voice recording/sending and a paginated per-conversation Image, Video, Audio/Voice, File, and Link library without introducing a second media store.

**Architecture:** Capture audio through Web `MediaRecorder`, stage bytes into the Phase 1 Rust-owned spool, enqueue one native voice outbox part with a stable client ID, and reconcile provider echoes exactly. Build the media panel as a read model over canonical Phase 1 messages/attachments with stable cursors, count parity, bounded jump-to-message context, and capability-driven desktop/Sheet presentation.

**Tech Stack:** Rust 2021, Tauri 2, SQLite/rusqlite, vendored `zca-rs`, Tokio, base64, SHA-256, Svelte 5 runes, TypeScript, Web MediaRecorder API, shadcn-svelte, Bun test runner, Testing Library, Tailwind CSS 4.

---

## Source of Truth

- Approved Phase 2B design: `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-2b-voice-media-design.md`
- Required Phase 1 design: `docs/superpowers/specs/2026-08-02-zalo-rich-chat-phase-1-design.md`
- Required Phase 1 plan: `docs/superpowers/plans/2026-08-02-zalo-rich-chat-phase-1-implementation-plan.md`

Phase 2B does not depend on Phase 2A. Do not execute it until Phase 1 implementation and acceptance gates are complete.

## Execution Rules

- Use `bun` and `bunx`; never run `npm` or `npx`.
- Read Agent Memory before implementation.
- Use `superpowers:using-git-worktrees` if isolation is requested.
- Run GitNexus impact before every existing symbol edit; warn and stop on HIGH/CRITICAL.
- Run GitNexus change detection at provider, backend, frontend, and final checkpoints.
- Preserve unrelated working-tree changes.
- Use `apply_patch` for manual edits.
- Do not commit unless explicitly authorized.
- Use only non-production accounts, consenting recipients, and non-sensitive audio/files for probes.
- Do not add transcoding or bundle FFmpeg.

## Planning-Time GitNexus Snapshot

| Symbol | Risk | Direct dependents | Execution note |
|---|---:|---:|---|
| `migrate` | LOW | 0 | Re-run before metadata/index changes |
| `ZaloClient` | LOW | 1 | `RealZaloClient`; all fakes require the new method |
| `process_next_outbox_job` | LOW | 0 indexed | Phase 1 direct tests remain authoritative |
| `ConversationMessage` | LOW | 1 | `timelineState.ts`; inspect Svelte consumers manually |

## Required File Map

### Backend files to create

```text
src-tauri/src/conversation/voice.rs
src-tauri/src/conversation/voice_test.rs
src-tauri/src/conversation/media_library.rs
src-tauri/src/conversation/media_library_test.rs
src-tauri/src/conversation/zalo/phase2b_probe.rs
src-tauri/src/conversation/zalo/phase2b_probe_test.rs
src-tauri/src/conversation/zalo/fixtures/phase2b/voice_direct_echo.json
src-tauri/src/conversation/zalo/fixtures/phase2b/voice_group_echo.json
src-tauri/src/conversation/zalo/fixtures/phase2b/voice_history.json
src-tauri/src/conversation/zalo/fixtures/phase2b/voice_runtime_matrix.json
src-tauri/src/conversation/zalo/fixtures/phase2b/voice_upload_metadata.json
src-tauri/src/conversation/zalo/fixtures/phase2b/voice_boundary_matrix.json
```

### Backend files to modify

```text
src-tauri/src/lib.rs
src-tauri/src/conversation/mod.rs
src-tauri/src/conversation/schema.sql
src-tauri/src/conversation/migration.rs
src-tauri/src/conversation/migration_test.rs
src-tauri/src/conversation/types.rs
src-tauri/src/conversation/error.rs
src-tauri/src/conversation/adapter.rs
src-tauri/src/conversation/staging.rs
src-tauri/src/conversation/staging_test.rs
src-tauri/src/conversation/repository.rs
src-tauri/src/conversation/repository_test.rs
src-tauri/src/conversation/projection.rs
src-tauri/src/conversation/service.rs
src-tauri/src/conversation/service_test.rs
src-tauri/src/conversation/commands.rs
src-tauri/src/conversation/commands_test.rs
src-tauri/src/conversation/zalo/mod.rs
src-tauri/src/conversation/zalo/client.rs
src-tauri/src/conversation/zalo/client_test.rs
src-tauri/src/conversation/zalo/normalizer.rs
src-tauri/src/conversation/zalo/normalizer_test.rs
src-tauri/vendor/zca-rs/src/models/message.rs
src-tauri/vendor/zca-rs/src/apis/message.rs
```

### Frontend files to create

```text
src/lib/components/conversation/voiceRecorderState.ts
src/lib/components/conversation/voicePlaybackCoordinator.ts
src/lib/components/conversation/conversationMediaState.ts
src/lib/components/conversation/VoiceRecorder.svelte
src/lib/components/conversation/VoiceRecordingPreview.svelte
src/lib/components/conversation/VoiceMessageContent.svelte
src/lib/components/conversation/ConversationMediaPanel.svelte
src/lib/components/conversation/ConversationMediaSheet.svelte
src/lib/components/conversation/ConversationMediaGrid.svelte
src/lib/components/conversation/ConversationMediaList.svelte
src/lib/components/conversation/ConversationMediaPreview.svelte
src/lib/components/conversation/__tests__/voiceRecorderState.spec.ts
src/lib/components/conversation/__tests__/voicePlaybackCoordinator.spec.ts
src/lib/components/conversation/__tests__/conversationMediaState.spec.ts
src/lib/components/conversation/__tests__/VoiceRecorder.spec.ts
src/lib/components/conversation/__tests__/VoiceMessageContent.spec.ts
src/lib/components/conversation/__tests__/ConversationMediaPanel.spec.ts
src/lib/components/conversation/__tests__/ConversationMediaPreview.spec.ts
```

### Frontend files to modify

```text
src/lib/services/conversationService.ts
src/lib/services/conversationPlatform.ts
src/lib/services/__tests__/conversationService.spec.ts
src/lib/services/__tests__/conversationPlatform.spec.ts
src/lib/components/conversation/composerDraftState.ts
src/lib/components/conversation/MessageComposer.svelte
src/lib/components/conversation/MessageContentRenderer.svelte
src/lib/components/conversation/ConversationTimeline.svelte
src/lib/components/conversation/ConversationWorkspace.svelte
src/lib/components/conversation/__tests__/composerDraftState.spec.ts
src/lib/components/conversation/__tests__/MessageComposer.spec.ts
src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts
src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts
src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

---

## Task 0: Verify Phase 1 and Establish Baseline

**Files:** None.

- [ ] **Step 1: Read memory and inspect worktree**

```text
mcp__agentmemory.memory_recall({query: "RustSale_v2 Zalo rich chat Phase 1 Phase 2B voice media staging outbox decisions", format: "narrative", limit: 10})
```

```bash
git status --short
git log -12 --oneline --decorate
```

Record all pre-existing changes.

- [ ] **Step 2: Verify required Phase 1 artifacts**

```bash
test -f src-tauri/src/conversation/staging.rs
test -f src-tauri/src/conversation/projection.rs
test -f src-tauri/src/conversation/retry.rs
test -f src/lib/components/conversation/AudioMessageContent.svelte
test -f src/lib/components/conversation/MessageContentRenderer.svelte
test -f src/lib/services/conversationPlatform.ts
rg -n "conversation-spool|send_conversation_batch|provider_client_message_id|safeConversationUrl" src-tauri/src/conversation src/lib/components/conversation src/lib/services
```

Expected: every command succeeds. Otherwise stop and execute Phase 1 first.

- [ ] **Step 3: Run Phase 1 regression baseline**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts src/lib/components/conversation/__tests__
bun run check
```

Expected: PASS or exact unrelated failures documented.

- [ ] **Step 4: Re-run impact gates**

Run impact for `migrate`, `ZaloClient`, Phase 1 staging functions, `process_next_outbox_job`, projection loaders, `ConversationMessage`, and every existing Svelte file before edit.

---

## Task 1: Capture Runtime and Native Voice Provider Contracts

**Files:**
- Create: `src-tauri/src/conversation/zalo/phase2b_probe.rs`
- Create: `src-tauri/src/conversation/zalo/phase2b_probe_test.rs`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2b/voice_direct_echo.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2b/voice_group_echo.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2b/voice_history.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2b/voice_runtime_matrix.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2b/voice_upload_metadata.json`
- Create: `src-tauri/src/conversation/zalo/fixtures/phase2b/voice_boundary_matrix.json`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`

- [ ] **Step 1: Write failing fixture tests**

| Test | Required assertion |
|---|---|
| `voice_direct_fixture_exposes_exact_ids` | provider and client IDs non-empty |
| `voice_group_fixture_exposes_exact_ids` | group IDs and thread scope normalize |
| `voice_history_fixture_matches_realtime_identity` | same message reconciles by exact IDs |
| `voice_fixture_exposes_duration_and_remote_url` | duration positive; safe remote metadata present |
| `voice_fixture_maps_content_type` | normalized content type is `voice` |
| `voice_runtime_matrix_has_supported_candidate_per_platform` | every supported desktop runtime has a recording/preview/provider-approved MIME |
| `voice_upload_metadata_uses_stable_id_and_duration_unit` | request metadata contains caller ID, MIME, extension, bytes, and duration unit |
| `voice_boundary_matrix_covers_application_limits` | empty, one-second, five-minute, below/above 5 MiB results recorded |
| `phase2b_probe_fixtures_are_sanitized` | no audio bytes, local paths, cookies, tokens, or account session data |

- [ ] **Step 2: Run fixture tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_probe_test
```

Expected: FAIL.

- [ ] **Step 3: Add probe summary types**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceProbeSummary {
    pub direct_verified: bool,
    pub group_verified: bool,
    pub supported_mime_types: Vec<String>,
    pub duration_unit: String,
    pub stable_client_id_verified: bool,
}
```

- [ ] **Step 4: Run desktop/runtime probe matrix**

For Linux, Windows, and macOS, record and preview 10-second samples using:

```text
audio/webm;codecs=opus
audio/webm
audio/ogg;codecs=opus
audio/mp4
```

Record `isTypeSupported`, actual Blob MIME, generated extension, bytes, duration, and preview result in `voice_runtime_matrix.json`. No real audio fixture enters source control; only sanitized metadata does.

- [ ] **Step 5: Run native direct/group provider probe**

Send each runtime-supported format through native voice API. Verify duration units, stable client ID placement, response IDs, realtime echo, and history echo; store sanitized request metadata in `voice_upload_metadata.json`. If a supported desktop platform has no provider-compatible MIME, keep voice capability disabled and stop release work for that platform.

- [ ] **Step 6: Run provider boundary probe**

For every provider-approved MIME, test empty, one-second, five-minute, just-below-5-MiB, and just-above-5-MiB recordings. Record sanitized results in `voice_boundary_matrix.json` and keep the application limits fixed at 300,000 ms and 5 MiB even if the provider accepts larger payloads.

- [ ] **Step 7: Run fixture tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_probe_test
```

Expected: PASS.

---

## Task 2: Add Caller-Controlled Native Voice API to `zca-rs`

**Files:**
- Modify: `src-tauri/vendor/zca-rs/src/models/message.rs`
- Modify: `src-tauri/vendor/zca-rs/src/apis/message.rs`

- [ ] **Step 1: Run provider symbol impact/context**

Inspect upload helpers, response normalization, and any existing audio/file send paths.

- [ ] **Step 2: Write failing vendored tests**

| Test | Required assertion |
|---|---|
| `phase2b_voice_direct_uses_caller_client_id` | exact direct request ID |
| `phase2b_voice_group_uses_caller_client_id` | exact group request ID |
| `phase2b_voice_duration_uses_probe_unit` | deterministic conversion |
| `phase2b_voice_rejects_empty_bytes` | typed payload error before network |
| `phase2b_voice_response_normalizes_ids` | receipt has message/client IDs |
| `phase2b_existing_send_methods_remain_compatible` | prior public calls compile/pass |

- [ ] **Step 3: Run vendored tests RED**

```bash
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2b_voice_
```

Expected: FAIL.

- [ ] **Step 4: Add exact types and method**

```rust
pub struct SendVoiceRequest {
    pub client_id: String,
    pub source: AttachmentSource,
    pub duration_ms: u64,
    pub thread_id: String,
    pub thread_type: ThreadType,
}

pub struct SendVoiceReceipt {
    pub msg_id: String,
    pub cli_msg_id: String,
}

pub async fn send_voice(&self, request: SendVoiceRequest) -> Result<SendVoiceReceipt, ZaloError>;
```

Use the probe's duration unit and endpoint shape. Preserve caller client ID exactly.

- [ ] **Step 5: Extract pure payload builder**

Test request JSON/multipart metadata without network. Accept only buffer/file sources already controlled by Rust; no arbitrary frontend path reaches this API.

- [ ] **Step 6: Run vendored tests GREEN**

```bash
cargo fmt --manifest-path src-tauri/vendor/zca-rs/Cargo.toml -- --check
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2b_voice_
```

Expected: PASS.

- [ ] **Step 7: Provider checkpoint**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

---

## Task 3: Add Phase 2B Contracts, Capabilities, and Errors

**Files:**
- Modify: `src-tauri/src/conversation/types.rs`
- Modify: `src-tauri/src/conversation/error.rs`
- Modify: `src-tauri/src/conversation/adapter.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`

- [ ] **Step 1: Run shared-contract impact**

Run impact for `ConversationCapabilities`, `ConversationSendPartRequest`, `ConversationMessage`, `ConversationAttachment`, `ConversationAdapter`, and `ConversationErrorCode`.

- [ ] **Step 2: Write failing serialization/capability tests**

Assert exact camelCase JSON for stage voice, voice part, media cursors/items/pages/counts, message context, and capabilities. Verify provider MIME list, 300,000 ms, 5 MiB, page size 40, and text-only disabled defaults.

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_contract
```

Expected: FAIL.

- [ ] **Step 4: Add exact types**

```rust
pub struct StageConversationVoiceRequest {
    pub conversation_id: i64,
    pub bytes_base64: String,
    pub mime_type: String,
    pub duration_ms: u64,
}

pub struct ConversationVoicePartRequest {
    pub client_part_id: String,
    pub staged_attachment_id: String,
    pub duration_ms: u64,
}

pub enum ConversationMediaTabKind { Image, Video, Audio, File, Link }
pub enum ConversationMediaItemKind { Image, Video, Audio, Voice, File, Link }
pub enum ConversationMediaDirection { Inbound, Outbound }

pub struct StagedConversationVoice {
    pub staged_attachment_id: String,
    pub file_name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub duration_ms: u64,
    pub preview_data_url: Option<String>,
}

pub struct ConversationMediaCursor {
    pub occurred_at: String,
    pub message_id: i64,
    pub attachment_id: Option<i64>,
}

pub struct ConversationMediaItem {
    pub kind: ConversationMediaItemKind,
    pub conversation_id: i64,
    pub message_id: i64,
    pub attachment_id: Option<i64>,
    pub direction: ConversationMediaDirection,
    pub sender_display_name: Option<String>,
    pub occurred_at: String,
    pub file_name: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub thumbnail_url: Option<String>,
    pub remote_url: Option<String>,
    pub link_title: Option<String>,
    pub link_description: Option<String>,
    pub link_domain: Option<String>,
}

pub struct ListConversationMediaRequest {
    pub conversation_id: i64,
    pub kind: ConversationMediaTabKind,
    pub cursor: Option<ConversationMediaCursor>,
    pub limit: Option<usize>,
}

pub struct ConversationMediaPage {
    pub items: Vec<ConversationMediaItem>,
    pub next_cursor: Option<ConversationMediaCursor>,
}

pub struct ConversationMediaCounts {
    pub image: u64,
    pub video: u64,
    pub audio: u64,
    pub file: u64,
    pub link: u64,
}

pub struct GetConversationMediaCountsRequest {
    pub conversation_id: i64,
}

pub struct GetConversationMessageContextRequest {
    pub conversation_id: i64,
    pub message_id: i64,
    pub before: Option<usize>,
    pub after: Option<usize>,
}

pub struct ConversationMessageContext {
    pub anchor_message_id: i64,
    pub messages: Vec<ConversationMessage>,
}

pub struct ConversationVoiceRecordingCapabilities {
    pub enabled: bool,
    pub max_duration_ms: u64,
    pub max_bytes: u64,
    pub supported_mime_types: Vec<String>,
}

pub struct ConversationMediaLibraryCapabilities {
    pub enabled: bool,
    pub page_size: usize,
    pub kinds: Vec<ConversationMediaTabKind>,
}
```

Add `Voice(ConversationVoicePartRequest)` to the Phase 1 send-part enum. Add these fields to the Phase 1 declarations:

```rust
// ConversationCapabilities
pub voice_recording: ConversationVoiceRecordingCapabilities,
pub media_library: ConversationMediaLibraryCapabilities,

// ConversationAttachment
pub duration_ms: Option<u64>,
pub media_role: Option<String>,
```

`ConversationCapabilities::text_only()` sets both new capabilities to disabled with zero limits and empty vectors. The accepted Zalo constructor uses `enabled = true`, `max_duration_ms = 300_000`, `max_bytes = 5 * 1024 * 1024`, provider-probed MIME values, `page_size = 40`, and all five media tab kinds. If Phase 2A already exists, preserve reaction `supported`, `forward`, `mentions`, `quick_templates`, message actions, and mention children while merging these fields.

- [ ] **Step 5: Add stable errors and adapter method**

Extend `ConversationErrorCode` with the exact stable variants:

```rust
MicrophoneUnavailable,
MicrophonePermissionDenied,
MediaRecorderUnsupported,
VoiceMimeUnsupported,
VoiceDurationExceeded,
VoiceSizeExceeded,
VoiceEmpty,
VoiceStageInvalid,
VoiceSendUnsupported,
VoiceDeliveryUnknown,
MediaKindUnsupported,
MediaAnchorNotFound,
```

Extend the adapter:

```rust
async fn send_voice(&self, request: ProviderVoiceRequest) -> ConversationResult<ProviderSendReceipt>;
```

Define the provider request in `adapter.rs` and reuse Phase 1 `ProviderSendReceipt`:

```rust
pub struct ProviderVoiceRequest {
    pub client_part_id: String,
    pub claimed_path: PathBuf,
    pub mime_type: String,
    pub duration_ms: u64,
    pub thread_id: String,
    pub thread_type: ProviderThreadType,
}
```

Non-Zalo adapters return `VoiceSendUnsupported`.

- [ ] **Step 6: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_contract
```

Expected: PASS.

---

## Task 4: Add Voice Metadata and Media Query Indexes

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration.rs`
- Modify: `src-tauri/src/conversation/migration_test.rs`

- [ ] **Step 1: Run `migrate` impact**

```text
mcp__gitnexus.impact({target: "migrate", direction: "upstream", repo: "RustSale_v2", includeTests: true})
```

- [ ] **Step 2: Write failing migration tests**

| Test | Required assertions |
|---|---|
| `phase2b_adds_voice_stage_metadata` | nullable `duration_ms`, `media_role` |
| `phase2b_adds_media_indexes` | exact indexed columns/order |
| `phase2b_migration_is_idempotent` | two runs preserve data |
| `phase2b_preserves_phase1_stages` | prior rows remain readable |
| `phase2b_voice_constraints_are_enforced_in_service` | legacy rows may stay null |

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_migration_
```

Expected: FAIL.

- [ ] **Step 4: Add columns before indexes**

Use Phase 1 `add_column_if_missing`. Add/verify indexes:

```text
messages(conversation_id, removed_at, occurred_at DESC, id DESC)
message_attachments(message_id, attachment_type, id)
```

Do not add FTS.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_migration_
```

Expected: PASS.

---

## Task 5: Implement Safe Voice Staging

**Files:**
- Create: `src-tauri/src/conversation/voice.rs`
- Create: `src-tauri/src/conversation/voice_test.rs`
- Modify: `src-tauri/src/conversation/staging.rs`
- Modify: `src-tauri/src/conversation/staging_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Run staging impact/context**

Run impact for Phase 1 clipboard staging, staged-file inspection, claim, discard, and cleanup functions.

- [ ] **Step 2: Write failing voice validation tests**

Cover empty bytes, invalid base64, data-URL prefix rejection, MIME allowlist, MIME/extension mapping, 5 MiB decoded limit, 300,000 ms limit, zero duration, checksum, atomic file write, database rollback cleanup, and no returned local path.

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml stage_voice_
```

Expected: FAIL.

- [ ] **Step 4: Add constants and MIME mapping**

```rust
pub const MAX_VOICE_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_VOICE_DURATION_MS: u64 = 300_000;

pub fn voice_extension(mime: &str) -> Option<&'static str> {
    match mime {
        "audio/webm;codecs=opus" | "audio/webm" => Some("webm"),
        "audio/ogg;codecs=opus" | "audio/ogg" => Some("ogg"),
        "audio/mp4" => Some("m4a"),
        _ => None,
    }
}
```

Use only MIME values accepted by the provider/runtime probe.

- [ ] **Step 5: Implement streaming/capped decode and stage**

```rust
pub fn stage_voice(
    &self,
    request: StageConversationVoiceRequest,
) -> ConversationResult<StagedConversationVoice>;
```

Reject a `data:` prefix. Decode with an output cap, compute SHA-256, write a random temporary file, fsync/rename into `staged`, then insert the stage row with `media_role = 'voice'` and duration. Return normalized metadata and opaque ID only.

- [ ] **Step 6: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml stage_voice_
```

Expected: PASS.

---

## Task 6: Enqueue Voice-Only Batches Atomically

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`

- [ ] **Step 1: Run Phase 1 batch enqueue impact**

Stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing enqueue tests**

| Test | Required assertions |
|---|---|
| `voice_only_batch_claims_stage_and_creates_one_job` | one batch/message/job |
| `voice_batch_rejects_text` | transaction creates no rows |
| `voice_batch_rejects_attachment_or_sticker_sibling` | no rows |
| `voice_duration_must_match_staged_metadata` | mismatch fails |
| `voice_stage_can_be_claimed_once` | second request fails |
| `voice_enqueue_rollback_restores_stage` | file/database ownership compensated |

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml voice_
```

Expected: FAIL.

- [ ] **Step 4: Extend batch validator**

A voice request contains exactly one `Voice` part and no other parts. Persist `content_type = 'voice'`, duration metadata, claimed spool attachment, and stable client part ID.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml voice_
```

Expected: PASS.

---

## Task 7: Dispatch, Retry, and Reconcile Native Voice

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/zalo/client.rs`
- Modify: `src-tauri/src/conversation/zalo/client_test.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`
- Modify: `src-tauri/src/conversation/projection.rs`

- [ ] **Step 1: Run worker, client, and normalizer impact**

Run impact for `process_next_outbox_job`, `ZaloClient`, and Phase 1 echo reconciliation.

- [ ] **Step 2: Write failing worker/reconciliation tests**

```text
voice_worker_reads_claimed_spool_and_calls_native_api
voice_worker_uses_persisted_client_id
voice_retry_uses_same_client_id
voice_permanent_validation_error_fails_without_retry
voice_running_after_restart_becomes_delivery_unknown
voice_delivery_unknown_is_not_requeued
voice_echo_updates_local_part_without_duplicate
voice_history_echo_matches_same_part
voice_projection_contains_duration_and_safe_remote_url
```

- [ ] **Step 3: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml voice_
```

Expected: FAIL.

- [ ] **Step 4: Extend `ZaloClient` and every fake**

```rust
async fn send_voice(&self, request: NormalizedVoiceSendRequest) -> anyhow::Result<ProviderSendReceipt>;
```

`RealZaloClient` converts the claimed spool file into the vendored request. Test fakes capture bytes/path ownership, duration, thread, and stable ID.

- [ ] **Step 5: Add worker dispatch**

Use Phase 1 retry classification. Missing/modified claimed files are permanent errors. Network/rate limit errors are bounded retries. Running-after-restart becomes delivery unknown and is never requeued.

- [ ] **Step 6: Normalize voice echoes**

Create/update `message_attachments` with type `voice`, MIME, size, duration, safe remote URL, and provider metadata. Reconcile provider ID first, then client ID, otherwise insert; never content/time match.

- [ ] **Step 7: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml voice_
```

Expected: PASS.

---

## Task 8: Implement Conversation Media Read Model

**Files:**
- Create: `src-tauri/src/conversation/media_library.rs`
- Create: `src-tauri/src/conversation/media_library_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`
- Modify: `src-tauri/src/conversation/projection.rs`

- [ ] **Step 1: Write failing media query tests**

Required tests:

| Test | Required assertions |
|---|---|
| `image_tab_returns_only_safe_image_items` | correct kind/order |
| `audio_tab_combines_audio_and_voice` | exact item kind preserved |
| `file_tab_excludes_image_video_audio` | file mapping stable |
| `link_tab_uses_normalized_metadata_only` | no network fetch/raw HTML |
| `media_query_excludes_tombstones` | recalled items absent |
| `media_cursor_is_exclusive_and_stable` | no duplicate/skip across equal timestamps |
| `media_limit_is_clamped_to_forty` | 41 request returns at most 40 |
| `media_counts_match_list_predicates` | counts equal exhaustive pages |
| `unsafe_or_missing_remote_urls_use_null` | item remains bounded/safe |

- [ ] **Step 2: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml media_library_test
```

Expected: FAIL.

- [ ] **Step 3: Implement exact cursor helpers**

```rust
pub struct MediaCursor {
    pub occurred_at: String,
    pub message_id: i64,
    pub attachment_id: Option<i64>,
}

pub fn list_conversation_media(
    &self,
    request: ListConversationMediaRequest,
) -> ConversationResult<ConversationMediaPage>;

pub fn get_conversation_media_counts(
    &self,
    conversation_id: i64,
) -> ConversationResult<ConversationMediaCounts>;
```

Use `occurred_at DESC, message_id DESC, COALESCE(attachment_id, 0) DESC`. Apply identical eligibility helpers to counts and pages.

- [ ] **Step 4: Map kinds without a second asset store**

Query canonical messages/attachments only. Audio tab includes native voice and generic audio; each returned item keeps its exact item kind.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml media_library_test
```

Expected: PASS.

---

## Task 9: Add Bounded Message Context Query

**Files:**
- Modify: `src-tauri/src/conversation/media_library.rs`
- Modify: `src-tauri/src/conversation/media_library_test.rs`
- Modify: `src-tauri/src/conversation/projection.rs`

- [ ] **Step 1: Write failing context tests**

Cover exact anchor inclusion, up to 30 before/after, chronological result order, conversation mismatch rejection, tombstone anchor behavior, missing anchor error, and rich child hydration parity with normal timeline pages.

- [ ] **Step 2: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation_message_context_
```

Expected: FAIL.

- [ ] **Step 3: Implement bounded query**

```rust
pub fn get_conversation_message_context(
    &self,
    request: GetConversationMessageContextRequest,
) -> ConversationResult<ConversationMessageContext>;
```

Clamp before/after to 0–30. Query anchor ownership first, then two bounded sides. Merge, sort chronologically, and hydrate attachments/replies/reactions/batches/mentions with existing projection helpers.

- [ ] **Step 4: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation_message_context_
```

Expected: PASS.

---

## Task 10: Add Phase 2B Services, Commands, and Startup Registration

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Write failing command tests**

Assert exact Tauri behavior for:

```text
stage_conversation_voice
list_conversation_media
get_conversation_media_counts
get_conversation_message_context
```

Test structured errors, account/capability validation, page clamp, and no local path in serialized responses.

- [ ] **Step 2: Run tests RED**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_command_
```

Expected: FAIL.

- [ ] **Step 3: Add service methods and thin commands**

Commands pass state/request to focused voice/media services. `stage_conversation_voice` requires a canonical conversation whose selected account supports voice MIME.

- [ ] **Step 4: Register commands and capabilities**

Register after migration and Phase 1 state setup. Capability MIME values come from accepted provider probe configuration, not browser assumptions.

- [ ] **Step 5: Run tests GREEN**

```bash
cargo test --manifest-path src-tauri/Cargo.toml phase2b_command_
```

Expected: PASS.

- [ ] **Step 6: Backend checkpoint**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

---

## Task 11: Add Frontend Voice/Media Contracts and Runtime Helpers

**Files:**
- Modify: `src/lib/services/conversationService.ts`
- Modify: `src/lib/services/conversationPlatform.ts`
- Modify: `src/lib/services/__tests__/conversationService.spec.ts`
- Modify: `src/lib/services/__tests__/conversationPlatform.spec.ts`

- [ ] **Step 1: Run service/platform impact and inspect imports**

Run impact for `ConversationMessage`; inspect all imports of both service modules.

- [ ] **Step 2: Write failing IPC tests**

Assert exact commands/arguments and camelCase results for stage voice, media page, counts, and message context. Verify base64 contains no data-URL prefix.

- [ ] **Step 3: Write failing runtime-helper tests**

Mock `MediaRecorder.isTypeSupported` and verify provider/runtime intersection order. Test unsupported runtime, permission-denied normalization, chunked Blob-to-base64, and decoded-size precheck using Blob size.

- [ ] **Step 4: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Mirror exact contracts and wrappers**

```ts
export type StageConversationVoiceRequest = {
  conversationId: number;
  bytesBase64: string;
  mimeType: string;
  durationMs: number;
};

export type StagedConversationVoice = {
  stagedAttachmentId: string;
  fileName: string;
  mimeType: string;
  sizeBytes: number;
  durationMs: number;
  previewDataUrl: null;
};

export type ConversationVoicePart = {
  clientPartId: string;
  type: 'voice';
  stagedAttachmentId: string;
  durationMs: number;
};

export type ConversationMediaTabKind = 'image' | 'video' | 'audio' | 'file' | 'link';
export type ConversationMediaItemKind = ConversationMediaTabKind | 'voice';

export type ConversationMediaCursor = {
  occurredAt: string;
  messageId: number;
  attachmentId: number | null;
};

export type ConversationMediaItem = {
  kind: ConversationMediaItemKind;
  conversationId: number;
  messageId: number;
  attachmentId: number | null;
  direction: 'inbound' | 'outbound';
  senderDisplayName: string | null;
  occurredAt: string;
  fileName: string | null;
  mimeType: string | null;
  sizeBytes: number | null;
  durationMs: number | null;
  thumbnailUrl: string | null;
  remoteUrl: string | null;
  linkTitle: string | null;
  linkDescription: string | null;
  linkDomain: string | null;
};

export type ListConversationMediaRequest = {
  conversationId: number;
  kind: ConversationMediaTabKind;
  cursor?: ConversationMediaCursor;
  limit?: number;
};

export type ConversationMediaPage = {
  items: ConversationMediaItem[];
  nextCursor: ConversationMediaCursor | null;
};

export type ConversationMediaCounts = Record<ConversationMediaTabKind, number>;

export type GetConversationMessageContextRequest = {
  conversationId: number;
  messageId: number;
  before?: number;
  after?: number;
};

export type ConversationMessageContext = {
  anchorMessageId: number;
  messages: ConversationMessage[];
};

export const stageConversationVoice = (request: StageConversationVoiceRequest) =>
  invoke<StagedConversationVoice>('stage_conversation_voice', { request });

export const listConversationMedia = (request: ListConversationMediaRequest) =>
  invoke<ConversationMediaPage>('list_conversation_media', { request });

export const getConversationMediaCounts = (conversationId: number) =>
  invoke<ConversationMediaCounts>('get_conversation_media_counts', {
    request: { conversationId }
  });

export const getConversationMessageContext = (request: GetConversationMessageContextRequest) =>
  invoke<ConversationMessageContext>('get_conversation_message_context', { request });

export type ConversationPlatformError = {
  code:
    | 'MicrophoneUnavailable'
    | 'MicrophonePermissionDenied'
    | 'MediaRecorderUnsupported'
    | 'VoiceMimeUnsupported';
  message: string;
};
```

Add `ConversationVoicePart` to the Phase 1 send-part union. Extend the Phase 1 capability and attachment contracts exactly:

```ts
export type ConversationCapabilities = {
  sendText: boolean;
  sendSticker: boolean;
  quoteReply: boolean;
  recallMessage: boolean;
  reactions: {
    read: boolean;
    write: boolean;
  };
  attachments: {
    image: boolean;
    video: boolean;
    audio: boolean;
    file: boolean;
    maxFilesPerBatch: number;
    maxBytesPerFile: number;
    maxBytesPerBatch: number;
  };
  voiceRecording: {
    enabled: boolean;
    maxDurationMs: number;
    maxBytes: number;
    supportedMimeTypes: string[];
  };
  mediaLibrary: {
    enabled: boolean;
    pageSize: number;
    kinds: ConversationMediaTabKind[];
  };
};

export type ConversationAttachment = {
  id: number;
  attachmentType: string;
  fileName: string | null;
  mimeType: string | null;
  sizeBytes: number | null;
  remoteUrl: string | null;
  thumbnailUrl: string | null;
  metadata: Record<string, unknown>;
  durationMs: number | null;
  mediaRole: string | null;
};
```

Replace the Phase 1 declarations with these additive final shapes; do not create duplicate aliases. Extend `ConversationSendPart` with `ConversationVoicePart`. Voice messages use `contentType = 'voice'`, `attachmentType = 'voice'`, and the new duration/media-role fields. If Phase 2A types already exist, retain reaction `supported`, `forward`, `mentions`, `quickTemplates`, message actions, and mention ranges in the same merged declarations.

- [ ] **Step 6: Implement runtime helpers**

```ts
export function selectVoiceMime(providerMimes: string[], recorder: Pick<typeof MediaRecorder, 'isTypeSupported'>): string | null;
export async function blobToBase64(blob: Blob): Promise<string>;
export function normalizeMicrophoneError(error: unknown): ConversationPlatformError;
```

`blobToBase64` strips any prefix and uses chunked conversion.

- [ ] **Step 7: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts
bun run check
```

Expected: PASS.

---

## Task 12: Build the Voice Recorder State Machine and Composer UI

**Files:**
- Create: `src/lib/components/conversation/voiceRecorderState.ts`
- Create: `src/lib/components/conversation/VoiceRecorder.svelte`
- Create: `src/lib/components/conversation/VoiceRecordingPreview.svelte`
- Create: `src/lib/components/conversation/__tests__/voiceRecorderState.spec.ts`
- Create: `src/lib/components/conversation/__tests__/VoiceRecorder.spec.ts`
- Modify: `src/lib/components/conversation/composerDraftState.ts`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`
- Modify: `src/lib/components/conversation/__tests__/composerDraftState.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`

- [ ] **Step 1: Run composer impact/context**

Inspect current toolbar/draft/send callbacks and stop on HIGH/CRITICAL.

- [ ] **Step 2: Write failing pure-state tests**

Cover `idle -> requesting -> recording -> preview -> staging -> sending`, permission failure, unsupported MIME, five-minute auto-stop, 5 MiB rejection, cancel reset, and submitted-snapshot clearing without deleting a newer recording.

- [ ] **Step 3: Write failing component tests**

Verify permission only after click, timer labels, stop/preview/cancel/send, disabled account/capability, one voice-only batch, staging failure preserves preview, success revokes URL, conversation/account switch cleanup, and no mixed text/attachments.

- [ ] **Step 4: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/voiceRecorderState.spec.ts src/lib/components/conversation/__tests__/VoiceRecorder.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement explicit state model**

```ts
export type VoicePreview = {
  blob: Blob;
  objectUrl: string;
  durationMs: number;
  mimeType: string;
};

export type VoiceRecorderState =
  | { kind: 'idle' }
  | { kind: 'requesting' }
  | { kind: 'recording'; startedAt: number; elapsedMs: number; mimeType: string }
  | { kind: 'preview'; preview: VoicePreview }
  | { kind: 'staging'; snapshotId: string; preview: VoicePreview }
  | { kind: 'sending'; snapshotId: string; preview: VoicePreview }
  | { kind: 'error'; code: string; message: string; preview: VoicePreview | null };
```

- [ ] **Step 6: Implement resource cleanup**

Stop tracks, recorder, timer, and object URL on cancel, success, unmount, conversation switch, and account switch. Guard `onstop` from reviving stale generations.

- [ ] **Step 7: Integrate voice-only send**

Stage Blob, then call Phase 1 batch send with exactly one voice part. Generate one client part/batch ID per snapshot. Never append current text or attachments.

- [ ] **Step 8: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/voiceRecorderState.spec.ts src/lib/components/conversation/__tests__/VoiceRecorder.spec.ts src/lib/components/conversation/__tests__/MessageComposer.spec.ts
bun run check
```

Expected: PASS.

---

## Task 13: Add Coordinated Native Voice Playback

**Files:**
- Create: `src/lib/components/conversation/voicePlaybackCoordinator.ts`
- Create: `src/lib/components/conversation/VoiceMessageContent.svelte`
- Create: `src/lib/components/conversation/__tests__/voicePlaybackCoordinator.spec.ts`
- Create: `src/lib/components/conversation/__tests__/VoiceMessageContent.spec.ts`
- Modify: `src/lib/components/conversation/MessageContentRenderer.svelte`
- Modify: `src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts`

- [ ] **Step 1: Write failing coordinator tests**

Verify registering players, starting B pauses A, unregistering active clears state, stale player callbacks are ignored, and only speeds 1/1.5/2 are accepted.

- [ ] **Step 2: Write failing renderer tests**

Verify safe URL gating, play/pause, current/total labels, progress, seek when duration exists, speed cycling, load/error fallback, and generic audio remains on `AudioMessageContent`.

- [ ] **Step 3: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/voicePlaybackCoordinator.spec.ts src/lib/components/conversation/__tests__/VoiceMessageContent.spec.ts src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts
```

Expected: FAIL.

- [ ] **Step 4: Implement focused coordinator**

```ts
export type VoicePlayerHandle = { id: string; pause: () => void };
export function activateVoicePlayer(handle: VoicePlayerHandle): void;
export function releaseVoicePlayer(id: string): void;
```

Keep no DOM references after unregister.

- [ ] **Step 5: Implement voice renderer**

Use native `<audio>` with Phase 1 safe URL helper. Bind time/duration/error events and release coordinator state on destroy.

- [ ] **Step 6: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/voicePlaybackCoordinator.spec.ts src/lib/components/conversation/__tests__/VoiceMessageContent.spec.ts src/lib/components/conversation/__tests__/MessageContentRenderer.spec.ts
bun run check
```

Expected: PASS.

---

## Task 14: Build Media Cache State and Desktop/Sheet Panel

**Files:**
- Create: `src/lib/components/conversation/conversationMediaState.ts`
- Create: `src/lib/components/conversation/ConversationMediaPanel.svelte`
- Create: `src/lib/components/conversation/ConversationMediaSheet.svelte`
- Create: `src/lib/components/conversation/ConversationMediaGrid.svelte`
- Create: `src/lib/components/conversation/ConversationMediaList.svelte`
- Create: `src/lib/components/conversation/__tests__/conversationMediaState.spec.ts`
- Create: `src/lib/components/conversation/__tests__/ConversationMediaPanel.spec.ts`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Run workspace impact/context**

Inspect existing right-side actions/layout and responsive breakpoints.

- [ ] **Step 2: Write failing state tests**

Cover cache key `conversationId:kind`, first load, next page, duplicate item merge, exclusive cursor, generation-based stale response rejection, per-conversation invalidation, counts invalidation, and no cross-conversation flash.

- [ ] **Step 3: Write failing panel tests**

Verify five tabs, count badges, loading/empty/error/retry states, 40-item pagination, image grid, video/audio/file/link rows, desktop panel, narrow Sheet, and same shared state instance.

- [ ] **Step 4: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/conversationMediaState.spec.ts src/lib/components/conversation/__tests__/ConversationMediaPanel.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 5: Implement media store**

```ts
export type MediaCacheKey = `${number}:${ConversationMediaTabKind}`;

export type ConversationMediaCacheEntry = {
  pages: ConversationMediaPage[];
  nextCursor: ConversationMediaCursor | null;
  state: 'idle' | 'loading-first-page' | 'loading-next-page' | 'ready' | 'error';
  error: string | null;
  generation: number;
};

export type ConversationMediaCountsEntry = {
  counts: ConversationMediaCounts | null;
  state: 'idle' | 'loading' | 'ready' | 'error';
  error: string | null;
  generation: number;
};

export class ConversationMediaCache {
  get(key: MediaCacheKey): ConversationMediaCacheEntry;
  beginPageLoad(key: MediaCacheKey, mode: 'first-page' | 'next-page'): number;
  replaceFirstPage(key: MediaCacheKey, page: ConversationMediaPage, generation: number): void;
  appendPage(key: MediaCacheKey, page: ConversationMediaPage, generation: number): void;
  failPageLoad(key: MediaCacheKey, error: string, generation: number): void;
  getCounts(conversationId: number): ConversationMediaCountsEntry;
  beginCountsLoad(conversationId: number): number;
  replaceCounts(conversationId: number, counts: ConversationMediaCounts, generation: number): void;
  failCountsLoad(conversationId: number, error: string, generation: number): void;
  invalidateConversation(conversationId: number): void;
}
```

Merge page items by `(messageId, attachmentId, kind)`, preserve backend order, and ignore responses whose generation no longer matches the entry.

- [ ] **Step 6: Implement shared responsive panel**

Desktop and Sheet receive the same typed state/callbacks. Components contain no Zalo channel checks and no direct URL fetching.

- [ ] **Step 7: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/conversationMediaState.spec.ts src/lib/components/conversation/__tests__/ConversationMediaPanel.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

---

## Task 15: Add Safe Preview and Jump to Original Message

**Files:**
- Create: `src/lib/components/conversation/ConversationMediaPreview.svelte`
- Create: `src/lib/components/conversation/__tests__/ConversationMediaPreview.spec.ts`
- Modify: `src/lib/components/conversation/ConversationMediaPanel.svelte`
- Modify: `src/lib/components/conversation/ConversationTimeline.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Write failing preview tests**

Verify image/video/audio/voice preview selection, file/link safe open, unsafe URL fallback, no metadata fetch, close behavior, and tombstoned item absence.

- [ ] **Step 2: Write failing context-anchor tests**

Verify context request with `before = 30`, `after = 30`, merge by message ID, chronological order, one scroll to `data-message-id`, temporary highlight, missing anchor error, and current Conversation mismatch protection.

- [ ] **Step 3: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/ConversationMediaPreview.spec.ts src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 4: Implement preview with Phase 1 URL sanitizer**

Never pass a local path. Links use `target="_blank" rel="noreferrer noopener"` or the existing Tauri opener wrapper after safe URL validation.

- [ ] **Step 5: Implement bounded anchor merge**

Expose a workspace helper:

```ts
async function jumpToConversationMessage(conversationId: number, messageId: number): Promise<void>;
```

Load bounded context, merge into timeline cache, wait for DOM update, scroll once, and clear highlight after 2 seconds.

- [ ] **Step 6: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/ConversationMediaPreview.spec.ts src/lib/components/conversation/__tests__/ConversationTimeline.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

---

## Task 16: Wire Media Invalidation and Voice Optimistic Updates

**Files:**
- Modify: `src/lib/components/conversation/conversationMediaState.ts`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/conversationMediaState.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] **Step 1: Write failing invalidation tests**

Cover inbound media event, outbound queued voice, voice echo remote metadata, recall/tombstone, identity/conversation merge, unrelated conversation event, and count/page generation invalidation together.

- [ ] **Step 2: Run tests RED**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/conversationMediaState.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
```

Expected: FAIL.

- [ ] **Step 3: Implement scoped invalidation**

Invalidate only the affected canonical conversation. A queued local voice may appear with `remoteUrl = null`; authoritative echo replaces it by message ID without duplicate media items.

- [ ] **Step 4: Run tests GREEN**

```bash
bun test --conditions=browser --conditions=svelte src/lib/components/conversation/__tests__/conversationMediaState.spec.ts src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun run check
```

Expected: PASS.

- [ ] **Step 5: Frontend checkpoint**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

---

## Task 17: Full Verification, Security Audit, and Live Gate

**Files:**
- Modify only defects in files already listed.
- Do not expand Phase 2B scope.

- [ ] **Step 1: Run targeted provider/backend suites**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2b_voice_
cargo test --manifest-path src-tauri/Cargo.toml phase2b_
cargo test --manifest-path src-tauri/Cargo.toml voice_
cargo test --manifest-path src-tauri/Cargo.toml media_library_
cargo test --manifest-path src-tauri/Cargo.toml conversation_message_context_
```

Expected: PASS.

- [ ] **Step 2: Run full backend validation**

```bash
cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Expected: PASS or exact unrelated baseline failures documented.

- [ ] **Step 3: Run frontend validation**

```bash
bun test --conditions=browser --conditions=svelte src/lib/services/__tests__/conversationService.spec.ts src/lib/services/__tests__/conversationPlatform.spec.ts src/lib/components/conversation/__tests__
bun run check
bun test --conditions=browser --conditions=svelte
bun run build
```

Expected: PASS.

- [ ] **Step 4: Audit voice/media invariants**

```bash
rg -n "file://|local_path|original.*path|cookies_json|bytesBase64" src/lib/components/conversation src/lib/services src-tauri/src/conversation
rg -n "channel === ['\"]zalo|channel == ['\"]zalo" src/lib/components/conversation
rg -n "fetch\(|axios|openGraph|metadata" src/lib/components/conversation/ConversationMedia* src-tauri/src/conversation/media_library.rs
rg -n "ffmpeg|transcod|speech.?to.?text|transcript" src-tauri/src/conversation src/lib/components/conversation
```

Expected: no frontend local paths/auth data, presentation channel checks, third-party metadata fetch, transcoding, or transcription. `bytesBase64` appears only in the staging request path and never persisted/projected.

- [ ] **Step 5: Run GitNexus final audit**

```text
mcp__gitnexus.detect_changes({scope: "all", repo: "RustSale_v2"})
```

Review changed symbols and all d=1 dependents.

- [ ] **Step 6: Execute live smoke matrix**

Run all 14 cases from design section 21 on every supported desktop runtime: permission denial, short direct/group voice, near-limit recording, over-limit rejection, network interruption, restart, no duplicate echo, player speeds, >40 media pagination, recall removal, bounded jump, and no path leakage.

- [ ] **Step 7: Verify independent capability rollback**

Release `mediaLibrary` first while `voiceRecording` remains disabled. Enable `voiceRecording` only after runtime/provider probes pass for the current desktop platform.

- [ ] **Step 8: Verify independent capability rollback**

Disable `mediaLibrary` and `voiceRecording` independently. Confirm Phase 1 rich chat remains functional and additive schema remains readable.

- [ ] **Step 9: Final diff review**

```bash
git diff --check
git status --short
git diff --stat
```

Do not commit without explicit authorization.

---

## Acceptance Checklist

- [ ] Runtime capability intersects provider and `MediaRecorder` MIME support.
- [ ] Microphone tracks, timers, recorder, and object URLs always clean up.
- [ ] Voice bytes enter the Rust-owned spool before enqueue.
- [ ] Voice batches contain exactly one voice part.
- [ ] Native voice send uses a stable provider client ID.
- [ ] Delivery-unknown voice jobs are never automatically resent.
- [ ] Realtime/history voice echoes reconcile without duplicate bubbles.
- [ ] Voice playback is safe and globally coordinated in the workspace.
- [ ] Media Library queries canonical messages/attachments only.
- [ ] Media pages and counts share identical eligibility rules.
- [ ] Tombstoned media is excluded.
- [ ] Jump-to-message loads at most 30 messages per side.
- [ ] No arbitrary local path, spool path, cookie, token, or raw session data reaches frontend contracts.
- [ ] UI behavior is capability-driven rather than channel-name-driven.
- [ ] Non-Zalo channels preserve existing behavior.
- [ ] All targeted/full validation gates pass or unrelated failures are documented exactly.
- [ ] GitNexus final scope matches approved Phase 2B files and flows.
