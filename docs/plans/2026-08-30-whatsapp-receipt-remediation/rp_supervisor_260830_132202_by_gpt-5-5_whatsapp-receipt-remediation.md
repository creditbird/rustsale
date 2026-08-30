# Supervisor Report: WhatsApp Receipt Remediation Resubmission

Verdict: REJECT

## Metadata

- Report file: `rp_supervisor_260830_132202_by_gpt-5-5_whatsapp-receipt-remediation.md`
- Review time: `260830 132202 Asia/Ho_Chi_Minh (+0700)`
- Reviewer: `gpt-5-5`
- Repo/project: `RustSale_v2` (Anvien registry name: `rustsale`)
- Scope reviewed: commit `53c8bf664be3cdca10c9afe52c45d753b4cae9b0`, merged at `3f82f64eaab363e0b19a58293e4bfc5f74f4e0fd`, against fixed point `f937999d00e3522c82ba09dc6caa0491ad4de3f2`
- Claim reviewed: all Standards 1-6 and Spec 1-5 are resolved and the implementation is `VERIFIED READY`
- Authority used: current user request; repository `AGENTS.md`; `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`; remediation plan/ledgers; current source; dependency contract; fresh build/test/Anvien evidence
- Related artifacts: the four files under `docs/plans/2026-08-30-whatsapp-receipt-remediation/`

## Executive Summary

- Problem: determine whether the remediation closes every prior finding and every Tier-3 Completion Lock condition.
- Decision: REJECT. Several local fixes are correct and the fresh build/tests pass, but one production WhatsApp failure path is still bypassable, the claimed listener/logging proof is narrower than the claim, the fixed-point whitespace gate fails, current Anvien scope does not match the ledger, and required E4/runtime evidence remains absent with the live Zalo probe still ignored.
- Required outcome: repair and adversarially test the real no-class NACK path, replace the indirect listener/logging test with boundary evidence, correct the diff/Anvien/requirement ledgers, and supply production-like runtime evidence or downgrade readiness with the exact external blocker.

## Blocking Findings

### [HIGH] Error ServerAck without `class` is still dropped

File: `src-tauri/src/conversation/whatsapp/supervisor.rs:787`

Issue: `map_whatsapp_receipt_event` accepts a `ServerAck` only when `ack.class == Some("message")`. The pinned `wacore 0.7.0` contract makes `class` optional, and the pinned `whatsapp-rust 0.7.0` dependency's own NACK test constructs an error ACK with an ID and `error = "479"` but no `class` or timestamp. That event returns `None` here and can never reach the new `failed` state transition. Its `from` value is also the server JID, so merely removing the class guard while retaining it as `chat_jid` would be rejected by the conversation-JID guard.

Evidence:

- `src-tauri/src/conversation/whatsapp/supervisor.rs:787-803`: requires `class == "message"` before mapping either success or failure ACK.
- `src-tauri/Cargo.lock:7118-7119` and `:7540-7541`: pin `wacore` and `whatsapp-rust` to `0.7.0`.
- Local dependency source `wacore-0.7.0/src/types/events.rs:1663-1674`: documents that `class` may be `None` and `error` carries the NACK code.
- Local dependency source `whatsapp-rust-0.7.0/src/client/tests.rs:337-353`: verifies a NACK with `ack.class.is_none()` and `ack.error == Some("479")`.
- `src-tauri/src/conversation/whatsapp/supervisor_test.rs:473-485`: the new test fabricates a friendlier error ACK containing both peer `from` and `class = "message"`, so it does not kill the production counterexample.

Why this blocks acceptance: Spec 3 explicitly claims all error ServerAck events map to `failed`. The dependency's valid no-class NACK shape bypasses the authoritative mapper, so the invariant is not closed.

Fix direction: handle an error ACK with a matching message ID even when `class` is absent, without weakening protection for ACKs belonging to other stanza classes. Correlation should occur against the account-scoped outbound message record; do not force a server JID through peer-chat validation.

Re-review evidence required: a killer test using the dependency-realistic NACK shape (`id`, `error`, server `from`, no `class`, no timestamp) that reaches the stored outbound message, sets `failed`, records the receipt, and publishes invalidation; plus negative coverage for unrelated non-message ACK IDs/classes.

### [HIGH] Tier-3 E4/runtime evidence is still missing

File: `src-tauri/src/conversation/zalo/probe_test.rs:148`

Issue: the full backend command still skips `live_zalo_conversation_probe`, and the remediation evidence contains only unit/integration tests. No real Zalo/WhatsApp session demonstrates receipt event -> normalizer -> SQLite sweep/state -> invalidation -> visible UI. No browser/desktop runtime or Playwright JSON/Markdown evidence exists for the changed `MessageBatchGroup` UI.

Evidence:

- Fresh `cargo test --manifest-path src-tauri/Cargo.toml`: `796 passed; 0 failed; 1 ignored`; the ignored test is `live_zalo_conversation_probe` at lines 148-150.
- `docs/IMPLEMENTATION_COMPLETION_HARNESS.md:56`, `:215-222`, and `:293-311`: Tier 3 requires real runtime/production-like evidence and skipped critical checks must affect readiness.
- `AGENTS.md` requires visible runtime evidence for UI behavior and official Playwright evidence under `Reports/qa/playwright/`; no `Reports` directory/evidence exists.
- Evidence ledger `E6-P1F-FULL` records only backend tests and five isolated `MessageBatchGroup` component tests.

Why this blocks acceptance: external integrations and DB/UI state transitions are Tier 3. E2/E3 tests cannot support `VERIFIED READY` for the E4 claim, and the ignored test was not disclosed in the verdict.

Fix direction: execute the exact staging scenario with live non-production credentials and record DB plus visible-runtime evidence. If credentials remain unavailable, use `CONDITIONALLY READY` with the exact missing evidence, affected requirement IDs, residual risk, and command/manual steps.

Re-review evidence required: live Zalo anchor/reaction cases and WhatsApp ACK/receipt cases with DB receipts/status and invalidation/UI evidence, plus official runtime artifacts; or a truthful conditional verdict.

### [HIGH] The whitespace gate was run against a vacuous post-commit diff

File: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-evidence.md:10`

Issue: on a clean committed worktree, `git diff --check HEAD` checks an empty diff and cannot validate the remediation commit. The correct fixed-point comparison fails.

Evidence:

- Fresh `git diff --check f937999d00e3522c82ba09dc6caa0491ad4de3f2..HEAD`: exit code `2`.
- Reported violations: new blank line at EOF in the benchmark ledger line 19 and evidence ledger line 22.
- Fresh `git status --short --branch`: clean and aligned with `origin/master`, which explains why `git diff --check HEAD` is trivially empty.

Why this blocks acceptance: the report explicitly claims zero whitespace issues and Harness Completion Lock requires the relevant fixed-point diff gate to pass.

Fix direction: remove the two EOF violations and validate the actual commit/range (`git show --check 53c8bf6` or fixed-point range), not an empty worktree diff.

Re-review evidence required: exit code 0 from fixed-point or commit-range whitespace verification.

### [HIGH] Requirement and scope ledgers do not match current evidence

File: `docs/plans/2026-08-30-whatsapp-receipt-remediation/2026-08-30-whatsapp-receipt-remediation-plan.md:55`

Issue: the plan has checklist slices but no Harness requirement ledger mapping each Standards 1-6 / Spec 1-5 item to an ID, invariant, positive case, negative case, required evidence, owner, and status. Its scope figures also conflate changed and affected files.

Evidence:

- Commit `53c8bf6` changes 23 paths, not 19.
- Fresh Anvien compare from `f937999..HEAD`: `risk_level = critical`, `1516 changed`, `23 changed_files`, `22 affected_count/processes`, and `19 affected_files`.
- The submitted report states `299 changed symbols`, `24 affected symbols`, and `19 changed files`.
- Fresh `anvien analyze --force`: `82,018 nodes`, `94,777 relationships`, not `82,020 / 94,779`.

Why this blocks acceptance: Completion Lock requires every finding to have a GREEN requirement row and requires detect-changes to match intended files/processes/modules. The durable ledgers do not provide that traceability and their current scope evidence is inaccurate.

Fix direction: add the mandatory requirement ledger, distinguish changed files from affected files, attach current Anvien output, and disposition all affected processes and direct dependencies.

Re-review evidence required: a complete 11-item ledger and fresh Anvien comparison matching the committed range.

### [MEDIUM] Listener logging/liveness test is narrower than claimed

File: `src-tauri/src/conversation/whatsapp/supervisor_test.rs:443`

Issue: the test calls the helper directly, first with an invalid DB path and then with a different valid DB path. It does not run the actual event listener, does not capture/assert the `warn!` record, and cannot demonstrate that a listener connected to one failing/corrupt dependency remains usable for a subsequent event.

Evidence:

- Lines 492-501 assert only `Err` from a direct helper call.
- Lines 503-536 change to a healthy DB path and call the helper again.
- The `warn!` exists in production at `supervisor.rs:817-823`, but no tracing collector assertion verifies the claimed observability behavior.

Why this blocks acceptance: Standards 3 and the plan's acceptance criterion 5 claim listener-boundary logging and liveness, while the evidence proves only helper re-entry under two different dependencies.

Fix direction: exercise the same event-handler/listener instance across the failure and success events and capture the warning. Use a recoverable failure on the same dependency/path or define the actual recovery contract explicitly.

Re-review evidence required: listener-boundary sequential-event test and asserted warning fields/message.

## Source-Level Clearance Notes

- `src-tauri/src/conversation/whatsapp/normalizer.rs`: mostly clear. Outbound-only lookup, chat-JID isolation, monotonic delivery transitions, terminal failed/read/removed behavior, and monotonic receipt timestamps are present at lines 1315-1451 and covered by focused tests.
- `src-tauri/src/conversation/whatsapp/supervisor.rs`: blocked. `ReceiptType::ServerError` mapping and error logging exist, but dependency-valid no-class NACKs are rejected at lines 787-803.
- `src-tauri/src/conversation/adapter.rs` and `conversation/mod.rs`: clear for the reviewed dead-code finding. Registry/capability abstractions and the untracked adapter test module are gone.
- `src-tauri/src/conversation/repository.rs`, `conversation/types.rs`, and Zalo normalizer/supervisor group: no new blocker found in this resubmission review; composite suffix matching, reaction anchor cutoff, monotonic receipts, group receipt rounds, seen normalization, and outbox notification remain present and pass the fresh backend suite.
- `src/lib/components/conversation/MessageBatchGroup.svelte`: local aggregate status logic is clear and component tests pass; real-runtime E4 evidence is absent.
- Root command/client formatting-only group: no behavioral blocker found; included in the 23-path committed range and must be accounted accurately.
- Plan/evidence/benchmark artifacts: blocked by false readiness, incomplete requirement mapping, stale graph counts, and vacuous whitespace command.

## Evidence Checked

Passed:

- `anvien analyze --force`: 497 files scanned, 381 parsed, 0 parse failures, 82,018 nodes, 94,777 relationships.
- `cargo build --manifest-path src-tauri/Cargo.toml`: exit 0.
- `bun run build`: exit 0; warnings remain but build completed.
- `cargo check --manifest-path src-tauri/Cargo.toml --tests`: exit 0.
- `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: exit 0.
- `cargo test --manifest-path src-tauri/Cargo.toml`: 796 passed, 0 failed, 1 ignored.
- `bun run check`: 0 errors, 6 warnings.
- `bun test src/lib/components/conversation/__tests__/`: 335 passed, 0 failed, 938 assertions.
- Worktree was clean before this supervisor report and matched `origin/master`.
- Verification freshness: fresh on 2026-08-30 after HEAD `3f82f64`.

Failed:

- `git diff --check f937999d00e3522c82ba09dc6caa0491ad4de3f2..HEAD`: exit 2, two new blank-line-at-EOF violations.
- Anvien fixed-point comparison does not match the report: 23 changed files and 22 affected processes/count versus the reported 19 and 24.
- Source/dependency contract inspection shows no-class NACK bypass.
- Verification freshness: fresh on current HEAD.

Not run:

- Live Zalo probe: requires external non-production credentials and is explicitly ignored.
- Live WhatsApp receipt/NACK scenario: no live session credentials supplied.
- Desktop/browser runtime and Playwright UI evidence: no runtime/evidence artifacts supplied; creating implementation QA artifacts is outside Supervisor's read-only review role.

## Invariant Closure

- Affected invariant: provider delivery/read/failure events must correlate only to the correct outbound message, transition status monotonically, persist monotonic audit receipts, publish invalidation after authoritative DB state change, and remain observable/resilient under dependency failure.
- Sibling surfaces checked: WhatsApp raw receipt mapper, ServerAck mapper, normalizer query/state machine, DB receipt upsert, invalidation publisher, listener helper/tests, Zalo live probe status, aggregate UI component/tests, plan/evidence ledgers, fixed-point diff, full backend and relevant frontend gates, Anvien graph and compare scope.
- Residual unverified same-invariant surfaces: dependency-realistic no-class NACK; live Zalo/WhatsApp transport; visible UI runtime; actual listener-boundary recovery/log capture.

## Required Fix List For Resubmission

1. Support and adversarially test the dependency-realistic error ACK that omits `class` and uses a server JID, while rejecting unrelated stanza ACKs.
2. Replace the helper-only logging/liveness test with same-listener, same-boundary evidence and assert the warning.
3. Remove the two committed EOF whitespace violations and run the fixed-point/commit-range gate.
4. Add a complete 11-item requirement ledger and correct Anvien/scope figures (23 changed files versus 19 affected files).
5. Produce Tier-3 production-like evidence for live receipt ingestion and visible UI; otherwise issue a truthful `CONDITIONALLY READY` report. Because item 1 remains an implementation defect, the present state is `NOT READY`, not merely conditional.

## Overall Evaluation

The resubmission correctly fixes outbound/JID isolation, receipt transition regression, monotonic timestamps, dead registry code, and the mapped error shapes covered by its tests. Fresh builds and broad suites are green. Acceptance is nevertheless rejected because the real dependency exposes a valid NACK shape that still bypasses failure ingestion, the Tier-3 runtime requirement remains unproven, and multiple Completion Lock artifacts/commands do not describe the committed range accurately.
