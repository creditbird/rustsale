# Agent Command Evidence & Verification Harness Actual Status

Title: Agent Command Evidence & Verification Harness
Date: 2026-10-01
Status: P0 Complete
Companion plan: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-plan.md`
Companion evidence: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-evidence.md`
Companion benchmark: `docs/plans/2026-10-01-command-evidence-harness/2026-10-01-command-evidence-harness-benchmark.md`

## Purpose

This file records the real current state before implementation.

Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

This file does not replace `evidence.md`. It classifies current state from evidence.

Use exact evidence IDs from `evidence.md`, such as `E0-P0A-SRC1`, not broad section IDs such as `E0` or `E1`.

## Freshness / Refresh Rules

This actual-status file is a living current-state record, not a one-time P0 snapshot.

P0 records the baseline before implementation. After implementation begins, keep the Current Status Matrix updated so the next agent can trust it as the latest repo reality.

Update this file:

- after each completed implementation slice;
- before starting the next phase if repo state changed;
- whenever evidence changes a current-state classification;
- whenever the next phase's status assumptions, next action, or work steps need updating because reality differs from the previous status.

When refreshing status:

- update only the rows affected by the completed work or new evidence;
- use explicit transitions such as `missing -> correct`, `partial -> correct`, `fake-or-stub -> removed`, or `unbound -> bound-correct`;
- append a Status Refresh Log row instead of deleting history;
- keep detailed proof in `evidence.md`; store only classifications, evidence IDs, touch mode, and plan consequences here.

## Scope

Target scope:

- Implement an automated Command Evidence & Verification Harness in RustSale_v2 modeled after pb-desk (`scripts/agent/`, `package.json`, `.agents/rules/command-evidence.md`, `.agents/task.json`, and `docs/agent-harness/README.md`).

Out of scope:

- Changing existing CRM business logic, Tauri Rust commands, Svelte UI components, or Playwright E2E suites.
- Enabling untested native hooks if CLI execution is unconfirmed.

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|-----------------------|----------------------|--------------------|----------------------|-------------|
| `package.json` | `E0-P0A-FD1` | 0 | Root configuration file for bun scripts and dependencies | low risk (add agent:* scripts) |
| `.agents/rules/command-evidence.md` | `E0-P0A-SRC1` | 0 | Agent always-on rule file (does not exist yet) | low risk (new file) |
| `.agents/task.json` | `E0-P0A-SRC2` | 0 | Task contract schema (does not exist yet) | low risk (new file) |
| `scripts/agent/` | `E0-P0A-SRC3` | 0 | Directory for core, verify, run, tests (does not exist yet) | low risk (new module) |
| `docs/agent-harness/README.md` | `E0-P0A-SRC4` | 0 | Documentation for harness (does not exist yet) | low risk (new docs) |
| `AGENTS.md` | `E0-P0A-SRC5` | 0 | Master rules file | low risk (reference integration) |

## Status Rules

| Status | Meaning | Allowed next action |
|--------|---------|---------------------|
| `correct` | Already behaves as required. | Preserve. Add evidence or tests only if needed. |
| `partial` | Some required behavior exists, but gaps remain. | Change only the missing parts. Preserve correct parts. |
| `wrong` | Current behavior, source, or contract is incorrect. | Replace with required behavior. Record the exact reason. |
| `missing` | Required behavior, source, or contract does not exist. | Implement the missing piece only. |
| `unbound` | Surface exists but is not wired to the real source, flow, or contract. | Bind to the real source only. Preserve approved surface. |
| `fake-or-stub` | Prototype, demo, mock, fallback, or placeholder data is being used as real behavior. | Remove fake behavior or replace it with an approved truthful state. |
| `blocked` | Source, authority, contract, or required evidence is unclear. | Stop. Do not implement until resolved. |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|------|---------------|----------------|--------|--------------------|----------|--------------------|
| `scripts/agent/core.ts` | Fully implemented engine with snapshot, process runner, and receipt evaluation | Complete validation, snapshot, process execution, and receipt evaluation engine | `correct` | 0 related files | `E1-P1A-CORE1` | preserve |
| `scripts/agent/tsconfig.json` | Strict TypeScript config for scripts/agent | TypeScript config for scripts/agent | `correct` | 0 related files | `E1-P1A-TS1` | preserve |
| `scripts/agent/core.test.ts` | 9 unit tests passing for snapshot, validation, process runner, and receipt evaluation | Unit tests for snapshot, validation, process runner, and receipt evaluation | `correct` | 0 related files | `E1-P1B-TEST1` | preserve |
| `scripts/agent/verify.ts` | Complete CLI runner with file locking, snapshot checking, and log generation | CLI command runner for in-session verification | `correct` | 0 related files | `E2-P2A-VERIFY1` | preserve |
| `scripts/agent/run.ts` | Complete headless AGY driver with feedback loop | Headless AGY independent driver with loop feedback | `correct` | 0 related files | `E2-P2B-RUN1` | preserve |
| `scripts/agent/run.test.ts` | 3 unit tests passing for driver envelope parsing and dotenv isolation | Unit tests for independent driver envelope parsing | `correct` | 0 related files | `E2-P2B-TEST1` | preserve |
| `package.json` | `agent:verify`, `agent:run`, `agent:eval`, `agent:typecheck` scripts configured and tested | Scripts added and callable via bun | `correct` | 0 related files | `E3-P3A-PKG1` | preserve |
| `.agents/rules/command-evidence.md` | Rule exists with trigger always_on pointing to RustSale_v2 | Always-on rule for AGY sessions in RustSale_v2 | `correct` | 0 related files | `E3-P3B-RULE1` | preserve |
| `.agents/task.json` | Valid task contract created with R1 requirement and acceptance checks | Contract schema template with sample checks | `correct` | 0 related files | `E3-P3B-TASK1` | preserve |
| `docs/agent-harness/README.md` | Comprehensive guide and specification in Vietnamese | User guide and harness specification | `correct` | 0 related files | `E4-P4A-DOC1` | preserve |
| `AGENTS.md` | Automated Command Evidence Harness callout added to project rules | Reference to command-evidence rule and agent:verify | `correct` | 0 related files | `E4-P4B-DOC2` | preserve |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---------|------|------------|----------------|----------------|----------|-------------------|
| R0 | 2026-10-01 | Baseline before P0 | Entire agent harness scope | Initial classification: missing/partial | `E0-P0A-FD1`, `E0-P0A-SRC1`..`E0-P0A-SRC5` | Proceed to P1-A |
| R1 | 2026-10-01 | after P1 completion | `scripts/agent/core.ts`, `tsconfig.json`, `core.test.ts` | `missing -> correct` for core engine & tests | `E1-P1A-TS1`, `E1-P1A-CORE1`, `E1-P1B-TEST1` | Proceed to P2-A (verify.ts) |
| R2 | 2026-10-01 | after P2 completion | `scripts/agent/verify.ts`, `run.ts`, `run.test.ts` | `missing -> correct` for runner & driver | `E2-P2A-VERIFY1`, `E2-P2B-RUN1`, `E2-P2B-TEST1` | Proceed to P3-A (package.json) |
| R3 | 2026-10-01 | after P3 completion | `package.json`, `command-evidence.md`, `task.json` | `partial/missing -> correct` for config & rules | `E3-P3A-PKG1`, `E3-P3B-RULE1`, `E3-P3B-TASK1` | Proceed to P4-A (docs) |
| R4 | 2026-10-01 | after P4 completion | `docs/agent-harness/README.md`, `AGENTS.md` | `missing/partial -> correct` for documentation | `E4-P4A-DOC1`, `E4-P4B-DOC2` | Proceed to P5-A (E2E verify) |
| R5 | 2026-10-01 | after P5 completion | Entire workspace verification | All gates passed, exit 0, status PASS | `E5-P5A-FINAL1` | Complete |

## Phase Touch Map

| Unit / File / Surface | Plan-Relevant Relationship File | Relationship to Target | Plan Item | Touch Mode | Evidence | Constraint |
|-----------------------|---------------------------------|------------------------|-----------|------------|----------|------------|
| `scripts/agent/core.ts` | `scripts/agent/verify.ts` | Engine dependency | P1-A | create | `E0-P0A-SRC3` | Must use bun, cargo, and custom rust paths |
| `scripts/agent/tsconfig.json` | `scripts/agent/core.ts` | Typecheck configuration | P1-A | create | `E0-P0A-SRC3` | Strict mode |
| `scripts/agent/core.test.ts` | `scripts/agent/core.ts` | Test suite | P1-B | create | `E0-P0A-SRC3` | All tests pass with bun test |
| `scripts/agent/verify.ts` | `scripts/agent/core.ts` | Consumer of core | P2-A | create | `E0-P0A-SRC3` | Must output JSON result + logs |
| `scripts/agent/run.ts` | `scripts/agent/core.ts` | Independent driver | P2-B | create | `E0-P0A-SRC3` | Must use agy CLI |
| `scripts/agent/run.test.ts` | `scripts/agent/run.ts` | Test suite | P2-B | create | `E0-P0A-SRC3` | All tests pass |
| `package.json` | `scripts/agent/verify.ts` | Script entry points | P3-A | edit | `E0-P0A-FD1` | Preserve existing scripts |
| `.agents/rules/command-evidence.md` | `.agents/task.json` | Rule specifies contract | P3-B | create | `E0-P0A-SRC1` | Correct repo path |
| `.agents/task.json` | `scripts/agent/verify.ts` | Input contract | P3-B | create | `E0-P0A-SRC2` | Valid schema |
| `docs/agent-harness/README.md` | `.agents/rules/command-evidence.md` | Documentation | P4-A | create | `E0-P0A-SRC4` | Clear instructions |
| `AGENTS.md` | `.agents/rules/command-evidence.md` | Instruction integration | P4-B | edit | `E0-P0A-SRC5` | Preserve existing rules |

## Detailed Findings

- Inspection of repo reveals that `.agents/rules/` does not exist; only `.agents/skills/` exists.
- `package.json` currently has `best-of-n` scripts but lacks `agent:verify` and `agent:run`.
- Rust source is in `src-tauri` with its own `Cargo.toml`. Cargo checks must pass `--manifest-path src-tauri/Cargo.toml`.
- Frontend check is `bun run check` and build is `bun run build`.
- State directory should be isolated under `~/.local/state/rustsale-agent/`.
- Snapshot hash must ignore build targets (`target`, `src-tauri/target`, `.svelte-kit`, `dist`, `build`, `.agent-evidence`, `.anvien`, `Reports`).
