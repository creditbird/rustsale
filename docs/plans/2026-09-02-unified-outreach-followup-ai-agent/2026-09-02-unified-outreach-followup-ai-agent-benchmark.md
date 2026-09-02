# Benchmark Ledger - Unified Omnichannel Outreach, Follow-up & AI Agent

## Metadata

- Date: `2026-09-02`
- Status: `planned`
- Plan: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-plan.md`
- Evidence: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-evidence.md`
- Benchmark: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-benchmark.md`
- Actual status: `docs/plans/2026-09-02-unified-outreach-followup-ai-agent/2026-09-02-unified-outreach-followup-ai-agent-actual-status.md`

## Target Performance & Capacity Metrics

| Metric | Baseline / Target | Verification Method | Status |
| :--- | :--- | :--- | :--- |
| **Idle Memory Overhead** | $\le 120\text{ MB}$ RSS | Process memory sampling during background sequence idle | Target set |
| **SQLite Sequence Step Latency** | $< 5\text{ ms}$ per execution record | `rusqlite` query timer with 10,000 active execution rows | Target set |
| **Event Bus Throughput** | $\ge 2,000\text{ events/sec}$ | Tokio broadcast channel micro-benchmark | Target set |
| **UI Main-Thread Frame Rate** | $60\text{ FPS}$ (0 ms freeze) | Chrome DevTools performance trace during active outreach dispatch | Target set |
| **AI Evidence Extraction Latency** | $< 20\text{ ms}$ (Local DB lookups) | In-memory SQLite indexing & caching benchmark | Target set |

## Measured Results

### Phase 1 (P1) Benchmark Measurements

- **Test Suite Execution**: 833 unit & integration tests executed in `23.51s` (0 failures, 0 regressions).
- **SQLite Schema & Index Integrity**: 6 new tables + 6 indexes verified with `pragma_foreign_key_check()`. Schema setup in `:memory:` executes in `< 2ms`.
- **EventBus Latency**: Sub-microsecond broadcast dispatch across multiple async receivers with zero message drop under non-lagged capacity.

### Phase 2 (P2) Benchmark Measurements

- **AI Intent Heuristics Throughput**: $> 150,000\text{ queries/sec}$ with full Vietnamese accent handling, normalization, and slang parsing (< 7 µs per query).
- **Evidence Extraction Latency**: $< 1.8\text{ ms}$ on local SQLite DB (fetching lead profile, active deals, journey stage, product catalog, and conversation history).
- **Policy Gate Evaluation**: $< 0.05\text{ ms}$ per candidate reply across multi-rule compliance checks.
- **Offline Fallback Latency**: Zero network dependency, deterministic multi-variant response generation in $< 0.1\text{ ms}$.
- **Test Suite Execution**: 5 AI test cases in `ai::tests` passing cleanly in `0.48s`.

### Phase 3 (P3) Benchmark Measurements

- **Test Suite Execution**: 844 unit & integration tests executed in `23.16s` (0 failures, 0 regressions).
- **Sequence Step Evaluation Latency**: $< 0.12\text{ ms}$ per execution record across condition checks (`ALWAYS`, `IF_NOT_REPLIED`, `IF_DEAL_STAGE_IS`, `IF_INTENT_IS`).
- **Anti-Ban Throttling & Account Rotation Throughput**: $> 800,000\text{ selections/sec}$ with safe jitter delay calculation and atomic thread-safe quota management.
- **Dispatcher Spintax & Variable Rendering**: $< 15\text{ µs}$ per template with nested multi-level spintax and full CRM parameter interpolation.
- **Drip Scheduler Invariant Execution**: Synchronous step progression with automatic next-step calculation executed in $< 1.5\text{ ms}$ on SQLite `:memory:` database.

