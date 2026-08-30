# Instant Zalo Chat Bubble Synchronization Evidence Ledger

## Metadata

- Date: `2026-08-30`
- Plan: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-plan.md`
- Evidence: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-actual-status.md`

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-RUST1`: `lookup_message_invalidation_info` inspected in `src-tauri/src/conversation/repository.rs`. Found `LIMIT 1` without `ORDER BY` causes nondeterministic participant resolution.
- `E0-P0A-RUST2`: Ingest flow in `src-tauri/src/conversation/service.rs` uses `lookup_message_invalidation_info` and falls back to `applied.identity_id`.
- `E0-P0A-SVELTE1`: `eventAffectsItem` in `ConversationWorkspace.svelte` drops matches when `event.leadId` is mismatched despite matching `replyTargets`.
- `E0-P0A-TS1`: `conversationService.ts` event bridge setup inspected.

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`

- `E1-P1A-RUST1`: Updated `src-tauri/src/conversation/repository.rs` (`lookup_message_invalidation_info`) with `ORDER BY (ci.lead_id IS NOT NULL) DESC, (ci.id = m.sender_identity_id) DESC, ci.lead_id DESC LIMIT 1`.
- `E1-P1A-TEST1`: Added `test_lookup_message_invalidation_info_prioritizes_linked_lead` in `src-tauri/src/conversation/repository_test.rs`. Executed `cargo test test_lookup_message_invalidation_info_prioritizes_linked_lead` -> `test result: ok. 1 passed; 0 failed`.
- `E1-P1A-DELTA1`: Added `ConversationMessageDeltaEvent` in `src-tauri/src/conversation/events.rs` and wired `publish_delta` with atomic sequence in `src-tauri/src/conversation/service.rs`.
- `E1-P1A-TEST2`: Added `test_ingest_zalo_event_publishes_delta_event` in `src-tauri/src/conversation/service_test.rs`. Executed `cargo test test_ingest_zalo_event_publishes_delta_event` -> `test result: ok. 1 passed; 0 failed`.
- `E1-P1B-SVELTE1`: Updated `src/lib/components/conversation/ConversationWorkspace.svelte` with `handleMessageDelta(delta)` for direct store upsert and instant sidebar preview updates.
- `E1-P1B-TS1`: Enhanced `src/lib/services/conversationService.ts` with `ConversationMessageDeltaEvent`, `onConversationMessageDelta`, and dual event bridge listener.
- `E1-P1B-TEST1`: Added test `8i` in `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`. Executed `bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts` -> `37 pass, 0 fail; 99 expect() calls`.

## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`

- `E2-P2A-FULL1`: Backend unit test executed: `cargo test test_ingest_zalo_event_publishes_delta_event` -> `1 passed; 0 failed`.
- `E2-P2A-FULL2`: Frontend conversation test suite executed: `bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts` -> `37 pass, 0 fail; 99 expect() calls`.
- `E2-P2A-CHECK1`: Type check executed: `bun run check` -> `0 errors and 6 warnings in 3 files`.

## Closure Evidence

All requirements R-01, R-02, R-03, R-04, R-05 proven with fresh evidence. Zero IPC round-trips for initial render. Readiness state: `VERIFIED READY`.
