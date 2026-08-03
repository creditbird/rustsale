# Task 13 Summary Report: Register Commands and Startup Recovery

## Overview
Implemented Task 13 of Zalo Rich Chat Phase 2A - Command Registration, Request Structs, Mention Candidate Querying, and Startup Recovery Sequence.

## Changes Implemented

1. **Phase 2A Command Handlers (`src-tauri/src/conversation/commands.rs`)**:
   - `set_conversation_reaction`: Sets or removes message reaction via session supervisor client.
   - `forward_conversation_message`: Enqueues message forwarding to target conversations.
   - `retry_conversation_forward_target`: Requeues a failed forward target within an operation.
   - `list_conversation_mention_candidates`: Returns mention candidates for `@all` and group participants.
   - `list_conversation_quick_templates`: Lists quick templates with optional search and category filter.
   - `create_conversation_quick_template`: Creates a quick template.
   - `update_conversation_quick_template`: Updates an existing quick template.
   - `delete_conversation_quick_template`: Deletes a quick template.
   - `mark_conversation_quick_template_used`: Increments usage count and updates `last_used_at`.

2. **Registered All 9 Phase 2A Commands in Tauri `invoke_handler!` (`src-tauri/src/lib.rs`)**:
   - Registered all 9 Phase 2A command endpoints in `tauri::generate_handler!`.

3. **Mention Candidate Querying (`repository.rs`, `service.rs`, `commands.rs`)**:
   - Implemented `list_mention_candidates` querying active group participants (`cp.left_at IS NULL`).
   - Supports filtering by prefix / substring on `display_name` or `phone`.
   - Includes `@all` candidate when appropriate for group conversations.

4. **Startup Recovery Sequence (`service.rs`)**:
   - `recover_startup` runs database migrations.
   - Marks running reaction mutations as `DeliveryUnknown`.
   - Marks running outbox jobs and forward target jobs as `DeliveryUnknown` before spawning worker tasks.

5. **Unit Tests Added & Verified (`commands_test.rs`, `service_test.rs`)**:
   - `phase2a_command_registration`: Validated registration and execution of all 9 Phase 2A commands.
   - `mention_candidates_query_matching_and_all`: Validated mention candidate search by phone prefix, substring, and `@all` inclusion.
   - `startup_recovery_sequence`: Validated migrations and marking running reaction mutations and outbox jobs as `DeliveryUnknown`.

## Verification Results
- `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`: **PASS**
- `cargo test --manifest-path src-tauri/Cargo.toml conversation::`: **PASS (189 passed, 0 failed, 1 ignored)**
