# Instant Zalo Outbound Send Evidence Ledger

## Metadata

- Date: `2026-08-31`
- Plan: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-plan.md`
- Evidence: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-evidence.md`
- Benchmark: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-benchmark.md`
- Actual status: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-actual-status.md`

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-COMPOSER1`: `MessageComposer.svelte` cleared input text immediately but delayed `onBatchQueued` until `await sendConversationBatch` resolved.
- `E0-P0A-WORKSPACE1`: `ConversationWorkspace.svelte` `handleBatchQueued` relied on `m.sendBatch?.sendBatchId` which was `None` from `enqueue_outbound_batch`.
- `E0-P0A-REPO1`: `src-tauri/src/conversation/repository.rs` `enqueue_outbound_batch` queried raw rows with `map_conversation_message` without calling `hydrate_messages`.
- `E0-P0A-SERVICE1`: `src-tauri/src/conversation/service.rs` `ingest_zalo_event` skipped delta publication when `applied.duplicate == true`.

## E1 - P1 & P2 Evidence

Matching plan item(s): `P1-A`, `P1-B`, `P2-A`, `P2-B`

- `E1-P1A-COMPOSER`: `MessageComposer.svelte` generates synchronous optimistic `ConversationMessage` (`id: -(Date.now() + idx)`, `status: 'queued'`, `sendBatch`) and fires `onBatchQueued` synchronously in the same tick as input reset.
- `E1-P1A-WORKSPACE`: `ConversationWorkspace.svelte` reconciles optimistic messages in both `handleBatchQueued` and `handleMessageDelta`, updating timeline DOM and sidebar preview simultaneously (<1ms).
- `E1-P1B-REPO`: `src-tauri/src/conversation/repository.rs` calls `crate::conversation::projection::hydrate_messages(&tx, &mut messages)` before committing transaction.
- `E1-P1B-SERVICE`: `src-tauri/src/conversation/service.rs` publishes `ConversationMessageDeltaEvent` in `start_direct_conversation` and on duplicate echo message arrival in `ingest_zalo_event`.
- `E1-P2A-TESTS`:
  - `MessageComposer.spec.ts`: 22/22 tests passed (asserting optimistic call and server reconciliation call).
  - `ConversationWorkspace.spec.ts`: 38/38 tests passed (including test 8j verifying instant optimistic DOM paint).
  - Backend `service_test.rs`: 57/57 tests passed (including `test_enqueue_outbound_batch_returns_hydrated_messages_and_publishes_delta` and `test_ingest_zalo_echo_publishes_delta_event`).
- `E1-P2B-GATES`:
  - `bun run check`: 0 errors.
  - `bun test`: 340/340 tests passed across 39 files.
  - `cargo test conversation::service_test`: 57/57 passed.
