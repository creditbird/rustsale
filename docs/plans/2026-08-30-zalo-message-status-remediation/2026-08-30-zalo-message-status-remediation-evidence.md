# Zalo Message Status Remediation Evidence Ledger

## Metadata

- Date: `2026-08-30`
- Plan: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-plan.md`
- Evidence: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-evidence.md`
- Benchmark: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-benchmark.md`
- Actual status: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-actual-status.md`

## Evidence Rules

- Record facts, code line references, and commands with their exact outcomes.
- Stable ID format: `E<phase>-<item>-<kind><n>`.

## E0 - P0 Baseline Evidence

- `E0-P0A-SRC1`: Source inspection of `src-tauri/src/conversation/zalo/normalizer.rs:101-106` reveals `Utc.timestamp_millis_opt(0)` yields `1970-01-01T00:00:00.000Z`, breaking sweep comparisons for 2026 messages.
- `E0-P0A-SRC2`: Source inspection of `src-tauri/vendor/zca-rs/src/models/event.rs:583-591` shows `thread_id` only pulled from `data.delivered_uids.first()`. When `seen=1`, `delivered_uids` is empty, causing `thread_id = ""`.
- `E0-P0A-SRC3`: Source inspection of `src-tauri/src/conversation/zalo/normalizer.rs:801-806` shows `if !input.msg_id.is_empty()` takes `"0"` if `msg_id` is `"0"`, ignoring `real_msg_id`.
- `E0-P0A-SRC4`: Source inspection of `src-tauri/src/conversation/repository.rs:2407-2521` confirms monotonic status progression via `status_rank` works correctly when valid thread ID and timestamp are supplied.
- `E0-P0A-SRC5`: Diagnosis of live test reproduction reveals `TDeliveredMessage`, `TGroupDeliveredMessage`, `TUserSeenMessage`, `TGroupSeenMessage` in `zca-rs` lacked `#[serde(default)]` and custom deserializers for numeric UIDs / omitted fields. When Zalo sends `seen=0` delivery events without `seenUids` or `realMsgId`, `serde_json::from_value` failed and `listener.rs` aborted the event batch via `?`.

## E1 - P1 Implementation & Validation Evidence

- `E1-P1A-VAL1`: Unit test in `src-tauri/src/conversation/zalo/normalizer_test.rs` asserting `provider_time_u64(0)` returns current UTC timestamp and does not contain `"1970"`. Passed in test suite.
- `E1-P1B-VAL1`: `UserDeliveredMessage::new` updated to fallback `thread_id` to `data.seen_uids.first()` and `normalize_delivered_receipt` updated to fallback `external_thread_id`. Unit test with `seen=1` and empty `delivered_uids` verified `external_thread_id == "active-client-uid"`.
- `E1-P1C-VAL1`: `normalize_delivered_receipt` and `SeenMessages` updated to check `msg_id != "0"`, falling back to `real_msg_id`. Unit test with `msg_id: "0", real_msg_id: "real-del-35"` verified `provider_message_id == "real-del-35"`.
- `E1-P1D-VAL1`: Added `deserialize_vec_string_or_number`, `deserialize_string_or_number_u64`, `deserialize_string_or_number_i32` and `#[serde(default)]` to `TDeliveredMessage`, `TGroupDeliveredMessage`, `TUserSeenMessage`, `TGroupSeenMessage`. Unit test in `zca-rs` verified minimal `seen=0` delivery payload parses without errors.
- `E1-P1D-VAL2`: In `zca-rs/src/listener.rs`, replaced `?` error escalation in item iteration with graceful logging to prevent single-item failure from dropping valid receipts.
- `E1-P1D-VAL3`: Ran initial `cargo test -p zca-rs`: 30 passed, 0 failed.
- `E1-P1D-VAL4`: Ran initial `cargo test --package rustsale-crm --lib conversation::zalo`: 80 passed, 0 failed.
- `E1-P1D-VAL5`: Ran initial `cargo test --package rustsale-crm --lib conversation`: 476 passed, 0 failed.

## E2 - P2 Semantic Validation, Adversarial Protection, Telemetry & Gate Evidence

- `E2-P2A-VAL1`: Added `is_valid()` method to `TDeliveredMessage`, `TGroupDeliveredMessage`, `TUserSeenMessage`, `TGroupSeenMessage` in `src-tauri/vendor/zca-rs/src/models/event.rs`. Added unit test `receipt_models_semantic_validation_rejects_empty_and_zero_ids` testing empty IDs, zero IDs without `real_msg_id`, and missing thread IDs. Ran `cargo test -p zca-rs`: 31 passed, 0 failed.
- `E2-P2A-VAL2`: Sanitized warning logs in `src-tauri/vendor/zca-rs/src/listener.rs` (cmd 502 & cmd 522). Log lines only output message type or sanitized failure reason, removing raw payloads and peer UIDs from logs.
- `E2-P2B-VAL1`: Updated `normalizer.rs` `normalize_delivered_receipt` to return `Option<NormalizedReceipt>` and reject receipts with empty/zero message ID or empty thread ID. Added adversarial tests `receipt_normalization_rejects_empty_zero_and_missing_thread_ids` and `receipt_normalization_mixed_batch_preserves_valid_and_drops_invalid` in `normalizer_test.rs`. Ran `cargo test --package rustsale-crm --lib conversation::zalo`: 82 passed, 0 failed.
- `E2-P2B-VAL2`: Guarded `apply_receipt` in `src-tauri/src/conversation/repository.rs:2407` with immediate return `Ok(None)` for empty or `"0"` message IDs and empty thread IDs. Added adversarial tests in `repository_test.rs` (`test_apply_receipt_rejects_empty_zero_or_missing_thread_anchor_without_mutating_db` and `test_apply_receipt_mixed_batch_applies_valid_and_leaves_invalid_unapplied`) asserting 0 message updates and 0 `message_receipts` rows.
- `E2-P2C-VAL1`: Added `Instant::now()` and `tracing::info!` telemetry to `send_part` and `forward_message` in `src-tauri/src/conversation/service.rs` to measure and log Zalo HTTP roundtrip duration for each outbox job.
- `E2-P2D-VAL1`: Added end-to-end integration test `zalo_listener_to_service_to_repository_receipt_flow_and_invalid_quarantine` in `src-tauri/src/conversation/service_test.rs` asserting delivery receipt transitions message from `sent` -> `delivered`, seen receipt transitions message from `delivered` -> `read`, and invalid receipts cause 0 state changes. Ran `cargo test --package rustsale-crm --lib conversation`: 481 passed, 0 failed.
- `E2-P2E-VAL1`: Ran `cargo fmt --manifest-path src-tauri/Cargo.toml --check`: exit 0, no formatting errors.
- `E2-P2E-VAL2`: Ran `bun test`: 625 passed, 1 skipped, 0 failed across 70 test files.
- `E2-P2E-VAL3`: Ran `anvien detect-changes --scope all`: all changed symbols and files cataloged cleanly.
