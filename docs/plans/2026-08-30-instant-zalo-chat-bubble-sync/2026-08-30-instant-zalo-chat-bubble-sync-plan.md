# Instant Zalo Chat Bubble Synchronization Plan

## Metadata

- Date: `2026-08-30`
- Status: `completed`
- Plan: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-plan.md`
- Evidence: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-actual-status.md`

## Goal

Achieve instantaneous, frame-level chat bubble rendering (< 16ms paint target, 0s perceived lag) upon incoming Zalo/WhatsApp/Messenger messages:
1. **Push-Delta Event Architecture**: Emit render-ready `ConversationMessageDeltaEvent` (with atomic sequence counter, hydrated `ConversationMessage`, timestamp) directly from backend upon message ingestion/outreach.
2. **Zero-IPC Direct Store Upsert**: When `conversation-message-delta` arrives at WebView, immediately upsert the message into the active timeline and update sidebar preview in-memory without waiting for an IPC round-trip.
3. **Deterministic Query Ordering & Invalidation**: Fix backend `lookup_message_invalidation_info` in `src-tauri/src/conversation/repository.rs` to prioritize the sender's identity and non-null `lead_id`.
4. **App-Root Dual Event Bridge**: Ref-counted singleton bridge managing both `conversation-invalidated` and `conversation-message-delta` listeners with auto-reconnection and retry backoff.
5. **Quality Gates & Evidence**: Automated unit tests for backend delta emission and frontend zero-IPC rendering.

## Requirements Ledger

| ID | Exact requirement | Invariant | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|
| R-01 | `lookup_message_invalidation_info` resolves sender `lead_id` and direct participant `lead_id` deterministically | Must order by non-null `lead_id` and sender identity match before `LIMIT 1` | Cargo test in `repository_test.rs` | P1-A | `PROVEN` |
| R-02 | `ConversationMessageDeltaEvent` carries sequence, metadata, and hydrated message | Must emit sequence counter and fully hydrated `ConversationMessage` | Cargo test `test_ingest_zalo_event_publishes_delta_event` in `service_test.rs` | P1-A | `PROVEN` |
| R-03 | Frontend direct store upsert renders bubble without calling `getContactTimeline` | Must upsert into `currentVariant.items` on message delta event with 0 IPC queries | Bun test `test 8i` in `ConversationWorkspace.spec.ts` | P1-B | `PROVEN` |
| R-04 | `eventAffectsItem` matches customer items by `replyTargets.conversationId` even if `leadId` is mismatched | Must return `true` when `event.conversationId` is in `item.replyTargets` | Bun test `test 8h` in `ConversationWorkspace.spec.ts` | P1-B | `PROVEN` |
| R-05 | `onConversationMessageDelta` and `onConversationInvalidated` maintain ref-counted bridge | Bridge automatically registers listeners and cleans up when ref count reaches 0 | Bun tests in `conversationService.spec.ts` & `ConversationWorkspace.spec.ts` | P1-B | `PROVEN` |

## Acceptance Criteria

1. Backend `lookup_message_invalidation_info` returns `lead_id = Some(114)` for direct messages sent by linked leads even when other participants exist.
2. Inbound messages trigger `publish_delta` carrying render-ready `ConversationMessage` and atomic sequence.
3. Frontend `onConversationMessageDelta` upserts new messages instantly into the active timeline and updates sidebar previews synchronously.
4. All cargo tests (`cargo test test_ingest_zalo_event_publishes_delta_event`), full frontend tests (`bun test`, 37/37 passed), and `bun run check` (0 errors) pass cleanly.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
- [x] P1-A: Remediate Backend Invalidation Resolution and implement Push-Delta Event publisher in `events.rs`, `service.rs`, and `repository.rs`.
- [x] P1-B: Implement Frontend Push-Delta listener, Direct Store Upsert, and Ref-Counted Bridge in `conversationService.ts` and `ConversationWorkspace.svelte`.
- [x] P2-A: Add comprehensive automated tests:
  - Backend: `test_ingest_zalo_event_publishes_delta_event` in `service_test.rs`.
  - Frontend: `test 8i. Direct ConversationMessageDeltaEvent renders new bubble immediately without requiring IPC timeline refetch` in `ConversationWorkspace.spec.ts`.
- [x] P2-B: Run Full Quality Gates and Capture Evidence.
