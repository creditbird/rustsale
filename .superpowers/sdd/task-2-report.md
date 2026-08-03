# Task 2 Report: Extend Vendored Zalo Mutation Contracts (Zalo Rich Chat Phase 2A)

## Overview
Implemented Task 2 of Zalo Rich Chat Phase 2A by extending the vendored `zca-rs` library contracts and API methods for message reactions, single-target message forwarding, and mention range serialization.

---

## Changes Implemented

### 1. Vendored Models (`src-tauri/vendor/zca-rs/src/models/message.rs`)
- **`ReactionType`**: Unified enum for standard and custom reaction emojis (`Standard(Reactions)`, `Custom { r_type, source, icon }`).
- **`ForwardReference`**: Struct capturing original message reference details (`id`, `ts`, `log_src_type`, `fw_lvl`).
- **`ReactionRequest`**: Struct containing `client_id`, `reaction`, `msg_id`, `cli_msg_id`, `thread_id`, and `thread_type`.
- **`ForwardMessageRequest`**: Struct containing `client_id`, `message`, `thread_id`, `thread_type`, `reference`, and `ttl`.
- **`ForwardMessageReceipt`**: Struct containing normalized `msg_id` and `cli_msg_id`, with a static `normalize(&serde_json::Value)` helper method handling numeric, string, and nested JSON response payloads.

### 2. API Methods & Payload Builders (`src-tauri/vendor/zca-rs/src/apis/message.rs`)
- **`set_reaction(&self, req: ReactionRequest)`**: API method on `Zalo` struct dispatches reaction addition/removal payloads to `/api/message/reaction` or `/api/group/reaction`.
- **`forward_message_one(&self, req: ForwardMessageRequest)`**: API method on `Zalo` struct forwards messages to a single user or group target and returns a normalized `ForwardMessageReceipt`.
- **`build_reaction_params(req: &ReactionRequest, imei: &str)`**: Extracted testable function for building reaction mutation JSON, preserving caller-supplied `client_id` and supporting reaction removal (`rType: -1`, `rIcon: ""`).
- **`build_forward_one_params(req: &ForwardMessageRequest, imei: &str)`**: Extracted testable function for single-target message forwarding, formatting `toIds` or `grids` with `totalIds = 1`.

---

## Unit Test Verification

Added 6 dedicated unit tests in `src-tauri/vendor/zca-rs/src/apis/message.rs`:

| Test Name | Result | Verification Objective |
| :--- | :---: | :--- |
| `phase2a_reaction_request_uses_caller_client_id` | **PASS** | Confirms caller-supplied `client_id` is preserved in reaction request payloads. |
| `phase2a_reaction_remove_matches_probe` | **PASS** | Confirms `reaction: None` and `Reactions::None` produce probe-matching removal payload (`rType: -1`, `rIcon: ""`). |
| `phase2a_forward_direct_uses_one_target` | **PASS** | Confirms single-target direct forwarding formats `toIds` array of size 1 and `totalIds: 1`. |
| `phase2a_forward_group_uses_one_target` | **PASS** | Confirms single-target group forwarding formats `grids` array of size 1 and `totalIds: 1`. |
| `phase2a_forward_response_normalizes_ids` | **PASS** | Confirms response normalization converts string, numeric, and nested response fields to normalized String IDs. |
| `phase2a_mentions_serialize_utf16_ranges` | **PASS** | Confirms mention offsets and lengths are properly formatted into JSON string inside `mentionInfo`. |

### Command Verification Outputs

```bash
cargo fmt --manifest-path src-tauri/vendor/zca-rs/Cargo.toml -- --check
# Exit code: 0 (Clean)

cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml phase2a_
# Output: 6 passed; 0 failed; 0 ignored; finished in 0.00s

cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml
# Output: 24 passed; 0 failed; finished in 0.10s
```

---

## Conclusion
Task 2 of Zalo Rich Chat Phase 2A is fully implemented, formatted, and verified with all unit tests passing.
