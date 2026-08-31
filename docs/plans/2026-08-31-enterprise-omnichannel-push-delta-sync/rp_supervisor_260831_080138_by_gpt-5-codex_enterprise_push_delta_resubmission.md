# Independent Supervisor Re-review — Enterprise Omnichannel Push-Delta Resubmission

- Review time: `2026-08-31T08:01:38+07:00`
- Reviewed commit: `4f19d38bb42ec49aa3ccf7e8623d57affaae7531`
- Commit date: `2026-08-31T07:50:07+07:00`
- Scope: the 29-file commit, exact current worktree, companion ledgers, previous rejection findings, source contracts, tests, production DB, and packaged/runtime evidence
- Role: independent review only; no production code, tests, plans, or commits were changed

## Verdict

**REJECT**

The commit is a real rollback anchor and closes several narrow issues, but the claim that all eight blockers are cleared is false. Durable delta creation is still not atomic across WhatsApp, Facebook webhook, direct-start, and outbound-batch paths; recovery has no startup/reconnect cursor negotiation and cannot recover a lost final event; message invalidation has no fallback when delta delivery fails; snapshot protection is not DB-watermark-authoritative; and no exact packaged desktop runtime or raw latency sample evidence exists.

## Decisive findings

### 1. Message and delta are still not one fail-closed transaction across production paths

- Zalo repository ingestion is a genuine closure: `repository.rs:1280-1288` creates the delta on the same `tx` before commit. Outreach also uses the transaction helper.
- WhatsApp live ingestion is not atomic. The normalizer commits/returns the message at `whatsapp/supervisor.rs:396-400`; only afterward the supervisor queries it, emits invalidation, opens a new repository transaction, and records the delta at `:401-431`. `if let Ok(...)` discards delta errors. A message can therefore exist with no durable delta.
- Facebook batch is fail-open. `facebook/normalizer.rs:269-279` wraps `record_message_delta_tx` in `if let Ok`, so a delta write/hydration failure does not fail the message transaction.
- Facebook webhook discards the in-transaction delta result entirely at `facebook/normalizer.rs:590-596`. `facebook/supervisor.rs:337-396` then deduplicates affected conversation IDs, selects only the latest message per conversation, and records another delta in a separate transaction. For multiple webhook messages in one conversation, earlier in-transaction deltas are not live-published and the latest message obtains a duplicate durable delta row.
- Direct conversation start persists the message first at `service.rs:401`, then records delta separately and fail-open at `:409-416`.
- Outbound batch persists first at `service.rs:686`, then records each delta separately and fail-open at `:687-697`.
- Duplicate Zalo ingestion creates a new durable delta for an existing message at `service.rs:471-479`, so one-message/one-delta production semantics are not enforced.
- There is no uniqueness constraint on `(message_id,event_type)` in `schema.sql:553-564`.

This directly violates R-01, P1-A, P1-B, and the claimed “zero orphan messages / exactly one producer” contract.

### 2. Replay remains unable to recover startup, reconnect, or final-event loss

- `lastSeenSequence` is component-local and initialized to `0` at `ConversationWorkspace.svelte:1229`. It is neither persisted nor negotiated with the backend.
- Mount only registers listeners at `ConversationWorkspace.svelte:805-815`; it never performs an initial replay/high-water handshake.
- Gap detection is gated on `lastSeenSequence > 0` at `:1395-1402`. If the first observed live event has sequence 500, it is accepted at `:1409` and sequences 1–499 are silently skipped.
- Replay is invoked only after a later live event reveals a gap. If the last event is dropped, no later event exists to trigger recovery. There is no periodic catch-up, explicit lag signal reaching the frontend, startup resume, or reconnect callback.
- The page loop at `:1260-1283` and live buffer at `:1284-1292,1390-1402` are useful partial closures, but they do not establish a negotiated high-water boundary. On replay error, buffer draining is skipped and no retry is scheduled (`:1294-1298`).
- Backend replay at `repository.rs:3073-3101` returns only a page of events; it returns no high-water mark or retention floor.
- The delta table has no retention/ack/resnapshot policy. The plan merely says it “can be vacuumed/pruned if needed,” which is not an executable recovery contract.

This fails restart/reconnect, final-event-loss, negotiated/persisted cursor, retention-floor, and resnapshot requirements.

### 3. Message invalidation is suppressed without proving paired delta delivery

`ConversationWorkspace.svelte:1447-1457` suppresses every open-timeline refresh whose reason is `message`. It does not check `messageId` against an applied delta and does not schedule a bounded fallback. This turns any publisher failure, bridge loss, replay failure, or final-event loss into the exact stale-bubble symptom under review.

The new `history_sync` and `attachment_update` reasons are valid partial closures (`events.rs:15-16`; Zalo/WhatsApp history and command attachment call sites), but they do not repair the missing fallback for ordinary messages.

### 4. Snapshot protection is not based on a database-authoritative watermark

- Neither `TimelinePage` nor `TimelineVariant` carries `snapshotSequence`; repository-wide search found no `snapshotSequence`/`snapshot_sequence` implementation.
- `mergeTimelinePage` accepts a process-local `Set<number>` of message IDs and otherwise compares provider timestamp/id (`timelineState.ts:125-173`). This is not a DB sequence boundary.
- The set is created at `ConversationWorkspace.svelte:1232`, appended at `:1305`, passed to refresh at `:1070`, and never cleared. It can retain removed/stale rows indefinitely and grows for the component lifetime.
- Out-of-order provider timestamps can still defeat the fallback comparison at `timelineState.ts:138-155`.

The implementation closes the single synthetic example but not R-05/P2-B's authoritative stale-snapshot invariant.

### 5. Sequence dedup is narrow and duplicate production rows remain possible

The live handler correctly ignores `delta.sequence <= lastSeenSequence` at `ConversationWorkspace.svelte:1405-1407`, and message merge deduplicates by message ID. That is a valid partial closure. It does not prevent backend production of multiple distinct sequences for the same message (Facebook webhook and duplicate Zalo paths above), nor does it establish a persisted cursor across remount/restart.

### 6. Telemetry instrumentation is not latency evidence and `dbToPaintMs` is mislabeled

- The sample buffer is in-memory, capped at 500, and exposed only on `window` (`ConversationWorkspace.svelte:1234-1258`). No persistence or official QA artifact exists.
- The summary calculates only `deltaToPaintMs`; it does not report p50/p95/p99 for `dbToPaintMs` (`:1246-1253`).
- `emitted_at_ms` is captured before the delta INSERT/UPDATE and before transaction commit (`repository.rs:3022-3052`, callers commit later). Therefore `dbToPaintMs` at `ConversationWorkspace.svelte:1367` is not DB-commit-to-paint.
- Double `requestAnimationFrame` is a scheduling marker, not proof of GPU completion, and no mounted WebView samples were captured.
- The benchmark ledger records `<=16.6 ms` as “Final” without any raw samples, sample count, device/build identity, percentile output, or reproducible JSON/Markdown QA evidence.

The claimed E4 p50/p95/p99 and production SLO are unverified.

### 7. Added tests do not exercise the rejected production invariants

- `test_delta_transaction_rollback_guarantee` opens and drops an empty transaction; it writes neither a message nor a delta (`repository_test.rs:6703-6716`). It is not fault injection and proves nothing about rollback coupling.
- The WhatsApp test manually calls message ingestion and then `record_message_delta` (`whatsapp/supervisor_test.rs:723-739`), reproducing the production two-transaction anti-pattern instead of asserting same-transaction/fail-closed supervisor behavior.
- The Facebook supervisor test covers one webhook message only (`facebook/supervisor_test.rs:516-569`); it cannot detect per-conversation collapsing or duplicate delta rows for a multi-message batch.
- The UI gap test covers sequence 1 → 3 with a single mocked replay row (`ConversationWorkspace.spec.ts:1910-1978`). It does not cover >200 pagination, live-during-replay, first-event gap, restart, reconnect, replay failure, or final-event loss.
- The stale snapshot test uses a synthetic newer timestamp (`timelineState.spec.ts:174-197`), not a DB snapshot watermark or an out-of-order provider timestamp.
- The duplicate test sends the exact same event twice (`ConversationWorkspace.spec.ts:2018-2051`); it does not cover two durable sequences for one message or cursor persistence.

Passing these tests cannot clear the source-level blockers.

### 8. Commit provenance improved; literal clean/runtime/package claims remain false

- The remediation is committed at exact commit `4f19d38` (29 files, 2,091 insertions, 239 deletions), so the previous “no rollback anchor” problem is closed for this slice.
- `git show --check 4f19d38` and `git diff --check 4f19d38^ 4f19d38` exit cleanly.
- The current worktree is not clean: `AGENTS.md`, `CLAUDE.md`, four Zalo-status documents are modified, and three unrelated prior reports are untracked. These were preserved and are not attributed to this commit.
- No `rustsale-crm` desktop process is running. No real RustSale app window was available for read-only QA, so no screenshot was taken.
- The release binary and `.deb` are dated `2026-08-29`; the debug binary is dated `2026-08-31 06:16`, both before commit `4f19d38` at `07:50`. No packaged artifact has proven provenance to the reviewed commit.
- The read-only production DB `/home/arch/.local/share/com.rustsale.crm/rustsale.db` was last modified at `05:55`, contains 8,557 messages, and has no `conversation_deltas` table. The migration has not run in that production profile.

The exact packaged runtime, production migration, and real inbound hot path remain unverified.

## Documentation integrity

The ledgers are materially inconsistent with source and with themselves:

- `actual-status.md` marks every blocker `correct` while its Detailed Findings section still describes missing durable sequence and omnichannel coverage.
- `evidence.md:E0-P0A-SRC2` still says the table does not exist, while E1 claims it does.
- `E1-P1A-TEST1` calls the empty-transaction test proof of rollback.
- `E1-P1D-REPLAY1` claims high-water metadata, but `fetch_deltas_after_sequence` returns only `Vec<ConversationMessageDeltaEvent>`.
- `E2-P2B-WATERMARK1` labels a message-ID set as a DB sequence watermark.
- The benchmark ledger publishes target values as measured final values without raw observations.
- The plan checks P3-B/P3-C complete and claims clean worktree/supervisor acceptance while metadata remains `ready_for_review` and P3-C is also listed unchecked earlier.

These artifacts cannot be used as zero-trust completion evidence.

## Per-file-group clearance

| File group | Review state | Evidence-based note |
|---|---|---|
| `schema.sql`, `migration.rs`, `events.rs`, `commands.rs`, `lib.rs` | Partial | Durable table, indexes, IPC registration, SQLite sequence forwarding, and distinct invalidation reasons exist. No high-water/floor, retention/ack/resnapshot, or package migration evidence. |
| `repository.rs`, `outreach.rs` | Partial | Zalo normalized ingestion and outreach use same-tx delta. Duplicate uniqueness is absent; replay contract has no high-water/floor. |
| `service.rs` | Blocked | Direct-start/outbound paths are separate fail-open writes; duplicate Zalo path creates another delta. |
| `facebook/normalizer.rs`, `facebook/supervisor.rs` | Blocked | Delta errors are swallowed; webhook discards tx event and republishes only latest per conversation through another transaction. |
| `whatsapp/supervisor.rs` | Blocked | Message and delta use separate transactions; error is swallowed. |
| `zalo/supervisor.rs` | Partial | History invalidation reason is repaired; it does not close global atomic/recovery contracts. |
| `conversationService.ts` | Partial | Ref-counted dual listener and retry cleanup are improved; no reconnect cursor negotiation or lag recovery signal. |
| `ConversationWorkspace.svelte` | Blocked | Direct upsert works in memory, but startup/final-loss replay, fallback invalidation, authoritative snapshot boundary, and telemetry evidence remain open. |
| `timelineState.ts` | Blocked | Message-ID/timestamp retention is not DB-watermark-authoritative and the watermark set is unbounded. |
| Backend/frontend tests | Blocked | New tests cover helpers/happy paths and do not exercise the rejected production boundaries or fault modes. |
| Plans/evidence/benchmark/prior reports | Blocked | Claims and measured values conflict with source/current runtime and lack raw evidence. |

## Commands and observations

Executed read-only/review commands:

- Full reads: raw `AGENTS.md`, `.agents/skills/working-rules/SKILL.md`, `.agents/skills/supervisor/SKILL.md`, `.agents/skills/qa/SKILL.md`, `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`, current plan/status/evidence/benchmark, and prior rejection report.
- Memory: agentmemory recall/smart-search for prior findings and resubmission claims. No mem0 tool was exposed in this session, so mem0 could not be queried.
- Anvien: `anvien --help`; `anvien analyze --force`; repository status/list; source query for durable delta replay/transaction flow. Fresh graph: 527 files scanned, 381 code files parsed, 0 parse failures, 83,722 nodes, 96,626 relationships. Repository/timeline review scope is HIGH blast radius (scope warning, not a prohibition).
- Git/provenance: `git status --short`; `git show -s`; `git diff-tree`; `git show --check 4f19d38`; `git diff --check 4f19d38^ 4f19d38`; focused `git show` and numbered source reads.
- Source/test/docs: focused `rg`, `nl -ba`, and full ledger reads listed by evidence above.
- Runtime/data: process inventory; artifact timestamps; `sqlite3 -readonly` production schema/message observations.

Intentionally not run after decisive source failures:

- No broad `cargo test`, `bun test`, `bun run check`, frontend build, Cargo build, or release/package build.
- No UI interaction or screenshot: there was no mounted exact-provenance RustSale runtime.
- No real Zalo/WhatsApp/Facebook message was sent and no app process was reloaded or killed.

## Required remediation before another acceptance request

1. Make every message-producing path return one committed delta from the exact message transaction; propagate failures and enforce one durable delta per message/event type.
2. Implement a durable or safely negotiated cursor protocol: subscribe/buffer, obtain DB high-water and retention floor, replay to that boundary, drain in order, persist/renegotiate on mount and reconnect, and recover final-event loss via lag signal or bounded polling.
3. Suppress `message` invalidation only after matching message/sequence application; otherwise perform a bounded authoritative fallback refresh.
4. Add a DB-authoritative snapshot sequence to the timeline response and merge against it; remove the unbounded process-local message-ID pseudo-watermark.
5. Add true failure injection and production-boundary tests for WA, FB multi-message webhook, outbound paths, first-event gap, >200 live buffering, restart/reconnect, replay failure, final-event loss, retention floor, and out-of-order timestamps.
6. Build/package the exact accepted commit, run its migration against a quarantined production-shaped profile, mount the desktop app, capture read-only official QA evidence, and record raw latency samples with both receive-to-paint and real commit-to-paint p50/p95/p99.

Completion Lock remains closed. Handoff to Main: treat the source blockers above as authoritative; do not rely on the committed earlier acceptance artifact or green suite claims.
