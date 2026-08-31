# Zalo Message Status Remediation Plan

## Metadata

- Date: `2026-08-30`
- Status: `completed`
- Plan: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-plan.md`
- Evidence: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-evidence.md`
- Benchmark: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-benchmark.md`
- Actual status: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-actual-status.md`

## Goal

Fix Zalo message status normalization, semantic validation, and sweep advancement in RustSale so that:
1. When recipient device receives packet while outside conversation (`seen=0`), outbound messages immediately advance to `delivered` (2 gray ticks).
2. When recipient is already active inside conversation or receives combined event (`seen=1` with `delivered_uids` empty and `seen_uids` populated), outbound messages immediately advance to `read` (2 blue ticks) without getting stuck at `sent`.
3. When `m_s_ts` is 0 or missing in Zalo payload, timestamp normalization falls back to `utc_now()` instead of epoch 1970, preserving sweep window correctness.
4. When `msg_id` is `"0"` in receipt payload, `real_msg_id` is selected so message resolution succeeds.
5. Structurally or semantically invalid receipts (empty `{}` or missing `msgId`/`realMsgId` or missing `threadId`) are rejected before persistence and never trigger an unanchored conversation sweep.
6. Listener warning logs are sanitized to prevent Zalo UID / identifier leakage.
7. Outbox execution includes precise duration telemetry distinguishing provider HTTP ACK latency from local scheduling.
8. All Rust files pass `cargo fmt --check` and full test suites.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- Hidden fallback is forbidden. Prefer a visible failure over a fallback that hides a broken primary path.
- Tests must prove product behavior.

## Problem

Supervisor review rejected previous implementation (`REJECT — chưa đạt chuẩn production`):
1. **Unanchored sweep vulnerability**: Receipts with missing/zero `msgId` and `realMsgId` but valid thread ID deserialized successfully and could trigger fallback sweeping of all outbound messages in that conversation.
2. **Missing adversarial & negative test coverage**: Direct-entry tests for `{}`, wrong types, missing IDs/threads, and mixed valid/invalid batches with DB non-mutation assertions were missing.
3. **Identifier leakage in logs**: Listener warning branches logged raw receipt values containing Zalo peer UIDs.
4. **Formatter gate failure**: `cargo fmt --check` failed in `normalizer.rs` and `normalizer_test.rs`.
5. **Ledger discrepancies**: Inconsistent evidence IDs, test counts, and premature readiness declaration.

## Scope

- In scope:
  - `src-tauri/vendor/zca-rs/src/models/event.rs`
  - `src-tauri/vendor/zca-rs/src/listener.rs`
  - `src-tauri/src/conversation/zalo/normalizer.rs`
  - `src-tauri/src/conversation/zalo/normalizer_test.rs`
  - `src-tauri/src/conversation/repository.rs`
  - `src-tauri/src/conversation/repository_test.rs`
  - `src-tauri/src/conversation/service.rs`
  - `src-tauri/src/conversation/service_test.rs`
  - `src-tauri/src/conversation/zalo/probe_test.rs`
- Out of scope:
  - Frontend UI redesign (preserve Svelte `MessageBubble` status mapping)
  - Non-Zalo channels (WhatsApp, Facebook, Email)

## Requirements

| ID | Exact requirement | Invariant | Positive case | Negative/failure case | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|---|---|
| R-01 | Fallback timestamp on `m_s_ts == 0` | Timestamp must never resolve to 1970 epoch | `provider_time_u64(0)` yields current UTC timestamp | `m_s_ts == 0` does not reject sweep of recent 2026 messages | Rust unit test | P1-A | `PROVEN` |
| R-02 | Extract thread_id and recipient from `seen_uids` when `delivered_uids` is empty | Thread ID must never be empty if `seen_uids` has valid UID | `seen=1, deliveredUids: [], seenUids: ["u123"]` extracts threadId `"u123"` | Empty string is not passed to conversation lookup | Rust unit test | P1-B | `PROVEN` |
| R-03 | Skip `"0"` message IDs in favor of `real_msg_id` | Effective ID must be the non-zero snowflake | `msg_id: "0", real_msg_id: "12345"` resolves to `"12345"` | `"0"` does not cause lookup failure | Rust unit test | P1-C | `PROVEN` |
| R-04 | End-to-end status advancement for Zalo delivered & seen | Status advances monotonically `sent -> delivered -> read` | Outbound messages update to `delivered` on delivery event and `read` on seen event | Status does not get stuck at `sent` | Backend integration test | P1-D | `PROVEN` |
| R-05 | Semantic validation & unanchored sweep defense | Empty/zero message IDs or empty thread IDs must NEVER mutate message status or message_receipts | Valid receipt advances status; receipt with `msg_id: ""`, `msg_id: "0"`, or missing thread ID is dropped | Malformed or unanchored receipt cannot sweep conversation | Rust unit & adversarial DB tests | P2-A, P2-B, P2-D | `PROVEN` |
| R-06 | Clean logs & Outbox telemetry | No raw UIDs in logs; outbox send duration measured | Logs indicate skipped malformed receipt without PII; duration measured for Zalo HTTP roundtrip | Raw payload is never logged in warning paths | Code inspection & unit test | P2-A, P2-C | `PROVEN` |
| R-07 | Formatting & quality gates | All touched files adhere to rustfmt and repo test suites | `cargo fmt --check` exits 0; all test suites green | Formatting diffs caught before commit | CI command execution | P2-E | `PROVEN` |

## Acceptance Criteria

1. `cargo test -p zca-rs` passes.
2. `cargo test --package rustsale-crm --lib conversation::zalo` passes.
3. `cargo test --package rustsale-crm --lib conversation` passes.
4. `bun test` passes.
5. `cargo fmt --check` exits 0.
6. Receipts with empty/zero message ID or missing thread ID are rejected and cause 0 DB changes.
7. Mixed batches apply valid items while rejecting invalid ones without dropping the batch.
8. End-to-end listener-to-service-to-repository test verifies delivered and read status transitions.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source-of-truth files, classify each surface, record blocked or missing pieces, and update later phase status assumptions, next actions, and work steps from evidence.
  - Implementation Gate: no implementation or editing starts until `actual-status.md` has a final P0 decision.
  - Acceptance: actual status identifies correct, partial, missing/unbound, fake-or-stub, and blocked surfaces for this scope.

### P1: Initial Zalo Normalizer & Timestamp Remediation (Completed)

- [x] P1-A: Fix timestamp zero fallback in `normalizer.rs`.
- [x] P1-B: Fix `thread_id` and recipient resolution when `delivered_uids` is empty.
- [x] P1-C: Fix non-zero `effective_provider_msg_id` selection.
- [x] P1-D: Run initial regression suite.

### P2: Semantic Validation, Adversarial Protection, Telemetry & Gate Closure

- Phase Goal: Enforce strict semantic validity at model/listener/normalizer/repository boundaries, eliminate unanchored sweep risks, sanitize logs, add outbox telemetry, add comprehensive adversarial tests, format code, and pass all quality gates.
- Phase Boundary: `zca-rs` (event, listener), `conversation` (normalizer, repository, service, probe, tests).

- [x] P2-A: Implement `is_valid()` in `zca-rs` models and sanitize listener logging.
  - Goal: Ensure only semantically valid delivery/seen receipts are emitted, and logs contain no raw payload/PII.
  - Scope Boundary: `src-tauri/vendor/zca-rs/src/models/event.rs` and `src-tauri/vendor/zca-rs/src/listener.rs`.
  - Work Steps:
    1. Add `is_valid()` to `TDeliveredMessage`, `TGroupDeliveredMessage`, `TUserSeenMessage`, `TGroupSeenMessage`.
    2. In `listener.rs`, validate each receipt before adding to events vector; log sanitized warning on invalid/failed items.
    3. Add unit tests in `event.rs` validating `is_valid()` on valid, empty, zero-ID, missing-thread, and wrong-type payloads.
  - Implementation Gate: `cargo test -p zca-rs` passes.
  - Acceptance: Malformed/unanchored receipts are filtered out at the listener layer with sanitized logs.

- [x] P2-B: Guard Normalizer and Repository against unanchored sweep.
  - Goal: Prevent `normalizer.rs` from emitting empty/zero receipts and ensure `repository.rs::apply_receipt` immediately rejects empty/zero anchor or missing thread ID.
  - Scope Boundary: `src-tauri/src/conversation/zalo/normalizer.rs` and `src-tauri/src/conversation/repository.rs`.
  - Work Steps:
    1. In `normalizer.rs`, filter out receipts where `effective_provider_msg_id` or `external_thread_id` is empty or `"0"`.
    2. In `repository.rs:apply_receipt`, add explicit check returning `Ok(None)` if `receipt.provider_message_id.trim().is_empty()` or `receipt.provider_message_id == "0"` or `receipt.external_thread_id.trim().is_empty()`.
  - Implementation Gate: unit tests compile and run.
  - Acceptance: Defense-in-depth ensures no unanchored sweep can ever execute from invalid receipts.

- [x] P2-C: Add outbox execution duration telemetry.
  - Goal: Log execution duration for Zalo HTTP calls in `service.rs`.
  - Scope Boundary: `src-tauri/src/conversation/service.rs`.
  - Work Steps:
    1. Instrument `client.send_part` and `client.forward_message` with `Instant::now()` elapsed time logging.
  - Implementation Gate: `cargo test --package rustsale-crm --lib conversation::service` passes.
  - Acceptance: Telemetry logs provide exact duration of Zalo HTTP requests.

- [x] P2-D: Add comprehensive adversarial & integration tests.
  - Goal: Direct tests for `{}`, wrong types, missing IDs/threads, mixed batches, DB non-mutation assertions, and cmd 502/522 end-to-end integration.
  - Scope Boundary: `normalizer_test.rs`, `repository_test.rs`, `service_test.rs`, `probe_test.rs`.
  - Work Steps:
    1. Add tests in `normalizer_test.rs` for malformed/missing-anchor payloads and mixed batches.
    2. Add tests in `repository_test.rs` asserting zero DB message status changes and zero `message_receipts` rows on invalid receipts.
    3. Add integration test in `service_test.rs` / `probe_test.rs` simulating cmd 502 and cmd 522 listener events through service and verifying DB readback for `Delivered` and `Read`.
  - Implementation Gate: all tests pass.
  - Acceptance: 100% test pass rate with adversarial proof.

- [x] P2-E: Formatter, Quality Gates & Ledger Refresh.
  - Goal: Ensure all touched files pass `cargo fmt --check`, backend suites pass, frontend suite passes, and ledgers are updated.
  - Scope Boundary: All touched files, plan ledgers.
  - Work Steps:
    1. Run `cargo fmt --check` (and `cargo fmt` if needed).
    2. Run `cargo test -p zca-rs`.
    3. Run `cargo test --package rustsale-crm --lib conversation::zalo`.
    4. Run `cargo test --package rustsale-crm --lib conversation`.
    5. Run `bun test`.
    6. Update `actual-status.md`, `evidence.md`, `benchmark.md`.
  - Implementation Gate: all quality gates pass.
  - Acceptance: Zero formatting or test errors.
