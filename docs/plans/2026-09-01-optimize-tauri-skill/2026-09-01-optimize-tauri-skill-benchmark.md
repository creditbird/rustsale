# Optimize Tauri Skill Benchmark Ledger

## Metadata

- Date: `2026-09-01`
- Plan: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-plan.md`
- Evidence: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-evidence.md`
- Benchmark: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-benchmark.md`
- Actual status: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-actual-status.md`

## Benchmark Rules

- Record measured skill-package inventory only.
- Do not represent documentation size reduction as application runtime or binary performance improvement.
- Use observed counts after the final relevant edit.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P0 | Main skill length | lines | 579 | 579 | 68 | `<150` | -511 | `E0-P0A-METRIC1` |
| P0 | Main skill size | bytes | 27,125 | 3,832 | 3,832 | informational | -23,293 | `E2-P2B-SKILL1` |
| P0 | Reference files | files | 0 | 0 | 6 | only necessary files | +6 | `E0-P0A-STRUCT1` |
| P0 | Metadata description | characters | 196 | 147 | 147 | `<200` | -49 | `E2-P2B-SKILL1` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P1 | Classified source lines | lines | 0 | 579 | 579 | 579 | +579 | `E1-P1A-AUDIT1` |
| P1 | Primary-source research note | lines | 0 | 85 | 85 | informational | +85 | `E1-P1B-RESEARCH1` |
| P1 | Canonical duplicate groups | groups | at least 4 | 8 | 8 | all classified | +4 identified | `E1-P1A-AUDIT1` |
| P1 | Genuine owner-policy conflicts | decisions | unknown | 5 | 5 | 0 unresolved | resolved | `E1-P1B-CONFLICT1` |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P2 | Main skill length | lines | 579 | 68 | 68 | `<150` | -511 | `E2-P2B-SKILL1` |
| P2 | Oversized reference files | files | 0 | 0 | 0 | 0 | 0 | `E2-P2A-FILES1` |
| P2 | Broken internal links | links | N/A | 0 | 0 | 0 | 0 | `E2-P2B-LINK1` |
| P2 | Invalid fenced snippets | blocks | N/A | 0 of 25 | 0 of 25 | 0 | 0 | `E2-P2C-SYNTAX1` |
| P2 | Broken primary-reference links | links | N/A | 0 of 17 | 0 of 17 | 0 | 0 | `E2-P2C-LINK1` |
| P2 | Duplicate canonical instructions | groups | 8 identified | 0 | 0 | 0 | -8 | `E2-P2A-REMOVE1` |

## Non-Benchmarkable Notes

This plan changes a global Markdown skill package, not the RustSale executable. Startup time, IPC latency, memory, bundle size, and build duration are explicitly not benchmarked or claimed.
