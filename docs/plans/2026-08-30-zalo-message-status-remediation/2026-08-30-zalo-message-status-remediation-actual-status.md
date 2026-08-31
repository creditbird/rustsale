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
| `normalize_delivered_receipt` & `SeenMessages` in `normalizer.rs` | `correct` | `E1-P1C-VAL1`, `E2-P2B-VAL1` | Edit | Filtered out `"0"` when resolving `effective_provider_msg_id`, guarded against empty thread ID, and return `Option<NormalizedReceipt>`. |
| `is_valid()` in `zca-rs/src/models/event.rs` | `correct` | `E2-P2A-VAL1` | Edit | Added semantic validity checks rejecting empty/zero message IDs or missing thread/recipient IDs. |
| `listener.rs` event validation & sanitized logging in `zca-rs` | `correct` | `E2-P2A-VAL2` | Edit | Drops invalid receipt items and logs sanitized warnings without raw payloads or peer UIDs. |
| `apply_receipt` in `repository.rs` | `correct` | `E2-P2B-VAL2` | Edit | Guarded immediately against empty or `"0"` message IDs and empty thread IDs, preventing unanchored sweeps. |
| Outbox send duration telemetry in `service.rs` | `correct` | `E2-P2C-VAL1` | Edit | Added `Instant::now()` and `tracing::info!` to measure exact Zalo HTTP response durations. |
| Listener-to-service-to-repository integration in `service_test.rs` | `correct` | `E2-P2D-VAL1` | Edit | Verified end-to-end status transitions (`sent` -> `delivered` -> `read`) and invalid receipt quarantine. |

## Status Refresh Log

- 2026-08-30: Initial P0 actual-status baseline completed from source inspection and real test reproduction.
- 2026-08-30: P1 remediation implemented and verified.
- 2026-08-30: P2 remediation implemented addressing all supervisor reject findings: semantic validation in `zca-rs`, unanchored sweep guard in `repository.rs`, sanitized logging in `listener.rs`, outbox telemetry in `service.rs`, `cargo fmt --check` clean, 31 passed in `zca-rs`, 82 passed in `conversation::zalo`, 481 passed in `conversation`, and 625 passed in `bun test`.
