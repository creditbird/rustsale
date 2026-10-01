# AGY verification harness execution ledger for RustSale_v2

## Final outcome

**VERIFIED: independent AGY driver and command evidence runner.**
**Native hooks: NOT ENABLED; execution could not be confirmed on AGY 1.2.3.**

Shipped entry points:
- In-session verification: `bun --no-env-file run agent:verify <sessionId>`
- External independent driver: `bun --no-env-file run agent:run "task" [model-id]`
- Evaluation tests: `bun run agent:eval`
- Harness typecheck: `bun run agent:typecheck`

## Requirements and coverage

| ID | Requirement | Implementation / evidence | Result |
|---|---|---|---|
| R1 | Real command outcomes; missing, stale, altered, failed and empty-test evidence cannot pass | `core.ts`, `verify.ts`; process and receipt evals in `core.test.ts` | PASS |
| R2 | Requirements map to executable checks; base gates cannot be replaced | `validateTask`; contract evals in `core.test.ts` | PASS |
| R3 | Enforce verification independently of model prose | `run.ts`; envelope and dotenv evals in `run.test.ts` | PASS |
| R4 | Preserve unrelated configuration, support isolated state | State isolated under `~/.local/state/rustsale-agent/`; `.tmp`, `.temp`, `paraglide`, `cache` skipped in snapshot | PASS |
| R5 | Fresh release checks, instructions, audit and rule sync | `docs/agent-harness/README.md`; `.agents/rules/command-evidence.md`; `AGENTS.md` | PASS |

## Baseline and recovery

- Adapted `BASE_CHECKS` for RustSale_v2 workspace structure (`src-tauri/Cargo.toml` manifest path).
- Observed 1 pre-existing credential-gated test in `src-tauri/src/conversation/zalo/probe_test.rs` (`#[ignore]`). Added `maxAllowedSkipped: 1` parameter to `rust-tests` check in `core.ts` while enforcing 0 skipped on all other checks.
- Handled ephemeral symlinks and generated build artifacts by adding `.tmp`, `.temp`, `scratch`, `.gstack`, `.gitnexus`, `.superpowers`, `paraglide`, `cache`, `__pycache__` to `SKIP_DIRS`.
- `agent:verify` verified on live codebase with exit code 0 and status `PASS`.

## Final command evidence

Evidence path: `/home/arch/.local/state/rustsale-agent/b7890786d3897ab36726/04c1b34623d385651fc5d753/evidence.json`
Status: `PASS`

| Check | Command | Exit | Outcome |
|---|---|---:|---|
| rust-clippy | `cargo clippy --manifest-path src-tauri/Cargo.toml` | 0 | PASS |
| rust-tests | `cargo test --manifest-path src-tauri/Cargo.toml` | 0 | PASS (905 tests passed, 1 pre-existing ignored) |
| frontend-check | `bun run check` | 0 | PASS (0 errors, 6 existing warnings) |
| frontend-build | `bun run build` | 0 | PASS |
| harness-types | `bun run agent:typecheck` | 0 | PASS |
| harness-evals | `bun test scripts/agent` | 0 | PASS (12 tests, 0 skipped) |
| harness-acceptance | `bun test scripts/agent` | 0 | PASS (12 tests, 0 skipped) |
