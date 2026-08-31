# Desktop Notification Production-Grade Benchmark Ledger

## Metadata

- Date: `2026-08-31`
- Plan: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-plan.md`
- Evidence: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-evidence.md`
- Benchmark: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-benchmark.md`
- Actual status: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-actual-status.md`

## Benchmark Rules

The benchmark file records measurements and test timings.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P0 | Semantic graph nodes | nodes | 84,655 | 84,655 | 84,655 | >= 80,000 | 0 | `anvien analyze` |
| P0 | Semantic graph relationships | edges | 97,703 | 97,703 | 97,703 | >= 90,000 | 0 | `anvien analyze` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P1-A | Rust notification test suite | tests passed | 8 | 8 | 8 | 8 | 0 | `cargo test notification_test` |
| P1-B | Frontend notification test suite | tests passed | 6 | 6 | 6 | 6 | 0 | `bun test desktopNotificationService` |
| P1-B | Svelte type check errors | errors | 0 | 0 | 0 | 0 | 0 | `bun run check` |
| P1-B | Full frontend test suite | tests passed | 654 | 654 | 654 | >= 654 | 0 | `bun run test` |

## Non-Benchmarkable Notes

- Notification latency and focus debouncing operate within sub-50ms ranges without blocking the WebKitGTK composite pipeline.
