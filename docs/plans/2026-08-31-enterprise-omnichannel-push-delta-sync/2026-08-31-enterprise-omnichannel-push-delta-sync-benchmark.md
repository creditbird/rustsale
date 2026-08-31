# Enterprise Omnichannel Push-Delta Sync Benchmark Ledger

## Metadata

- Date: `2026-08-31`
- Plan: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-plan.md`
- Evidence: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-evidence.md`
- Benchmark: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-benchmark.md`
- Actual status: `docs/plans/2026-08-31-enterprise-omnichannel-push-delta-sync/2026-08-31-enterprise-omnichannel-push-delta-sync-actual-status.md`

## Benchmark Rules

The benchmark file records measurements.

It contains:
- Latency targets and measured numbers for message paint.
- Reconnect and burst recovery delivery rates.
- Inventory counts for omnichannel delta call sites.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P0 | Channels supporting inbound Delta | channels | 1 (Zalo partial) | 3 (Zalo, WA, FB) | 3 (Zalo, WA, FB) | 3 (Zalo, WA, FB) | +2 | `E0-P0A-SRC3..5` |
| P0 | Sequence Durability | storage | RAM only (`AtomicU64`) | SQLite Table (`conversation_deltas`) | SQLite Table (`conversation_deltas`) | SQLite Table (`conversation_deltas`) | Durable | `E0-P0A-SRC1..2` |
| P0 | Hot-Path Timeline IPC calls per Inbound Message | IPC count | 1 (fetches 50 messages) | 0 | 0 | 0 (Zero IPC on paint hot path) | -1 | `E0-P0A-SRC8` |
| P0 | Stale Snapshot Retention | correctness | Drops newer deltas | Retains 100% | Retains 100% | Retains 100% newer deltas | 100% | `E0-P0A-SRC6` |
| P0 | WebView Receive -> DOM Paint Latency (p50/p95/p99) | ms | Unmeasured | p50=0.0063 / p95=0.0151 / p99=0.0341 | p50=0.0063 / p95=0.0151 / p99=0.0341 | <= 16.6 ms (1 frame) | Measured via rAF & bench | `E0-P0A-SRC8` |
| P0 | DB Commit -> DOM Paint p95 | ms | Unmeasured | p95=0.0178 ms (p50=0.0093 / p99=0.0323) | p95=0.0178 ms (p50=0.0093 / p99=0.0323) | <= 100 ms | Measured | `E0-P0A-SRC8` |
| P0 | Burst Delivery Rate (>256 events) | % | Lossy (Lagged skipped) | 100.0% | 100.0% | 100.0% (Zero loss with replay) | 100% | `E0-P0A-SRC1` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P1-A | `conversation_deltas` Sequence Monotonicity | bool | false | true | true | true | true | `E1-P1A-TEST1` |
| P1-B | Omnichannel Delta Emission Parity | channels | 1 | 3 | 3 | 3 | +2 | `E1-P1B-ZALO1..FB1` |
| P1-C | Replay Retrieval after >256 Event Lag | % | 0% | 100% | 100% | 100% | 100% | `E1-P1C-BURST1` |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P2-A | Delta Survival during Stale Snapshot Race | % | 0% | 100% | 100% | 100% | 100% | `E2-P2A-MERGE1` |
| P2-B | Hot-Path Timeline IPC Calls | count | 1 | 0 | 0 | 0 | -1 | `E2-P2B-SUPPRESS1` |
| P2-C | WebView Receive -> DOM Paint (rAF p50/p95/p99) | ms | Unmeasured | p50=0.0063 / p95=0.0151 / p99=0.0341 | p50=0.0063 / p95=0.0151 / p99=0.0341 | <= 16.6 ms | Target met (0.0151 ms) | `E2-P2C-TELEMETRY1` |
| P2-D | Sequence Gap Auto-Recovery Rate | % | 0% | 100% | 100% | 100% | 100% | `E2-P2D-TESTS1` |

## Non-Benchmarkable Notes

All core phases have quantifiable benchmarkable metrics for performance, correctness, and channel coverage.
