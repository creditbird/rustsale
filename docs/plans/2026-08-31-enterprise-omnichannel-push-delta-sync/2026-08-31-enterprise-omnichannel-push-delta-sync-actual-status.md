# Enterprise Omnichannel Push-Delta Sync Actual Status

Title: Enterprise Omnichannel Push-Delta Sync
Date: 2026-08-31
Status: P0 Complete
Companion plan: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-plan.md`
Companion evidence: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`
Companion benchmark: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-benchmark.md`

## Purpose

This file records the real current state before implementation.

Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

This file does not replace `evidence.md`. It classifies current state from evidence.

Use exact evidence IDs from `evidence.md`, such as `E0-P0A-SRC1`, not broad section IDs such as `E0` or `E1`.

## Freshness / Refresh Rules

This actual-status file is a living current-state record, not a one-time P0 snapshot.

P0 records the baseline before implementation. After implementation begins, keep the Current Status Matrix updated so the next agent can trust it as the latest repo reality.

Update this file:

- after each completed implementation slice;
- before starting the next phase if repo state changed;
- whenever evidence changes a current-state classification;
- whenever the next phase's status assumptions, next action, or work steps need updating because reality differs from the previous status.

## Scope

Target scope:

- SQLite durable delta change sequence and replay queries in `schema.sql`, `migration.rs`, `repository.rs`.
- Event forwarder and IPC command `resume_conversation_deltas` in `events.rs`, `commands.rs`.
- Omnichannel supervisor delta emission in `whatsapp/supervisor.rs`, `facebook/supervisor.rs`, `service.rs`.
- Frontend store watermark merge in `timelineState.ts`.
- Frontend bridge, gap detection, resume replay, and invalidation suppression in `conversationService.ts`, `ConversationWorkspace.svelte`.
- Frame-level DOM paint telemetry (`requestAnimationFrame`) in `ConversationWorkspace.svelte`.

Out of scope:

- Outbound message send optimizations (already completed in `161fd5b`).
- External channel provider auth protocols.

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|-----------------------|----------------------|--------------------|----------------------|-------------|
| `src-tauri/src/conversation/events.rs` | `E0-P0A-SRC1` | 8 | Event publisher and forwarder for Tauri frontend | Medium scope warning |
| `src-tauri/src/conversation/repository.rs` | `E0-P0A-SRC2` | 15 | SQLite database repository for conversation and messages | High scope warning |
| `src-tauri/src/conversation/service.rs` | `E0-P0A-SRC3` | 12 | Conversation service orchestrator | High scope warning |
| `src-tauri/src/conversation/whatsapp/supervisor.rs` | `E0-P0A-SRC4` | 6 | WhatsApp message listener and ingestion | Medium scope warning |
| `src-tauri/src/conversation/facebook/supervisor.rs` | `E0-P0A-SRC5` | 5 | Facebook webhook & sync supervisor | Medium scope warning |
| `src/lib/components/conversation/timelineState.ts` | `E0-P0A-SRC6` | 4 | In-memory timeline message merge helper | Low scope warning |
| `src/lib/services/conversationService.ts` | `E0-P0A-SRC7` | 7 | Frontend Tauri event bridge and API service | Medium scope warning |
| `src/lib/components/conversation/ConversationWorkspace.svelte` | `E0-P0A-SRC8` | 14 | Main conversation UI component | High scope warning |

## Status Rules

| Status | Meaning | Allowed next action |
|--------|---------|---------------------|
| `correct` | Already behaves as required. | Preserve. Add evidence or tests only if needed. |
| `partial` | Some required behavior exists, but gaps remain. | Change only the missing parts. Preserve correct parts. |
| `wrong` | Current behavior, source, or contract is incorrect. | Replace with required behavior. Record the exact reason. |
| `missing` | Required behavior, source, or contract does not exist. | Implement the missing piece only. |
| `unbound` | Surface exists but is not wired to the real source, flow, or contract. | Bind to the real source only. Preserve approved surface. |
| `fake-or-stub` | Prototype, demo, mock, fallback, or placeholder data is being used as real behavior. | Remove fake behavior or replace it with an approved truthful state. |
| `blocked` | Source, authority, contract, or required evidence is unclear. | Stop. Do not implement until resolved. |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|------|---------------|----------------|--------|--------------------|----------|--------------------|
| Message + Delta Transaction Atomicity | `record_message_delta_tx` inside exact same SQLite transaction | `record_message_delta_tx` inside exact same SQLite transaction; rollback together | `correct` | 5 related files | `E1-P1A-TX1` | preserve |
| Sequence Authority in `events.rs` | 100% sequences originate monotonically from SQLite `conversation_deltas` table | Remove volatile sequence; 100% sequences originate from SQLite `conversation_deltas` | `correct` | 3 related files | `E1-P1B-SEQ1` | preserve |
| Facebook Batch Delta Emission | Emits 1 transactional delta per inserted message in batch | Emits 1 transactional delta per inserted message in batch | `correct` | 4 related files | `E1-P1C-FB1` | preserve |
| History Sync & Attachment Invalidation | Emits distinct non-suppressed reasons (`history_sync`, `attachment_update`, `metadata`) | Emits distinct non-suppressed reasons (e.g. `history_sync`, `attachment_update`) | `correct` | 6 related files | `E1-P1C-REASONS1` | preserve |
| Replay Protocol & Gap Catch-up | Subscribe -> Buffer live -> Paginated loop to high-water mark -> Monotonic apply -> Drain buffer | Subscribe -> Buffer live -> Paginated loop to high-water mark -> Monotonic apply -> Drain buffer | `correct` | 4 related files | `E2-P2A-REPLAY1` | preserve |
| Snapshot Watermark & Deduplication | Merges against DB sequence watermark; deduplicates by sequence before store mutation | Merges against DB sequence watermark; deduplicates by sequence before store mutation | `correct` | 4 related files | `E2-P2B-WATERMARK1` | preserve |
| DOM Paint Telemetry Samples | Collects raw samples array; calculates p50/p95/p99 metrics into persistent summary | Collects raw samples array; calculates p50/p95/p99 metrics into persistent summary | `correct` | 2 related files | `E2-P2C-TELEMETRY1` | preserve |
| Git Hygiene & Worktree Cleanliness | Clean `git diff --check`, structured commit slices with clear rollback anchors | Clean `git diff --check`, structured commit slices with clear rollback anchors | `correct` | 23 related files | `E3-P3B-GITHYGIENE1` | preserve |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---------|------|------------|----------------|----------------|----------|-------------------|
| R0 | 2026-08-31 | baseline before P0 | Full Omnichannel Push-Delta Scope | Initial baseline classification | `E0-P0A-SRC1..E0-P0A-SRC8` | Proceed to P1-A |
| R1 | 2026-08-31 | Initial remediation | All P1, P2, P3 scopes | Updated to `correct` (provisional) | `E1-P1A-SCHEMA1..E3-P3A-CARGOBIN` | Submitted to Supervisor |
| R2 | 2026-08-31 | Supervisor Re-Review REJECT | 8 Architectural Blockers Identified | Reclassified to `wrong`/`partial` | `rp_supervisor_260831_063454` | Proceed to Phase P1 remediation |
| R3 | 2026-08-31 | Remediation Completed | All 8 Blockers Architecturally Fixed & Tested | Reclassified all rows to `correct` | `E1-P1A-TX1..E3-P3B-GITHYGIENE1` | Submitted to Supervisor |
| R4 | 2026-08-31 | Supervisor 4-Blocker Remediation | Fallback Replay Await, Cursor/Buffer Safety, Adversarial Tests 28a/b/c, DB Migration, Tauri Distributable | Reclassified all rows to `correct` with transactional safety proofs | `E2-P2A-REPLAY1`, `E2-P2D-TESTS1`, `E3-P3A-DIST1` | Submit for Final Supervisor Acceptance |

## Phase Touch Map

| Unit / File / Surface | Plan-Relevant Relationship File | Relationship to Target | Plan Item | Touch Mode | Evidence | Constraint |
|-----------------------|---------------------------------|------------------------|-----------|------------|----------|------------|
| `schema.sql` | `src-tauri/src/conversation/schema.sql` | source-of-truth | P1-A | edit | `E0-P0A-SRC2` | Add `conversation_deltas` table |
| `migration.rs` | `src-tauri/src/conversation/migration.rs` | source-of-truth | P1-A | edit | `E0-P0A-SRC2` | Add migration step for `conversation_deltas` |
| `repository.rs` | `src-tauri/src/conversation/repository.rs` | source-of-truth | P1-A | edit | `E0-P0A-SRC2` | Add delta insertion & replay queries |
| `events.rs` | `src-tauri/src/conversation/events.rs` | source-of-truth | P1-A | edit | `E0-P0A-SRC1` | Forward DB sequence in delta event |
| `commands.rs` | `src-tauri/src/conversation/commands.rs` | source-of-truth | P1-A | edit | `E0-P0A-SRC1` | Expose `resume_conversation_deltas` |
| `whatsapp/supervisor.rs` | `src-tauri/src/conversation/whatsapp/supervisor.rs` | consumer | P1-B | edit | `E0-P0A-SRC4` | Ingest message delta & publish |
| `facebook/supervisor.rs` | `src-tauri/src/conversation/facebook/supervisor.rs` | consumer | P1-B | edit | `E0-P0A-SRC5` | Ingest message delta & publish |
| `service.rs` | `src-tauri/src/conversation/service.rs` | consumer | P1-B | edit | `E0-P0A-SRC3` | Ingest message delta with DB sequence |
| `timelineState.ts` | `src/lib/components/conversation/timelineState.ts` | source-of-truth | P2-A | edit | `E0-P0A-SRC6` | Retain newer delta messages on refresh |
| `conversationService.ts` | `src/lib/services/conversationService.ts` | source-of-truth | P2-B | edit | `E0-P0A-SRC7` | Add resume delta replay call |
| `ConversationWorkspace.svelte` | `src/lib/components/conversation/ConversationWorkspace.svelte` | source-of-truth | P2-B, P2-C | edit | `E0-P0A-SRC8` | Gap detection, IPC suppression, rAF telemetry |

## Detailed Findings

### Durable Change Sequence & Replay
Current state: `events.rs` uses `AtomicU64::new(0)`. No persistent sequence table exists in SQLite. Reconnecting client cannot replay missed deltas.
Required state: `conversation_deltas` table in SQLite with `sequence INTEGER PRIMARY KEY AUTOINCREMENT`. `resume_conversation_deltas` returns ordered missed deltas.
Evidence: `E0-P0A-SRC1`, `E0-P0A-SRC2`.
Classification: `missing`.
Allowed next action: Implement table and IPC query in P1-A.

### Omnichannel Supervisor Coverage
Current state: WhatsApp and Facebook supervisors only publish `ConversationInvalidatedEvent`.
Required state: WhatsApp and Facebook supervisors publish `ConversationMessageDeltaEvent` carrying hydrated `ConversationMessage` and durable sequence.
Evidence: `E0-P0A-SRC4`, `E0-P0A-SRC5`.
Classification: `missing`.
Allowed next action: Wire delta emission in P1-B.

### Zero-IPC Hot Path & Stale Snapshot Protection
Current state: Message invalidation triggers 50-message IPC reload. `timelineState.ts` drops delta messages absent from fetched page.
Required state: Suppress timeline IPC reload on message delta. `timelineState.ts` preserves messages newer than fetched page.
Evidence: `E0-P0A-SRC6`, `E0-P0A-SRC8`.
Classification: `wrong`.
Allowed next action: Fix `timelineState.ts` in P2-A, update `ConversationWorkspace.svelte` in P2-B.

## Implementation Gate

- [x] Target scope is listed in Current Status Matrix.
- [x] Each target unit has a status.
- [x] Each status has evidence IDs.
- [x] Each target file has relationship count evidence.
- [x] Phase Touch Map lists plan-relevant relationship files.
- [x] Phase Touch Map defines touch mode for every unit.
- [x] Next phase status assumptions, next action, and work steps have been updated from this status file.
- [x] Status Refresh Log has an R0 baseline row.

## Final P0 Decision

- [x] P0 complete. Next phase can proceed unchanged.

Decision note:
Baseline classification complete. All 6 supervisor rejection root causes are mapped to specific executable slices in P1 and P2. Proceeding to P1-A.
