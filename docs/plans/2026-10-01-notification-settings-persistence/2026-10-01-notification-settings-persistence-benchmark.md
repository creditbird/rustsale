# Notification and System Tray Settings Persistence Benchmark Ledger

## Metadata

- Date: `2026-10-01`
- Plan: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-plan.md`
- Evidence: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-evidence.md`
- Benchmark: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-benchmark.md`
- Actual status: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-actual-status.md`

## Benchmark Rules

Record quantitative measurements for test suites and graph metrics.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Target | Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| P0 | Anvien graph nodes | count | 97256 | >= 97256 | `E0-P0A-ANVIEN` |
| P0 | Anvien graph relationships | count | 111812 | >= 111812 | `E0-P0A-ANVIEN` |
| P0 | Rust notification unit tests | passed | 13 | >= 13 | `E0-P0A-CARGO` |
| P0 | Rust tray unit tests | passed | 3 | >= 3 | `E0-P0A-CARGO` |
| P0 | Frontend NotificationSettings tests | passed | 5 | >= 5 | `E0-P0A-BUN` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Target | Final | Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| P1 | Rust notification unit tests | passed | 13 | >= 15 | - | - |
| P1 | Rust tray unit tests | passed | 3 | >= 5 | - | - |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Target | Final | Evidence |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| P2 | Full agent:verify status | status | RUNNING | PASS | - | - |
