# Zalo Message Status Remediation Plan

## Metadata

- Date: `2026-08-30`
- Status: `complete`
- Plan: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-plan.md`
- Evidence: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-evidence.md`
- Benchmark: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-benchmark.md`
- Actual status: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-actual-status.md`

## Goal

Fix Zalo message status normalization and sweep advancement in RustSale so that:
1. When recipient device receives packet while outside conversation (`seen=0`), outbound messages immediately advance to `delivered` (2 gray ticks).
2. When recipient is already active inside conversation or receives combined event (`seen=1` with `delivered_uids` empty and `seen_uids` populated), outbound messages immediately advance to `read` (2 blue ticks) without getting stuck at `sent`.
3. When `m_s_ts` is 0 or missing in Zalo payload, timestamp normalization falls back to `utc_now()` instead of epoch 1970, preserving sweep window correctness.
4. When `msg_id` is `"0"` in receipt payload, `real_msg_id` is selected so message resolution succeeds.
5. All fixes preserve 100% of RustSale's omnichannel database architecture (`messages.status`, `status_rank`, `message_receipts`).

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

During real-world testing of Zalo messaging in RustSale:
1. Outbound messages do not display `delivered` (2 gray ticks) when recipient's device receives the packet.
2. Outbound messages remain stuck at `sent` (1 tick) when the recipient is actively inside the chat window, only flipping to `read` (2 blue ticks) if the recipient exits to inbox and clicks the chat open again.
3. Root cause investigation identified:
   - `provider_time_u64(0)` in `normalizer.rs` converted `0` to `1970-01-01T00:00:00.000Z`, breaking `sweep_outbound_messages_status_tx` cutoff checks.
   - When Zalo coalesces delivery+seen into `seen=1`, `delivered_uids` is empty `[]` and the recipient UID is in `seen_uids`. `UserDeliveredMessage::new` extracted an empty `thread_id = ""`, causing `resolve_message_id` and conversation lookup to fail silently.
   - When `msg_id` is `"0"` and snowflake is in `real_msg_id`, `effective_provider_msg_id` selected `"0"` instead of `real_msg_id`.

## Scope

- In scope:
  - `src-tauri/src/conversation/zalo/normalizer.rs`
  - `src-tauri/src/conversation/zalo/normalizer_test.rs`
  - `src-tauri/vendor/zca-rs/src/models/event.rs`
  - `src-tauri/vendor/zca-rs/src/listener.rs`
  - `src-tauri/src/conversation/repository.rs` (if any matching adjustments needed)
  - Regression unit & integration tests in `src-tauri/src/conversation/`
- Out of scope:
  - Frontend schema redesign (preserve Svelte `MessageBubble` and `MessageStatus`)
  - Changing WhatsApp, Facebook, or Email channels

## Non-Goals

- Do not switch from DB `status_rank` to frontend timestamp derivation.
- Do not modify UI components unless visual display of `delivered` / `read` requires bugfix.

## Requirements

| ID | Exact requirement | Invariant | Positive case | Negative/failure case | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|---|---|
| R-01 | Fallback timestamp on `m_s_ts == 0` | Timestamp must never resolve to 1970 epoch | `provider_time_u64(0)` yields current UTC timestamp | `m_s_ts == 0` does not reject sweep of recent 2026 messages | Rust unit test | P1-A | `PROVEN` |
| R-02 | Extract thread_id and recipient from `seen_uids` when `delivered_uids` is empty | Thread ID must never be empty if `seen_uids` has valid UID | `seen=1, deliveredUids: [], seenUids: ["u123"]` extracts threadId `"u123"` | Empty string is not passed to conversation lookup | Rust unit test | P1-B | `PROVEN` |
| R-03 | Skip `"0"` message IDs in favor of `real_msg_id` | Effective ID must be the non-zero snowflake | `msg_id: "0", real_msg_id: "12345"` resolves to `"12345"` | `"0"` does not cause lookup failure | Rust unit test | P1-C | `PROVEN` |
| R-04 | End-to-end status advancement for Zalo delivered & seen | Status advances monotonically `sent -> delivered -> read` | Outbound messages update to `delivered` on delivery event and `read` on seen event | Status does not get stuck at `sent` | Backend integration test | P1-D | `PROVEN` |

## Acceptance Criteria

1. `cargo test -p zca-rs` passes (29 passed; 0 failed).
2. `cargo test --package rustsale-crm --lib conversation::zalo` passes (80 passed; 0 failed).
3. `cargo test --package rustsale-crm --lib conversation` passes (476 passed; 0 failed).
4. `bun test` passes (625 passed; 0 failed).
5. Delivered receipts (`seen=0`) with `m_s_ts=0` advance message status to `Delivered`.
6. Merged delivered receipts (`seen=1`) with empty `delivered_uids` advance message status to `Read`.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect source-of-truth files, classify each surface, record blocked or missing pieces, and update later phase status assumptions, next actions, and work steps from evidence.
  - Implementation Gate: no implementation or editing starts until `actual-status.md` has a final P0 decision.
  - Acceptance: actual status identifies correct, partial, missing/unbound, fake-or-stub, and blocked surfaces for this scope.

### P1: Zalo Normalizer & Timestamp Remediation

- Phase Goal: Fix timestamp zero fallback, thread ID extraction from `seen_uids`, and `msg_id == "0"` resolution.
- Phase Boundary:
  - In scope: `normalizer.rs`, `zca-rs/src/models/event.rs`, `repository.rs`, tests.
  - Out of scope: Frontend modifications.
- Ordered Slice List:
  - P1-A: Fix timestamp zero fallback in `normalizer.rs`.
  - P1-B: Fix `thread_id` and recipient resolution when `delivered_uids` is empty in `zca-rs` & `normalizer.rs`.
  - P1-C: Fix non-zero `effective_provider_msg_id` selection in `normalizer.rs` and `resolve_message_id`.
  - P1-D: Add comprehensive regression tests and verify full build.

- [x] P1-A: Fix timestamp zero fallback in `normalizer.rs`
  - Goal: Ensure `provider_time_u64(0)` returns current UTC time instead of 1970 epoch.
  - Scope Boundary: `src-tauri/src/conversation/zalo/normalizer.rs`
  - Work Steps:
    1. Update `provider_time_u64` to return `crate::conversation::types::utc_now()` when `millis == 0`.
    2. Add unit test asserting `provider_time_u64(0)` does not contain `"1970"`.
  - Implementation Gate: unit test passes.
  - Acceptance: `m_s_ts: 0` produces valid current timestamp.

- [x] P1-B: Fix `thread_id` and recipient resolution when `delivered_uids` is empty
  - Goal: Ensure `UserDeliveredMessage` sets `thread_id` from `seen_uids` if `delivered_uids` is empty.
  - Scope Boundary: `src-tauri/vendor/zca-rs/src/models/event.rs` and `src-tauri/src/conversation/zalo/normalizer.rs`.
  - Work Steps:
    1. In `UserDeliveredMessage::new`, fall back to `data.seen_uids.first()` when `delivered_uids` is empty.
    2. In `normalize_delivered_receipt`, ensure `thread_id` uses non-empty fallback from `seen_uids` or `delivered_uids`.
    3. Add test verifying `seen=1` event with empty `delivered_uids` normalizes with valid `external_thread_id`.
  - Implementation Gate: unit test passes.
  - Acceptance: `external_thread_id` is populated properly.

- [x] P1-C: Fix non-zero `effective_provider_msg_id` selection
  - Goal: Ensure `msg_id == "0"` does not shadow valid `real_msg_id`.
  - Scope Boundary: `src-tauri/src/conversation/zalo/normalizer.rs`.
  - Work Steps:
    1. Update `normalize_delivered_receipt` and `SeenMessages` mapping to check `!msg_id.is_empty() && msg_id != "0"`.
    2. Add test verifying `msg_id: "0", real_msg_id: "999888"` normalizes to `provider_message_id: "999888"`.
  - Implementation Gate: unit test passes.
  - Acceptance: `provider_message_id` is accurately populated.

- [x] P1-D: Run comprehensive regression suite & full build
  - Goal: Verify all conversation tests pass and no regression introduced.
  - Scope Boundary: `src-tauri` workspace tests.
  - Work Steps:
    1. Run `cargo test -p zca-rs`.
    2. Run `cargo test --package rustsale-crm --lib conversation`.
    3. Record evidence in `evidence.md`.
  - Implementation Gate: all tests green.
  - Acceptance: 100% test pass rate.
