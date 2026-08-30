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
| P1 | `zca-rs` test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL1` (29/29) |
| P1 | `conversation::zalo` test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL2` (80/80) |
| P1 | `conversation` module test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL3` (476/476) |
| P1 | Frontend test suite pass rate | % | 100 | 100 | 100 | 100 | 0 | `E1-P1D-VAL4` (625/625) |

## Non-Benchmarkable Notes

- This remediation corrects event normalization and date parsing semantics without altering database table schema.
