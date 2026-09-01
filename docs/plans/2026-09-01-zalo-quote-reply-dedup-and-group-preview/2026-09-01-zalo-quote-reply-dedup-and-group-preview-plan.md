# Zalo Quote Reply Deduplication & Group Message Quote Preview Plan

## Metadata

- Date: `2026-09-01`
- Status: `completed`
- Plan: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-plan.md`
- Evidence: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-evidence.md`
- Benchmark: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-benchmark.md`
- Actual status: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-actual-status.md`

## Goal

Fix two critical quote/reply issues in Zalo Direct & Group conversations on RustSale CRM:
1. **Fix Optimistic Reply Message Duplication**:
   - Resolve client-server batch ID reconciliation mismatch in `ConversationWorkspace.svelte` (`realBatchIds` must include `result.clientBatchId` and `result.batchId` to correctly evict negative ID optimistic items).
   - In `MessageComposer.svelte`, inherit full quote preview metadata (`senderDisplayName`, `textContent`, `mediaType`, `contentType`) in the optimistic message `replyTarget` so that the temporary bubble immediately shows the real author and quoted content instead of `"Message" / "Text"`.
   - In `timelineState.ts`, add TTL / reconciliation protection to prevent orphan optimistic messages (`id < 0`) from persisting forever in memory cache.
2. **Fix Inbound Group & Direct Message Quote Preview**:
   - In `src-tauri/src/conversation/repository.rs` (`apply_normalized_message`), resolve `reply_to_message_id` by matching quoted message ID/client ID in local SQLite DB, and include `reply_to_message_id` in both the `INSERT INTO messages` and `UPDATE messages` statements (previously omitted).
   - In `src-tauri/src/conversation/projection.rs` (`hydrate_messages`), add fallback quote parser for Zalo payloads in `raw_payload_json` (extracting quoted sender, text, media type, and resolving display names from `channel_identities`) so quotes whose source message is not stored locally still render full quote preview cards.
   - In `src-tauri/vendor/zca-rs/src/models/message.rs`, ensure `TQuote` uses robust serde deserializers and defaults so that varying Zalo quote formats never fail message parsing.
3. **Verify with Comprehensive Automated Test Suites**:
   - Frontend Vitest tests in `ConversationWorkspace.spec.ts`, `MessageComposer.spec.ts`, `MessageReplyPreview.spec.ts`, and `timelineState.spec.ts`.
   - Backend Rust unit tests in `repository_test.rs`, `normalizer_test.rs`, and `projection.rs` tests.

## Requirements Ledger

| ID | Exact requirement | Invariant | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|
| R-01 | Reconcile optimistic reply messages upon IPC batch return | `cleanedItems` in `handleBatchQueued` must evict `id < 0` messages matching `clientBatchId` or `sendBatchId` without duplication | Vitest test in `ConversationWorkspace.spec.ts` (test 8k) | P1-A | `PASSED` |
| R-02 | Optimistic message retains quoted author and text preview | `replyTarget` in `optimisticMessages` must copy `preview.senderDisplayName` and `preview.textContent` | Vitest test in `MessageComposer.spec.ts` | P1-A | `PASSED` |
| R-03 | Inbound Zalo message persists and links `reply_to_message_id` | `apply_normalized_message` must resolve and write `reply_to_message_id` to SQLite | Cargo test in `repository_test.rs` (`test_reply_to_message_id_is_resolved_and_hydrated_in_group_message`) | P1-B | `PASSED` |
| R-04 | Inbound Zalo message hydrates `reply_target` even without local message row | `hydrate_messages` must extract Zalo quote fallback from `raw_payload_json` | Cargo test in `repository_test.rs` (`test_zalo_quote_fallback_hydrates_when_original_message_not_in_local_db`) | P1-B | `PASSED` |
| R-05 | `TQuote` in `zca-rs` resiliently parses all Zalo quote formats | Strings/numbers for IDs and optional/null fields do not fail deserialization | Cargo test in `repository_test.rs` & `normalizer.rs` | P1-B | `PASSED` |
| R-06 | Full Quality Gates (cargo test + bun test + svelte-check + build) pass cleanly | 0 errors across entire workspace | Full CI command run logs | P2-A / P2-B | `PASSED` |

## Checklist

- [x] P0-A: Complete actual status analysis before implementation work.
- [x] P1-A: Implement Frontend Optimistic Quote Preview & Batch Reconcile Fix in `MessageComposer.svelte`, `ConversationWorkspace.svelte`, and `timelineState.ts`.
- [x] P1-B: Implement Backend Quote Ingestion, `reply_to_message_id` Resolution & Hydration Fallback in `repository.rs`, `projection.rs`, and `zca-rs`.
- [x] P2-A: Add and update automated tests in frontend (`ConversationWorkspace.spec.ts`, `MessageComposer.spec.ts`) and backend (`repository_test.rs`).
- [x] P2-B: Execute Full Quality Gates (Cargo test + Bun test + svelte-check + build) and Record Evidence.
