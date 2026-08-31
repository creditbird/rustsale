# Zalo Message Status Remediation Benchmark Ledger

## Metadata

- Date: `2026-08-30`
- Plan: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-plan.md`
- Evidence: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-evidence.md`
- Benchmark: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-benchmark.md`
- Actual status: `docs/plans/2026-08-30-zalo-message-status-remediation/2026-08-30-zalo-message-status-remediation-actual-status.md`

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P0 | Zalo delivered status success rate | % | 0 | 100 | 100 | 100 | +100 | `E1-P1A-VAL1` |
| P0 | Active chat seen status success rate | % | 0 | 100 | 100 | 100 | +100 | `E1-P1B-VAL1` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P1 | `zca-rs` test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL3` (30/30) |
| P1 | `conversation::zalo` test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL4` (80/80) |
| P1 | `conversation` module test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL5` (476/476) |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P2 | `zca-rs` semantic validation suite | % | 100 | 100 | 100 | 100 | 0 | `E2-P2A-VAL1` (31/31) |
| P2 | `conversation::zalo` suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E2-P2B-VAL1` (82/82) |
| P2 | `conversation` module test pass rate | % | 100 | 100 | 100 | 100 | 0 | `E2-P2D-VAL1` (481/481) |
| P2 | Frontend test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E2-P2E-VAL2` (625/625) |
| P2 | `cargo fmt --check` pass status | exit code | 1 | 0 | 0 | 0 | -1 | `E2-P2E-VAL1` (clean) |

## Non-Benchmarkable Notes

- This remediation hardens receipt deserialization, enforces semantic validity at the edge, protects the repository against unanchored sweeps, adds outbox telemetry, and sanitizes logs.
