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
- `E1-P1D-VAL3`: Ran `cargo test -p zca-rs`: 30 passed, 0 failed.
- `E1-P1D-VAL4`: Ran `cargo test --package rustsale-crm --lib conversation::zalo`: 80 passed, 0 failed.
- `E1-P1D-VAL5`: Ran `cargo test --package rustsale-crm --lib conversation`: 476 passed, 0 failed.
