# Supervisor Report: Zalo Message Status Remediation

Verdict: REJECT

## Metadata

- Report file: `rp_supervisor_260830_182433_by_gpt-5-codex_zalo-message-status-remediation.md`
- Review time: `260830 182433 Asia/Ho_Chi_Minh (UTC+07:00)`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: commit `f076588` and the same-invariant Zalo receipt path from WebSocket decoding through normalization, persistence, invalidation, and UI rendering
- Claim reviewed: the remediation fully fixes Zalo `sent -> delivered -> read` status behavior and is ready for production
- Authority used: current user request; repository `AGENTS.md`; `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`; active plan; current source, diff, tests, and quality-gate output
- Related artifacts: user screenshot; the plan, evidence, benchmark, and actual-status ledgers in this directory; commit `f076588`

## Executive Summary

- Problem: Zalo outbound messages can remain at `sent`, while the remediation claims flexible WebSocket receipt decoding now advances them reliably to `delivered` or `read`.
- Decision: acceptance is blocked. The external receipt boundary is fail-open for structurally invalid receipts, which can cause conversation-wide false status advancement; the touched Rust files also fail the formatter gate, and no real Zalo runtime evidence proves the Tier 3 integration.
- Required outcome: reject structurally invalid receipts before normalization/application (or prove and enforce an authoritative watermark contract), add adversarial end-to-end coverage, make all relevant gates green, and capture fresh live runtime evidence.

## Blocking Findings

### [HIGH] Structurally invalid receipts can falsely advance an entire conversation

File: `src-tauri/vendor/zca-rs/src/models/event.rs:3-60,602-626,683-703`; `src-tauri/vendor/zca-rs/src/listener.rs:961-1038`; `src-tauri/src/conversation/zalo/normalizer.rs:700-765,813-850`; `src-tauri/src/conversation/repository.rs:2407-2520,5990-6065,6392-6484`

Issue: the new deserializers convert wrong types to empty strings, empty vectors, or zero, and `#[serde(default)]` makes all receipt fields optional. Consequently `{}` or a receipt with a valid peer UID but no `msgId`/`realMsgId` deserializes successfully. The listener emits it, the normalizer produces an empty provider message ID and uses current time for a zero timestamp, and `apply_receipt` falls back to sweeping outbound messages in the matching conversation when message resolution fails.

Evidence:

- `event.rs:3-60` returns empty/zero values instead of rejecting unsupported values; `event.rs:602-626` and `683-703` default every delivered/seen field.
- `listener.rs:966-968`, `983-985`, `1001-1007`, and `1022-1028` treat any successfully deserialized default record as a valid event; there is no semantic validity check.
- `normalizer.rs:814-822` can still produce `provider_message_id == ""` or `"0"`; `normalizer.rs:849` converts missing/zero provider time to the current time.
- `repository.rs:5990-5995` produces no ID candidate for empty/`"0"`; `repository.rs:2488-2516` then resolves by conversation and invokes the unanchored sweep.
- Fresh targeted test `cargo test --package rustsale-crm --lib read_receipt_fallback_sweeps_conversation_when_anchor_msg_id_not_found` passed 1/1 and proves the fallback intentionally marks all matching outbound rows when the anchor cannot be resolved (`repository_test.rs:1883-1921`). No negative test rejects an empty/malformed receipt before that fallback.

Why this blocks acceptance: Zalo WebSocket data is an external trust boundary. A malformed or contract-drifted receipt can create false `delivered`/`read` state for messages that were not acknowledged, violating the message-status integrity invariant and the harness requirement for malformed-input and superficial-patch-killer coverage.

Fix direction: separate tolerant representation parsing from semantic validation. Require a non-empty, non-`"0"` provider message anchor before applying a receipt unless an explicitly documented provider watermark contract independently authorizes an unanchored sweep. Reject/quarantine invalid items without dropping valid siblings.

Re-review evidence required: direct tests for `{}`, wrong types, missing IDs, missing thread identifiers, mixed valid/invalid batches, and a receipt with valid peer UID but missing IDs; the tests must prove no DB status or receipt row changes for invalid items while valid siblings still apply.

### [HIGH] Tier 3 external-integration behavior has no E4 closure

File: `src-tauri/src/conversation/zalo/probe_test.rs:148-171`; `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-plan.md:63-77`; user screenshot

Issue: the claimed product behavior is a real Zalo WebSocket-to-DB-to-UI state transition, but the live Zalo probe is ignored and the supplied screenshot only shows one gray tick (`Sent`), not a delivered/read transition. The changed tests validate decoder/deserializer and normalizer pieces, not a real cmd 502/522 packet through listener, service, repository, invalidation, and visible UI.

Evidence:

- `probe_test.rs:148-150` marks the only live Zalo probe ignored because credentials/session are required.
- `aes.rs:459-498` checks only that three decoded shapes expose a `delivereds` key.
- `event.rs:1478-1507` verifies one valid minimal delivery and one seen payload locally.
- The plan marks R-04 `PROVEN` at line 68, but commit `f076588` adds no full listener-to-persistence integration test and no live runtime artifact.

Why this blocks acceptance: the completion harness classifies external integrations as Tier 3 and requires production-like/runtime evidence. Unit coverage cannot prove Zalo actually emits the assumed shapes or that the installed desktop app advances persisted status and renders two ticks.

Fix direction: run the reusable live probe or equivalent controlled non-production Zalo session and record the actual packet shape plus DB/UI state transitions for `sending -> sent -> delivered -> read`, including recipient outside and inside the chat.

Re-review evidence required: sanitized raw-event evidence, message/receipt DB readback, visible desktop screenshots or video, timestamps for each transition, and disclosure of all ignored/skipped checks.

### [MEDIUM] Required formatting gate fails in files changed by the remediation

File: `src-tauri/src/conversation/zalo/normalizer.rs:739-754`; `src-tauri/src/conversation/zalo/normalizer_test.rs:511-514`

Issue: fresh `cargo fmt --check` exits 1 and reports diffs in two files modified by commit `f076588`.

Evidence: the command proposed reformatting the multiline effective-ID expression and the `recipient_external_ids` assertion. `git diff --check f076588^ f076588` passes, but it does not replace the repository formatter gate.

Why this blocks acceptance: the Completion Lock requires relevant formatting/static gates to pass after the last edit.

Fix direction: format the touched Rust files and rerun the full relevant release gates.

Re-review evidence required: fresh successful `cargo fmt --check` plus the relevant backend and frontend suites after the formatting edit.

### [MEDIUM] Completion/evidence ledgers overstate and misreference readiness

File: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-actual-status.md:5,25-37`; `...-benchmark.md:13-25`; `...-evidence.md:24-33`; `...-plan.md:63-77,127-135`

Issue: the plan and actual-status files declare completion/Verified Ready even though the evidence ladder is incomplete and the formatter gate is red. Evidence IDs are also inconsistent: benchmark lines 22-25 cite `E1-P1D-VAL1..4` as four suite results, while the evidence ledger uses `VAL1`/`VAL2` for implementation descriptions and `VAL4` for the Zalo backend suite; there is no frontend `bun test` record in the evidence ledger. Actual status reports 29 zca tests while the evidence ledger reports 30.

Evidence: direct inspection of the four ledgers; no full-build record, live runtime record, DB readback, or Supervisor PASS artifact exists for this remediation before this report.

Why this blocks acceptance: the Completion Lock forbids readiness claims from stale, mismatched, indirect, or missing evidence.

Fix direction: reopen affected ledger rows, assign stable evidence IDs to actual fresh commands/artifacts, distinguish validation timing from benchmarks, and set final readiness only after all lock conditions close.

Re-review evidence required: corrected plan/status/evidence/benchmark ledgers that point to current command output and E4 artifacts without skipped or inferred proof.

## Source-Level Clearance Notes

- `src-tauri/vendor/zca-rs/src/crypto/aes.rs`: partial clearance — lines 306-405 support top-level, direct-object, plaintext, and encrypted/decompressed JSON shapes; lines 459-498 provide local shape tests. No end-to-end receipt proof exists.
- `src-tauri/vendor/zca-rs/src/models/event.rs`: blocked — lines 3-60 and 602-703 make malformed external receipt data indistinguishable from legitimately omitted optional fields.
- `src-tauri/vendor/zca-rs/src/listener.rs`: blocked — lines 961-1038 isolate per-item parse errors, but semantic-invalid default records do not produce parse errors and are emitted. The warning paths also log the raw receipt value at lines 971, 988, 1010, and 1031, which should be reviewed for identifier leakage.
- `src-tauri/src/conversation/zalo/normalizer.rs`: blocked — lines 700-765 and 813-850 emit receipts without enforcing non-empty anchor/thread identity; the file fails `cargo fmt --check`.
- `src-tauri/src/conversation/zalo/normalizer_test.rs`: blocked — new tests cover valid fallback cases but not malformed/missing-anchor rejection; the file fails `cargo fmt --check`.
- Same-invariant repository path: blocked — `repository.rs:2488-2516` and `6392-6484` implement an unanchored conversation-wide sweep, so upstream semantic validation is mandatory.
- UI status rendering: clear for mapping only — `src/lib/components/conversation/MessageBubble.svelte:81-97,631-648` maps sent to one check, delivered to two gray checks, and read to two blue checks. It does not prove the backend supplies those states at runtime.

## Evidence Checked

Passed:

- Fresh `anvien analyze --force`: 510 files scanned, 381 code files parsed, 0 parse failures; graph current at commit `8f01b0e`.
- Fresh Anvien query/file-detail/impact: `normalize_delivered_receipt` is in a high-risk file and its upstream blast radius is `CRITICAL`, with 1 direct affected node, 1 module, and 6 processes; the graph has substantial unresolved Rust references, so source inspection was used as authority for behavior.
- Fresh `cargo test -p zca-rs delivered_and_seen_message_deserialization_handles_omitted_and_numeric_fields`: 1 passed, 0 failed, 29 filtered out.
- Fresh `cargo test --package rustsale-crm --lib read_receipt_fallback_sweeps_conversation_when_anchor_msg_id_not_found`: 1 passed, 0 failed, 798 filtered out.
- Fresh `git diff --check f076588^ f076588`: passed.
- Verification freshness: current for the reviewed worktree and commit.

Failed:

- Fresh `cargo fmt --check`: exit 1 with diffs in the touched normalizer and normalizer test files.
- Source-level adversarial review: a receipt with a valid thread UID but no message ID reaches the unanchored status sweep.
- Evidence integrity review: ledger IDs/counts and readiness declarations are inconsistent and do not include E4 proof.
- Verification freshness: current for the reviewed worktree and commit.

Not run:

- Live Zalo probe: requires an explicit non-production session and credentials; its test is ignored by source annotation.
- Real desktop runtime receipt flow and database readback: no controllable live Zalo session was supplied to this Supervisor lane.
- Full release/publish script: release was not requested and must not be used to compensate for source/evidence blockers.

## Invariant Closure

- affected invariant: an outbound message may advance monotonically to `delivered` or `read` only from an authentic, semantically valid Zalo receipt that can be tied to the correct account/conversation/message or to a documented authoritative watermark.
- sibling surfaces checked: WebSocket decode, receipt models/deserializers, listener batch handling, normalizer, service ingestion, repository ID resolution and sweep fallback, repository regression test, live probe gate, UI status renderer, and remediation ledgers.
- residual unverified same-invariant surfaces: actual cmd 502/522 payloads from Zalo, live reconnect/duplicate/out-of-order behavior, persisted DB transitions, UI invalidation timing, and real desktop rendering.

## Required Fix List For Resubmission

1. Enforce semantic receipt validity at the external/authoritative boundary; do not allow empty/`"0"` anchors to invoke the unanchored sweep without a proven watermark contract.
2. Add adversarial direct-entry tests for empty, wrong-type, missing-ID, missing-thread, mixed-batch, duplicate, and out-of-order receipts, including DB non-mutation assertions.
3. Add a listener-to-service-to-repository integration test for cmd 502/522 payload variants and invalid siblings.
4. Make `cargo fmt --check` and all applicable release gates pass after the final edit.
5. Capture E4 live Zalo and visible desktop evidence for `sent`, `delivered`, and `read`, including timing instrumentation that distinguishes provider HTTP acknowledgement from local scheduler/UI delay.
6. Correct the plan, evidence, benchmark, and actual-status ledgers; do not mark the task production-ready until an independent Supervisor re-review passes.

## Overall Evaluation

The remediation contains useful compatibility improvements for valid reduced/numeric Zalo payloads and preserves per-item batch liveness. It is not acceptable for production because tolerance was added without semantic validation at an external boundary, creating a false-status path through the existing conversation-wide fallback. Evidence quality and formatter status independently keep the Completion Lock open.
