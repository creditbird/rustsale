# Supervisor Report: Enterprise Push-Delta Committed Resubmission

Verdict: REJECT

## Metadata

- Review time: `260831 10:31:44 UTC+7`
- Reviewer: `gpt-5-codex`
- Repo: `RustSale_v2`
- Exact scope: clean HEAD `5c3fd8e728909b5e88d064d67b1d5f24bee86ab6`, 13 commits ahead of upstream
- Commit subject: `fix(omnichannel): full architectural push-delta remediation for transactional snapshot, burst fallback coalescing, and lag recovery`
- Commit size: 37 files, 2,153 insertions, 366 deletions
- Prior finding source: `rp_supervisor_260831_094049_by_gpt-5-codex_enterprise_push_delta_dirty_resubmission.md`
- Authority: user request, repository rules, working-rules, Supervisor and QA skills, Completion Harness, current source/tests/runtime/database
- Boundary: independent read-only review. No production source, test, plan, app process, or commit was changed. This report is the only review artifact created.

## Executive Summary

This revision closes most concrete blockers from the preceding review: timeline rows, hydration, and the global sequence watermark now share one SQLite read transaction; malformed replay payloads now fail closed in Rust; backend lag is emitted to the frontend; reconnect/focus/visibility reconciliation hooks exist; burst fallback uses a pending set; prior duplicate delta rows are reconciled before the unique index is created; and Facebook attachment projection precedes delta hydration.

The production-ready claim is still not acceptable. A malformed durable replay row causes the frontend to retry the same poisoned row forever without an authoritative snapshot fallback. Separately, an empty authoritative timeline page unconditionally drops a concurrently pushed delta even when its durable sequence is newer than the page watermark. The claimed fault-injection closure is also incomplete for the Zalo production ingestion entrypoint. Finally, no exact packaged artifact, migrated production database, mounted runtime, or raw latency distribution exists for this commit.

Required outcome: close the replay and empty-page convergence counterexamples, add the missing production-boundary failure test, then generate and visibly validate an exact package against a production-shaped migrated profile with raw telemetry.

## Blocking Findings

### [CRITICAL] Malformed durable replay never falls back to an authoritative snapshot

File: `src-tauri/src/conversation/repository.rs:3149-3161`; `src/lib/components/conversation/ConversationWorkspace.svelte:1375-1417`

Issue: Rust correctly converts malformed `payload_json` into an error. The caller, however, catches every replay error and schedules the same replay again after 500 ms. It neither refreshes the authoritative open timeline nor advances/quarantines the poisoned row.

Executable counterexample: sequences 101, 102, and 103 exist; row 102 has malformed JSON; the mounted client is at 100 and high-water is 103. Every page fetch fails while deserializing 102. The catch at workspace lines 1409-1415 invokes `resumeMissedDeltas(lastSeenSequence, targetSeq)` again with cursor 100, so 101-103 never converge into the UI.

Why blocking: fail-closed deserialization without a caller recovery path turns one corrupt durable row into permanent message starvation and an unbounded retry loop.

Required fix/evidence: invoke an authoritative resnapshot on replay corruption (with bounded retry/error state), renegotiate the cursor only after that snapshot, and add a component/IPC-boundary test with a valid row before and after a malformed row.

### [CRITICAL] Empty snapshot page deletes a newer pushed bubble

File: `src/lib/components/conversation/timelineState.ts:155-171`

Issue: `retainedNewerItems` evaluates the durable `messageSequenceMap` only when `page.items.length > 0`; for an empty page it is hard-coded to `[]`.

Executable counterexample: refresh starts at snapshot sequence 10 and its query yields zero rows; a delta with sequence 11 is direct-upserted while the read is in flight; the empty page returns with `snapshotSequence=10`. Merge discards the sequence-11 item at lines 157-171 despite it being newer than the authoritative boundary.

Why blocking: the exact stale-snapshot race that the durable watermark is intended to prevent remains executable for an empty conversation/filter result.

Required fix/evidence: apply sequence retention independently of page cardinality and add a test for an empty page plus a concurrent newer delta, including channel-filter and conversation-switch variants.

### [HIGH] Fault injection does not cover the Zalo production ingestion entrypoint

File: `src-tauri/src/conversation/repository_test.rs:6704-6770,6803-6896`; `src-tauri/src/conversation/whatsapp/normalizer_test.rs:2608-2655`; `src-tauri/src/conversation/facebook/normalizer_test.rs:437-474`

Issue: trigger-based production-boundary failure tests now exist for WhatsApp, Facebook, direct-start, and outbound batch. The generic repository test at 6704 manually abandons a transaction; it does not inject a delta insert failure through `apply_normalized_message`, the Zalo live/history persistence entrypoint.

Why blocking: the Completion Harness requires dependency-failure proof at every in-scope authoritative entrypoint; manually dropping a successful transaction cannot prove Zalo propagates a real delta persistence failure and leaves neither message nor child projection committed.

Required fix/evidence: install the same abort trigger, invoke the real Zalo normalized-message entrypoint (including an attachment/history representative where applicable), and assert returned error plus zero message, attachment, and delta rows.

### [HIGH] Exact production/runtime and performance proof is absent

File: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-benchmark.md:29`; `...-evidence.md:56,67-70`

Issue: the benchmark still marks DB-commit-to-paint as unmeasured. The evidence ledger records command summaries and in-memory telemetry implementation, not raw exact-artifact observations.

Fresh observations: no RustSale/Tauri/Cargo/Bun build or test process was running. No release/debug binary or bundle file was present at the checked repo paths. The production DB `/home/arch/.local/share/com.rustsale.crm/rustsale.db` was last modified `2026-08-31 05:55:42 +07`, contains 8,557 messages, and contains neither the `conversation_deltas` table nor its unique index. It therefore has not exercised this migration. No app was available for read-only visible QA.

Why blocking: source and mocked component tests cannot establish the claimed user-visible latency or upgrade behavior for the packaged desktop application.

Required fix/evidence: build/package the exact accepted commit, record artifact hash and provenance, run its migration on a quarantined production-shaped DB, mount that artifact, and retain raw correlated p50/p95/p99 samples plus required visible QA evidence.

## Partial Clearances and Source-Level Notes

| Production group | State | Current evidence |
|---|---|---|
| `repository.rs` timeline variants/projection | Clear for consistent read boundary | Contact, identity, and conversation queries create a SQLite transaction, read `MAX(sequence)`, query rows, and hydrate through the same transaction (`2181-2244`, `2253-2305`, `2314-2361`). |
| `repository.rs` replay | Partial | JSON corruption is surfaced (`3151-3159`), but frontend caller recovery is blocked as above. |
| `events.rs`, `commands.rs`, `types.rs`, `lib.rs` | Clear for reviewed wiring | Delta lag emits `conversation-delta-lagged` (`events.rs:104-109`); HWM/replay command contracts are registered. |
| `conversationService.ts` | Partial | Lag and reconnect listener sets, retry registration, ref counting, and cleanup exist (`633-781`). Established-listener loss still relies on lag/focus/visibility detection; exact runtime behavior is unverified. |
| `ConversationWorkspace.svelte` | Partial | Mount/reconnect/lag/focus/visibility hooks and cleanup exist (`808-847`); pending invalidations are coalesced (`1284-1306`, `1580-1590`); malformed replay fallback remains blocked. |
| `timelineState.ts` | Blocked | Non-empty page uses sequence comparison, but empty page discards all retained newer items (`155-171`). |
| `migration.rs`, `schema.sql` | Clear at source/test level | Duplicate cleanup keeps the maximum sequence before unique-index creation (`migration.rs:269-276`); migration test seeds duplicates and reruns idempotently (`migration_test.rs:1525-1594`). Production migration remains unexecuted. |
| Facebook normalizer/supervisor | Clear for reviewed attachment ordering | Batch and webhook attachments are inserted before `record_message_delta_tx` (`normalizer.rs:269-335,559-599`). |
| WhatsApp normalizer/supervisor | Clear for reviewed atomic failure path | Trigger test crosses the real normalizer and verifies message/delta rollback (`normalizer_test.rs:2608-2655`). |
| Zalo/repository path | Blocked on failure evidence | Same-transaction implementation remains, but no trigger-based real-entrypoint rollback test was found. |
| Direct-start/outbound repository/service | Clear for reviewed failure path | Trigger tests invoke production repository methods and verify rollback (`repository_test.rs:6803-6896`). |
| Test/build/evidence ledgers | Partial | New focused tests exist, but the current review did not reproduce broad claimed counts; runtime and latency evidence are absent. |
| Commit provenance/hygiene | Partial | HEAD is a clean rollback anchor and `git diff --check HEAD^ HEAD` is clean. The commit mixes the remediation with generated `AGENTS.md`/`CLAUDE.md`, unrelated Zalo-status documents, and three prior reports, weakening slice hygiene without by itself disproving source correctness. |

## Evidence Checked

Passed/current:

- Full applicable repository rules, working-rules, Supervisor skill, QA skill, Completion Harness, and previous 094049 report were consumed for this review.
- Agentmemory query returned prior same-scope decisions. No mem0 tool was exposed, so mem0 could not be queried.
- `anvien --help` completed; `anvien analyze --force` scanned 529 files, parsed 381 code files with zero failures, and built 84,238 nodes/97,210 relationships. `repository.rs` remains a HIGH hotspot with 109 fan-in and 43 fan-out edges.
- `git status --short` was empty; exact HEAD and upstream distance were observed; `git diff --check HEAD^ HEAD` emitted no errors.
- Focused current source, test assertions, process inventory, artifact paths, plan/evidence/benchmark claims, and production DB schema/counts were inspected read-only.

Failed/blocking:

- Source counterexamples for poisoned replay and empty snapshot page remain executable.
- Zalo real-entrypoint delta failure injection is absent.
- Production DB has no durable delta schema; exact package and mounted runtime evidence are absent.
- Benchmark records the principal DB-to-paint metric as unmeasured and provides no raw sample artifact.

Not run:

- Broad Cargo suites, Bun suites, `bun run check`, frontend build, Cargo build, and package/release build were intentionally not run after decisive source blockers were established. Historical counts were not adopted as fresh evidence.
- No mounted desktop interaction, screenshot, reload, process termination, or real channel message was performed because no exact runtime with proven provenance was available.

## Invariant Closure and Resubmission Requirements

- Affected invariant: every committed channel message must converge exactly once into the currently visible timeline across corruption, loss, replay, burst, snapshot, migration, and packaged-runtime boundaries.
- Sibling surfaces checked: all three timeline queries, replay/HWM IPC, event forwarder/bridge, lifecycle handlers, fallback state, timeline merge, migration/index, Facebook attachments, channel fault tests, evidence ledgers, process/package state, and production DB.
- Residual failures: poisoned replay recovery, empty-page stale snapshot, Zalo production-entrypoint rollback evidence, exact migration/runtime/package proof, and raw latency distribution.

For the next review:

1. Add authoritative bounded fallback for replay deserialization failure and prove recovery across a poisoned row.
2. Preserve sequence-newer deltas when the returned snapshot page is empty and add the adversarial race test.
3. Add trigger-based Zalo production-ingest rollback coverage.
4. Keep the corrective commit narrowly scoped and refresh all exact-commit gates.
5. Package and mount that commit, migrate a production-shaped copy, and retain raw, correlated all-channel latency and visible QA artifacts.

## Overall Evaluation

The backend/data-layer remediation is materially stronger and closes most prior source findings. Two frontend convergence defects remain at the exact recovery and snapshot boundaries, while required Zalo failure and exact desktop runtime evidence are incomplete. Completion Lock remains closed; handoff to Main is to remediate the three code/test blockers before spending time on packaging and production telemetry.
