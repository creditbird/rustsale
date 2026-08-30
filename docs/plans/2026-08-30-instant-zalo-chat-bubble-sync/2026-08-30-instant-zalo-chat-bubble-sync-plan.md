# Instant Zalo Chat Bubble Synchronization Plan

## Metadata

- Date: `2026-08-30`
- Status: `completed`
- Plan: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-plan.md`
- Evidence: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-actual-status.md`

## Goal

Achieve instantaneous, frame-level chat bubble rendering (< 30ms latency, 0s perceived lag) upon incoming Zalo/WhatsApp/Messenger messages:
1. Fix backend `lookup_message_invalidation_info` in `src-tauri/src/conversation/repository.rs` to prioritize the sender's identity and non-null `lead_id`.
2. Ensure backend supervisor in `service.rs` publishes `ConversationInvalidatedEvent` with accurate `lead_id` and `conversation_id`.
3. Fix frontend `eventAffectsItem` in `ConversationWorkspace.svelte` so that customer conversations match by `replyTargets`, `conversationId`, `leadId`, and `channel` without false-negative drops.
4. Enable automatic connection in `onConversationInvalidated` inside `conversationService.ts`.
5. Verify with automated regression tests, backend unit tests, and full test suites.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Code first, tests updated after behavior is implemented.
- Run full build and test suites before claiming completion.

## Problem

When a new Zalo inbound message arrives:
1. The desktop notification appears immediately (< 10ms) via the backend tokio task.
2. But the chat window takes ~5 seconds to display the new message bubble because:
   - `lookup_message_invalidation_info` returns `lead_id: None` (or wrong participant) due to missing `ORDER BY` before `LIMIT 1`.
   - Frontend `eventAffectsItem` encounters `leadId` mismatch or missing `replyTargets` check and returns `false`, skipping `refreshOpenTimeline()`.
   - The UI only updates after subsequent manual interaction or window focus event.

## Scope

- `src-tauri/src/conversation/repository.rs`
- `src-tauri/src/conversation/service.rs`
- `src-tauri/src/conversation/repository_test.rs`
- `src/lib/components/conversation/ConversationWorkspace.svelte`
- `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`
- `src/lib/services/conversationService.ts`

## Non-Goals

- Modifying DB table schemas or migrations.
- Altering Zalo WebSocket transport protocol in `zca-rs`.

## Requirements

| ID | Exact requirement | Invariant | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|
| R-01 | `lookup_message_invalidation_info` resolves sender `lead_id` and direct participant `lead_id` deterministically | Must order by non-null `lead_id` and sender identity match before `LIMIT 1` | Cargo test in `repository_test.rs` | P1-A | `PROVEN` |
| R-02 | `eventAffectsItem` matches customer items by `replyTargets.conversationId` even if `leadId` is mismatched or null | Must return `true` when `event.conversationId` is in `item.replyTargets` | Bun test in `ConversationWorkspace.spec.ts` | P1-B | `PROVEN` |
| R-03 | `onConversationInvalidated` ensures bridge is connected | Must lazily call `ensureConversationEventBridgeConnected` when listeners subscribe | Bun test in `conversationService.spec.ts` | P1-B | `PROVEN` |

## Acceptance Criteria

1. Backend `lookup_message_invalidation_info` returns `lead_id = Some(114)` for direct messages sent by linked leads even when other participants exist.
2. Inbound messages trigger `refreshOpenTimeline()` instantly (0ms microtask) for the open conversation.
3. All cargo tests (804 passed) and frontend bun tests (628 passed) pass cleanly.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state and identify exact failure paths.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: actual status matrix defines touch mode and evidence IDs.

### P1: Implementation

- [x] P1-A: Remediate Backend Invalidation Resolution in `repository.rs` and `service.rs`.
  - Goal: Ensure `lookup_message_invalidation_info` and `service.rs` always emit accurate `lead_id` in `ConversationInvalidatedEvent`.
  - Work Steps:
    1. Update SQL query in `lookup_message_invalidation_info` with `ORDER BY (ci.lead_id IS NOT NULL) DESC, (ci.id = m.sender_identity_id) DESC, ci.lead_id DESC LIMIT 1`.
    2. Add unit test in `repository_test.rs` verifying lead resolution with multiple participants.
  - Implementation Gate: Cargo compiles cleanly.
  - Acceptance: Cargo test passes (`test_lookup_message_invalidation_info_prioritizes_linked_lead`).

- [x] P1-B: Remediate Frontend Realtime Event Matching & Bridge Connectivity.
  - Goal: Ensure `eventAffectsItem` in `ConversationWorkspace.svelte` never drops open chat updates and bridge connects reliably.
  - Work Steps:
    1. Update `eventAffectsItem` in `ConversationWorkspace.svelte` to match customer items if `event.conversationId` exists in `item.replyTargets`.
    2. Enhance `onConversationInvalidated` in `conversationService.ts` to call `initConversationEventBridge()` / `ensureConversationEventBridgeConnected()`.
    3. Update `ConversationWorkspace.spec.ts` with test `8h` verifying instant event trigger.
  - Implementation Gate: Bun test compiles cleanly.
  - Acceptance: All frontend tests pass (36/36 in ConversationWorkspace.spec.ts, 35/35 in conversationService.spec.ts).

### P2: Verification & Quality Gates

- [x] P2-A: Run Full Quality Gates and Capture Evidence.
  - Goal: Verify full cargo test suite, full bun test suite, and svelte-check.
  - Work Steps:
    1. Run `cargo test --lib` (804 passed).
    2. Run `bun run test` (628 passed).
    3. Run `bunx svelte-check --tsconfig ./tsconfig.json` (0 errors).
    4. Record benchmark and evidence ledgers.
  - Acceptance: 100% pass across all test suites.
