# Instant Zalo Outbound Send & Optimistic Bubble Rendering Plan

## Metadata

- Date: `2026-08-31`
- Status: `completed`
- Plan: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-plan.md`
- Evidence: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-evidence.md`
- Benchmark: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-benchmark.md`
- Actual status: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-actual-status.md`

## Goal

Achieve 0ms perceived latency (< 16ms render target) when composing and sending outbound messages in Zalo/Omnichannel:
1. **Frontend Instant Optimistic Message Bubble**: When the user clicks Send or hits Enter in `MessageComposer.svelte`, immediately construct an optimistic `ConversationMessage` (`status = 'queued'`, `id = -Date.now()`, `sendBatch`) and pass to `onBatchQueued` in the exact same synchronous frame.
2. **Seamless Backend Reconciliation**: When `sendConversationBatch` IPC returns, `handleBatchQueued` seamlessly replaces the temporary negative ID optimistic message with the real server message.
3. **Backend Message Hydration in Enqueue**: Ensure `enqueue_outbound_batch` in `repository.rs` calls `hydrate_messages(&tx, &mut messages)` so that `send_batch`, `attachments`, and `mentions` are fully populated on return.
4. **WebSocket Echo & Duplicate Status Propagation**: In `ingest_zalo_event` (`service.rs`), publish delta / invalidation on message echo updates so that status transitions (`queued` -> `sent`) and official `provider_message_id` updates are pushed to the frontend in real time.
5. **Delta Listener Optimistic Cleanup**: Ensure `handleMessageDelta` in `ConversationWorkspace.svelte` cleans up matching optimistic messages before upserting server deltas.
6. **Direct Conversation Delta**: Emit `ConversationMessageDeltaEvent` in `start_direct_conversation`.
7. **Quality Gates & Automated Tests**: Comprehensive unit tests covering optimistic outbound render and echo reconciliation.

## Requirements Ledger

| ID | Exact requirement | Invariant | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|
| R-01 | `MessageComposer.svelte` renders optimistic bubble immediately upon send | Must call `onBatchQueued` synchronously with negative ID message before `await sendConversationBatch` | Bun test in `MessageComposer.spec.ts` | P1-A | `PASSED` |
| R-02 | `ConversationWorkspace.svelte` reconciles optimistic bubble when server batch arrives | Must replace negative ID message with real message without flicker or duplication | Bun test in `ConversationWorkspace.spec.ts` (test 8j) | P1-A | `PASSED` |
| R-03 | `repository.enqueue_outbound_batch` returns fully hydrated messages | `res.messages` must have non-null `send_batch` | Cargo test in `service_test.rs` (`test_enqueue_outbound_batch_returns_hydrated_messages_and_publishes_delta`) | P1-B | `PASSED` |
| R-04 | `service.ingest_zalo_event` propagates status updates on message echo | Must publish invalidation/delta on duplicate message update | Cargo test in `service_test.rs` (`test_ingest_zalo_echo_publishes_delta_event`) | P1-B | `PASSED` |
| R-05 | Full quality gates pass cleanly | 0 cargo test errors (57/57 passed), 0 bun test errors (340/340 passed), 0 svelte-check errors | Full CI test runs | P2-A | `PASSED` |

## Checklist

- [x] P0-A: Complete actual status before implementation work.
- [x] P1-A: Implement Frontend Instant Optimistic Message Bubble & Cleanup in `MessageComposer.svelte` and `ConversationWorkspace.svelte`.
- [x] P1-B: Implement Backend Message Hydration in `repository.rs` and Echo Status Delta in `service.rs`.
- [x] P2-A: Add automated unit tests in `MessageComposer.spec.ts`, `ConversationWorkspace.spec.ts`, and backend tests.
- [x] P2-B: Run Full Quality Gates and Capture Evidence.
