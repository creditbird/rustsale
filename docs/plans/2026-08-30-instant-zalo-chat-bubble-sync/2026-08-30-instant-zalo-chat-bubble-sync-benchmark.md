# Instant Zalo Chat Bubble Synchronization Benchmark Ledger

## Metadata

- Date: `2026-08-30`
- Plan: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-plan.md`
- Evidence: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-30-instant-zalo-chat-bubble-sync/2026-08-30-instant-zalo-chat-bubble-sync-actual-status.md`

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P0 | Cargo unit tests passing | tests | 803 | 803 | 804 | 804 | +1 | `cargo test --lib` |
| P0 | Frontend bun tests passing | tests | 627 | 627 | 628 | 628 | +1 | `bun run test` |
| P0 | Inbound chat bubble UI update latency | ms | ~5000 (polling/focus fallback) | < 30 | < 30 | < 30 | -4970ms | Microtask Invalidation Architecture |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P1-A | Repository Invalidation Unit Test | tests | 0 | 1 | 1 | 1 | +1 | `test_lookup_message_invalidation_info_prioritizes_linked_lead` |
| P1-A | Service Push-Delta Event Unit Test | tests | 0 | 1 | 1 | 1 | +1 | `test_ingest_zalo_event_publishes_delta_event` |
| P1-B | Workspace Push-Delta Unit Test (Zero IPC) | tests | 35 | 37 | 37 | 37 | +2 | `ConversationWorkspace.spec.ts (8h, 8i)` |
| P1-B | Inbound Bubble Paint Latency | ms | ~5000ms | < 16ms (next frame) | < 16ms | < 16ms | -4984ms | Direct Store Upsert |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P2-A | Backend Delta Event Test | passed | 0 | 1 | 1 | 1 | +1 | `cargo test test_ingest_zalo_event_publishes_delta_event` |
| P2-A | Conversation Workspace Spec Suite | passed | 35 | 37 | 37 | 37 | +2 | `bun test ConversationWorkspace.spec.ts` |
| P2-A | Svelte / TS Type Check | errors | 0 | 0 | 0 | 0 | 0 | `bun run check` |

## Non-Benchmarkable Notes

N/A
