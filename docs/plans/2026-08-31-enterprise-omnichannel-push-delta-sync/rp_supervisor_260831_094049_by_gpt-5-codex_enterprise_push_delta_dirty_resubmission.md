# Supervisor Report: Enterprise Push-Delta Dirty-Worktree Resubmission

Verdict: REJECT

## Metadata

- Report file: `rp_supervisor_260831_094049_by_gpt-5-codex_enterprise_push_delta_dirty_resubmission.md`
- Review time: `260831 09:40:49 UTC+7`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: HEAD `4f19d38bb42ec49aa3ccf7e8623d57affaae7531` plus the exact uncommitted tracked diff captured at review start
- Dirty-diff SHA-256: `b78015d62e8a0a473e9d6f0c43921bea0e4076afcad99971a6a12c5c889a691a`
- Diff size: 27 tracked files, 818 insertions, 314 deletions; scoped conversation source-name hash `22b5ed910c94836ddcde2b927e3edcfc1ea697ad8209aff0d0a315539b8158ad`
- Claim reviewed: newest remediation closes report `rp_supervisor_260831_080138...` and is production-ready
- Authority: user request, raw `AGENTS.md`, working-rules, supervisor and QA skills, Completion Harness, previous report, exact source/diff/runtime/DB state
- Boundary: independent review only. No production code, test, plan, or commit was changed; this report is the only new artifact from this review

## Executive Summary

The resubmission genuinely repairs several backend atomicity paths: WhatsApp now returns its transaction-created delta, Facebook propagates delta failures and publishes per webhook event, and direct/outbound message repositories return deltas created before message commit. However, production readiness is still blocked by four executable failure modes: no recovery trigger for a dropped final event or listener reconnect, a single 60 ms fallback timer that loses missing-message obligations during bursts, a snapshot guard whose DB sequence comparison is bypassed by the old unbounded message-ID set, and an upgrade migration that adds a unique index without reconciling duplicate rows produced by the previous implementation. Exact packaged-runtime and raw latency evidence also remains absent.

Required outcome: close the same invariants at source level, add adversarial tests that exercise the real boundaries, commit the slice, then build/package and validate that exact artifact on a migrated production-shaped profile.

## Blocking Findings

### [HIGH] Final-event loss and listener reconnect still have no recovery trigger

File: `src-tauri/src/conversation/events.rs:87-103`; `src/lib/components/conversation/ConversationWorkspace.svelte:807-810,1286-1297`

Issue: backend broadcast lag only writes a log. The frontend requests a high-water mark once on component mount; no bridge reconnect callback, periodic reconciliation, or frontend lag event invokes catch-up afterward.

Evidence:

- `events.rs:87-103` handles `RecvError::Lagged` with `tracing::warn!` and emits no recovery signal.
- `ConversationWorkspace.svelte:807-810` invokes `initializeHighWaterMarkAndCatchUp()` only inside `onMount`.
- On a fresh component, `:1290-1293` sets `lastSeenSequence` directly to the database maximum without replaying. `minSequence` is defined by the IPC contract but never consumed anywhere in frontend source.
- Gap replay still requires a later observed event (`:1442-1447`). A final dropped event has no later sequence to expose the gap.

Why this blocks acceptance: an app that stays mounted can permanently miss the last message after broadcast lag or listener interruption—the original delayed/missing bubble invariant.

Fix direction: wire an explicit lag/reconnect recovery signal or bounded high-water reconciliation loop; on (re)connect negotiate min/max, choose replay versus authoritative resnapshot, and persist or safely renegotiate the cursor.

Re-review evidence required: production-boundary tests for first mount, remount, listener reconnect, broadcast lag, and final-event loss with no later event.

### [HIGH] The 60 ms invalidation fallback loses obligations during message bursts

File: `src/lib/components/conversation/ConversationWorkspace.svelte:334,1506-1518`

Issue: every missing message uses the same `timelineRefreshTimer`. A later invalidation cancels the earlier timer, and the replacement callback checks only the later message ID.

Counterexample: invalidation A arrives and delta A is lost; invalidation B arrives within 60 ms and clears A's timer; delta B arrives successfully; B's callback sees B rendered and performs no refresh. Message A remains missing forever. If `messageId` is absent/zero, `stillMissing` is falsy and no refresh occurs.

Why this blocks acceptance: the fallback does not guarantee convergence under ordinary burst ordering, exactly where event loss is most likely.

Fix direction: track a set/map of pending invalidation message IDs per timeline and coalesce one refresh when any remains unresolved; define behavior for an absent message ID.

Re-review evidence required: fake-timer component test with two invalidations where the first delta is lost and the second arrives, plus missing-ID behavior and real mounted burst evidence.

### [HIGH] DB snapshot watermark is present in the contract but bypassed in the merge

File: `src/lib/components/conversation/ConversationWorkspace.svelte:1071-1075,1237-1238,1350-1351`; `src/lib/components/conversation/timelineState.ts:163-176`; `src-tauri/src/conversation/repository.rs:2181-2243`

Issue:

- Refresh passes both the legacy `deltaUpsertedMessageIds` set and `messageSequenceMap`.
- `timelineState.ts:168-176` retains an item immediately when its ID is in that set, before comparing its sequence to `page.snapshotSequence`. Every applied delta is inserted into both collections, and neither is pruned. Therefore the DB watermark cannot authorize reconciliation of any delta-upserted row.
- The repository reads global `MAX(sequence)` at `repository.rs:2182-2189`, then queries messages and hydrates them in later autocommit statements (`:2196-2243`) rather than one SQLite read transaction. The returned page and watermark are not one consistent snapshot.

Why this blocks acceptance: the implementation remains a process-lifetime pseudo-watermark and can indefinitely retain rows excluded by an authoritative snapshot. It does not prove the claimed DB-authoritative stale-response invariant.

Fix direction: remove the unconditional ID-set bypass, retain only messages whose recorded delta sequence is greater than the page boundary, prune acknowledged mappings, and obtain page rows/hydration/watermark from one consistent database snapshot with documented boundary semantics.

Re-review evidence required: out-of-order provider timestamp, authoritative removal/exclusion, concurrent snapshot/delta commit, and map-pruning tests.

### [HIGH] Unique-index migration can fail on the exact prior schema state

File: `src-tauri/src/conversation/migration.rs:252-269`; `src-tauri/src/conversation/schema.sql:553-567`

Issue: the migration executes `CREATE UNIQUE INDEX ... (message_id,event_type)` directly. It does not detect, merge, or delete duplicate rows. The preceding reviewed implementation could create multiple durable sequences for one message through Facebook webhook and duplicate Zalo paths.

Why this blocks acceptance: an existing database that exercised those paths can fail migration/startup instead of upgrading. The new index is correct for fresh databases but unsafe as an upgrade.

Fix direction: add deterministic duplicate reconciliation in the migration, preserve the chosen canonical sequence/payload contract, then create the unique index transactionally.

Re-review evidence required: migration test seeded with duplicate delta rows from the prior schema, including foreign-key/message deletion behavior and idempotent rerun.

### [HIGH] Facebook batch delta is hydrated before its attachments are inserted

File: `src-tauri/src/conversation/facebook/normalizer.rs:263-286,292-320`

Issue: batch ingestion calls `record_message_delta_tx` and stores the serialized hydrated event before inserting the message's attachments. The published/replayed payload for an attachment message therefore cannot contain attachments added later in the same transaction.

Why this blocks acceptance: the claimed render-ready per-message Facebook history/batch delta is incomplete; the UI may render a generic/empty bubble until an invalidation refresh.

Fix direction: insert all message children first, then hydrate and record the durable delta.

Re-review evidence required: Facebook batch/history attachment test asserting the emitted and replayed delta contains the attachment projection.

### [HIGH] Failure and recovery tests still do not kill the superficial implementation

File: `src-tauri/src/conversation/repository_test.rs:6703+`; affected frontend/test files

Issue:

- The rollback test is improved: it manually inserts a message, records a delta, then intentionally drops the transaction. It still does not inject a delta INSERT/hydration failure through a real Zalo, WhatsApp, Facebook, direct-start, or outbound entrypoint and prove that the production call returns an error with neither row committed.
- No frontend tests were modified in this resubmission. Repository search found no test for high-water min/max semantics, initial cursor negotiation, reconnect/final loss, burst fallback cancellation, DB snapshot consistency, or message-sequence map pruning.
- No migration test seeds existing duplicate delta rows.

Why this blocks acceptance: Completion Harness Tier 3 requires dependency-failure and superficial-patch-killer evidence at the authoritative boundary, not a manually abandoned happy-path transaction.

Fix direction: add a trigger/constraint fault that makes delta persistence fail after message insertion, invoke each production family, and assert fail-closed rollback; add the recovery/migration/race tests named above.

## Partial Clearances

- WhatsApp live/history ingestion now records a delta in the same normalizer transaction and returns it for publication (`whatsapp/normalizer.rs:1098-1134,1288-1314`; `whatsapp/supervisor.rs:393-417,541-550`).
- Facebook batch/webhook no longer swallows transaction-helper errors and the webhook supervisor publishes returned per-event deltas (`facebook/normalizer.rs:273-285,586-599`; `facebook/supervisor.rs:339-365`).
- Direct-start and outbound message/batch repository paths now record deltas before the message transaction commits (`repository.rs:3250-3346,3886-3937,4644-4655`; `service.rs:398-412,674-687`).
- Duplicate Zalo publication no longer creates another delta in `service.rs`; the SQLite unique index is correct for fresh databases.
- Durable sequence remains SQLite-authoritative; the volatile event counter is absent.
- High-water IPC/types/registration exist (`commands.rs:1721+`, `types.rs:737-749`, `lib.rs:507+`).
- Telemetry summary now calculates both delta-to-paint and DB-to-paint statistics in memory, but no fresh raw E4 samples establish the SLO and `emittedAtMs` is still captured before commit.

## Source-Level Clearance Notes

| Production group | State | Evidence |
|---|---|---|
| `schema.sql`, `migration.rs` | Blocked | Fresh-schema uniqueness is clear; prior-row reconciliation is missing at `migration.rs:252-269`. |
| `repository.rs`, `types.rs`, `commands.rs`, `lib.rs` | Partial | Transactional outbound and watermark contracts exist; snapshot statements are not one read transaction and replay silently skips malformed payload JSON at `repository.rs:3146-3153`. |
| `service.rs` | Clear for reviewed write-path delta duplication | Uses transaction-returned deltas; outreach fallback does not create a new message. |
| `whatsapp/normalizer.rs`, `whatsapp/supervisor.rs` | Clear for same-transaction delta path | Returned `AppliedWhatsAppMessage` carries the committed delta; publication follows commit. |
| `facebook/normalizer.rs`, `facebook/supervisor.rs` | Blocked | Atomic/error propagation improved, but batch attachment delta is serialized before attachments. |
| Zalo paths | Partial | Existing `apply_normalized_message` same-transaction path remains intact; mounted live/history behavior is unverified. |
| `conversationService.ts` | Partial | High-water invoke and bridge retry exist; no reconnect/lag callback drives reconciliation. |
| `ConversationWorkspace.svelte` | Blocked | Mount-only cursor initialization, final-loss gap, burst-unsafe fallback, and unbounded watermark collections remain. |
| `timelineState.ts` | Blocked | DB sequence branch exists but is shadowed by unconditional ID-set retention. |

## Evidence Checked

Passed:

- Rules/skills/harness and previous report were read from current files.
- Agentmemory was queried; mem0 was not exposed in this session and could not be queried.
- `anvien --help` and `anvien analyze --force`: 528 files scanned, 381 code files parsed, 0 failures, 84,009 nodes, 96,963 relationships.
- Anvien file detail: `repository.rs` HIGH risk, 108 inbound/43 outbound references and 10 linked flows; `ConversationWorkspace.svelte` HIGH risk with 54 linked flows.
- Exact diff hashes/status, focused source diffs, numbered source, test assertions, process inventory, artifact timestamps, and read-only production DB inspection were checked fresh.

Failed/blocking observations:

- No `rustsale-crm`, Tauri, Cargo, or Bun build/test process was running.
- Release binary and `.deb` are dated `2026-08-29`; debug binary is dated `2026-08-31 06:16`, all predating this dirty implementation.
- Production DB `/home/arch/.local/share/com.rustsale.crm/rustsale.db` contains 8,557 messages and still has no `conversation_deltas` table.
- The implementation is uncommitted, so no exact commit or packaged artifact provenance exists.

Not run:

- Broad Cargo/Bun suites, typecheck, frontend build, Cargo build, and package/release build were not run after decisive source blockers were proven.
- No mounted desktop QA, screenshot, real channel message, runtime reload, or process termination was performed.
- Claimed historical green counts were not used as evidence for this dirty snapshot.

## Invariant Closure

- Affected invariant: every committed message must converge exactly once into the visible timeline across channel, restart/reconnect, loss, burst, snapshot, and migration boundaries.
- Sibling surfaces checked: Zalo repository/service, WhatsApp live/history normalizer and supervisor, Facebook batch/webhook normalizer and supervisor, direct-start/outbound repository/service, schema/migration/replay IPC, event forwarder, bridge, workspace, timeline merge, tests, runtime artifacts, and production DB.
- Residual unverified/broken surfaces: final-event recovery, reconnect negotiation, retention-floor use, burst fallback, consistent DB snapshot boundary, existing-data migration, Facebook batch attachments, real failure injection, telemetry samples, exact packaged desktop runtime, and production migration.

## Required Fix List For Resubmission

1. Provide a reconnect/final-loss recovery trigger and consume both high-water maximum and retention minimum with replay/resnapshot semantics.
2. Replace the single-message fallback timer with pending-message convergence logic that survives bursts and absent IDs.
3. Make timeline page data and snapshot sequence one consistent DB snapshot; remove the ID-set bypass and prune acknowledged message-sequence mappings.
4. Reconcile duplicate rows before creating the unique index and test upgrade/idempotency.
5. Record Facebook batch deltas only after attachments and all child projections are inserted.
6. Add real delta-failure injection through production entrypoints and adversarial recovery/race tests.
7. Commit the completed slice, run fresh full gates, build/package that exact commit, migrate a production-shaped database, mount the desktop artifact, and capture official raw p50/p95/p99 evidence.

## Overall Evaluation

Backend atomicity is materially better than in report 080138, but the end-to-end loss/recovery and authoritative snapshot contracts are still incomplete. Evidence is limited to dirty source and narrow tests; production runtime provenance is absent. Completion Lock remains closed.

Handoff to Main - remediate the six source invariants above before requesting another production-readiness review.
