# Supervisor Report: Instant Zalo Chat Bubble Sync Re-review

Verdict: REJECT

## Metadata
- Report file: `rp_supervisor_260830_233325_by_gpt-5-codex_instant_zalo_chat_bubble_sync.md`
- Review time: `260830 23:33:25 UTC+7`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: commit `d13686d`, its four plan/evidence ledgers, current worktree, relevant backend/frontend source, active desktop runtime, and current production SQLite data
- Claim reviewed: incoming Zalo/omnichannel messages update the open chat bubble immediately in `< 30 ms`, with the former approximately 5-second delay fully fixed
- Authority used: latest Owner report that the delay remains, repository rules, committed source/diff, current source, active runtime, current SQLite state, Anvien/GitNexus topology evidence
- Related artifacts: commit `d13686d`; instant-Zalo plan, evidence, benchmark, and actual-status ledgers

## Executive Summary
- Problem: desktop notification is immediate, while the open conversation timeline still updates slowly.
- Decision: REJECT. The implementation improves event routing, but the completion and `< 30 ms` claims are not supported by an end-to-end desktop measurement. The critical path still performs a full timeline IPC re-read after a fire-and-forget invalidation, and the commit does not contain the `service.rs` wiring that its plan/evidence says is complete.
- Required outcome: replace the unmeasured invalidation/refetch claim with an observable, replayable conversation-delta path and prove commit-to-paint latency in the real desktop runtime.

## Blocking Findings

### [CRITICAL] The `< 30 ms` benchmark is asserted, not measured
File: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md:17`

Issue: the ledger records `~5000 -> <30 ms` using “Microtask Invalidation Architecture” as evidence. There are no timestamps, runtime samples, percentile results, screenshots, or desktop trace. The production code uses `setTimeout(..., 0)`, which is a macrotask and does not guarantee a paint deadline.

Evidence:
- Benchmark line 17 contains the latency claim without a measurement artifact.
- `ConversationWorkspace.spec.ts:989-1021` mocks `getContactTimeline` to return the new message and waits 60 ms before asserting. This proves matching under a mocked data source, not `<30 ms` production latency.
- Neighboring cases wait 60-80 ms and likewise bypass the real Tauri/SQLite/render boundary.

Why this blocks acceptance: the user-visible performance invariant is the entire claim, and it was never measured at the real boundary.

Fix direction: instrument socket receive, DB commit, backend publish, WebView receive, store upsert, and next painted frame using one correlation ID/message ID and a monotonic clock.

Re-review evidence required: real desktop samples with p50/p95/p99 and raw trace records for direct, group, identity-unlinked, burst, reconnect, and DB-contention cases.

### [HIGH] Commit `d13686d` does not contain the backend ingestion wiring claimed by the plan
File: `src-tauri/src/conversation/service.rs:450-471` in the current worktree

Issue: the plan says `service.rs` publishes invalidations using `lookup_message_invalidation_info`, but `git diff d13686d^ d13686d -- src-tauri/src/conversation/service.rs` is empty and `service.rs` is absent from the commit file list. That wiring exists only as an uncommitted current-worktree modification.

Evidence:
- Commit `d13686d` changes nine files but not `src-tauri/src/conversation/service.rs`.
- Current `git diff` shows the lookup/fallback wiring as an uncommitted addition at the ingestion path.
- Evidence ledger line 16 says the ingest flow uses the lookup, while commit reality contradicts that statement.

Why this blocks acceptance: the commit advertised as the fix does not ship the claimed backend behavior.

Fix direction: implement and verify the event contract in a coherent implementation slice, then commit the actual production wiring and its end-to-end regression.

Re-review evidence required: committed diff, impact report, change detection, backend integration test asserting the emitted event from `ingest_zalo_event`, and real runtime trace.

### [HIGH] Event receipt still does not make the message render-ready
File: `src/lib/components/conversation/ConversationWorkspace.svelte:1015-1107`

Issue: `ConversationInvalidatedEvent` contains IDs only. After receiving it, the open UI calls a full first-page timeline query and hydration, merges the page, and only then renders. Therefore event matching can be correct while the bubble remains delayed by IPC scheduling, DB contention/error, in-flight fetch churn, main-thread work, or a missed bridge event.

Evidence:
- `handleInvalidatedEvent` schedules `refreshOpenTimeline` at `setTimeout(..., 0)`.
- `refreshOpenTimeline` calls `fetchVariantFirstPage`, which invokes `getContactTimeline`, `getIdentityTimeline`, or `getConversationTimeline` for 50 messages.
- Notification and the Tauri frontend forwarder subscribe to the same backend broadcast event. An immediate notification proves that message ingestion/event publication completed; it does not prove that the WebView received, queried, merged, and painted the message.

Why this blocks acceptance: the architecture has no render-ready fast path and no stage telemetry, so the real delay remains both possible and opaque.

Fix direction: publish a committed `ConversationDelta` carrying a render-ready message projection (or fetch exactly `messageId`), upsert it synchronously into a central conversation store, and perform full-page reconciliation off the paint path.

Re-review evidence required: a real listener-event-to-DOM test without mocked timeline data, plus stage timing and missed-event recovery evidence.

### [HIGH] The bridge recovery path is lossy and the 5-second retry is not a correctness mechanism
File: `src/lib/services/conversationService.ts:582-643`

Issue: bridge reconnect backs off to 5 seconds, but Tauri invalidations are not persisted or replayed. An event emitted while the listener is unavailable can be lost. In addition, `ConversationWorkspace.svelte:803-805` explicitly initializes the bridge while `onConversationInvalidated` now initializes it again, producing two ref-count owners for one component.

Evidence:
- `BRIDGE_RETRY_MAX_DELAY_MS = 5_000`.
- Retry only re-registers `listen`; it does not request missed sequence numbers/messages.
- Workspace calls both `initConversationEventBridge()` and `onConversationInvalidated(...)`; the latter now calls `initConversationEventBridge()` internally.

Why this blocks acceptance: reconnection can restore future delivery but cannot guarantee the current bubble appears, and lifecycle ownership is duplicated.

Fix direction: one app-root bridge owner; durable monotonically sequenced change log; subscribe-with-resume handshake; gap detection and replay before live continuation.

Re-review evidence required: disconnect/reconnect test proving messages emitted during outage are replayed exactly once and rendered.

### [MEDIUM] Routing remains coupled to ambiguous lead resolution and broad channel fallback
File: `src/lib/components/conversation/ConversationWorkspace.svelte:1160-1223`

Issue: the UI sometimes falls back to matching every item that has the same channel when `leadId` is null. Backend lead selection prefers any non-null conversation participant before sender identity. This may avoid false negatives, but it can invalidate unrelated conversations and create refresh storms; group conversations make a single `leadId` an especially weak routing key.

Evidence:
- Customer/unknown branches accept channel-only fallback.
- The SQL ordering is `(lead_id IS NOT NULL) DESC` before sender equality.
- GitNexus rates the frontend `ConversationInvalidatedEvent` interface change as MEDIUM with eight direct importing modules. Anvien's Rust event graph has unresolved cross-file references, so its LOW symbol result is not sufficient to narrow the backend blast radius.

Why this blocks acceptance: correctness and performance depend on a business identity field that is not a stable event-routing key.

Fix direction: route deltas primarily by stable `conversationId`; maintain explicit conversation-to-inbox-item membership; use `leadId` only for aggregation/presentation.

Re-review evidence required: direct/group/unlinked/multi-account routing matrix with no unrelated timeline refreshes under burst traffic.

### [HIGH] Completion harness/runtime gates were not closed
File: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-plan.md:21-28`

Issue: the plan requires a full build before claiming completion, but the evidence lists cargo tests, frontend tests, and Svelte check only. There is no full build, mounted desktop validation, Playwright/runtime evidence, adversarial verification, or Completion Lock record.

Evidence: evidence ledger lines 30-36 list no full build or real runtime command/artifact.

Why this blocks acceptance: green unit suites cannot prove the desktop latency invariant.

Fix direction: follow `docs/IMPLEMENTATION_COMPLETION_HARNESS.md` for the remediation slice and validate the mounted desktop runtime before acceptance.

Re-review evidence required: full build, real app validation, reusable QA evidence, regression evidence, and Supervisor PASS.

## Source-Level Clearance Notes
- `src-tauri/src/conversation/repository.rs`: partial improvement only. Deterministic lookup is useful but does not close render latency or group routing.
- `src-tauri/src/conversation/service.rs`: blocked. Claimed lookup wiring is not present in commit `d13686d`; it is uncommitted in the current worktree.
- `src/lib/components/conversation/ConversationWorkspace.svelte`: blocked. Event matching improved, but the critical path still refetches and has no measured commit-to-paint guarantee.
- `src/lib/services/conversationService.ts`: blocked. Auto-connect duplicates workspace lifecycle ownership and retry has no replay semantics.
- Tests: blocked for performance acceptance. They use mocked timelines and fixed waits, not the real desktop boundary.

## Evidence Checked

Passed:
- Active desktop app visually inspected: RustSale `v0.1.14`, Conversations screen open on the Zalo contact “Thao”.
- Current production DB latest sampled direct inbound row: message `11694`, conversation `101629`, sender lead `114`; provider timestamp to ingestion timestamp was approximately 38 ms.
- Equivalent indexed SQLite timeline query for lead `114` completed in approximately 5 ms in the sampled no-contention inspection and included the current direct conversation path.
- `anvien analyze --force` refreshed the graph at commit `d13686d`.
- GitNexus was refreshed with `bunx gitnexus analyze`; frontend event interface blast radius is MEDIUM with eight direct importers.
- Source and commit diff inspection is fresh for the current review.

Failed:
- User reports the 5-second UI delay remains in the active desktop app.
- `<30 ms` benchmark has no measurement evidence.
- Commit contents contradict the documented `service.rs` completion claim.
- No runtime evidence proves the new bubble path.

Not run:
- A fresh inbound end-to-end latency sample: a 90-second monitor on open conversation `101629` received no new inbound message, so no honest DB-to-paint latency number was produced.
- Full build/test rerun: the active Tauri/Vite/dev desktop processes hold the runtime/build environment; repository rules require terminating build-related processes first, and this review did not have authority to stop the user's open app.
- Production code modification: Supervisor review is read-only by role boundary.

## Invariant Closure
- Affected invariant: every committed inbound message for the currently open conversation must become visible within a defined commit-to-paint SLO, survive bridge disconnects, and never be erased by stale snapshots.
- Sibling surfaces checked: Zalo ingestion, message lookup, backend broadcast/Tauri forwarder, desktop notification subscriber, frontend bridge, event matching, timeline cache/fetch/merge, tests, benchmark/evidence ledgers, active desktop, and current SQLite data.
- Residual unverified same-invariant surfaces: real inbound sample with stage timestamps; WebView console/long-task timing; DB contention case; reconnect replay; burst ordering; other channels.

## Required Fix List For Resubmission
1. Add stage-level monotonic telemetry keyed by `messageId`/event sequence and reproduce the 5-second sample on the real desktop app before selecting a fix.
2. Replace fire-and-forget invalidation as the render fast path with a render-ready `ConversationDelta` or exact-message fetch; upsert immediately into a central store.
3. Add a durable sequence/change log and subscribe-with-resume replay so reconnect cannot lose messages.
4. Make one app-root service own the Tauri listener; remove component-level duplicate bridge ownership.
5. Route primarily by `conversationId`; make lead aggregation a separate concern and remove broad channel-only refreshes from the hot path.
6. Reconcile full timeline pages in the background with generation/sequence guards so stale responses cannot overwrite newer deltas.
7. Prove SLOs in the mounted desktop runtime: suggested targets are DB commit-to-paint p95 <= 100 ms, p99 <= 250 ms; zero missing/duplicate bubbles across reconnect and 100-message bursts.
8. Close the implementation harness, full build, runtime QA, evidence ledger, impact/change detection, and Supervisor re-review on the exact committed artifact.

## Production Architecture Recommendation

The production path should be:

`Zalo socket -> normalize -> one SQLite transaction (message + conversation metadata + durable change sequence) -> publish ConversationDelta(seq, conversationId, message projection) -> app-root WebView bridge -> central store upsert -> next-frame paint`

The existing 50-message timeline query remains as background reconciliation and recovery, not as the user-visible render dependency. On startup/reconnect, the frontend supplies its last applied sequence, replays any missing deltas, then switches to live delivery. This design removes lead-resolution ambiguity from routing, makes missed events recoverable, and provides exact latency observability.

Rollout should be feature-flagged and dual-run first: emit both legacy invalidations and deltas, compare resulting message IDs/store state, canary the delta renderer, then remove the legacy hot-path refetch only after zero-gap evidence.

## Overall Evaluation
The local changes address two plausible false-negative paths, but the original investigation stopped at event matching and substituted unit-test timing for production latency. The user-visible invariant is still open, the shipped commit is incomplete relative to its own ledger, and the architecture has no replayable render-ready event. The result cannot be accepted as production-ready.
