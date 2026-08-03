# Task 12 Summary Report - Dispatch Forward Parts and Recalculate Status

## Overview
Implemented Task 12 of Zalo Rich Chat Phase 2A, completing the end-to-end forward message dispatch worker, aggregate status recalculation logic, startup recovery, manual target retry, and echo reconciliation.

## Implemented Work

1. **Forward Outbox Worker (`src-tauri/src/conversation/service.rs`)**:
   - Added support for `ConversationSendPartContent::Forward` / `content_type == "forward"`.
   - Extracted `sourceMessageId` and `clientOperationId` from `job.content_json`.
   - Checked source message's `provider_message_id`.
   - Updated target status in `message_forward_targets` to `Sending` before dispatching.
   - Built `ProviderForwardRequest` and dispatched via `ZaloClient.forward_message`.
   - Updated target status to `Sent` upon success, or `Failed` / `Retryable` on errors.

2. **Zalo Client Forwarding API (`src-tauri/src/conversation/zalo/client.rs`)**:
   - Defined `ProviderForwardRequest` struct.
   - Added `forward_message` method to `ZaloClient` trait and `RealZaloClient` calling `zca_rs::Zalo::forward_message_one`.
   - Updated `FakeZaloClient` in test suites (`client_test.rs` & `service_test.rs`) to capture forward requests.

3. **Status Recalculation & Target Helpers (`src-tauri/src/conversation/forwarding.rs`)**:
   - Implemented `compute_aggregate_operation_status` handling all state combinations:
     - `queued` when all targets are `queued`
     - `sending` when all targets are `sending` (or sending/queued with 0 sent/failed/unknown)
     - `sent` when all targets are `sent`
     - `failed` when all targets are `failed`
     - `delivery_unknown` when all targets are `delivery_unknown`
     - `partial` for mixed states (e.g. sent + failed, sent + queued)
   - Implemented `recalculate_forward_operation_status`, `update_forward_target_status`, and `retry_forward_target`.

4. **Startup Recovery (`src-tauri/src/conversation/repository.rs`)**:
   - Updated `mark_running_jobs_delivery_unknown` to update running/sending forward target jobs to `delivery_unknown` and recalculate forward operation aggregate status upon app launch.

5. **Echo Reconciliation (`src-tauri/src/conversation/repository.rs`)**:
   - Updated `apply_normalized_message` so that inbound echo matching a forwarded message updates target status to `sent` and recalculates operation status without duplicating message records.

6. **Exposed Service Retry Method (`src-tauri/src/conversation/service.rs`)**:
   - Exposed `retry_forward_target` on `ConversationService`.

7. **Unit Test Suite**:
   - Added all 7 required test cases in `service_test.rs`:
     - `forward_worker_calls_one_target_with_stable_client_id`
     - `forward_success_persists_provider_ids`
     - `forward_echo_updates_queued_target_without_duplicate`
     - `successful_target_is_not_requeued_when_sibling_fails`
     - `forward_running_after_restart_becomes_delivery_unknown`
     - `forward_operation_status_rules_cover_mixed_states`
     - `manual_retry_requeues_only_failed_target`

## Verification
- `cargo test --manifest-path src-tauri/Cargo.toml forward_`: **Passed (14/14 tests)**
- `cargo test --manifest-path src-tauri/Cargo.toml forwarding_test`: **Passed (7/7 tests)**
- `bunx gitnexus analyze`: **Indexed successfully**
