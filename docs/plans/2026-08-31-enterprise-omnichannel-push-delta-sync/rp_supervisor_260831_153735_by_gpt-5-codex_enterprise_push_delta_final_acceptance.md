# Supervisor Report: Enterprise Push-Delta Final Acceptance

Verdict: REJECT

## Metadata

- Review time: `260831 15:37:35 UTC+7`
- Reviewer: `gpt-5-codex`
- Exact scope: clean HEAD `38fc5ba38bff92ee5327201974f710d5f4a79b44`, two commits ahead of `origin/master`
- Source commit: `5a30267233caa708d9a0b9ff572585096f84817a`
- Evidence/manifest commit: `38fc5ba38bff92ee5327201974f710d5f4a79b44`
- Previous report: `rp_supervisor_260831_132938_by_gpt-5-codex_enterprise_push_delta_four_blocker_resubmission.md`
- Authority: user request, repository rules, working-rules, Supervisor and QA skills, Completion Harness, exact source/tests/release workflow/artifacts/database/runtime evidence
- Boundary: independent read-only review. No source, test, plan, DB, artifact, release, app process, or commit was changed. This report is the only new artifact.

## Executive Summary

The new code fixes the prior fire-and-forget path in the simplest case: it awaits an authoritative timeline response and preserves the live buffer when that request rejects. The dry-run release workflow also genuinely builds a Tauri `.deb`, signs/scans it, generates local `latest.json`, and exits before upload. The production DB now contains the intended table and three indexes and passes read-only integrity checking.

Acceptance is nevertheless blocked by three replay lifecycle races. A late snapshot mutates the global sequence cursor before the code verifies that the user is still viewing the same conversation. A snapshot rejection after component unmount can create a new retry timer after teardown. A retry timer that fires while another replay owns the single `isResumingDeltas` flag returns immediately and loses the recovery obligation. The new tests do not assert these cursor, teardown, or backoff-collision invariants.

The package is real, but no packaged runtime was mounted and no raw p50/p95/p99 evidence exists. The signature text matches local `latest.json`, but cryptographic verification could not be performed with the available Tauri CLI, which exposes signing but no verification command. No pre-migration backup provenance was found.

Required outcome: make replay recovery generation/key-aware and teardown-safe, serialize/coalesce retry obligations instead of dropping them, add adversarial cursor/lifecycle assertions, then perform visible exact-package QA and collect raw telemetry.

## Blocking Findings

### [CRITICAL] Late snapshot advances the global cursor before conversation-key validation

File: `src/lib/components/conversation/ConversationWorkspace.svelte:1115-1130,1426-1445`

Issue: `fetchVariantFirstPage` updates `lastSeenSequence` from `page.snapshotSequence` and prunes `messageSequenceMap` at lines 1121-1129 for every completed variant fetch. Only after `refreshOpenTimeline()` returns does replay recovery compare `currentKey` with the captured `targetKey` at lines 1433-1444.

Executable counterexample: conversation A starts poisoned-replay recovery; the user switches to B; a B delta is lost; the delayed A snapshot returns with global watermark 103. It advances the single cursor to 103 although A's 50-row snapshot did not recover B's message. Subsequent HWM at 103 sees no gap, so B can remain missing. Test 28c checks only that B's visible timeline is not overwritten; it never asserts cursor/HWM recovery or message-map preservation.

Why blocking: a per-conversation snapshot cannot acknowledge a global event cursor before ownership/generation validation.

Required fix/evidence: move cursor and map acknowledgement behind a captured-key/generation check, or return an unapplied snapshot result and commit it atomically only for the still-active recovery target. Test switch A→B with a missing B delta and a subsequent HWM call.

### [HIGH] Component teardown can be followed by a newly scheduled replay timer

File: `src/lib/components/conversation/ConversationWorkspace.svelte:835-848,1431-1452,1470-1478`

Issue: teardown clears only the retry timer that exists at that instant. There is no mounted/abort generation flag. If a pending snapshot rejects after unmount, the catch calls `scheduleReplayFallbackRetry`, creating a fresh timer after cleanup. That timer later invokes component state/IPC work.

Why blocking: recovery survives beyond its component owner, risks leaks and stale writes, and violates required unmount cleanup semantics.

Required fix/evidence: maintain a lifecycle generation or abort signal checked after every await and before scheduling/draining; test unmount while the snapshot promise is pending, then reject/resolve it and prove no timer, IPC retry, cursor mutation, or render occurs.

### [HIGH] Backoff retry is silently lost when another replay is active

File: `src/lib/components/conversation/ConversationWorkspace.svelte:1395-1397,1470-1478`

Issue: the retry timer clears itself and calls `resumeMissedDeltas`. If another focus/reconnect/gap replay currently holds `isResumingDeltas`, the function returns immediately at line 1396. The timer obligation is not requeued and the preserved buffer may never be reconciled.

Why blocking: ordinary lifecycle overlap can cancel the only poisoned-row recovery attempt without success or terminal error handling.

Required fix/evidence: serialize recovery through one queued target/high-water obligation or reschedule when the current owner finishes. Add a fake-timer test where the backoff fires during a delayed concurrent resume and prove eventual snapshot/buffer convergence.

### [HIGH] Exact packaged runtime latency remains unverified

File: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-benchmark.md:28-29,46`

Issue: no RustSale process was running, no visible packaged QA artifact exists under `Reports/qa`, and the benchmark still labels receive-to-paint and DB-to-paint as unmeasured while separately marking a target met. Source-level rAF aggregation is not a raw runtime sample set.

Why blocking: package existence cannot prove the user-visible `<16 ms`/p95 SLO or real Zalo hot path.

Required fix/evidence: mount the exact distributable read-only, exercise authorized test inbound scenarios, and store raw correlated samples plus calculated p50/p95/p99 and visible QA evidence tied to artifact hash.

## Partial Clearances and Per-Group Notes

| Group | State | Evidence |
|---|---|---|
| Basic poisoned-replay fallback | Partial | Snapshot is awaited and rejection preserves buffer (`1431-1452`); successful buffer drain is ordered (`1458-1467`). Ownership, teardown, and retry collision remain blocked. |
| Component tests | Partial | Tests 28a/28b cover successful snapshot and reject-then-success. Test 28c covers visible non-overwrite only. No null/unmount/backoff-collision or post-switch cursor/HWM assertion was found. |
| Empty-page merge and Zalo rollback | Clear from prior review | No regression in the two new commits; the focused boundary and real-entrypoint trigger tests remain present. |
| Release workflow | Clear for dry-run semantics | `publish-release.ts:97-145` runs frontend/Tauri build; `338-342` exits before R2 upload. No release was performed during review. |
| `.deb` provenance | Partial | `.deb` SHA-256 `1e84c43a6b1cbe299b24ca1aeb784905b35e7259ba76ed0c23c14bc508907b8a`, timestamp `15:27:07 +07`, lies after source commit `5a30267` (`15:20:56`) and before docs commit `38fc5ba` (`15:27:45`). The docs-only difference is compatible with packaging source commit, but no embedded commit identity/build log independently proves a pristine build input. |
| Signature/manifest | Partial | `.sig` exists (420 bytes), SHA-256 `bc5e90113a3ebc15d0f1400628d96465bcc25b943fe0c33311b46b20867d6d12`; its exact text equals local `latest.json` signature. `bunx tauri signer` exposes `sign`/`generate`, not verify, so cryptographic verification was not completed. Local manifest records v0.1.14 and correct encoded `.deb` URL; dry-run source proves it was not uploaded. |
| Production DB migration | Partial | Read-only inspection shows `conversation_deltas`, indexes `idx_conversation_deltas_conv`, `idx_conversation_deltas_seq`, and unique `idx_conversation_deltas_message_event`; zero delta rows; `PRAGMA integrity_check` returns `ok`, and foreign-key check returned no rows. DB mtime is `15:20:24`, before source commit; no backup file/provenance was found. Migration structure is present, but real post-migration message ingestion/replay was not observed. |
| Test/build ledgers | Partial | Evidence claims current suites/builds, but no raw logs tied to the final commits were stored. Broad counts were not independently rerun after decisive source findings. |
| Runtime/telemetry | Blocked | No mounted exact package, screenshots/trace, raw samples, or percentile artifact. |

## Evidence Checked

Passed/current:

- Full applicable repository rules, working-rules, Supervisor, QA, Completion Harness, and report 132938 were consumed.
- Agentmemory was queried. No mem0 tool was exposed, so it could not be queried.
- `anvien --help`; `anvien analyze --force`: 531 files scanned, 381 code files parsed, zero failures, 84,344 nodes and 97,351 relationships. `repository.rs` remains HIGH blast radius (113 fan-in, 43 fan-out).
- Exact commit window/status/diffs, replay source, test assertions, release script, local manifest, artifact hashes/timestamps, signature text equality, process list, telemetry artifact search, and production DB schema/index/count/integrity were inspected fresh and read-only.
- `git diff --check 5b5324a..38fc5ba` emitted no errors.

Failed/blocking:

- Conversation-switch cursor ownership, unmount scheduling, and retry-collision invariants remain false or unguarded in source.
- Critical adversarial tests for those paths are absent.
- No visible exact-package runtime or raw latency evidence exists.
- Cryptographic signature verification and pre-migration backup provenance were not established.

Not run:

- Broad Cargo/Bun suites, typecheck, frontend build, Tauri build, and release workflow were not rerun after decisive source blockers were proven. Claimed counts were not adopted as independent fresh results.
- No app interaction, real message, reload, process termination, DB write/migration, artifact modification, publish, or release was performed.
- `.deb` payload inspection was unavailable because `dpkg-deb` is not installed; the failed read-only pipeline produced no extracted files.

## Invariant Closure and Handoff

- Affected invariant: a replay/snapshot may acknowledge a global durable sequence only for the still-live, still-owned recovery generation; no recovery obligation or buffered event may be lost through switch, teardown, or concurrency.
- Sibling surfaces checked: fetch application, cursor/map update, replay catch/drain/backoff, lifecycle cleanup, component tests, release build/dry-run/sign/manifest flow, artifacts, production DB, runtime processes, and telemetry ledgers.
- Residual failures: late-snapshot cursor acknowledgement, post-unmount retry, retry collision, runtime SLO evidence, signature verification, and backup provenance.

Handoff to Main: repair the replay recovery coordinator and add the three adversarial lifecycle tests before another acceptance request. After source acceptance, perform visible QA on the exact `.deb`, verify its signature with an independent compatible verifier, and record production-shaped migration backup/readback plus raw latency evidence.
