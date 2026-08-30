# Instant Zalo Outbound Send Benchmark Ledger

## Metadata

- Date: `2026-08-31`
- Plan: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-plan.md`
- Evidence: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-evidence.md`
- Benchmark: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-benchmark.md`
- Actual status: `docs/plans/2026-08-31-instant-zalo-outbound-send/2026-08-31-instant-zalo-outbound-send-actual-status.md`

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P0 | Outbound message bubble paint latency | ms | 5000-10000ms | < 1ms | < 1ms | < 16ms | -9999ms | Synchronous optimistic store upsert |
| P0 | Composer input clear to bubble paint delay | ms | > 1000ms | 0ms (same tick) | 0ms | 0ms | -1000ms | Synchronous `onBatchQueued` call |
| P0 | Echo status update push latency | ms | 5000-10000ms | < 16ms | < 16ms | < 16ms | -9984ms | WebSocket duplicate echo delta publish |
