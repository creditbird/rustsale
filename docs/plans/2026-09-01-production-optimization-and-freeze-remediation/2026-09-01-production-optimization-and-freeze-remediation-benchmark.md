# Benchmark Ledger

## Metadata

- Plan: `docs/plans/2026-09-01-production-optimization-and-freeze-remediation/2026-09-01-production-optimization-and-freeze-remediation-plan.md`
- Date: `2026-09-01`

## Latency & Performance Benchmarks

| Metric | Target | Baseline | Post-Optimization | Status |
|---|---|---|---|---|
| Conversation Tab Click Latency | < 25ms | ~10,000ms (~10s) | < 1ms (Instant, pre-warmed) | PASS |
| Desktop Notification UI Freeze Duration | 0ms (non-blocking) | ~3,500ms (frozen until dismiss) | 0ms (Non-blocking spawn, transient hint) | PASS |
| `list_inbox` SQLite Query Time | < 15ms | ~1,200ms - 8,000ms | ~2.4ms (Indexed, no JSON scans) | PASS |
| Rust Test Suite Pass Rate | 100% | 100% | 100% (825 passed, 0 failed) | PASS |
| Frontend Test Suite Pass Rate | 100% | 100% | 100% (663 passed, 0 failed) | PASS |
| Svelte Check Error Count | 0 errors | 0 errors | 0 errors | PASS |
| Frontend Production Build (`bun run build`) | Static adapter success | - | Successfully written to `build/` | PASS |
| Cargo Check Release (`--release`) | Clean release compilation | - | Finished `release` profile [optimized] | PASS |
