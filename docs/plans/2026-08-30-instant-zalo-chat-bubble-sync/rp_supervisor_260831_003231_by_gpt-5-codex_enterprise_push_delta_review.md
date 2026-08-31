# Supervisor Report: Enterprise Push-Delta Production Readiness

Verdict: REJECT

## Metadata
- Report file: `rp_supervisor_260831_003231_by_gpt-5-codex_enterprise_push_delta_review.md`
- Review time: `260831 00:32:31 UTC+7`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: commit `5659137`, prior rejection report, current plan/evidence/benchmark/status ledgers, relevant backend/frontend source, current dirty worktree, refreshed Anvien/GitNexus graphs, active Tauri dev runtime, and production SQLite metadata
- Claim reviewed: Enterprise Push-Delta and Zero-IPC Direct Store Upsert are production-ready and render incoming Zalo/omnichannel bubbles on the next frame in `<16 ms`
- Authority used: Owner request, `AGENTS.md`, `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`, active plan, source/runtime/data reality, and prior Supervisor blockers
- Related artifacts: commit `d13686d`, commit `5659137`, `rp_supervisor_260830_233325_by_gpt-5-codex_instant_zalo_chat_bubble_sync.md`

## Executive Summary
- Problem: the previous invalidation-only path could notify immediately while the WebView waited for a timeline IPC refresh.
- Decision: REJECT. Commit `5659137` adds a real render-ready Zalo delta and synchronous in-memory upsert, closing an important part of the previous rejection. It does not close the production invariant: the sequence is volatile and unused for gap recovery, the event transport remains lossy, the actual production path still emits an invalidation that triggers a timeline IPC refresh, Messenger/WhatsApp inbound paths do not emit deltas, and `<16 ms` was not measured at DOM paint.
- Required outcome: make the delta stream resumable and sequence-aware, remove full timeline refresh from the message paint hot path, cover every claimed channel, and prove DB-commit-to-paint SLO on the exact built desktop artifact.

## Blocking Findings

### [CRITICAL] Sequence is volatile metadata, not a replay contract
File: `src-tauri/src/conversation/events.rs:39`

Issue: `sequence` is an in-memory `AtomicU64` initialized to zero on each process start. Deltas are sent through a bounded broadcast channel; lagged events are explicitly skipped. Frontend code only prints the sequence and never stores a last-applied sequence, rejects stale events, detects gaps, or requests replay.

Evidence:
- `events.rs:42,52,56-57` initializes and increments a process-local counter.
- `events.rs:72-73` ignores the broadcast send result.
- `events.rs:98-107` logs `Lagged(skipped)` and continues without recovery.
- `conversationService.ts:603-630` retries listener registration with up to 5-second backoff but has no resume cursor or replay command.
- `ConversationWorkspace.svelte:1228-1235` uses `delta.sequence` only in a debug log.

Why this blocks acceptance: a listener outage, WebView reload, application startup race, or 256-event burst can permanently lose bubbles. Reconnect restores only future delivery.

Fix direction: persist a monotonically ordered change record in the same SQLite transaction as the message; subscribe with `lastAppliedSequence`; replay gaps before switching to live delivery; apply idempotently and acknowledge progress.

Re-review evidence required: restart/reconnect and >256-event burst tests proving zero loss, zero duplicate bubbles, deterministic ordering, and recovery from an intentional sequence gap.

### [HIGH] The real Zalo path is not Zero-IPC
File: `src-tauri/src/conversation/service.rs:610`

Issue: every message publication first sends `conversation-invalidated`, then sends `conversation-message-delta`. The frontend invalidation handler still schedules `refreshOpenTimeline()` at zero delay, which calls the 50-message timeline IPC path. The added component test emits only a delta, so it bypasses the event pair used in production.

Evidence:
- `service.rs:610-626` publishes invalidation and then delta for message events.
- `events.rs:84-110` forwards the two independent broadcast channels through `tokio::select!`, so cross-channel arrival order is not guaranteed.
- `ConversationWorkspace.svelte:1327-1336` schedules `refreshOpenTimeline()` for the same open message invalidation.
- `ConversationWorkspace.svelte:1154-1160` routes that refresh into `fetchVariantFirstPage`.
- `ConversationWorkspace.spec.ts:1056-1075` clears the IPC mock and calls only `emitDelta`; it never emits the paired invalidation.

Why this blocks acceptance: the test proves a narrower synthetic fast path, not the actual runtime path. The real path still creates IPC/SQLite/main-thread work immediately alongside paint and can preserve the reported delay or introduce flicker under contention.

Fix direction: for a successfully applied message delta, mark the affected variant reconciled through its sequence and suppress the paired message invalidation refresh. Keep invalidation for receipts/reactions/metadata and schedule message-page reconciliation off the paint path.

Re-review evidence required: a production-path integration test emitting the same invalidation+delta pair as Rust, asserting immediate bubble insertion and zero hot-path timeline IPC before first paint.

### [HIGH] Refresh reconciliation can temporarily erase a newer delta
File: `src/lib/components/conversation/timelineState.ts:125`

Issue: refresh-mode merge treats the fetched page as authoritative and retains missing current messages only when queued/sending or older than the page cursor. A received delta can therefore disappear when an older in-flight page resolves, then reappear after the invalidation epoch loop refetches.

Evidence:
- `timelineState.ts:131-148` drops a received current message absent from the fetched page.
- `ConversationWorkspace.svelte:1051-1089` writes the fetched page into the cache before checking whether the invalidation epoch changed.
- The delta handler has no sequence watermark that protects newer in-memory rows from an older snapshot.

Why this blocks acceptance: the invariant requires a committed bubble to appear once and remain visible; transient removal is a user-visible regression and undermines the latency SLO.

Fix direction: merge snapshots against a per-variant applied-delta watermark, never replace messages newer than the snapshot boundary, and check generation/epoch before committing the snapshot to visible state.

Re-review evidence required: adversarial deferred-fetch test where a stale response resolves after a delta and the bubble never disappears.

### [HIGH] Omnichannel coverage claim is false
File: `src-tauri/src/conversation/whatsapp/supervisor.rs:396`

Issue: live WhatsApp and Messenger ingestion publish only invalidation events. Repository-wide source search finds `publish_delta` call sites only in `events.rs` and `ConversationService`; neither channel supervisor hydrates and publishes an incoming message delta.

Evidence:
- `whatsapp/supervisor.rs:396-421` ingests a live inbound message and publishes only `ConversationInvalidatedEvent`.
- `facebook/supervisor.rs:345-363` publishes only invalidation after webhook ingestion.
- `facebook/supervisor.rs:223-242` does the same after sync ingestion.
- `rg "publish_delta\\(" src-tauri/src/conversation` returns no delta publication from the Facebook or WhatsApp supervisors.

Why this blocks acceptance: the plan explicitly targets Zalo/WhatsApp/Messenger, but two claimed channels remain on the old invalidation/refetch path.

Fix direction: normalize all message commits behind one transactional message-delta publisher, or explicitly narrow the plan and release claim to Zalo until channel parity is implemented.

Re-review evidence required: direct/group inbound integration and mounted runtime traces for each claimed channel.

### [HIGH] `<16 ms` is still asserted rather than measured
File: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md:25`

Issue: the benchmark ledger assigns `<16 ms` to “Direct Store Upsert” without raw samples or a paint observer. The runtime debug value measures wall-clock Rust emit to JavaScript handler entry, not DB commit to DOM paint.

Evidence:
- `ConversationWorkspace.svelte:1228-1234` stops timing at handler entry and uses `Date.now()`.
- `ConversationWorkspace.spec.ts:1068-1075` waits `tick()` plus 20 ms and asserts DOM presence; it does not measure a frame or enforce a latency threshold.
- No trace artifact, p50/p95/p99 sample set, `requestAnimationFrame` paint marker, or correlation record exists in the evidence/benchmark ledgers.

Why this blocks acceptance: the product claim is a user-visible performance SLO, which requires E4 runtime evidence; mocked component tests cannot prove it.

Fix direction: correlate provider receive, transaction commit, delta publish, WebView receive, store upsert, and post-render frame using one message/event ID and monotonic stage timings.

Re-review evidence required: raw desktop traces and p50/p95/p99 for direct, group, reconnect, burst, and DB-contention scenarios on the packaged artifact.

### [HIGH] Completion Lock and exact-artifact runtime QA remain open
File: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-evidence.md:32`

Issue: the ledger contains a focused Rust test, one component suite, and typecheck only. It has no full build, full relevant backend/frontend regression, packaged desktop run, failure-path matrix, replay/burst test, or Supervisor PASS. The active app is a Tauri dev runtime built from a dirty worktree, not the exact clean commit.

Evidence:
- The evidence ledger lists no `bun run build`, packaged Tauri build, full backend suite, or mounted performance trace.
- Current `git status --short` contains unrelated modified/untracked files.
- GitNexus change detection against `d13686d` reports HIGH scope in the current worktree (`29` files, `261` changed symbols, `12` affected processes), so the running dev artifact cannot be attributed solely to commit `5659137`.
- Active Tauri/Vite build processes prevent a rule-compliant fresh full build without stopping the Owner's open app; that authority was not assumed.

Why this blocks acceptance: Tier 3 native IPC/DB/external integration work cannot be production-approved from focused tests or a dirty dev runtime.

Fix direction: close the harness on a clean commit and freshly built desktop artifact after the correctness blockers above are fixed.

Re-review evidence required: full build, full relevant suites, packaged desktop QA, adversarial matrix, Anvien/GitNexus scope evidence, and a fresh Supervisor PASS.

## Source-Level Clearance Notes
- `src-tauri/src/conversation/events.rs`: blocked - render-ready payload exists, but transport is bounded/lossy and sequence is not durable or recoverable.
- `src-tauri/src/conversation/service.rs`: partial clearance - commit now contains the previously missing Zalo ingestion wiring and hydrates the message; it still emits paired invalidation and silently drops hydration/publish failures.
- `src/lib/services/conversationService.ts`: blocked - dual listener singleton exists, but reconnect has no replay; `Promise.all` also has no cleanup for a partially successful listener registration.
- `src/lib/components/conversation/ConversationWorkspace.svelte`: partial clearance - synchronous message upsert and sidebar update exist; sequence/gap handling and stale-snapshot protection do not.
- Facebook/WhatsApp supervisors: blocked for the omnichannel claim - incoming message paths are invalidation-only.
- Tests and ledgers: blocked for production/performance acceptance - happy-path E2 evidence is narrower than the E4 claim.

## Evidence Checked

Passed:
- Commit `5659137` contains the new Rust delta event, Zalo service publisher, dual frontend listener, and direct in-memory upsert.
- `git show --check 5659137` and `git diff --check d13686d..5659137` produced no whitespace errors.
- Anvien graph refreshed at commit `5659137`: 516 files scanned, 381 code files parsed, zero parse failures.
- GitNexus refreshed at commit `5659137`: 7,733 nodes, 19,676 edges, 300 flows.
- Active dev desktop was visually opened to the Conversations surface. A fresh Zalo inbound DB row was observed at `2026-08-31 00:27:03 UTC+7`; provider timestamp to ingestion timestamp was about 206 ms. This proves ingestion only, not paint latency.

Failed:
- Durable resume/replay and frontend sequence consumption are absent.
- Actual Zalo event-pair path still triggers timeline IPC refresh.
- Messenger/WhatsApp incoming deltas are absent.
- `<16 ms` DOM-paint evidence is absent.
- Plan status `completed` / evidence status `VERIFIED READY` contradict the open Completion Lock.

Not run:
- Focused tests and typecheck were not rerun in this review because repository rules require a fresh full build before validation, while the Owner's open Tauri/Vite dev processes are active and were not terminated.
- Fresh full/package build and packaged runtime QA were not run for the same reason.
- Open-conversation commit-to-paint timing was not measured: the fresh inbound observed during review belonged to a different group from the open thread, and the dev window became unavailable after a diagnostic reload.
- Production code fixes were not made; Supervisor/QA review remained read-only.

## Invariant Closure
- Affected invariant: every committed inbound message for the open conversation must be rendered once, remain visible, survive listener/runtime reconnects, preserve ordering, and meet a defined commit-to-paint SLO across every claimed channel.
- Sibling surfaces checked: Zalo ingest/service publication, event forwarder, native notification subscriber, frontend bridge, timeline store/refresh merge, sidebar update, Messenger webhook/sync, WhatsApp live ingestion, tests, ledgers, active dev runtime, and SQLite metadata.
- Residual unverified same-invariant surfaces: packaged app behavior, open-thread live sample, WebView paint/long-task trace, burst/backpressure, reconnect/restart replay, DB contention, and multi-channel parity.

## Required Fix List For Resubmission
1. Persist delta sequence/change rows transactionally and add resume, gap detection, replay, idempotency, and acknowledgements.
2. Stop paired message invalidations from calling `refreshOpenTimeline` on the first-paint path; reconcile later with sequence/generation guards.
3. Protect direct-upserted messages from stale in-flight snapshot overwrite.
4. Route Facebook Messenger and WhatsApp inbound commits through the same render-ready delta contract, or narrow all claims to Zalo.
5. Add partial-listener-init cleanup and reconnect/retry failure tests.
6. Replace the synthetic `<16 ms` ledger entry with real packaged-desktop DB-commit-to-paint traces and percentile results.
7. Run the complete build, relevant regression suites, adversarial matrix, exact-artifact visible QA, change detection, and Supervisor re-review.

## Overall Evaluation
The implementation is a meaningful latency-path improvement for Zalo and is directionally correct: a hydrated message can now be inserted without waiting for a timeline query. It is not yet an enterprise production implementation. Delivery remains best-effort, the sequence does not enforce correctness, the real Zalo path still launches the old IPC refresh, other channels are not covered, and the performance number is unsupported. The correct readiness state is `NOT READY`.
