# Agent Command Evidence & Verification Harness Benchmark Ledger

## Metadata

- Date: `2026-10-01`
- Plan: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-plan.md`
- Evidence: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-evidence.md`
- Benchmark: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-benchmark.md`
- Actual status: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-actual-status.md`

## Benchmark Rules

The benchmark file records measurements.

It contains:
- metadata and companion files;
- benchmark rules;
- benchmark sections such as `B0`, `B1`, `B2`, etc.;
- metric tables with unit, baseline, latest, final, target, and delta;
- inventory count, test execution time, and command verification duration.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P0 | Cargo check time | seconds | 154s | 154s | - | <300s | 0 | `E0-P0A-BUILD1` |
| P0 | Frontend check time | seconds | 12s | 12s | - | <30s | 0 | `E0-P0A-BUILD2` |
| P0 | Number of agent test scripts | count | 0 | 0 | - | >0 | 0 | `E0-P0A-SRC3` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P1 | Unit tests count | count | 0 | 12 | 12 | >=5 | +12 | `E2-P2B-TEST1` |
| P1 | Unit test execution time | ms | - | 907ms | 907ms | <2000ms | - | `E2-P2B-TEST1` |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P2 | Runner syntax check | exit code | - | 2 | 2 | 0/2 | pass | `E2-P2A-VERIFY1` |

## B5 - P5 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P5 | Overall verify run time | seconds | - | 148s | 148s | <300s | - | `E5-P5A-FINAL1` |
| P5 | Base gates passed | count | 0 | 6 | 6 | 6 | +6 | `E5-P5A-FINAL1` |
| P5 | Task gates passed | count | 0 | 1 | 1 | 1 | +1 | `E5-P5A-FINAL1` |

## Non-Benchmarkable Notes

P3, P4, and P5 involve configuration, documentation, and live process orchestration; they do not alter application payload sizes or runtime memory footprints.
