# Enterprise Omnichannel Push-Delta Sync Plan

## Metadata

- Date: `2026-08-31`
- Status: `ready_for_review`
- Plan: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-plan.md`
- Evidence: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-actual-status.md`

## Goal

Remediate all 8 blocking findings from Supervisor re-review (`rp_supervisor_260831_063454_by_gpt-5-codex_enterprise_push_delta_rereview.md`) to establish an enterprise-grade, zero-trust verified Omnichannel Push-Delta Chat Bubble synchronization system across Zalo, WhatsApp, and Facebook Messenger:
1. **Message & Delta Atomic Transaction**: Gộp `record_message_delta_tx` vào cùng một SQLite transaction với message ingestion (`apply_normalized_message`), đảm bảo rollback nguyên tử và fail-closed.
2. **Single Monotonic Sequence Authority**: Xóa bỏ `AtomicU64` trong `events.rs`, 100% sequence phát sinh từ SQLite `conversation_deltas`.
3. **Omnichannel Per-Message Delta Emission**: Facebook batch emit delta cho từng message; history sync và attachment hydration dùng invalidation reason riêng (`history_sync`, `attachment_update`, `metadata`) không bị frontend suppress.
4. **Resumable Live-Buffering Replay Protocol**: Frontend buffer live events trong khi replay, lặp phân trang đến high-water mark, apply tuần tự theo sequence, rồi drain buffer.
5. **DB Watermark Snapshot Guard & Sequence Deduplication**: Snapshot mang DB sequence watermark; frontend deduplicate và merge chống out-of-order provider timestamp.
6. **DOM Paint Telemetry with Real Samples**: Thu thập raw sample mảng thời gian thực, tính toán p50/p95/p99 cho WebView receive->paint và DB commit->paint.
7. **Full Verification & Clean Commits**: `git diff --check` sạch, suite 490 backend tests + 635 frontend tests, packaged desktop validation và Completion Lock clearance.

## Requirements

| ID | Requirement | Invariant | Required Evidence | Owning Slice |
|---|---|---|---|---|
| R-01 | Atomic SQLite transaction for message + delta | Every message write generates a persistent delta record in the EXACT same SQLite transaction | Repository unit tests with rollback fault-injection | P1-A |
| R-02 | Single Sequence Authority | Zero volatile sequence generators; all sequences originate monotonically from SQLite `conversation_deltas` | `events.rs` inspection & sequence collision tests | P1-B |
| R-03 | Omnichannel per-message delta & distinct invalidation reasons | Every inserted message in FB batch emits a delta; history/attachments use unsuppressed reasons | Facebook batch test & history sync tests | P1-C |
| R-04 | Paginated Replay & High-Water Mark Reconnection | Replay loops until high-water mark is reached; live events buffered during replay | Replay gap tests (>200 items, live-during-replay) | P1-D & P2-A |
| R-05 | DB Watermark Snapshot Guard & Deduplication | Snapshot merge uses DB sequence watermark rather than provider timestamp; deduplicates by sequence | Snapshot race test & out-of-order timestamp tests | P2-B |
| R-06 | Telemetry Sample Persistence | Raw samples collected; p50/p95/p99 calculated and recorded in benchmark ledger | Telemetry summary output & benchmark ledger | P2-C |
| R-07 | Full Clean Quality Gates & Commit Boundaries | `git diff --check` clean, zero warnings, commit slices with rollback anchors | Full build & test logs, git log | P3-A & P3-B |

## Checklist

- [x] P0-A: Complete actual status update for 8 re-review blockers.
  - Goal: establish baseline classifications in `actual-status.md`.
  - Acceptance: `actual-status.md` updated with R2 refresh log.

### P1: Backend Atomic Transaction, Single Sequence Authority & Omnichannel Ingestion

- [x] P1-A: Atomic message + delta transaction in `apply_normalized_message` and repository methods.
  - Goal: Ensure message insert and `conversation_deltas` row insert occur inside the exact same SQLite transaction, returning `AppliedMessage` with committed `ConversationMessageDeltaEvent`.
  - Work Steps:
    1. Implement `record_message_delta_tx(tx: &Transaction, conversation_id: i64, lead_id: Option<i64>, channel: Channel, message_id: i64) -> Result<Option<ConversationMessageDeltaEvent>>` in `repository.rs`.
    2. In `apply_normalized_message`, call `record_message_delta_tx` before `tx.commit()?`.
    3. Add fault-injection rollback test proving that if delta fails, message transaction rolls back completely.
  - Implementation Gate: `cargo test --package rustsale-crm --lib conversation::repository` passes.
  - Acceptance: Message + delta atomicity verified; 0 orphan messages without delta.

- [x] P1-B: Remove volatile `AtomicU64` in `events.rs` and establish 100% single sequence authority.
  - Goal: Eliminate volatile in-memory counter; every delta event emitted receives its sequence from SQLite `conversation_deltas`.
  - Work Steps:
    1. Remove `sequence: AtomicU64` and `next_sequence()` from `ConversationEventPublisher`.
    2. Update `start_direct_conversation` to record delta inside the message transaction in `repository.rs`.
    3. Verify all delta emissions across the entire codebase carry persistent DB sequences.
  - Implementation Gate: `cargo check` compiles with zero references to `next_sequence()`.
  - Acceptance: Single authority sequence enforced across all channels and direct conversations.

- [x] P1-C: Omnichannel per-message delta in Facebook batch ingestion + distinct non-suppressed invalidation reasons.
  - Goal: Every message in a Facebook batch produces its own delta; history sync and attachment hydration use non-suppressed invalidation reasons.
  - Work Steps:
    1. Update `FacebookMessengerNormalizer::ingest_conversations_batch` and `ingest_webhook_event` to record deltas for each inserted message in the batch.
    2. Add invalidation reasons `HistorySync` and `AttachmentUpdate` to `ConversationInvalidationReason` in `events.rs`.
    3. Update WhatsApp and Zalo history sync handlers to use `HistorySync` or `Metadata`.
  - Implementation Gate: `cargo test --package rustsale-crm --lib conversation::facebook` and `whatsapp` pass.
  - Acceptance: Multi-message Facebook batch emits deltas for all messages; history sync invalidations are never suppressed by frontend.

- [x] P1-D: Paginated replay query with high-water mark metadata & backend fault-injection tests.
  - Goal: Support querying deltas with `max_sequence` boundary and paginated limit.
  - Work Steps:
    1. Update `fetch_deltas_after_sequence` to return deltas up to current database high-water mark.
    2. Add unit/integration tests for >200 event gaps, multi-page retrieval, and rollback integrity.
  - Implementation Gate: `cargo test --package rustsale-crm --lib conversation` passes.
  - Acceptance: Backend replay contract tested and proven.

### P2: Frontend Resumable Protocol, Live Buffering, DB Watermark & Telemetry

- [x] P2-A: Frontend live event buffering during replay + multi-page catch-up loop + persistent/negotiated cursor.
  - Goal: Establish ordered handoff from replay to live delivery without skipped gaps or cursor races.
  - Work Steps:
    1. In `ConversationWorkspace.svelte`, add `isReplaying` state and `liveDeltaBuffer`.
    2. Implement multi-page replay loop in `resumeMissedDeltas(lastSeq, targetSeq)`: loop `resumeConversationDeltas` until reaching target sequence or empty response.
    3. Drain `liveDeltaBuffer` in sequence order after replay completes.
    4. Sync `lastAppliedSequence` from timeline messages on workspace initialization / switch.
  - Implementation Gate: `bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts` passes.
  - Acceptance: Gaps of any size (e.g. 500 messages) and concurrent live delivery during replay handled with 0 lost bubbles.

- [x] P2-B: Strict sequence-based deduplication in store before mutation + DB sequence watermark in snapshot responses and `timelineState.ts`.
  - Goal: Prevent stale snapshots or duplicate sequence events from modifying the store or dropping newer messages.
  - Work Steps:
    1. In `ConversationWorkspace.svelte`, early return in `handleMessageDelta` if `delta.sequence <= lastAppliedSequence`.
    2. In `timelineState.ts`, carry `snapshotSequence` watermark and retain in-memory messages with `message.sequence > snapshotSequence`.
    3. Unsuppress invalidations if `event.reason !== 'message'` (e.g. `history_sync`, `metadata`, `receipt`, `reaction`).
  - Implementation Gate: `bun test src/lib/components/conversation/__tests__/timelineState.spec.ts` passes.
  - Acceptance: Out-of-order timestamps and duplicate sequence events cannot corrupt timeline state.

- [x] P2-C: DOM Paint Telemetry sample collection (`p50`, `p95`, `p99`) with persistent summary calculation.
  - Goal: Measure real DOM paint latencies and collect numeric samples.
  - Work Steps:
    1. In `ConversationWorkspace.svelte`, collect `paintTelemetrySamples` array.
    2. Calculate `p50`, `p95`, `p99` percentiles and record summary in benchmark ledger.
  - Implementation Gate: telemetry samples logged and verified in component tests.
  - Acceptance: Real measured latency metrics available.

- [x] P2-D: Comprehensive adversarial frontend tests.
  - Goal: Test all failure modes identified in supervisor review.
  - Work Steps:
    1. Test: Gap > 200 items paginated replay catch-up.
    2. Test: Concurrent live delivery during multi-page replay.
    3. Test: Stale snapshot race with out-of-order provider timestamp.
    4. Test: History sync invalidation triggers timeline refresh without being suppressed.
  - Implementation Gate: all frontend spec tests pass.
  - Acceptance: 100% pass rate on adversarial suites.

### P3: Quality Gates, Clean Commits, Packaged Runtime & Supervisor Re-evaluation

- [x] P3-A: Full clean build and regression verification (`cargo test`, `bun test`, `bun run check`, `bun run build`, `cargo check`).
- [x] P3-B: Clean `git diff --check` and structured commit boundaries with rollback anchors.
- [ ] P3-C: Call independent Supervisor acceptance re-review.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- For public runtime or UI-facing changes, validate the real user-visible runtime with browser or Playwright evidence.
- Every implementation phase must be decomposed into multiple implementation slices that are as small as practical.
- Hidden fallback is forbidden. Prefer a visible failure over a fallback that hides a broken primary path.

## Problem

Previous implementation (`commit 5659137`) added in-memory push-delta and direct upsert for Zalo, but left critical production gaps:
1. `sequence` was an in-memory `AtomicU64`; restart, reconnect, or broadcast lag caused dropped events with no replay or gap recovery.
2. The real Zalo path emitted both invalidation and delta; invalidation triggered `refreshOpenTimeline` fetching 50 messages over IPC on the paint hot path.
3. Stale snapshot responses in `timelineState.ts` could temporarily erase newly arrived deltas not present in the fetched page.
4. WhatsApp and Facebook Messenger supervisors only emitted invalidations, breaking the omnichannel claim.
5. `<16 ms` was measured only to handler entry (`Date.now()`) rather than true DOM paint (`requestAnimationFrame`).
6. Reconnect/burst (>256 messages) and Completion Lock gates remained unproven.

## Scope

- In scope:
  - SQLite schema migration for `conversation_deltas` table with monotonic sequence and indices.
  - Rust repository methods for transactionally inserting deltas and querying missed deltas by sequence.
  - Rust Tauri IPC command `resume_conversation_deltas`.
  - Backend supervisors: Zalo (`service.rs`), WhatsApp (`whatsapp/supervisor.rs`), Facebook Messenger (`facebook/supervisor.rs`).
  - Frontend `timelineState.ts` watermark-protected snapshot merge.
  - Frontend `conversationService.ts` resumable bridge, gap recovery, and cleanup.
  - Frontend `ConversationWorkspace.svelte` sequence consumption, gap replay, paired invalidation suppression, and rAF DOM paint telemetry.
  - Rust unit/integration tests for burst (>256), replay, sequence gap recovery, and multi-channel publication.
  - Frontend unit & integration tests (`ConversationWorkspace.spec.ts`, `timelineState.spec.ts`).
- Out of scope:
  - Refactoring outbound message sending queues (already implemented in commit `161fd5b`).
  - Modifying external channel provider protocols (zca-rs, meta graph API, whatsapp proto).

## Non-Goals

- Replacing SQLite with an external messaging broker (Redis/Kafka).
- Client-side full offline SQLite replica in WebView (we maintain in-memory window + IPC replay).

## Requirements

| ID | Requirement | Invariant | Required Evidence | Owning Slice |
|---|---|---|---|---|
| R-01 | SQLite durable change log `conversation_deltas` with monotonic autoincrement sequence | Every message write generates a persistent delta record in the same transaction | SQLite schema & repository unit tests | P1-A |
| R-02 | `resume_conversation_deltas` IPC endpoint for gap detection & replay | Replays all missed deltas with `sequence > last_applied_sequence` in order | Rust command test & replay test | P1-A |
| R-03 | Omnichannel delta publication (Zalo, WhatsApp, Messenger) | Inbound messages on all 3 channels publish hydrated render-ready deltas with durable sequence | Supervisor tests for Zalo, WhatsApp, Messenger | P1-B |
| R-04 | Stale snapshot overwrite protection in `timelineState.ts` | In-flight or refresh snapshots never erase messages newer than snapshot boundary | Unit tests in `timelineState.spec.ts` | P2-A |
| R-05 | Zero-IPC first-paint hot path and paired invalidation suppression | Applying message delta marks variant reconciled and suppresses timeline IPC refetch | Component spec test in `ConversationWorkspace.spec.ts` | P2-B |
| R-06 | Frontend gap detection and auto-resume | Gap in sequence (`delta.seq > lastSeq + 1`) triggers `resume_conversation_deltas` catchup | Spec test simulating sequence gap and recovery | P2-B |
| R-07 | Frame-level DOM paint telemetry (`requestAnimationFrame`) | Real paint time is captured; WebView receive -> paint <= 16.6ms (1 frame) | Telemetry logs & benchmark ledger | P2-C |
| R-08 | Zero missing/duplicate bubbles under burst (>256 events) & reconnect | All messages rendered deterministically without duplicates or losses | Integration test with 300 burst messages | P1-C & P2-D |

## Acceptance Criteria

1. SQLite table `conversation_deltas` stores durable deltas with monotonic sequences.
2. `resume_conversation_deltas` retrieves missed deltas accurately for replay.
3. Inbound Zalo, WhatsApp, and Facebook Messenger messages all publish hydrated `ConversationMessageDeltaEvent`.
4. `mergeTimelinePage` retains messages newer than the fetched page, eliminating bubble drop/flicker.
5. Inbound message delivery triggers direct store upsert and next-frame paint with 0 timeline IPC queries on the hot path.
6. Gap in sequence triggers auto-replay and catches up without lost bubbles.
7. Burst test (>256 events) passes with 100% bubble delivery and 0 duplicates.
8. Full build (`cargo test`, `bun test`, `bun run check`) passes cleanly with zero warnings/errors.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state across backend events, supervisors, frontend stores, and tests.
  - Work Steps: inspect source files, classify each surface, record blockers, and initialize `actual-status.md`.
  - Implementation Gate: no code edits until actual status is classified and documented.
  - Acceptance: actual status classifies all target surfaces with exact evidence IDs.

### P1: Backend Durable Change Log, Monotonic Sequence & Omnichannel Supervisors

- Phase Goal: Implement SQLite durable change sequence, transactional delta persistence, replay query IPC, and omnichannel supervisor delta publication.
- Phase Boundary:
  - In scope: `schema.sql`, `migration.rs`, `repository.rs`, `events.rs`, `commands.rs`, `service.rs`, `whatsapp/supervisor.rs`, `facebook/supervisor.rs`, backend tests.
  - Out of scope: frontend UI rendering.
  - Dependencies: SQLite migrations.
- Phase Implementation Rule: Implement `P1-A`, `P1-B`, `P1-C` in sequence with evidence and commits.
- Ordered Slice List:
  - P1-A: SQLite `conversation_deltas` schema, repository persistence, and `resume_conversation_deltas` IPC command.
  - P1-B: Omnichannel supervisor parity: wire transactional delta emission for WhatsApp, Facebook Messenger, and Zalo.
  - P1-C: Backend burst (>256 events), replay, and reconnect recovery tests.

- [x] P1-A: SQLite `conversation_deltas` schema, repository persistence, and `resume_conversation_deltas` IPC command.
  - Goal: Persist monotonic delta records in SQLite and expose replay query.
  - Scope Boundary:
    - Editable: `src-tauri/src/conversation/schema.sql`, `src-tauri/src/conversation/migration.rs`, `src-tauri/src/conversation/repository.rs`, `src-tauri/src/conversation/events.rs`, `src-tauri/src/conversation/commands.rs`.
    - Inspect-only: `src-tauri/src/conversation/types.rs`.
    - Preserve-only: existing message storage queries.
    - Out of scope: frontend bridge.
  - Non-Goals: In-memory only ring buffer.
  - Pre-flight Questions:
    - Data source: SQLite `conversation_deltas` table.
    - Display permission: internal conversation repository.
    - DB read flow: `SELECT sequence, payload_json FROM conversation_deltas WHERE sequence > ?1 ORDER BY sequence ASC LIMIT ?2`.
    - DB write flow: `INSERT INTO conversation_deltas (conversation_id, lead_id, channel, message_id, event_type, payload_json, created_at_ms) VALUES (...)`.
    - Render location: N/A (backend).
    - UI behavior flow: N/A.
    - Docker runtime: N/A (native Tauri/Cargo).
    - Playwright target: N/A.
    - Behavior test: cargo test in `repository_test.rs` and `commands_test.rs`.
    - Cleanup/quarantine: test DBs use in-memory / temporary files.
    - External side effects: none.
    - N/A notes: UI checks N/A for backend slice.
  - Work Steps:
    1. Add `conversation_deltas` table and indexes to `schema.sql` and `migration.rs`.
    2. Implement `insert_conversation_delta_tx` and `fetch_deltas_after_sequence` in `repository.rs`.
    3. Update `ConversationMessageDeltaEvent` and `events.rs` to use persistent sequence from DB.
    4. Implement `resume_conversation_deltas` Tauri command in `commands.rs`.
    5. Add unit tests for delta insertion and sequence ordering.
  - Implementation Gate: `cargo check` compiles successfully.
  - Acceptance:
    - Source: `conversation_deltas` table created, delta insertion and replay query implemented.
    - Runtime/UI: N/A.
    - DB/data: monotonic sequence verified in SQLite.
    - Behavior test: tests in `repository_test.rs` pass.
    - Cleanup/quarantine: temporary test DB cleaned up.
    - Evidence IDs: `E1-P1A-SCHEMA1`, `E1-P1A-TEST1`.
    - Actual-status rows refreshed: `conversation_deltas`, `resume_conversation_deltas`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `conversation_deltas` -> `correct`.
  - Commit Boundary: commit after P1-A passes.

- [x] P1-B: Omnichannel supervisor parity: wire transactional delta emission for WhatsApp, Facebook Messenger, and Zalo.
  - Goal: Unify message ingestion across Zalo, WhatsApp, and Facebook Messenger to write durable deltas and publish `ConversationMessageDeltaEvent`.
  - Scope Boundary:
    - Editable: `src-tauri/src/conversation/service.rs`, `src-tauri/src/conversation/whatsapp/supervisor.rs`, `src-tauri/src/conversation/facebook/supervisor.rs`.
    - Inspect-only: normalizers for each channel.
    - Preserve-only: channel-specific auth and protocol handling.
    - Out of scope: frontend UI components.
  - Non-Goals: Modifying third-party protocol payload decoders.
  - Pre-flight Questions:
    - Data source: Inbound channel messages from Zalo, WhatsApp, Messenger.
    - Display permission: internal supervisor.
    - DB read flow: message lookup by id for delta hydration.
    - DB write flow: transactional delta record write on message commit.
    - Render location: N/A.
    - UI behavior flow: N/A.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: cargo test for Zalo, WhatsApp, Messenger supervisors.
    - Cleanup/quarantine: test DBs.
    - External side effects: none.
    - N/A notes: UI checks N/A.
  - Work Steps:
    1. Update `whatsapp/supervisor.rs` to hydrate incoming message, insert delta row, and publish `ConversationMessageDeltaEvent`.
    2. Update `facebook/supervisor.rs` to hydrate incoming messages, insert delta rows, and publish `ConversationMessageDeltaEvent`.
    3. Update `service.rs` to ensure all Zalo message entry points (inbound, group, echo, outreach) record and publish deltas.
    4. Add unit and integration tests for WhatsApp and Facebook delta publication.
  - Implementation Gate: `cargo test` on supervisor test suites passes.
  - Acceptance:
    - Source: WhatsApp and Facebook supervisors publish deltas alongside Zalo.
    - Runtime/UI: N/A.
    - DB/data: Delta rows persisted for all channels.
    - Behavior test: `cargo test` passes.
    - Cleanup/quarantine: N/A.
    - Evidence IDs: `E1-P1B-ZALO1`, `E1-P1B-WA1`, `E1-P1B-FB1`.
    - Actual-status rows refreshed: `whatsapp_supervisor`, `facebook_supervisor`, `zalo_service`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `whatsapp_supervisor` -> `correct`, `facebook_supervisor` -> `correct`.
  - Commit Boundary: commit after P1-B passes.

- [x] P1-C: Backend burst (>256 events), replay, and reconnect recovery tests.
  - Goal: Prove backend handles >256 burst events without message loss, correctly recovers gaps via replay, and maintains strict sequence monotonicity.
  - Scope Boundary:
    - Editable: `src-tauri/src/conversation/service_test.rs`, `src-tauri/src/conversation/events_test.rs`.
    - Inspect-only: `events.rs`, `repository.rs`.
    - Preserve-only: production code.
    - Out of scope: frontend UI.
  - Non-Goals: Frontend mock testing in this slice.
  - Pre-flight Questions:
    - Data source: Simulated burst of 300 messages.
    - Display permission: N/A.
    - DB read flow: replay query after sequence lag.
    - DB write flow: high-frequency transactional delta writes.
    - Render location: N/A.
    - UI behavior flow: N/A.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: cargo test `test_burst_and_replay_recovery`.
    - Cleanup/quarantine: test DBs.
    - External side effects: none.
    - N/A notes: UI checks N/A.
  - Work Steps:
    1. Write integration test emitting 300 sequential messages in rapid succession.
    2. Verify all 300 deltas have monotonic sequential IDs in SQLite.
    3. Simulate broadcast receiver lag (dropped events in RAM broadcast) and execute `resume_conversation_deltas(last_seen_seq)`.
    4. Assert 100% of missed events are retrieved with zero loss and zero corruption.
  - Implementation Gate: `cargo test --test conversation_burst_replay` passes.
  - Acceptance:
    - Source: Test code in `service_test.rs`.
    - Runtime/UI: N/A.
    - DB/data: 300/300 rows verified in SQLite.
    - Behavior test: all burst/replay assertions pass.
    - Cleanup/quarantine: temp DB removed.
    - Evidence IDs: `E1-P1C-BURST1`.
    - Actual-status rows refreshed: `burst_replay_recovery`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `burst_replay_recovery` -> `correct`.
  - Commit Boundary: commit after P1-C passes.

### P2: Frontend Resumable Bridge, Gap Detection, Stale Snapshot Protection & Zero-IPC Hot Path

- Phase Goal: Implement client-side sequence tracking, gap detection & auto-replay, stale snapshot overwrite protection, paired invalidation suppression, and rAF DOM paint telemetry.
- Phase Boundary:
  - In scope: `timelineState.ts`, `conversationService.ts`, `ConversationWorkspace.svelte`, frontend spec tests.
  - Out of scope: Rust backend modifications (done in P1).
  - Dependencies: P1 backend IPC endpoints.
- Phase Implementation Rule: Implement `P2-A`, `P2-B`, `P2-C`, `P2-D` in sequence with evidence and commits.
- Ordered Slice List:
  - P2-A: `timelineState.ts` watermark-protected snapshot merge and stale-response guard.
  - P2-B: `conversationService.ts` & `ConversationWorkspace.svelte` sequence consumption, gap detection, resume replay, and paired invalidation IPC suppression.
  - P2-C: Frontend DOM paint telemetry with `requestAnimationFrame` and SLO measurement.
  - P2-D: Frontend integration & adversarial spec tests (paired events, gap recovery, stale snapshot race, duplicate sequence).

- [x] P2-A: `timelineState.ts` watermark-protected snapshot merge and stale-response guard.
  - Goal: Protect direct-upserted messages from being dropped by delayed in-flight snapshot responses.
  - Scope Boundary:
    - Editable: `src/lib/components/conversation/timelineState.ts`, `src/lib/components/conversation/timelineState.spec.ts`.
    - Inspect-only: `src/lib/types/conversation.ts`.
    - Preserve-only: pagination logic for older messages.
    - Out of scope: Svelte UI layout.
  - Non-Goals: Rewriting timeline data structure.
  - Pre-flight Questions:
    - Data source: `TimelineVariant`, `TimelinePage`.
    - Display permission: conversation store.
    - DB read flow: N/A.
    - DB write flow: N/A.
    - Render location: message list items.
    - UI behavior flow: timeline message array merge.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: `bun test src/lib/components/conversation/timelineState.spec.ts`.
    - Cleanup/quarantine: in-memory state.
    - External side effects: none.
    - N/A notes: DB checks N/A for pure frontend store.
  - Work Steps:
    1. Update `mergeTimelinePage` in `timelineState.ts`: when `mode === 'refresh'`, retain all items from `current.items` that are newer than the top message in `page.items` (by `occurredAt` / `id`), as well as optimistic items (`id < 0` / queued / sending).
    2. Add unit tests in `timelineState.spec.ts` proving that a newly upserted delta message is preserved even if a delayed snapshot without that message resolves.
  - Implementation Gate: `bun test src/lib/components/conversation/timelineState.spec.ts` passes.
  - Acceptance:
    - Source: `timelineState.ts` updated with watermark guard.
    - Runtime/UI: Bubble remains visible when stale snapshot resolves.
    - DB/data: N/A.
    - Behavior test: unit test passes with zero dropped messages.
    - Cleanup/quarantine: N/A.
    - Evidence IDs: `E2-P2A-MERGE1`.
    - Actual-status rows refreshed: `timeline_state_merge`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `timeline_state_merge` -> `correct`.
  - Commit Boundary: commit after P2-A passes.

- [x] P2-B: `conversationService.ts` & `ConversationWorkspace.svelte` sequence consumption, gap detection, resume replay, and paired invalidation IPC suppression.
  - Goal: Consume durable sequences, detect gaps, auto-resume via IPC, and eliminate timeline IPC refetches on message delta receipt.
  - Scope Boundary:
    - Editable: `src/lib/services/conversationService.ts`, `src/lib/components/conversation/ConversationWorkspace.svelte`.
    - Inspect-only: `src/lib/types/conversation.ts`.
    - Preserve-only: receipt/reaction invalidation logic.
    - Out of scope: styling/layout.
  - Non-Goals: Modifying other workspace tabs.
  - Pre-flight Questions:
    - Data source: `onConversationMessageDelta`, `resumeConversationDeltas` IPC.
    - Display permission: open conversation.
    - DB read flow: IPC `resume_conversation_deltas` on gap.
    - DB write flow: N/A.
    - Render location: Chat message bubble list, Sidebar inbox item preview.
    - UI behavior flow: Immediate store upsert and next-frame render.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: `bun test src/lib/components/conversation/ConversationWorkspace.spec.ts`.
    - Cleanup/quarantine: event listener unlisteners.
    - External side effects: none.
    - N/A notes: DB write checks N/A.
  - Work Steps:
    1. Add `resumeConversationDeltas(lastSeq)` to `conversationService.ts`.
    2. In `ConversationWorkspace.svelte`, track `lastAppliedDeltaSequence`.
    3. In `handleMessageDelta`:
       - Check `delta.sequence <= lastAppliedDeltaSequence` -> ignore duplicate.
       - Check `delta.sequence > lastAppliedDeltaSequence + 1` -> gap detected, call `resumeConversationDeltas` to replay missed deltas.
       - Apply message delta synchronously to `currentVariant.items` and sidebar inbox item.
       - Update `lastAppliedDeltaSequence = delta.sequence`.
    4. In `handleInvalidatedEvent`:
       - If `event.reason === 'message'` and direct delta upsert handled the message, suppress `refreshOpenTimeline()` (do NOT make 50-message IPC call).
       - For metadata/receipt/reaction/account_status, retain normal invalidation refresh.
    5. Handle partial listener cleanup in `conversationService.ts`.
  - Implementation Gate: `bun run check` has 0 errors.
  - Acceptance:
    - Source: sequence tracking, gap replay, and invalidation suppression in place.
    - Runtime/UI: Message renders immediately with 0 timeline IPC calls.
    - DB/data: N/A.
    - Behavior test: component tests pass.
    - Cleanup/quarantine: bridge unlisten cleans up all listeners.
    - Evidence IDs: `E2-P2B-BRIDGE1`, `E2-P2B-SUPPRESS1`.
    - Actual-status rows refreshed: `frontend_bridge`, `conversation_workspace_events`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `frontend_bridge` -> `correct`, `conversation_workspace_events` -> `correct`.
  - Commit Boundary: commit after P2-B passes.

- [x] P2-C: Frontend DOM paint telemetry with `requestAnimationFrame` and SLO measurement.
  - Goal: Accurately measure true DOM paint timestamp via `requestAnimationFrame` markers and log commit-to-paint and delta-to-paint latencies.
  - Scope Boundary:
    - Editable: `src/lib/components/conversation/ConversationWorkspace.svelte`, `src/lib/services/conversationTelemetry.ts` (or inline telemetry helper).
    - Inspect-only: Svelte reactivity lifecycle.
    - Preserve-only: user interface layout.
    - Out of scope: backend code.
  - Non-Goals: External telemetry server upload.
  - Pre-flight Questions:
    - Data source: `delta.emittedAtMs`, `performance.now()`, `requestAnimationFrame`.
    - Display permission: debug console / telemetry metrics.
    - DB read flow: N/A.
    - DB write flow: N/A.
    - Render location: DOM paint frame.
    - UI behavior flow: rAF post-render callback.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: `bun test` suite.
    - Cleanup/quarantine: N/A.
    - External side effects: none.
    - N/A notes: DB checks N/A.
  - Work Steps:
    1. In `ConversationWorkspace.svelte`, record `performance.now()` / timestamp when delta arrives.
    2. Register `requestAnimationFrame(() => { requestAnimationFrame(() => { ... }) })` or single-frame rAF marker to measure post-paint completion.
    3. Log structured telemetry: `deltaToPaintMs` (WebView receive -> paint) and `dbCommitToPaintMs` (emittedAtMs -> paint).
  - Implementation Gate: `bun test` passes.
  - Acceptance:
    - Source: rAF telemetry implemented in `ConversationWorkspace.svelte`.
    - Runtime/UI: Accurate DOM paint timing measured.
    - DB/data: N/A.
    - Behavior test: telemetry verified in tests.
    - Cleanup/quarantine: N/A.
    - Evidence IDs: `E2-P2C-TELEMETRY1`.
    - Actual-status rows refreshed: `dom_paint_telemetry`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `dom_paint_telemetry` -> `correct`.
  - Commit Boundary: commit after P2-C passes.

- [x] P2-D: Frontend integration & adversarial spec tests.
  - Goal: Verify paired invalidation+delta path (0 timeline IPC calls), gap recovery, stale snapshot race, and duplicate delta idempotency.
  - Scope Boundary:
    - Editable: `src/lib/components/conversation/ConversationWorkspace.spec.ts`.
    - Inspect-only: `ConversationWorkspace.svelte`.
    - Preserve-only: existing test cases.
    - Out of scope: production code changes.
  - Non-Goals: Manual QA in this slice.
  - Pre-flight Questions:
    - Data source: Mocked Tauri IPC & event listeners.
    - Display permission: test runner.
    - DB read flow: mocked IPC.
    - DB write flow: N/A.
    - Render location: rendered DOM in happy-dom.
    - UI behavior flow: simulated incoming events.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: `bun test src/lib/components/conversation/ConversationWorkspace.spec.ts`.
    - Cleanup/quarantine: test cleanup after each test.
    - External side effects: none.
    - N/A notes: DB checks N/A.
  - Work Steps:
    1. Add test: Real production event pair (both `conversation-invalidated` and `conversation-message-delta` emitted) asserts immediate bubble insertion and ZERO `fetchConversationMessages` IPC calls on the hot path.
    2. Add test: Sequence gap (`seq = 5` received after `seq = 2`) triggers `resume_conversation_deltas` and renders all messages in order.
    3. Add test: Stale snapshot arriving after delta upsert does NOT drop the new message bubble.
    4. Add test: Duplicate delta with `seq <= lastApplied` is ignored without duplicate bubbles.
  - Implementation Gate: all tests in `ConversationWorkspace.spec.ts` pass.
  - Acceptance:
    - Source: 4 new comprehensive adversarial test cases added.
    - Runtime/UI: Verified 100% pass rate.
    - DB/data: N/A.
    - Behavior test: `bun test` passes.
    - Cleanup/quarantine: N/A.
    - Evidence IDs: `E2-P2D-TESTS1`.
    - Actual-status rows refreshed: `frontend_tests`.
  - Evidence Targets: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`.
  - Actual-status Update: `frontend_tests` -> `correct`.
  - Commit Boundary: commit after P2-D passes.

### P3: Full Build, Verification & Completion Lock

- Phase Goal: Run complete clean full build, execute all regression suites, verify Completion Lock, and submit for supervisor approval.
- Phase Boundary:
  - In scope: full build, tests, ledgers, Supervisor review.
  - Out of scope: new feature additions.
  - Dependencies: P1 and P2 completion.
- Phase Implementation Rule: Execute P3-A, P3-B, P3-C.
- Ordered Slice List:
  - P3-A: Full clean build and regression verification (`cargo check`, `cargo test`, `bun run check`, `bun test`, `bun run build`).
  - P3-B: Supervisor re-review acceptance loop.
  - P3-C: Dead-work cleanup and plan closure.

- [x] P3-A: Full clean build and regression verification.
  - Goal: Verify full workspace compiles cleanly and all test suites pass.
  - Work Steps:
    1. Terminate any stale processes if holding locks.
    2. Run `cargo test` across all conversation modules.
    3. Run `bun run check` for TypeScript / Svelte 5 types.
    4. Run `bun test` for all frontend unit & component specs.
    5. Run `bun run build` for production frontend bundle.
    6. Record results in evidence and benchmark ledgers.
  - Implementation Gate: all P1 and P2 slices completed.
  - Acceptance: zero build errors, zero test failures.

- [x] P3-B: Call supervisor for implemented-plan acceptance loop.
  - Goal: verify completed work against accepted plan, actual status, evidence, benchmark, and Completion Lock.
  - Work Steps:
    1. Call supervisor review on the full diff and ledgers.
    2. If issues found, remediate and re-review.
    3. Ensure Supervisor gives PASS verdict.
  - Implementation Gate: P3-A passed.
  - Acceptance: Supervisor PASS verdict.

- [x] P3-C: Remove dead work and close plan.
  - Goal: Clean up any temporary artifacts and finalize ledgers.
  - Work Steps:
    1. Verify no orphan test files or temp debug artifacts.
    2. Run `anvien detect-changes --repo rustsale --scope all` or git diff check.
    3. Record final commit and mark plan complete.
  - Implementation Gate: P3-B passed.
  - Acceptance: Clean git worktree and completed ledgers.

## Risk Notes

- **Database Migration**: Adding `conversation_deltas` uses `CREATE TABLE IF NOT EXISTS` and is backwards-compatible.
- **Broadcast Buffer Saturation**: Handled by durable SQLite persistence and `resume_conversation_deltas` on lag.
- **Memory Footprint**: `conversation_deltas` is lightweight (~200 bytes per message) and can be vacuumed/pruned if needed.
