# Lead Import Excel CSV Benchmark Ledger

## Metadata

- Date: `2026-09-09`
- Plan: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-plan.md`
- Evidence: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-evidence.md`
- Benchmark: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-benchmark.md`
- Actual status: `docs/plans/2026-09-09-lead-import-excel-csv/2026-09-09-lead-import-excel-csv-actual-status.md`

## Benchmark Rules

The benchmark file records measurements.

It contains:
- metadata and companion files;
- benchmark rules;
- benchmark sections `B0`, `B1`, `B2`, `B3`;
- metric tables with unit, baseline, latest, final, target, and delta;
- inventory counts;
- runtime and performance metrics;
- before/after numbers.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P0 | Supported file formats | count | 0 | 0 | - | 3 (.xlsx, .xls, .csv) | +3 | E0-P0A-SRC1 |
| P0 | Import throughput | rows/sec | 0 | 0 | - | >= 5,000 | +5,000 | E0-P0A-SRC2 |
| P0 | File inspection latency | ms | 0 | 0 | - | < 20ms | < 20ms | E0-P0A-SRC3 |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P1 | Inspection latency (10k rows) | ms | - | - | - | < 20ms | - | To be recorded |
| P1 | In-memory template generation time | ms | - | - | - | < 5ms | - | To be recorded |
| P1 | Batch import throughput | rows/sec | - | - | - | >= 5,000 | - | To be recorded |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P2 | IPC Roundtrip invocation | ms | - | - | - | < 10ms | - | To be recorded |

## B3 - P3 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P3 | UI modal mount time | ms | - | - | - | < 16ms (60fps) | - | To be recorded |
| P3 | Streaming progress UI tick | fps | - | - | - | >= 30fps | - | To be recorded |

## Non-Benchmarkable Notes

- Column mapping accuracy is evaluated via deterministic unit tests with synthetic and real Vietnamese/English column variations.
- SQLite database integrity check is verified via `PRAGMA integrity_check` pass/fail in evidence.
