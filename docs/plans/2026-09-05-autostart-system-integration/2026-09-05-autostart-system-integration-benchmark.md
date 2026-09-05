# Autostart System Integration Benchmark Ledger

## Metadata

- Date: `2026-09-05`
- Plan: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-plan.md`
- Evidence: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-evidence.md`
- Benchmark: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-benchmark.md`
- Actual status: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-actual-status.md`

## Benchmark Rules

The benchmark file records measurements and inventory counts.
Benchmark sections follow the plan phases: `B0` (baseline), `B1` (backend), `B2` (frontend/UI), `B3` (validation).

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P0 | Tauri Plugins Registered | count | 6 | 6 | 6 | 7 | 0 | `E0-P0A-FD1` |
| P0 | Capability Permissions | count | 7 | 7 | 7 | 8 | 0 | `E0-P0A-INSPECT2` |
| P0 | Notification Settings Toggles | count | 5 | 5 | 5 | 6 | 0 | `E0-P0A-INSPECT5` |
| P0 | Frontend Type Check Errors | count | 0 | 0 | 0 | 0 | 0 | `bun run check` |
| P0 | Backend Cargo Check Warnings/Errors | count | 0 | 0 | 0 | 0 | 0 | `cargo check` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P1 | Tauri Plugins Registered | count | 6 | 7 | 7 | 7 | +1 | `E1-P1B-BACKEND` |
| P1 | Capability Permissions | count | 7 | 8 | 8 | 8 | +1 | `E1-P1B-BACKEND` |

## B2 - P2 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|---|---|---|---|---|---|---|---|---|
| P2 | Notification Settings Toggles | count | 5 | 6 | 6 | 6 | +1 | `E2-P2B-UI` |
| P2 | i18n Localization Keys Added | count | 0 | 4 | 4 | 4 | +4 | `E2-P2A-SERVICE` |

## Non-Benchmarkable Notes

Autostart system entry points rely on operating-system native registries and launch agents; latency is instantaneous at UI toggle level and non-blocking for background startup execution.
