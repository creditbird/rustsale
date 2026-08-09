# Zalo Reaction Reliability Remediation Implementation Plan

> **For agentic workers:** Execute inline with test-first checkpoints. Do not commit unless the user explicitly requests it.

**Goal:** Prevent reaction loss, preserve raw provider icons, continuously complete reaction history, and recover previously missed reactions.

**Architecture:** SQLite stages unmatched reactions and replays them atomically during message ingestion. The Zalo listener preserves unknown strings, while the supervisor resumes bounded backfill bursts until exhausted.

**Tech Stack:** Rust, rusqlite, Tokio, vendored zca-rs, Svelte 5, Bun.

---

### Task 1: Persistence and migration

**Requirements:** REQ-001, REQ-002, REQ-003, REQ-004, REQ-008

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration.rs`
- Modify: `src-tauri/src/conversation/migration_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`

- [x] Add failing migration tests for pending table and one-time history reset.
- [x] Add failing repository tests for reaction-before-message replay.
- [x] Add pending table and recovery marker migration.
- [x] Stage unresolved reactions and replay them atomically on message ingest.
- [x] Run focused migration and repository tests.

### Task 2: Raw icon preservation

**Requirements:** REQ-006

**Files:**
- Modify: `src-tauri/vendor/zca-rs/src/models/event.rs`
- Modify: `src-tauri/vendor/zca-rs/src/models/event_test.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`

- [x] Add failing unknown-icon round-trip and normalizer tests.
- [x] Implement custom `EventReactions` string serde.
- [x] Normalize through the preserved raw provider value.
- [x] Run vendor and app normalizer tests.

### Task 3: Continuous bounded history

**Requirements:** REQ-005

**Files:**
- Modify: `src-tauri/src/conversation/zalo/supervisor.rs`
- Modify: `src-tauri/src/conversation/zalo/supervisor_test.rs`

- [x] Add failing paused-stream resume test.
- [x] Preserve the next cursor when a burst reaches five pages.
- [x] Resume paused streams from a supervisor ticker.
- [x] Run focused supervisor tests.

### Task 4: Unsupported reaction UI

**Requirements:** REQ-007

**Files:**
- Modify: `src/lib/components/conversation/reactionDisplay.ts`
- Modify: `src/lib/components/conversation/MessageReactionSummary.svelte`
- Modify: `src/lib/components/conversation/MessageBubble.svelte`
- Modify: `src/lib/components/conversation/__tests__/MessageReactionSummary.spec.ts`

- [x] Add failing unsupported-icon interaction test.
- [x] Return `null` for unsupported writable reaction codes.
- [x] Render unsupported icons read-only with actor expansion.
- [x] Run focused Bun tests.

### Task 5: Desktop mutation semantics and echo suppression

**Requirements:** REQ-009, REQ-010, REQ-011

**Files:**
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src-tauri/src/conversation/reactions.rs`
- Modify: `src-tauri/src/conversation/reactions_test.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`
- Create: `src-tauri/src/conversation/echo_cache.rs`

- [x] Add failing tests for Haha `rType=0`, selective local removal, missing provider IDs, cache lifecycle, and self identity.
- [x] Correct add/remove normalization semantics.
- [x] Make desktop removal local-only and hard-delete only the selected canonical emoji.
- [x] Add account-scoped, consume-once self-echo suppression with alias and failure cleanup.
- [x] Persist the real client self UID and reject missing identity before the provider call.
- [x] Run focused normalizer, reaction, and service tests.

### Task 6: Integration verification

**Requirements:** REQ-001 through REQ-008

- [x] Run focused Rust reaction, migration, supervisor, and normalizer tests.
- [x] Run vendored zca-rs tests.
- [x] Run focused frontend tests and `bun run check`.
- [x] Run Rust formatting and clippy.
- [x] Run `git diff --check` and GitNexus change detection.
- [x] Confirm pre-existing unrelated changes remain intact.
