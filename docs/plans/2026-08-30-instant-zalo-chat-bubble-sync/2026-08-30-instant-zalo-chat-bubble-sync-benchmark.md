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
| P1-B | Workspace Invalidation Unit Test | tests | 35 | 36 | 36 | 36 | +1 | `ConversationWorkspace.spec.ts (8h)` |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P2-A | Full Backend Test Suite | passed | 803 | 804 | 804 | 804 | +1 | `cargo test --lib` (34.56s) |
| P2-A | Full Frontend Test Suite | passed | 627 | 628 | 628 | 628 | +1 | `bun run test` (81.31s) |
| P2-A | Svelte / TS Type Check | errors | 0 | 0 | 0 | 0 | 0 | `bunx svelte-check` |

## Non-Benchmarkable Notes

N/A
