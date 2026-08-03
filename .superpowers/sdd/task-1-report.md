# Task 1 Report: Capture Phase 2A Provider Contract

**Date:** 2026-08-02  
**Module:** `conversation::zalo::phase2a_probe`  
**Status:** Completed  

---

## 1. Executive Summary

Task 1 of Zalo Rich Chat Phase 2A has been fully implemented and verified. We established the provider contract probe infrastructure and standardized test fixtures for Phase 2A capabilities: Reactions, Message Forwarding, and Mentions (with UTF-16 code unit alignment).

---

## 2. Artifacts Created & Modified

### Created Fixtures (`src-tauri/src/conversation/zalo/fixtures/phase2a/`)
1. [`reaction_add_heart.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_add_heart.json) - Sanitized Heart reaction add payload.
2. [`reaction_add_haha.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_add_haha.json) - Sanitized Haha reaction add payload.
3. [`reaction_remove_heart.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_remove_heart.json) - Sanitized Heart reaction removal payload.
4. [`reaction_multi_active.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_multi_active.json) - Multi-actor active reactions fixture.
5. [`reaction_old_reactions.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/reaction_old_reactions.json) - Historical reactions payload per message.
6. [`forward_text_echo.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/forward_text_echo.json) - Forwarded text message with provider & client msg IDs.
7. [`forward_content_matrix.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/forward_content_matrix.json) - Matrix of forwarded content types (text, photo, video, file, sticker, link).
8. [`mention_vietnamese_emoji.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/mention_vietnamese_emoji.json) - Vietnamese mention payload with emoji and UTF-16 offset.
9. [`mention_utf16_matrix.json`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/fixtures/phase2a/mention_utf16_matrix.json) - Comprehensive Unicode test matrix (ASCII, Vietnamese diacritics, surrogate pairs).

### Created Source Files
1. [`phase2a_probe.rs`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/phase2a_probe.rs) - Core struct definitions (`ProbeReaction`, `ProbeForwardMessage`, `ProbeMention`, `Phase2AProbeSummary`), `utf16_slice`, `classify_reaction_fixture`, `classify_forward_fixture`, `extract_mentions_fixture`, `is_sanitized_fixture`, `generate_phase2a_probe_summary`.
2. [`phase2a_probe_test.rs`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/phase2a_probe_test.rs) - Comprehensive unit test suite covering all 10 required assertions.

### Modified Files
1. [`mod.rs`](file:///home/arch/Project/test/RustSale_v2/src-tauri/src/conversation/zalo/mod.rs) - Registered `pub mod phase2a_probe;` and `#[cfg(test)] mod phase2a_probe_test;`.

---

## 3. Test Suite Matrix

| # | Test Name | Purpose / Assertion | Status |
|---|---|---|---|
| 1 | `heart_fixture_is_classified_as_add` | Classifies Heart reaction add correctly | PASS |
| 2 | `haha_fixture_is_not_classified_as_remove` | Verifies Haha reaction add is not marked removed | PASS |
| 3 | `remove_fixture_identifies_exact_reaction` | Verifies reaction removal classification | PASS |
| 4 | `multi_active_fixture_preserves_other_reaction` | Preserves multi-actor reactions state | PASS |
| 5 | `old_reactions_fixture_round_trips_actor_state` | Round-trips historical reaction state per message | PASS |
| 6 | `forward_fixture_exposes_provider_and_client_ids` | Extracts provider & client IDs from forward payload | PASS |
| 7 | `forward_matrix_classifies_every_content_type` | Validates all 6 content types in forward matrix | PASS |
| 8 | `mention_fixture_uses_utf16_offsets` | Verifies mention extraction using UTF-16 offsets | PASS |
| 9 | `mention_matrix_covers_required_unicode_cases` | Validates ASCII, Vietnamese, and Emoji surrogate pairs | PASS |
| 10 | `phase2a_probe_fixtures_are_sanitized` | Guarantees no sensitive tokens/cookies/numbers in fixtures | PASS |

---

## 4. Verification

- Ran `gitnexus_impact({target: "probe", repo: "RustSale_v2", direction: "upstream"})`: Risk LOW (0 callers affected).
- Executed `cargo test --manifest-path src-tauri/Cargo.toml phase2a_probe_test`: All 10 tests passed cleanly.
