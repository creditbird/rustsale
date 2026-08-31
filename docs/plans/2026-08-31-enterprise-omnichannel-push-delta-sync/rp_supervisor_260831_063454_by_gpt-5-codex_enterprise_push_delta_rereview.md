# Supervisor Report: Enterprise Omnichannel Push-Delta Re-review

Verdict: REJECT

## Metadata
- Report file: `rp_supervisor_260831_063454_by_gpt-5-codex_enterprise_push_delta_rereview.md`
- Review time: `260831 06:34:54 UTC+7`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: exact dirty worktree at HEAD `161fd5bbdebf12691006a30d58a107b1b6cfe01d`, including the uncommitted Push-Delta remediation, prior rejection, new plan/evidence/benchmark/status ledgers, and the existing claimed Supervisor report
- Claim reviewed: all six prior blockers are closed; Zalo, WhatsApp, and Messenger have production-ready transactional durable deltas, deterministic replay, zero-IPC first paint, stale-snapshot safety, and measured next-frame paint
- Authority used: owner request, `AGENTS.md`, `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`, active plan, current source/diff, Anvien evidence, process/artifact state, and production SQLite schema state
- Related artifacts: `rp_supervisor_260831_003231_by_gpt-5-codex_enterprise_push_delta_review.md`; `rp_supervisor_260831_0616_by_gpt-5-codex_enterprise_push_delta_verdict.md`

## Executive Summary
- Problem: the resubmission claims durable, lossless, omnichannel push rendering and complete production clearance.
- Decision: reject. The implementation adds useful SQLite replay primitives and more channel publication call sites, but it does not close the durable-stream invariant. Message and delta writes are separate transactions; failures are silently swallowed after the frontend has disabled the message-invalidation fallback. Replay is capped, uncoordinated with live delivery, and has neither startup resume nor a persistent cursor. A volatile sequence producer still shares the event stream with SQLite sequences. Several history/batch paths emit no per-message delta. The performance ledger contains no runtime samples and the exact packaged desktop artifact was not run.
- Required outcome: use one durable sequence namespace and transactional message+delta commit, implement ordered paginated catch-up with live buffering and a recovery handshake, condition suppression on proven delta coverage, cover every message-producing path, and collect raw exact-artifact paint traces before production approval.

## Blocking Findings

### [CRITICAL] Message commit and durable delta are not one transaction
File: `src-tauri/src/conversation/repository.rs:836`

Issue: `apply_normalized_message` owns and commits the message transaction before delta persistence begins. `record_message_delta` then opens another database connection and executes independent autocommit statements. The stated R-01 invariant is therefore false.

Evidence:
- `repository.rs:836-838,1263` opens the message transaction and commits it before returning.
- `service.rs:617-624` calls `record_message_delta` only after message ingestion has returned.
- `repository.rs:2974-2982,3008-3027` opens a new connection, inserts the delta, then updates its payload in a second statement; no transaction object is shared with the message write.
- `service.rs:618-625`, `whatsapp/supervisor.rs:423-432`, and `facebook/supervisor.rs:395-404` discard every delta persistence/hydration error through `if let Ok(Some(...))`.

Why this blocks acceptance: a committed message can exist with no durable delta. The frontend suppresses all `reason === 'message'` timeline refreshes at `ConversationWorkspace.svelte:1373-1381`, so exactly this failure can leave the notification visible while the chat bubble remains stale indefinitely.

Fix direction: insert the replay/outbox row through the same SQLite transaction that inserts or mutates the message, return the committed durable event, and make publication failure recoverable. Suppress invalidation only when the event is covered by an applied durable sequence; otherwise retain an authoritative reconciliation path.

Re-review evidence required: rollback/fault-injection tests proving neither message nor delta can commit alone, plus a mounted recovery test where persistence or publication fails and the bubble still converges without waiting for user navigation.

### [CRITICAL] Replay protocol can permanently skip gaps
File: `src/lib/components/conversation/ConversationWorkspace.svelte:1229`

Issue: the cursor is component-local RAM, the first observed sequence bypasses gap recovery, catch-up fetches only one page of 200, and the higher live event is applied before catch-up completes. The implementation does not establish ordered handoff from replay to live delivery.

Evidence:
- `ConversationWorkspace.svelte:1229-1244` initializes `lastSeenSequence` to zero, prevents concurrent resumes, requests one fixed 200-row page, and never loops until caught up.
- `ConversationWorkspace.svelte:1311-1318` starts replay fire-and-forget, then immediately applies the high live sequence and advances the cursor.
- `service.rs:631-638` and `repository.rs:3034-3045` implement a capped query, not a resumable stream handshake.
- `events.rs:98-107` only logs broadcast lag. If the final event is lost, no later sequence exists to trigger frontend gap detection.
- No startup/reconnect resume call or persisted client cursor exists in the frontend source.

Why this blocks acceptance: a gap larger than 200 can fetch only its first page while the live event advances the cursor past the missing middle. Remount/reconnect starts from zero and treats the first future event as valid. The claimed zero-loss restart/reconnect contract is not implemented.

Fix direction: subscribe and buffer live events, obtain a durable high-water mark, page replay until that mark is reached, apply strictly by sequence, then drain buffered live events. Persist or safely negotiate the resume cursor; emit an explicit lag/recovery signal; define resnapshot behavior when history is pruned.

Re-review evidence required: adversarial tests for gaps of 201, 500, and greater than broadcast capacity; loss of the final live event; bridge disconnect during replay; app/component restart; concurrent live delivery during multi-page replay; replay failure/retry; and duplicate/out-of-order events.

### [CRITICAL] Two incompatible sequence producers share one frontend cursor
File: `src-tauri/src/conversation/events.rs:39`

Issue: the process-local `AtomicU64` was not removed. `start_direct_conversation` still publishes events from that volatile sequence into the same `conversation-message-delta` channel as SQLite-assigned sequences.

Evidence:
- `events.rs:42,52,56-57` retains the process-local counter.
- `service.rs:409-416` uses `next_sequence()` for an outbound delta.
- `ConversationWorkspace.svelte:1229-1249` treats all deltas as one monotonically ordered namespace.

Why this blocks acceptance: restart resets the volatile counter, and its values can collide with or move behind/ahead of durable SQLite sequences. One frontend cursor cannot deduplicate or detect gaps correctly across two authorities.

Fix direction: every event on this stream must receive its sequence from the same durable change log, or volatile events must use a separate non-replay namespace and consumer contract.

Re-review evidence required: mixed inbound/outbound and restart tests proving strict total order and no sequence collisions.

### [HIGH] Omnichannel parity is incomplete and suppression hides uncovered paths
File: `src-tauri/src/conversation/facebook/supervisor.rs:322`

Issue: Facebook batches collapse all events for a conversation to one affected conversation and emit a delta only for the latest database message. WhatsApp and Zalo history paths still publish only a message invalidation. The frontend now suppresses those invalidations unconditionally.

Evidence:
- `facebook/supervisor.rs:346-357` deduplicates affected conversation IDs; `378-403` selects and emits only the latest message. Two new messages for the same conversation in one webhook/sync batch therefore produce one delta.
- `whatsapp/supervisor.rs:621-636` publishes history-sync message invalidations with `message_id: None` and no delta.
- `zalo/supervisor.rs:1048-1056,1143-1152` does the same for direct/group history sync.
- Attachment thumbnail updates also publish message-only invalidations at `commands.rs:1275-1283,1653-1662`; these refreshes are now suppressed.
- The new WhatsApp test in `whatsapp/supervisor_test.rs:692-740` manually invokes the normalizer and repository method; it never drives the supervisor event handler claimed by the test name.

Why this blocks acceptance: committed messages or message projections on these real paths can remain absent/stale in an open timeline. The implementation does not satisfy “every inbound message on all three channels.”

Fix direction: emit one transactional delta for each committed message/projection change, not one per affected conversation. Use distinct invalidation reasons/contracts for attachment/history reconciliation, or attach durable sequence coverage before suppressing them.

Re-review evidence required: multi-message same-thread Facebook batch, WhatsApp history, Zalo direct/group history, attachment hydration, duplicates, and live supervisor-path tests at the real boundary.

### [HIGH] Frontend deduplication and stale-snapshot proofs are narrower than the invariant
File: `src/lib/components/conversation/ConversationWorkspace.svelte:1247`

Issue: `applySingleDelta` does not reject `sequence <= lastSeenSequence`; it always applies the payload. The duplicate test passes only because it reuses the same message ID and `mergeMessages` deduplicates by message. Snapshot protection uses provider `occurredAt`/message ID as a proxy rather than a snapshot sequence watermark.

Evidence:
- `ConversationWorkspace.svelte:1247-1250` conditionally advances the cursor but has no early return for stale/duplicate sequence.
- Test 24 emits the same sequence and the same message ID; it cannot detect a different payload/message using an already-applied sequence.
- `timelineState.ts:140-165` retains missing rows only when their parsed `occurredAt`/ID compares newer than the fetched page. Provider timestamps may be delayed or reordered and do not prove snapshot generation.

Why this blocks acceptance: a duplicate sequence with a different message can be applied, and a newly committed delta carrying an older provider timestamp can still be erased by a stale snapshot.

Fix direction: apply and deduplicate by durable sequence before mutating stores. Carry a database snapshot/delta watermark through timeline responses and merge against that authority rather than provider time.

Re-review evidence required: same-sequence/different-message, out-of-order provider timestamp, equal timestamp, empty page, delayed response generation, and message deletion/recall tests.

### [HIGH] Paint SLO is asserted without measurement
File: `src/lib/components/conversation/ConversationWorkspace.svelte:1320`

Issue: the implementation adds nested `requestAnimationFrame` console logging, but the benchmark promotes the target itself to a measured result. No raw sample set, correlation artifact, percentile calculation, screenshot/trace, or mounted packaged run exists.

Evidence:
- `ConversationWorkspace.svelte:1320-1333` logs two wall-clock differences only. Nested rAF callbacks do not directly observe GPU paint, and two scheduled frames do not prove a `<=16.6 ms` elapsed time.
- `repository.rs:2997-3019` sets `emitted_at_ms` before the delta INSERT; it is not a database commit timestamp, so `dbToPaintMs` is mislabeled.
- The benchmark ledger records `<=16.6 ms` as Latest/Final without any numeric samples; the evidence ledger cites source presence only.
- No push-delta performance JSON/Markdown, Playwright trace, or raw telemetry artifact exists under `Reports/qa` or the plan folder.

Why this blocks acceptance: a Tier-3 user-visible performance SLO requires fresh E4 evidence. Instrumentation presence is not a benchmark result.

Fix direction: correlate provider receipt, authoritative transaction commit, native publish, WebView receive, store mutation, and a defensible rendered-frame marker by event/message ID. Persist raw samples and calculate p50/p95/p99 for the declared scenarios.

Re-review evidence required: exact packaged-desktop traces for open direct/group conversations, all claimed channels, reconnect, burst, DB contention, and WebView/main-thread contention.

### [HIGH] Completion Lock and claimed clean release state are contradicted by repo/runtime reality
File: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-plan.md:411`

Issue: all remediation source and evidence remains uncommitted, the plan folder and claimed approval report are untracked, no exact packaged runtime is open, and fresh whitespace validation fails. The plan nevertheless marks P3-C and clean-worktree acceptance complete.

Evidence:
- Exact basis is HEAD `161fd5b` plus 23 tracked modified files and untracked enterprise plan/report artifacts; there is no implementation commit or rollback anchor.
- Fresh Anvien analysis indexed 526 files/83,541 nodes. `detect-changes --scope all` reports CRITICAL scope, 23 changed files, 369 changed nodes, and 18 affected files.
- Fresh `git diff --check` fails on three modified remediation docs with extra EOF blank lines and `whatsapp/supervisor_test.rs:703` trailing whitespace.
- No `rustsale-crm` process is running. The release binary/deb are dated `2026-08-29`; the dirty debug binary is dated `2026-08-31 06:16` but is not running. The current production database has no `conversation_deltas` table, proving the new migration has not run in the user runtime.
- The new evidence ledger contains prose summaries but no raw logs. The claimed `0 warnings` also conflicts with the user's own later `0 errors, 6 warnings` statement.

Why this blocks acceptance: the Completion Harness forbids readiness while exact-artifact runtime evidence, fresh failure-path evidence, change scope, and repository integrity remain open. A debug compile plus frontend bundle is not packaged desktop QA.

Fix direction: fix the correctness blockers first, commit each approved slice, run fresh full gates, create a distributable Tauri build, open that exact artifact, execute visible QA, and preserve raw evidence linked to the commit/artifact hash.

Re-review evidence required: clean committed snapshot, successful `git diff --check`, raw gate logs with timestamps/hash/skips, packaged artifact hash, visible desktop evidence in required JSON+Markdown form, production DB migration/readback, and a new independent review.

## Source-Level Clearance Notes
- `schema.sql` / `migration.rs`: partial — startup-compatible `CREATE TABLE IF NOT EXISTS` and indexes exist (`schema.sql:553-566`, `migration.rs:247,252-269`), but schema presence does not provide transactional insertion, retention/compaction, acknowledgements, or resnapshot semantics.
- `repository.rs`: blocked — replay query is ordered and capped safely, but message/delta atomicity is false, payload INSERT/UPDATE is split, duplicate delta uniqueness is absent, and no retention floor is defined.
- `events.rs`: blocked — bounded transport remains lossy and the incompatible volatile sequence authority remains active.
- `commands.rs` / `lib.rs`: partial — IPC is registered, but it exposes a single capped page and cannot by itself guarantee catch-up; several message-only invalidations are now incorrectly suppressed.
- `service.rs`: blocked — Zalo live publication exists, but durable-write errors are swallowed after invalidation and volatile outbound sequences share the stream.
- `whatsapp/supervisor.rs`: blocked — live handler attempts delta publication, but history paths are invalidation-only and errors are swallowed.
- `facebook/supervisor.rs`: blocked — one-message cases publish a delta, but same-conversation batches collapse to the latest message only and errors are swallowed.
- `conversationService.ts`: partial — replay IPC wrapper exists; the bridge still uses `Promise.all` without cleaning a listener that succeeds when its sibling registration rejects (`conversationService.ts:648-688`).
- `ConversationWorkspace.svelte`: blocked — direct store upsert is real, but catch-up ordering, cursor lifecycle, deduplication, fallback suppression, and telemetry evidence are unsafe/incomplete.
- `timelineState.ts`: partial — the new time-based retention closes the single tested example, not the authoritative stale-snapshot invariant.
- Test files: blocked as acceptance proof — focused tests cover happy paths but omit transaction failure, real WhatsApp supervisor dispatch, large paginated gaps, startup/reconnect, same-sequence/different-message, Facebook same-thread batches, and mounted runtime behavior.
- Plan/evidence/benchmark/report artifacts: blocked — claims overstate measured results and Completion Lock closure; the pre-existing PASS report is not independently substantiated.

## Evidence Checked
Passed:
- Fresh source/diff inspection confirms the table, migration, replay query, IPC registration, direct upsert, and several new channel publication attempts exist.
- Fresh Anvien graph rebuild completed: 526 files scanned, 381 code files parsed, zero parse failures; relevant repository/workspace files are high-risk and overall dirty change scope is CRITICAL.
- Existing focused tests are structurally relevant to monotonic repository rows, one Facebook event, one synthetic frontend gap, one timestamp-ordered snapshot example, and same-message merge deduplication.

Failed:
- Transactional message+delta invariant.
- Ordered/lossless replay across restart, large gaps, and final-event loss.
- Single sequence authority.
- Per-message omnichannel parity and safe invalidation fallback.
- Authoritative stale-snapshot/deduplication invariant.
- Measured paint SLO and exact packaged runtime proof.
- Fresh `git diff --check` and clean committed completion state.

Not run:
- Fresh cargo/bun suites were not used to seek acceptance after source inspection disproved the core invariant. Existing counts in the untracked ledger have no raw command artifacts and are not adopted as current-review facts.
- Fresh packaged Tauri build was not run because the reviewed implementation is already not ready at source level.
- Visible inbound-message QA was not run: no RustSale desktop process/window is currently mounted, the release artifact predates the remediation, and the new debug binary is not running.
- No production messages were sent and no user app was reloaded or terminated.
- Mem0 was unavailable in this tool session; agentmemory was queried and the previous rejection was consumed.

## Invariant Closure
- Affected invariant: every authoritative message/projection commit must create exactly one durable ordered delta atomically; every client must resume and apply the stream exactly once in order; an uncovered failure/path must reconcile authoritatively; an open conversation must render and retain the result within a measured SLO across all claimed channels.
- Sibling surfaces checked: SQLite schema/migration/repository, event transport, Zalo service and history paths, WhatsApp live/history paths, Facebook webhook/sync batching, Tauri IPC registration, frontend bridge/replay, workspace invalidation/delta handling, snapshot merge, focused tests, plan/evidence/benchmark/report, process/artifact state, and production DB schema.
- Residual unverified same-invariant surfaces: exact packaged runtime, real open-thread inbound samples, failure injection, restart/reconnect, large paginated gaps, stream retention floor, DB contention, raw paint telemetry, and all-channel mounted behavior.

## Required Fix List For Resubmission
1. Move delta creation into the authoritative message transaction and fail closed; never swallow a missing durable delta while suppressing reconciliation.
2. Establish one durable sequence authority for every event consumed by the cursor; remove or isolate volatile sequence events.
3. Implement subscribe/buffer/high-water/paginated replay/drain-live handoff with persistent or negotiated cursor, lag notification, retry, and resnapshot behavior.
4. Emit one durable delta per committed message/projection change for Facebook batches and every Zalo/WhatsApp history/attachment path, or narrow the product claim and preserve fallback refresh for uncovered paths.
5. Deduplicate before store mutation by sequence and merge snapshots against a database sequence watermark, not provider timestamp.
6. Add adversarial tests that kill the superficial implementation: transaction fault injection, >200 gaps, final-event loss, restart, live-during-replay, same-sequence/different-message, out-of-order provider time, and multi-message Facebook batches.
7. Replace target-as-result benchmark rows with raw correlated runtime samples and percentiles from the exact packaged desktop artifact.
8. Commit the implementation slices, clear diff hygiene, run full fresh gates, produce required visible QA evidence, and request a new independent review.

## Overall Evaluation
The resubmission contains meaningful building blocks, especially the durable table, replay query, direct store upsert, and broader channel hooks. The production claim is nevertheless unsafe. The central transaction and replay invariants are still missing, the blanket invalidation suppression makes failures and uncovered paths worse, and the performance/release evidence is asserted rather than demonstrated. Completion state under the repository harness is `NOT READY`.
