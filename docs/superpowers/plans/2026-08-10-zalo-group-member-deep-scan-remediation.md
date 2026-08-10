# Zalo Group Member Deep Scan Production Remediation Implementation Plan

> **For agentic workers:** Execute inline with strict test-first checkpoints. Preserve all unrelated uncommitted work. Do not commit unless the user explicitly requests it.

**Goal:** Make hidden-group UID discovery reliable, supervisor-owned, transactional, diagnosable, import-safe, and accessible without changing the approved modal layout.

**Architecture:** The existing Zalo supervisor owns account-wide command-511 pagination and exposes a bounded singleflight control request. Board/Poll sources return structured partial-success reports. Repository transactions canonicalize identities and imports. The modal performs local reads on open and explicit stale-safe Deep Scans.

**Tech Stack:** Rust, Tokio, rusqlite, vendored zca-rs, Tauri 2, Svelte 5, shadcn-svelte/bits-ui, Bun.

**Design:** `docs/superpowers/specs/2026-08-10-zalo-group-member-deep-scan-remediation-design.md`

**Environment:** `/tmp` is full. Run Rust checks with `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810`. Use only `bun`/`bunx`, never `npm`/`npx`.

---

### Task 1: Lock provider history semantics

**Requirements:** REQ-002, REQ-005

**Files:**
- Modify: `src-tauri/vendor/zca-rs/src/apis/group.rs`
- Modify: `src-tauri/vendor/zca-rs/src/listener.rs`
- Modify: `src-tauri/src/conversation/zalo/probe_test.rs`

- [ ] Run GitNexus impact for the history decoder and `request_old_messages` before edits.
- [ ] Add failing vendor tests proving malformed `groupMsgs` entries return contextual deserialization errors instead of being skipped.
- [ ] Add or extend a listener test proving command 511 payload has no group identifier and retains account-wide semantics.
- [ ] Remove the silent `if let Ok` message drop and preserve the original failing item context.
- [ ] Remove application use of `api/group/history`; retain vendor methods only if another verified consumer exists.
- [ ] Run focused vendored zca-rs and probe tests.

### Task 2: Add supervisor-controlled singleflight backfill

**Requirements:** REQ-001, REQ-003, REQ-004

**Files:**
- Modify: `src-tauri/src/conversation/zalo/supervisor.rs`
- Modify: `src-tauri/src/conversation/zalo/supervisor_test.rs`
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`

- [ ] Run GitNexus impact for `ZaloSessionSupervisor`, `HistoryPaginationState`, `run_session_loop`, and affected command registration symbols.
- [ ] Add failing tests for explicit group-message re-arm from the durable cursor after in-memory completion.
- [ ] Add failing tests proving concurrent requests join one active run and issue only one command-511 sequence.
- [ ] Add failing tests for disconnected/unavailable, completed, repeated-cursor, empty-page, and bounded-timeout outcomes.
- [ ] Add a supervisor control channel and typed `GroupMessageBackfillReport`/outcome contract.
- [ ] Store the control sender in each `SessionTask`; handle requests inside the connected session loop alongside listener events and the resume ticker.
- [ ] Re-arm only `HistoryStream::GroupMessages`; never reset reaction streams or realtime watermarks.
- [ ] Expose a bounded `backfill_group_messages` supervisor method for Tauri commands.
- [ ] Run focused supervisor and conversation-command tests.

### Task 3: Fully paginate Board and Poll UID sources

**Requirements:** REQ-006, REQ-007, REQ-008

**Files:**
- Modify: `src-tauri/src/conversation/zalo/client.rs`
- Modify: `src-tauri/src/conversation/zalo/client_test.rs`
- Modify: `src-tauri/vendor/zca-rs/src/apis/group.rs`

- [ ] Run GitNexus impact for `ZaloClient`, `ZcaZaloClient`, and the Board API method before edits.
- [ ] Add failing pure extraction tests for top-level `data`, decoded `data.params`, creator/owner keys, direct voters, and sentinel filtering.
- [ ] Add failing tests for numeric and numeric-string poll IDs plus Poll creator/voter extraction.
- [ ] Add failing pagination tests for multiple pages, short/empty exhaustion, repeated-page protection, defensive page bound, Board page failure, and individual Poll failure.
- [ ] Replace `Vec<String>` with a structured `GroupBoardUidReport` containing committed UID candidates, coverage counters, stop reason, and warnings.
- [ ] Decode non-poll `data.params` strictly enough to report malformed payloads while preserving the original item.
- [ ] Fetch every Board page sequentially and continue after isolated Poll-detail errors.
- [ ] Run focused client and vendored zca-rs tests.

### Task 4: Canonicalize Zalo identities transactionally

**Requirements:** REQ-009, REQ-010, REQ-011

**Files:**
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer.rs`
- Modify: `src-tauri/src/conversation/zalo/normalizer_test.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands_test.rs`

- [ ] Run GitNexus impact for `apply_normalized_message`, `NormalizedReplyReference`, `link_identity`, and current UID helper symbols before edits.
- [ ] Add failing repository tests for canonical `zalo_uid` creation, duplicate namespace collision, lead-link preservation, metadata precedence, and invalid/sentinel UID rejection.
- [ ] Add failing rollback tests for participant conflicts and reference migration across participants, message senders, and mentions.
- [ ] Add failing normalizer/repository tests proving quote-owner UID survives normalization, is persisted canonically, and appears only in the selected conversation's evidence.
- [ ] Add repository transaction methods for one UID and a deduplicated UID batch; remove command-layer ignored SQL results.
- [ ] Coalesce participant rows without losing role or lifecycle timestamps, migrate message and mention references, then retire duplicate identities safely.
- [ ] Extend stored reply/quote data with the quote-owner identity reference required by local evidence queries.
- [ ] Replace `ensure_zalo_uid_identity` call sites with repository/service operations that propagate failures.
- [ ] Run focused repository, normalizer, command, and database tests.

### Task 5: Rebuild Deep Scan orchestration and diagnostics

**Requirements:** REQ-001, REQ-002, REQ-005, REQ-008, REQ-011, REQ-018

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands_test.rs`
- Modify: `src-tauri/src/conversation/zalo/client.rs`
- Modify: `src-tauri/src/conversation/zalo/supervisor.rs`
- Modify: `src/lib/services/conversationService.ts`
- Modify: `src/lib/services/__tests__/conversationService.spec.ts`

- [ ] Run GitNexus impact for the command file/registered Deep Scan route and all helper symbols edited in this task.
- [ ] Add failing command tests for non-group rejection, disconnected partial result, supervisor timeout, Board partial failure, persistence rollback, and accurate committed counts.
- [ ] Add failing isolation tests proving account-wide history is filtered through selected-conversation SQLite evidence before membership is returned.
- [ ] Remove the direct command-511 call and the nonfunctional HTTP history loop.
- [ ] Orchestrate snapshot, supervisor backfill, Board/Poll extraction, transactional UID attachment, local evidence re-query, and bounded profile hydration.
- [ ] Replace the flattened warning string with structured direct-snapshot, Board/Poll, backfill, local-evidence, hydration, and overall diagnostics.
- [ ] Mirror every serialized Rust field and enum in TypeScript; add invoke-contract tests for argument names and response typing.
- [ ] Run focused Rust command tests and `bun test src/lib/services/__tests__/conversationService.spec.ts`.

### Task 6: Make batch import exact and atomic

**Requirements:** REQ-012, REQ-013, REQ-014, REQ-015

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`
- Modify: `src/lib/services/conversationService.ts`

- [ ] Run GitNexus impact for `batch_import_group_members_to_leads`, `link_identity`, and repository lead/category mutation symbols before edits.
- [ ] Add failing tests rejecting identities not evidenced by the requested conversation and request items with conflicting IDs.
- [ ] Add failing tests for duplicate request collapse, exact linked-lead reuse, `skip_existing`, idempotent categories, and removal of `about LIKE` matching.
- [ ] Add failure-injection tests proving category, lead, identity-link, or SQL errors roll back the full batch and do not inflate counts.
- [ ] Add tests for `link_source`, confidence, verification/update timestamps, and one post-commit conversation invalidation.
- [ ] Add `conversation_id` to the request contract and resolve canonical identities from selected-conversation evidence.
- [ ] Move import mutations into one repository/service transaction and publish invalidation only after commit.
- [ ] Run focused command, repository, service, and TypeScript contract tests.

### Task 7: Harden modal state, CSV, and accessibility

**Requirements:** REQ-016, REQ-017, REQ-018, REQ-019, REQ-020

**Files:**
- Modify: `src/lib/components/conversation/GroupMembersModal.svelte`
- Create: `src/lib/components/conversation/groupMembersCsv.ts`
- Create: `src/lib/components/conversation/__tests__/GroupMembersModal.spec.ts`
- Create: `src/lib/components/conversation/__tests__/groupMembersCsv.spec.ts`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`

- [ ] Run GitNexus impact for `ConversationWorkspace.svelte`; document that the untracked modal is isolated from the current index.
- [ ] Capture baseline modal screenshots and element measurements before changing markup or classes.
- [ ] Add failing UI tests proving open performs local reads only, Deep Scan is explicit, duplicate clicks are disabled, post-import refresh stays local, and stale responses are ignored after close/conversation switch.
- [ ] Add failing diagnostics tests distinguishing complete provider membership from locally observed/account-wide evidence and rendering structured warnings.
- [ ] Add failing CSV tests for formulas, leading whitespace, commas, quotes, line breaks, BOM, and stable columns.
- [ ] Add failing accessibility tests for dialog role/name/description, Escape close, initial focus, focus containment, focus restoration, and live loading/error announcements.
- [ ] Move CSV escaping to a pure helper and sanitize spreadsheet formulas before quoting.
- [ ] Use the existing shadcn-svelte Dialog primitives or equivalent bits-ui behavior while preserving the current modal dimensions, spacing, colors, filters, and table layout.
- [ ] Add request-generation guards and remove automatic scan-on-open and scan-after-import behavior.
- [ ] Re-measure and compare the modal against baseline; allow no unintended pixel drift.
- [ ] Run focused Bun component tests and `bun run check`.

### Task 8: Integration and production verification

**Requirements:** REQ-001 through REQ-021

- [ ] Run focused Rust tests after every task with the configured `TMPDIR`.
- [ ] Run full application Rust tests: `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810 cargo test --manifest-path src-tauri/Cargo.toml`.
- [ ] Run vendored tests: `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810 cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml`.
- [ ] Run frontend tests: `bun test`.
- [ ] Run frontend checks and build: `bun run check` and `bun run build`.
- [ ] Run Rust formatting: `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check`.
- [ ] Run clippy with warnings denied: `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810 cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.
- [ ] Run release build: `TMPDIR=/home/arch/.cache/codex-rustsale-review-20260810 cargo build --manifest-path src-tauri/Cargo.toml --release`.
- [ ] Run `git diff --check`.
- [ ] Run GitNexus `detect_changes(scope="all")`; verify all d=1 dependents are updated and warn if the pre-existing working-tree risk remains HIGH/CRITICAL.
- [ ] Confirm no tests send live Zalo messages, reactions, invitations, or other provider mutations.
- [ ] Report any pre-existing unrelated failures separately; do not hide or repair them without scope approval.

### Task 9: Preserve legacy database startup compatibility

**Requirements:** REQ-022

**Files:**
- Modify: `src-tauri/src/conversation/schema.sql`
- Modify: `src-tauri/src/conversation/migration.rs`
- Modify: `src-tauri/src/conversation/migration_test.rs`

- [ ] Run GitNexus impact for `migrate` and `init_db`; warn before editing if either result is HIGH or CRITICAL.
- [ ] Add a failing migration test that builds the legacy `messages` shape without `reply_sender_identity_id` and reproduces the dependent-index startup error.
- [ ] Keep the column in the fresh schema, but create `idx_messages_reply_sender_identity` only after `add_column_if_missing` has upgraded existing databases.
- [ ] Run the focused migration and database suites, full Rust tests, clippy with warnings denied, and the release build.
- [ ] Back up the real SQLite database, run `bun run tauri dev`, and verify the upgraded column, foreign key, partial index, and zero foreign-key violations.
