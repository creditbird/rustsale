# Configure Local Mem0 MCP Plan

## Metadata

- Date: `2026-08-30`
- Status: `in_progress`
- Risk: `Tier 3 — local external integration with persistent and destructive memory tools`
- Plan: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-plan.md`
- Evidence: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-evidence.md`
- Benchmark: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-benchmark.md`
- Actual status: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-actual-status.md`

## Goal

Register the already-installed self-hosted local Mem0 stdio server with Codex, preserve the existing on-disk memory store and user scope, and verify that the expected Mem0 tools can be discovered and exercised without modifying existing memories.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes and measurements in the benchmark ledger.
- Preserve `/home/arch/.local/share/mem0-mcp-server/server.py` and `/home/arch/.local/share/mem0/` unless a later finding explicitly changes scope.
- Do not terminate the active process holding the embedded-Qdrant lock without explicit authority.
- Use an isolated repo-local `.tmp` memory directory for destructive round-trip validation, then remove only that plan-created directory.
- Run Anvien detect-changes before the implementation commit.
- Keep unrelated app code, existing commits, and user work untouched.
- A real-store startup blocked by another active stdio Mem0 instance must remain visible and prevents `VERIFIED READY` until resolved or verified after the holder exits.

## Problem

The local Mem0 implementation exists and exposes the required tool names, but Codex has no `mem0` MCP registration. The persistent Qdrant directory is currently locked by a Mem0 stdio server owned by another active `agy` process, so a second server cannot open the shared store concurrently.

## Scope

- Add a `mem0` stdio entry to the user's Codex MCP configuration using the existing absolute Python and server paths.
- Preserve `MEM0_DIR=/home/arch/.local/share/mem0` and `MEM0_DEFAULT_USER_ID=arch` so Codex targets the existing store and identity.
- Validate the configured command, tool inventory, and add/search/get/delete behavior against an isolated temporary store.
- Record the real-store lock as a visible integration condition and avoid disrupting its current owner.

## Non-Goals

- Do not install or configure Mem0 Cloud.
- Do not introduce `MEM0_API_KEY`.
- Do not modify or migrate the Mem0 wrapper, Qdrant data, SQLite history, or application source.
- Do not redesign the local server into shared HTTP or Qdrant-server architecture.
- Do not kill PID `15304` or any successor lock holder.

## Requirements

| ID | Exact requirement | Invariant | Positive case | Negative/failure case | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|---|---|
| R-01 | Codex registers the existing local self-hosted Mem0 server as `mem0`. | The registration launches only the existing local wrapper and does not point to Mem0 Cloud. | `codex mcp get mem0` resolves the expected absolute command and args. | No duplicate name, hosted URL, or API-key dependency is introduced. | Codex config/list output with secrets absent. | P1-A | `GREEN` |
| R-02 | The registration preserves the existing memory scope. | `MEM0_DIR` remains `/home/arch/.local/share/mem0` and `MEM0_DEFAULT_USER_ID` remains `arch`. | A fresh process receives both expected non-secret values. | Existing store/user is not silently replaced by a new default. | Codex MCP inspection and server launch evidence. | P1-A | `GREEN` |
| R-03 | The MCP contract exposes the required memory operations. | At minimum `add_memory`, `search_memories`, and `get_memories` exist, with update/delete operations unchanged. | MCP initialize and list-tools returns the seven local wrapper tools. | Startup or protocol failures are surfaced, not reported as success. | Direct stdio MCP handshake and inventory. | P1-B | `GREEN` |
| R-04 | Validation proves a memory round trip without contaminating existing data. | Test data is confined to a plan-created `.tmp` store and removed after validation. | Add, search/get, and delete succeed through MCP. | Shared-store lock and dependency failures remain visible; no fallback writes to the real store. | Isolated MCP round trip plus cleanup proof. | P1-B | `GREEN` |
| R-05 | Existing Mem0 consumers remain uninterrupted. | The active lock holder is not terminated or reconfigured by this task. | PID/lock ownership remains unchanged during registration. | No destructive process control is used. | Process and lock inspection before/after. | P1-B | `GREEN` |

## Acceptance Criteria

- `codex mcp get mem0` reports the local stdio launcher, shared data directory, and existing user ID.
- Direct MCP validation discovers the seven expected tools.
- An isolated add/search/get/delete round trip succeeds and plan-created test data is removed.
- The active shared-store lock is either gone and real-store startup succeeds, or is disclosed as an external blocker with no false completion claim.
- Anvien detect-changes confirms no application code or flow was changed.
- Supervisor acceptance and dead-work review pass before closure.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the installed server, Codex registration, persistent-store state, and active lock owner.
  - Work Steps: inspect source-of-truth server/config surfaces, run Context7/documentation checks, refresh/query Anvien, classify each target, and update P1 from evidence.
  - Implementation Gate: no Codex configuration edit starts until the actual-status file records a final P0 decision.
  - Acceptance: local installation is `correct`, Codex registration is `missing`, the real-store concurrent launch is `blocked`, and touch modes are explicit.

### P1: Register and Verify Local Mem0

- Phase Goal: make the existing local Mem0 server available to new Codex sessions without changing existing memory data or disrupting another client.
- Phase Boundary:
  - In scope: Codex MCP registration, direct stdio contract validation, isolated round trip, evidence and cleanup.
  - Out of scope: app code, Mem0 wrapper changes, persistent-data migration, shared-server redesign.
  - Dependencies: existing Python venv/server paths; active embedded-Qdrant holder may block real-store launch.
- Phase Implementation Rule: do not implement P1 directly; complete P1-A, refresh status, then complete P1-B.
- Ordered Slice List:
  - P1-A: Register the existing local stdio server with Codex.
  - P1-B: Validate the MCP contract and isolated memory round trip.

- [x] P1-A: Register the existing local stdio server with Codex.
  - Goal: create one `mem0` Codex MCP entry that targets the user's existing self-hosted wrapper and identity.
  - Scope Boundary:
    - Editable: `/home/arch/.codex/config.toml` via `codex mcp add`; this plan set.
    - Inspect-only: active Mem0 process and Codex MCP catalog.
    - Preserve-only: local server source and persistent memory data.
    - Out of scope: all RustSale application source and runtime behavior.
  - Non-Goals: no cloud URL, API key, package reinstall, server modification, or process termination.
  - Pre-flight Questions:
    - Data source: local Qdrant/SQLite under `/home/arch/.local/share/mem0`.
    - Display permission: N/A; no UI.
    - DB read flow: Codex -> stdio MCP -> Mem0 -> local Qdrant/SQLite.
    - DB write flow: same boundary, but no real-store write during registration.
    - Render location: N/A; CLI tool catalog only.
    - UI behavior flow: N/A; MCP integration.
    - Docker runtime: N/A; existing self-host uses a Python stdio wrapper, not the RustSale app runtime.
    - Playwright target: N/A; no browser-visible behavior.
    - Behavior test: Codex MCP config inspection.
    - Cleanup/quarantine: no persistent test data in this slice.
    - External side effects: one user-level Codex configuration entry.
    - N/A notes: app build/UI/DB paths are not touched by this slice.
  - Work Steps:
    1. Add `mem0` using the absolute venv Python and server paths plus the existing non-secret environment scope.
       - UI flow check: N/A; no UI.
       - DB/data flow check: confirm the config targets the existing directory and user ID without opening the store.
       - Render location check: confirm `codex mcp list/get` shows the server.
       - Mini QA: inspect the Codex MCP catalog after the edit.
       - Evidence target: `E1-P1A-CFG1` and `E1-P1A-CFG2`.
    2. Recheck active PID/lock ownership and verify it was not disrupted.
       - UI flow check: N/A.
       - DB/data flow check: persistent store remains owned by the pre-existing process.
       - Render location check: N/A.
       - Mini QA: process/lock inspection.
       - Evidence target: `E1-P1A-LOCK1`.
  - Implementation Gate:
    - P0 is complete; no repo symbol is edited, so symbol impact is N/A.
    - `codex mcp get mem0` must initially report no registration.
  - Acceptance:
    - Source: Codex config resolves exact local paths.
    - Runtime/UI: server is enabled in the Codex catalog; UI N/A.
    - DB/data: existing store and user scope are preserved.
    - Behavior test: config inspection passes.
    - Cleanup/quarantine: no cleanup needed.
    - Evidence IDs: `E1-P1A-CFG1`, `E1-P1A-CFG2`, `E1-P1A-LOCK1`.
    - Actual-status rows refreshed: Codex MCP registration and active lock holder.
  - Evidence Targets: config diff/list/get and lock-holder continuity.
  - Actual-status Update: `missing -> correct` for Codex registration; lock remains `blocked` if holder persists.
  - Commit Boundary: commit plan/evidence updates after this slice passes.

- [x] P1-B: Validate the MCP contract and isolated memory round trip.
  - Goal: prove the registered command exposes the expected tools and performs persistent operations without touching the real store.
  - Scope Boundary:
    - Editable: repo-local `.tmp/mem0-mcp-validation` during the test; this plan set.
    - Inspect-only: Codex MCP config and current real-store lock.
    - Preserve-only: `/home/arch/.local/share/mem0` and the installed wrapper.
    - Out of scope: app runtime and existing memories.
  - Non-Goals: no concurrent real-store workaround, migration, or lock-holder termination.
  - Pre-flight Questions:
    - Data source: isolated Mem0 directory created only for the validation run.
    - Display permission: N/A.
    - DB read flow: MCP client -> stdio server -> isolated Qdrant/SQLite.
    - DB write flow: add test fact, query it, delete it, then remove the isolated directory.
    - Render location: command evidence.
    - UI behavior flow: N/A.
    - Docker runtime: N/A; stdio MCP boundary is the real runtime.
    - Playwright target: N/A.
    - Behavior test: initialize/list tools and add/search/get/delete.
    - Cleanup/quarantine: delete only `.tmp/mem0-mcp-validation` after the run.
    - External side effects: local model/cache reads and isolated test files.
    - N/A notes: no app UI or product DB.
  - Work Steps:
    1. Start the configured server with isolated `MEM0_DIR`, initialize MCP, and assert the seven expected tool names.
       - UI flow check: N/A.
       - DB/data flow check: all paths resolve under the isolated directory.
       - Render location check: tool inventory is captured in evidence.
       - Mini QA: direct MCP handshake and list-tools.
       - Evidence target: `E1-P1B-MCP1`.
    2. Add a unique fact, search/list it, delete it, verify absence, and remove the isolated store.
       - UI flow check: N/A.
       - DB/data flow check: positive and negative state transitions are asserted.
       - Render location check: tool results captured without secret data.
       - Mini QA: direct round trip at the real stdio boundary.
       - Evidence target: `E1-P1B-ROUND1` and `E1-P1B-CLEAN1`.
  - Implementation Gate:
    - P1-A registration passes and the validation path resolves inside this repository.
    - The real-store directory remains preserve-only.
  - Acceptance:
    - Source: installed wrapper remains unchanged.
    - Runtime/UI: MCP initialize and tools/list pass; UI N/A.
    - DB/data: isolated round trip and negative post-delete check pass.
    - Behavior test: superficial-patch killer proves actual MCP operations, not config text only.
    - Cleanup/quarantine: isolated directory removed.
    - Evidence IDs: `E1-P1B-MCP1`, `E1-P1B-ROUND1`, `E1-P1B-CLEAN1`.
    - Actual-status rows refreshed: MCP contract and isolated validation.
  - Evidence Targets: direct tool inventory, round trip, failure visibility, cleanup.
  - Actual-status Update: contract `unbound -> bound-correct`; real-store concurrency stays blocked until lock-free verification.
  - Commit Boundary: commit plan/evidence updates after this slice passes.

- [ ] P2-A: Call supervisor for the implemented-plan acceptance loop.
  - Goal: independently verify configuration, evidence, scope, and readiness state.
  - Work Steps:
    1. Apply the supervisor skill to the full plan and current reality.
    2. Repair only rejected scope, rerun affected evidence, and repeat until pass or blocker.
  - Implementation Gate: P1-A and P1-B are complete or explicitly blocked.
  - Acceptance: supervisor passes or records an exact blocker.

- [ ] P2-B: Remove dead work created during this plan.
  - Goal: retain only the accepted config and durable plan artifacts.
  - Work Steps:
    1. Remove plan-created `.tmp` data and obsolete attempts.
    2. Verify existing memories and server files were not changed.
    3. Run supervisor dead-work review.
  - Implementation Gate: remove only artifacts created by this plan.
  - Acceptance: no validation store or rejected config remains; supervisor passes cleanup.

- [ ] P2-C: Close the plan.
  - Goal: record final validation, change detection, commits, and readiness.
  - Work Steps:
    1. Run the nearest real MCP boundary checks and the repo's required full build if applicable to this non-app configuration slice.
    2. Run Anvien detect-changes and `git diff --check`.
    3. Record final evidence, readiness, and commit hashes; verify worktree state.
  - Implementation Gate: P2-A and P2-B pass or record blockers.
  - Acceptance: evidence is fresh, commits exist, worktree is known, and readiness wording matches the Completion Lock.

## Risk Notes

- The embedded Qdrant store supports one process; concurrent stdio clients contend on `/home/arch/.local/share/mem0/qdrant/.lock`.
- The local wrapper exposes destructive delete tools. Validation must use an isolated store.
- A future shared HTTP server or Qdrant-server migration could solve concurrency, but that is a separate architectural task requiring explicit scope and data-migration evidence.
