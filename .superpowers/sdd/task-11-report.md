# Task 11 Report: Enqueue Native Forward Operations Atomically

## Summary
Implemented Task 11 of Zalo Rich Chat Phase 2A - Enqueue Native Forward Operations Atomically.

## Created Files
- `src-tauri/src/conversation/forwarding.rs`
- `src-tauri/src/conversation/forwarding_test.rs`

## Modified Files
- `src-tauri/src/conversation/types.rs`
- `src-tauri/src/conversation/repository.rs`
- `src-tauri/src/conversation/repository_test.rs`
- `src-tauri/src/conversation/service.rs`
- `src-tauri/src/conversation/service_test.rs`
- `src-tauri/src/conversation/mod.rs`

## Key Implementation Details
1. **Core Forward Enqueue Logic (`forwarding.rs`)**:
   - Validates target count is between 1 and 10 (`ForwardTargetLimitExceeded`).
   - Validates target conversation IDs are unique (`ForwardTargetLimitExceeded`).
   - Ensures operation is idempotent by `client_operation_id` (returns existing op if already present).
   - Validates source message exists and possesses `provider_message_id` and `provider_client_message_id` (`ForwardSourceMissingProviderIds`).
   - Validates channel capabilities support forwarding (`ForwardNotSupported`).
   - Validates all target conversations belong to the same `channel_account_id` as the source message (`ForwardTargetAccountMismatch`).
   - Atomically inserts:
     - Record in `message_forward_operations`.
     - Deterministic send batch records in `message_send_batches` (`client_batch_id = format!("{}:{}", request.client_operation_id, target_id)`).
     - Target records in `message_forward_targets`.
     - Outbound forward target messages in `messages` (`content_type = 'forward'`, text_content = `'Đã chuyển tiếp một tin nhắn'`).
     - Pending outbox jobs in `message_outbox_jobs`.

2. **Repository & Service Layer Integration**:
   - `ConversationRepository::forward_conversation_message(&self, request: &ForwardConversationMessageRequest)` opens DB transaction and invokes `enqueue_forward_operation`.
   - `ConversationService::forward_conversation_message(&self, request: ForwardConversationMessageRequest)` delegates to repository.

3. **Unit Tests Implemented**:
   - `forward_operation_enqueues_one_batch_job_per_target`
   - `forward_operation_idempotent_by_client_operation_id`
   - `forward_target_limit_exceeded_rejected_at_11_targets`
   - `duplicate_target_conversations_rejected`
   - `cross_account_forward_targets_rejected`
   - `source_missing_provider_ids_rejected`
   - `transaction_rollback_on_any_target_error`
   - `forward_conversation_message_enqueues_batch` (repository test)
   - `forward_conversation_message_service_delegates_to_repository` (service test)

## Verification
- `cargo test --manifest-path src-tauri/Cargo.toml forwarding_test`
- `cargo test --manifest-path src-tauri/Cargo.toml forward_`
