# Zalo Reaction Reliability Remediation Ledger

## Boundaries

- Preserve pre-existing uncommitted changes in `src-tauri/src/conversation/repository.rs` and `src-tauri/src/conversation/repository_test.rs` related to inbox title selection.
- Use `bun` and `bunx`; never use `npm` or `npx`.
- Do not send live Zalo messages or reactions during automated verification.
- Do not commit without explicit user instruction.

## Coverage Matrix

| Requirement | Owner Task | Primary Files | Evidence |
|---|---|---|---|
| REQ-001 | Task 1 | repository, schema | `reaction_before_message_is_staged_then_replayed_by_provider_message_id`; `reaction_with_zero_provider_message_id_replays_by_client_message_id` |
| REQ-002 | Task 1 | repository | `pending_reaction_replay_rolls_back_message_when_cleanup_fails` |
| REQ-003 | Task 1 | repository | `pending_reaction_replay_is_chronological_and_idempotent` |
| REQ-004 | Task 1 | repository, service | `duplicate_message_reconciliation_replays_pending_reactions`; `duplicate_message_replay_emits_reaction_invalidation`; `reaction_checkpoint_is_isolated_from_message_watermark` |
| REQ-005 | Task 3 | supervisor | `reaction_history_pauses_after_five_pages_then_resumes_from_next_cursor`; `reactive_history_requests_exactly_five_pages_and_persists_the_last_cursor`; `empty_reaction_history_page_completes_without_resume`; `failed_history_ingest_does_not_request_the_next_page` |
| REQ-006 | Task 2 | zca-rs event, normalizer | `unknown_reaction_icon_round_trips_raw_provider_value`; `unknown_reaction_icon_preserves_raw_provider_value`; `hydrated_reactions_normalize_slash_colon_provider_aliases` |
| REQ-007 | Task 4 | reaction UI | `MessageReactionSummary.spec.ts`: 5 tests, including unsupported read-only and slash-colon aliases |
| REQ-008 | Task 1 | migration | `reaction_reliability_migration_creates_pending_storage_and_repair_markers`; `reaction_history_recovery_resets_only_zalo_history_scopes`; `reaction_history_recovery_does_not_reset_new_cursor_after_marker` |
| REQ-009 | Task 5 | normalizer | `haha_reaction_type_zero_is_addition`; `empty_reaction_icon_marks_removal_even_with_non_negative_type` |
| REQ-010 | Task 5 | service, reactions | `remove_one_local_reaction_preserves_other_emoji_without_provider_remove`; `local_remove_does_not_require_provider_message_ids`; `multi_emoji_remove_hard_deletes_only_selected_reaction` |
| REQ-011 | Task 5 | service, echo cache | `successful_add_self_echo_is_suppressed_without_second_invalidation`; `provider_failure_clears_expected_echo_so_later_event_is_applied`; `client_self_identity_is_persisted_for_includes_me_projection`; `missing_self_identity_rejects_reaction_before_provider_call`; echo-cache account and alias tests |

## Execution Log

| Time | Command | Result | Scope |
|---|---|---|---|
| 2026-08-09 | Live SQLite query for marked message | FAIL expected 5, actual 0 | Reproduction |
| 2026-08-09 | `cargo test --manifest-path src-tauri/Cargo.toml` | 424 passed, 1 ignored | Final Rust suite |
| 2026-08-09 | `cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml` | 25 passed, 1 doctest passed | Vendored provider crate |
| 2026-08-09 | `bun run check` | 0 errors, 0 warnings | Svelte/TypeScript |
| 2026-08-09 | `bun test --conditions=browser --conditions=svelte` | 343 passed, 1 skipped, 0 failed | Full frontend |
| 2026-08-09 | `bun run build` | Exit 0; chunk-size advisory only | Production frontend build |
| 2026-08-09 | `cargo fmt --all -- --check` | Exit 0 | Rust formatting |
| 2026-08-09 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Exit 0 | App Rust lint |
| 2026-08-09 | Vendor `cargo clippy --all-targets` | Blocked by pre-existing crate-wide denies (`indexing_slicing`, `unwrap_used`, `panic`) outside this remediation | Baseline note |
| 2026-08-09 | First full Rust run | 422 passed, 1 failed; exposed immediate `interval()` tick plus timing-sensitive assertions | Debugging evidence |
| 2026-08-09 | `git diff --check` | Exit 0 | Whitespace gate |
| 2026-08-09 | `gitnexus_detect_changes({scope: "all"})` | 26 source/document files, 302 changed symbols, 70 affected; `CRITICAL` expected for the combined dirty-tree cross-layer conversation/Zalo remediation | Impact review |
| 2026-08-09 | `cargo test --manifest-path src-tauri/Cargo.toml` | 437 passed, 1 ignored | Final Rust suite after desktop mutation fix |
| 2026-08-09 | `cargo test --manifest-path src-tauri/vendor/zca-rs/Cargo.toml` | Exit 0 | Final vendored provider suite |
| 2026-08-09 | `bun run check` | 0 errors, 0 warnings | Final Svelte/TypeScript check |
| 2026-08-09 | `bun test --conditions=browser --conditions=svelte` | 343 passed, 1 skipped, 0 failed | Final frontend suite |
| 2026-08-09 | `bun run build` | Exit 0; chunk-size advisory only | Final production frontend build |
| 2026-08-09 | `cargo check --manifest-path src-tauri/Cargo.toml` | Exit 0 | Final Rust compilation |
| 2026-08-09 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | Exit 0 | Final Rust lint |
| 2026-08-09 | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` and `git diff --check` | Exit 0 | Final formatting and whitespace gates |

## Task State

- Task 1: complete
- Task 2: complete
- Task 3: complete
- Task 4: complete
- Task 5: complete
- Task 6: complete
