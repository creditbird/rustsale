# Supervisor Report: Enterprise Push-Delta Four-Blocker Resubmission

Verdict: REJECT

## Metadata

- Review time: `260831 13:29:38 UTC+7`
- Reviewer: `gpt-5-codex`
- Exact scope: clean HEAD `e0e35521f3dae5cb111642f53d9e9fe19d800607`, 14 commits ahead of upstream
- Commit: 7 files, 383 insertions, 27 deletions
- Previous report: `rp_supervisor_260831_103144_by_gpt-5-codex_enterprise_push_delta_committed_resubmission.md`
- Authority: user request, repository rules, working-rules, Supervisor and QA skills, Completion Harness, exact source/tests/artifacts/runtime/database
- Boundary: review-only. No source, test, plan, application process, or commit was changed. This is the sole new report.

## Executive Summary

The resubmission closes two prior items at source/test level: empty-page timeline merging now consults the durable message sequence, and a trigger-driven Zalo `apply_normalized_message` test proves rollback of message, attachment, and delta rows. The poisoned replay change, however, implements the exact unsafe behavior the review required it to avoid: it launches a snapshot refresh without awaiting success, advances the durable cursor to the target immediately, skips buffered events at or below that target, and clears the buffer. A failed snapshot can therefore permanently hide committed messages while future reconciliation believes the cursor is current.

The runtime claim also remains unsupported: the fresh file is a `cargo build --release` executable created before the commit, while the distributable `.deb` predates the remediation by two days. No exact packaged app is mounted, the production database has not run the delta migration, and the benchmark still contains no raw latency distribution.

Required outcome: make poisoned-replay recovery transactional in frontend state—await successful authoritative snapshot before cursor advancement or buffer disposal—and prove both snapshot failure and live-buffer ordering. Then package and visibly validate the exact accepted commit.

## Blocking Findings

### [CRITICAL] Replay fallback falsely advances the cursor before snapshot success

File: `src/lib/components/conversation/ConversationWorkspace.svelte:1417-1435`

Issue: the catch block calls `void refreshOpenTimeline()` at lines 1420-1422, then immediately sets `lastSeenSequence = targetSeq` at lines 1423-1425. Because the refresh is neither awaited nor checked, cursor truth is detached from snapshot success.

Executable counterexample: replay of 101-103 fails on corrupt row 102; target is 103; the subsequent timeline IPC rejects or the component switches conversations. The code still records cursor 103. Later high-water reconciliation sees no gap and never recovers messages 101-103.

The same block sorts the live buffer, applies only events greater than the now-advanced target, and clears the complete buffer at lines 1426-1435. A buffered sequence 103 or lower is discarded even though the failed replay and failed snapshot never rendered it.

Why this blocks acceptance: the durable cursor is a claim of applied/authoritatively snapshotted state. Advancing it before authoritative success causes permanent silent data loss.

Required fix: await the snapshot; advance the cursor only from the successfully returned snapshot watermark; preserve/reconcile buffered events against that watermark; on refresh failure keep the old cursor and buffer with bounded retry/error state. Capture the selected timeline key so a late snapshot cannot validate a different conversation.

Required re-review evidence: tests for rejected snapshot, delayed snapshot with live events before/at/after target, conversation switch during fallback, and successful snapshot watermark advancement. Assert rendered messages, cursor behavior through subsequent high-water calls, buffer preservation, and bounded retry—not merely that the timeline IPC was invoked.

### [HIGH] The new replay test proves invocation, not recovery correctness

File: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts:2205-2243`

Issue: Test 28 makes replay throw and only asserts `mockGetContactTimeline` was called. It does not make the fallback snapshot fail, assert that the cursor stays unchanged, inject live buffered deltas, or prove eventual rendered convergence.

Why this blocks acceptance: this test passes the unsafe fire-and-forget implementation above and is not the required superficial-patch killer.

Fix/evidence direction: replace or extend it with the adversarial cases listed in the preceding finding.

### [HIGH] Exact packaged runtime, migrated production data, and raw latency proof are absent

Files: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md:53-76`; `...-benchmark.md:28-29,46`

Issue and evidence:

- Evidence line 76 calls `cargo build --release` a packaged desktop binary. It produced `src-tauri/target/release/rustsale-crm`, SHA-256 `41f160cfa91f83e8924a2479574112cb80ce04601d674601a76c04b25d0dbd73`, timestamp `12:39:35 +07`, 99 seconds before commit `e0e3552` at `12:41:14 +07`. This is not a Tauri distributable and exact committed-source provenance was not recorded.
- The actual `.deb`, SHA-256 `9c8a119c1fae86979fccdf3132fd09eb9237e4ab701479ee9b653c8c90cee781`, is dated `2026-08-29 11:10:41 +07`, before both remediation commits.
- No RustSale/Tauri/Cargo/Bun build/test process was running and no exact app was mounted for read-only QA.
- Production DB `/home/arch/.local/share/com.rustsale.crm/rustsale.db` remains at `2026-08-31 05:55:42 +07`, has 8,557 messages, and has neither `conversation_deltas` nor its unique index.
- Benchmark lines 28-29 explicitly say both relevant measures are unmeasured, while line 46 labels a target as met without raw samples. No raw p50/p95/p99 runtime artifact was found.

Why this blocks acceptance: a Rust executable compilation does not prove Tauri packaging, migration, mounted WebView behavior, or the claimed latency SLO.

Required re-review evidence: build a distributable from the final clean commit using the repository release workflow, record commit/artifact hashes and timestamps, migrate a quarantined production-shaped DB with readback, mount that artifact visibly, and store raw correlated all-channel samples plus calculated p50/p95/p99 and official QA evidence.

## Partial Clearances and Per-File-Group Notes

| Group | State | Evidence |
|---|---|---|
| `timelineState.ts` and boundary spec | Clear for empty-page sequence retention | Lines 157-170 now evaluate `messageSequenceMap` before page-cardinality fallback. The focused spec at lines 230-277 covers sequence 11 retention and sequence 9 rejection against snapshot 10. |
| Zalo repository ingestion/test | Clear for reviewed rollback case | `repository_test.rs:6898-6952` installs a delta abort trigger, calls real `apply_normalized_message` with a Zalo image attachment, and asserts zero message, attachment, and delta rows. |
| `ConversationWorkspace.svelte` replay fallback | Blocked | Lines 1420-1435 advance cursor and destroy buffered obligations before the unawaited snapshot proves success. |
| `ConversationWorkspace.spec.ts` | Blocked for poisoned replay | Test 28 checks only that a refresh call occurred; Test 29 gives useful mounted-component support for the empty-page change. |
| Evidence/benchmark docs | Blocked | Source/test claims are partly accurate, but line 53 explicitly documents unsafe cursor advancement as success; package and performance claims exceed observed evidence. |
| Prior backend transaction/migration/channel groups | No regression found in this seven-file commit | This commit does not modify those production files; the partial clearances from report 103144 stand, subject to exact runtime/migration proof. |
| Commit provenance | Clear as source rollback anchor | HEAD and worktree are clean and the corrective source/test slice is committed; the release executable predates the commit and is not a distributable provenance anchor. |

## Evidence Checked

Passed/current:

- Required rules, working-rules, Supervisor, QA, Completion Harness, and report 103144 were consumed.
- Agentmemory was queried. No mem0 tool was exposed, so mem0 could not be queried.
- `anvien --help`; `anvien analyze --force`: 530 files scanned, 381 code files parsed, zero failures, 84,296 nodes and 97,288 relationships. `repository.rs` remains HIGH blast radius with 113 fan-in and 43 fan-out relationships.
- Exact HEAD/status/diff, focused source diff, production source, test assertions, evidence/benchmark files, process list, artifact hashes/timestamps, and production DB schema/counts were checked fresh and read-only.
- `git diff --check HEAD^ HEAD` completed without output.

Failed/blocking:

- Poisoned replay cursor/buffer invariant is false in current source.
- Test 28 cannot detect that failure.
- No exact Tauri distributable/runtime, migrated production DB, or raw p50/p95/p99 evidence exists.

Not run:

- Broad Cargo/Bun suites, typecheck, frontend build, Cargo build, and packaging were not rerun after the decisive source blocker was proven. Ledger counts were not adopted as fresh independent results.
- No app interaction, reload, process termination, screenshot, or real channel message was performed because no exact mounted artifact existed.

## Invariant Closure and Handoff

- Affected invariant: a cursor may advance only after every prior durable event is applied or a successful authoritative snapshot proves equivalent state; buffered live events must survive failed recovery.
- Sibling surfaces checked: replay loop/catch, HWM trigger, refresh path, live buffer, component tests, empty-page merge/spec, Zalo rollback test, package artifacts, evidence ledger, benchmark, processes, and production DB.
- Residual failures: poisoned replay snapshot/cursor atomicity, buffered-live-event preservation, exact package/migration/runtime proof, and raw latency distribution.

Handoff to Main: fix and adversarially test the replay recovery state machine first. Do not spend another build cycle on the standalone Cargo release executable; after source acceptance, create and mount the actual Tauri distributable from the exact commit and collect migration plus raw telemetry evidence.
