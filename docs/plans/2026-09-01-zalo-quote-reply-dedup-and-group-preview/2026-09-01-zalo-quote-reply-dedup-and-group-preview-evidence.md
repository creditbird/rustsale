# Zalo Quote Reply Deduplication & Group Message Quote Preview Evidence Ledger

## Metadata

- Date: `2026-09-01`
- Status: `completed`
- Plan: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-plan.md`
- Actual status: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-actual-status.md`
- Benchmark: `docs/plans/2026-09-01-zalo-quote-reply-dedup-and-group-preview/2026-09-01-zalo-quote-reply-dedup-and-group-preview-benchmark.md`

## Problem Evidence

- `E0-P0A-USER1`: User submitted screenshot showing sent reply message duplicated into 2 bubbles (Bubble 1: "Message / Text" with clock icon 🕒; Bubble 2: "Ktv Dung / @Hello World..." with checkmark ✓).
- `E0-P0A-USER2`: User submitted screenshot showing incoming group message from "Nguyễn Hoàng Quốc Huy" replying to "Ktv Dung", but no quote box preview was rendered in the message bubble.
- `E0-P0A-COMPOSER`: In `MessageComposer.svelte`, `handleSend` passed `senderDisplayName: null`, `textContent: null` in optimistic `replyTarget`.
- `E0-P0A-WORKSPACE`: In `ConversationWorkspace.svelte`, `handleBatchQueued` checked `realBatchIds` which only contained server `sendBatchId`, failing to match `item.sendBatch.sendBatchId` (client batch ID).
- `E0-P0A-REPO`: In `repository.rs`, `apply_normalized_message` lacked `reply_to_message_id` in SQL `INSERT INTO messages`.
- `E0-P0A-PROJECTION`: In `projection.rs`, `hydrate_messages` fallback only handled WhatsApp and ignored Zalo `raw_payload_json` quote data.
- `E0-P0A-ZCARS`: In `zca-rs/src/models/message.rs`, `TQuote` lacked serde defaults and coercion for numeric string IDs and optional fields.

## Implementation Evidence

- `E1-P1A-COMPOSER`: `MessageComposer.svelte` updated in `handleSend` and `handleStickerSelect` to copy `currentReplyTarget.preview.senderDisplayName`, `textContent`, `contentType`, `mediaType`, and `occurredAt` into optimistic `replyTarget`. Verified via `MessageComposer.spec.ts` (22/22 passed).
- `E1-P1A-WORKSPACE`: `ConversationWorkspace.svelte` updated `handleBatchQueued` and `handleMessageDelta` to match both `clientBatchId` and `sendBatchId`, and `timelineState.ts` added 30s TTL + content-matching eviction. Verified via `ConversationWorkspace.spec.ts` (test 8k, 59/59 passed).
- `E1-P1B-REPO`: `repository.rs` updated in `apply_normalized_message` to resolve `reply_to_message_id` by querying existing messages matching `reply.provider_message_id` or `reply.provider_client_message_id`, and added `reply_to_message_id` column to both `INSERT INTO messages` and `UPDATE messages`. Verified via `test_reply_to_message_id_is_resolved_and_hydrated_in_group_message` (passed).
- `E1-P1B-PROJECTION`: `projection.rs` updated in `hydrate_messages` to parse Zalo quote from `raw_payload_json` and resolve sender display names from `channel_identities` (or `from_d` / `reply_sender_identity_id`). Verified via `test_zalo_quote_fallback_hydrates_when_original_message_not_in_local_db` (passed).
- `E1-P1B-ZCARS`: `zca-rs/src/models/message.rs` updated `TQuote` with `#[serde(default)]` and `deserialize_string_or_number_u64`, `u32`, `i32` helpers.
- `E1-P2B-GATES`:
  - Cargo test: 825 passed, 0 failed.
  - Bun test: 661 passed, 1 skipped, 0 failed across 70 test files.
  - Svelte check: 0 errors across entire workspace.
  - Bun build: Production build succeeded in 44s.
