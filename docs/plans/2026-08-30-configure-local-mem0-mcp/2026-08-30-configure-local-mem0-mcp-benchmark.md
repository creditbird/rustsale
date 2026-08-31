# Configure Local Mem0 MCP Benchmark Ledger

## Metadata

- Date: `2026-08-30`
- Plan: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-plan.md`
- Evidence: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-evidence.md`
- Benchmark: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-benchmark.md`
- Actual status: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-actual-status.md`

## Benchmark Rules

- Record measured inventory and capacity facts only.
- Keep validation pass/fail in the evidence ledger.
- Use exact evidence IDs for traceability.

## B0 - P0 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P0 | Installed Mem0 MCP tools declared | tools | 7 | 7 | 7 | >= 3 required tools | 0 | `E0-P0A-TOOL1`, `E1-P1B-MCP1` |
| P0 | Codex `mem0` registrations | servers | 0 | 1 | 1 | 1 | +1 | `E1-P1A-CFG2` |
| P0 | Active real-store lock holders | processes | 1 | 1 | 1 | 0 for a second stdio launch | 0 | `E0-P0A-LOCK1`, `E1-P1B-LOCK1` |
| P0 | Repo app files intended for edit | files | 0 | 0 | 0 | 0 | 0 | `E0-P0A-GRAPH2` |

## B1 - P1 Benchmarks

| Phase | Metric | Unit | Baseline | Latest | Final | Target | Delta | Evidence |
|-------|--------|------|----------|--------|-------|--------|-------|----------|
| P1 | Codex `mem0` registrations | servers | 0 | 1 | 1 | 1 | +1 | `E1-P1A-CFG2` |
| P1 | MCP tools discovered at runtime | tools | 0 | 7 | 7 | 7 | +7 | `E1-P1B-MCP1` |
| P1 | Isolated memory round trips | round trips | 0 | 1 | 1 | 1 | +1 | `E1-P1B-ROUND1` |

## B2 - P2 Benchmarks

No additional performance or capacity benchmark is planned for acceptance; validation and review results belong in the evidence ledger.

## Non-Benchmarkable Notes

Build/test command outcomes, lock errors, supervisor verdicts, and configuration inspection are evidence rather than benchmarks.
