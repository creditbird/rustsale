# Zalo Message Status Remediation Actual Status

Title: Zalo Message Status Remediation
Date: 2026-08-30
Status: Verified Ready
Companion plan: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-plan.md`
Companion evidence: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-evidence.md`
Companion benchmark: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-benchmark.md`

## Purpose

This file records the real current state before and after implementation.

## Scope

Target scope:
- `src-tauri/src/conversation/zalo/normalizer.rs`
- `src-tauri/vendor/zca-rs/src/models/event.rs`
- `src-tauri/src/conversation/repository.rs`

Out of scope:
- Frontend Svelte changes
- Non-Zalo channels

## Current Status Matrix

| Unit / File / Surface | Status | Evidence ID | Touch Mode | Plan Decision |
|---|---|---|---|---|
| `provider_time_u64` & `provider_time` in `normalizer.rs` | `correct` | `E1-P1A-VAL1` | Edit | Replaced `0` input parsing to return `utc_now()` instead of 1970 epoch. |
| `UserDeliveredMessage::new` in `zca-rs/src/models/event.rs` | `correct` | `E1-P1B-VAL1` | Edit | Fallback `thread_id` to `seen_uids.first()` when `delivered_uids` is empty. |
| `normalize_delivered_receipt` & `SeenMessages` in `normalizer.rs` | `correct` | `E1-P1C-VAL1` | Edit | Filtered out `"0"` when resolving `effective_provider_msg_id` and ensured non-empty `thread_id`. |
| `resolve_message_id` & `apply_receipt` in `repository.rs` | `correct` | `E1-P1D-VAL1` | Preserve | Correctly matches and sweeps outbound messages to Delivered and Read. |

## Status Refresh Log

- 2026-08-30: Initial P0 actual-status baseline completed from source inspection and real test reproduction.
- 2026-08-30: P1 remediation implemented and verified with 100% test pass rate across `zca-rs` (29 passed), `conversation::zalo` (80 passed), and `conversation` (476 passed).
