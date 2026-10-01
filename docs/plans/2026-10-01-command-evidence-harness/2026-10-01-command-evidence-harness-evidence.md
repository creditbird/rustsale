# Agent Command Evidence & Verification Harness Evidence Ledger

## Metadata

- Date: `2026-10-01`
- Plan: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-plan.md`
- Evidence: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-evidence.md`
- Benchmark: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-benchmark.md`
- Actual status: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-actual-status.md`

## Evidence Rules

The evidence file explains why the work is known to be correct.

It contains:
- metadata and companion files;
- evidence rules;
- evidence sections by phase (`E0`, `E1`, `E2`, `E3`, `E4`, `E5`);
- source inspection, codebase facts, and document facts;
- commands run and pass/fail result;
- impact or blast-radius evidence when code/graph behavior changes;
- implementation evidence: files changed and behavior changed;
- validation evidence: build, tests, traces;
- failures encountered and how they were handled;
- detect-changes before commit;
- commit hash and closure evidence.

### Evidence ID Naming

Format:
```text
E<phase>-<item>-<kind><n>
```

- `E0`: P0 baseline and discovery evidence.
- `E1`: P1 Core engine & types evidence.
- `E2`: P2 Verifier runner & independent driver evidence.
- `E3`: P3 Scripts wiring & rule evidence.
- `E4`: P4 Documentation evidence.
- `E5`: P5 Final end-to-end verification and closure evidence.

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-FD1`: `anvien file-detail package.json --repo /home/arch/Project/test/RustSale_v2 --json` confirmed 0 symbols, 0 related files, low risk.
- `E0-P0A-SRC1`: File check confirmed `.agents/rules/command-evidence.md` does not exist in workspace.
- `E0-P0A-SRC2`: File check confirmed `.agents/task.json` does not exist in workspace.
- `E0-P0A-SRC3`: File check confirmed `scripts/agent/` directory does not exist in workspace.
- `E0-P0A-SRC4`: File check confirmed `docs/agent-harness/` directory does not exist in workspace.
- `E0-P0A-SRC5`: Inspection of `AGENTS.md` confirmed manual completion harness is present, but automated runner rules are not yet specified.
- `E0-P0A-BUILD1`: `cargo check --manifest-path src-tauri/Cargo.toml` passed with exit code 0.
- `E0-P0A-BUILD2`: `bun run check` passed with exit code 0 (0 errors, 6 existing warnings).

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`

- `E1-P1A-TS1`: Created `scripts/agent/tsconfig.json` with strict type checking targeting `bun-types`. `bunx tsc -p scripts/agent/tsconfig.json` verified 0 errors.
- `E1-P1A-CORE1`: Implemented `scripts/agent/core.ts` with custom `BASE_CHECKS` for RustSale_v2 (`src-tauri/Cargo.toml` manifest path), `rustsale-agent` state base directory, `SKIP_DIRS` ignoring build/report artifacts, and full receipt evaluation rules.
- `E1-P1B-TEST1`: Created `scripts/agent/core.test.ts`. `bun test scripts/agent/core.test.ts` passed 9/9 tests (33 expect calls) in 1.89s with 0 fails.

## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`

- `E2-P2A-VERIFY1`: Implemented `scripts/agent/verify.ts`. Running without sessionId exited with code 2 and usage message: `Usage: bun --no-env-file run agent:verify <sessionId>`.
- `E2-P2B-RUN1`: Implemented `scripts/agent/run.ts` with headless `agy` CLI integration, envelope parsing, and verification loop.
- `E2-P2B-TEST1`: Created `scripts/agent/run.test.ts`. `bun test scripts/agent` executed all 12 tests (40 expect calls) across `core.test.ts` and `run.test.ts` in 977ms with 100% pass and 0 fail.

## E3 - P3 Evidence

Matching plan item(s): `P3-A`, `P3-B`

- `E3-P3A-PKG1`: Added `agent:run`, `agent:verify`, `agent:eval`, and `agent:typecheck` to `package.json`. Ran `bun run agent:eval` (12 tests pass) and `bun run agent:typecheck` (0 errors).
- `E3-P3B-RULE1`: Created `.agents/rules/command-evidence.md` with trigger `always_on` and workspace `/home/arch/Project/test/RustSale_v2`.
- `E3-P3B-TASK1`: Created `.agents/task.json` with sessionId matching current conversation (`aab52c95-7119-437d-b47f-c7624793f411`) and R1 acceptance checks.

## E4 - P4 Evidence

Matching plan item(s): `P4-A`, `P4-B`

- `E4-P4A-DOC1`: Created `docs/agent-harness/README.md` documenting in-session `agent:verify` usage, independent driver `agent:run`, task contract schema, and base gates.
- `E4-P4B-DOC2`: Updated `AGENTS.md` with the `AUTOMATED COMMAND EVIDENCE & VERIFICATION HARNESS` rule callout.

## E5 - P5 Evidence

Matching plan item(s): `P5-A`

- `E5-P5A-FINAL1`: Executed `bun --no-env-file run agent:verify aab52c95-7119-437d-b47f-c7624793f411`.
  - rust-clippy: exit 0, PASS
  - rust-tests: exit 0, PASS (905 tests ok, 1 pre-existing ignored)
  - frontend-check: exit 0, PASS
  - frontend-build: exit 0, PASS
  - harness-types: exit 0, PASS
  - harness-evals: exit 0, PASS (12 tests ok)
  - harness-acceptance: exit 0, PASS (12 tests ok)
  - Overall status: `PASS`
  - Evidence receipt: `/home/arch/.local/state/rustsale-agent/b7890786d3897ab36726/04c1b34623d385651fc5d753/evidence.json`

## Closure Evidence

- `CLOSURE-ANVIEN`: `anvien detect-changes --repo /home/arch/Project/test/RustSale_v2 --scope all` to verify modified symbols and files.
- `CLOSURE-RECEIPT`: Validated receipt with matching before/after snapshot hashes and zero failed checks.

