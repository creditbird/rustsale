# Configure Local Mem0 MCP Actual Status

Title: Configure Local Mem0 MCP
Date: 2026-08-30
Status: Blocked
Companion plan: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-plan.md`
Companion evidence: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-evidence.md`
Companion benchmark: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-benchmark.md`

## Purpose

This file records the true installation, registration, data-scope, and lock state before configuration changes. Detailed proof remains in the evidence ledger.

## Freshness / Refresh Rules

- Refresh affected rows after each implementation slice.
- Append transitions to the Status Refresh Log.
- Preserve detailed output in evidence and classifications here.
- Do not treat a config entry as proof that the server can access the real store.

## Scope

Target scope:

- Existing local Mem0 wrapper and venv.
- Codex global MCP registration.
- Existing Qdrant/SQLite store identity and active lock.
- Isolated validation store under the repo `.tmp` directory.

Out of scope:

- RustSale source and runtime.
- Mem0 Cloud and API keys.
- Server redesign, data migration, and process termination.

## Relationship / Impact Evidence

The executable/configuration targets are outside the indexed repository, so `file-detail` relationship counts are not applicable. Anvien graph evidence confirms no repo-owned MCP surface is in scope.

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|-----------------------|----------------------|--------------------|----------------------|-------------|
| RustSale app source | `E0-P0A-GRAPH2` | 0 plan-relevant files | No repo MCP integration surface; query results were unrelated configuration/in-memory-test matches. | low; preserve-only |
| `/home/arch/.codex/config.toml` | `E0-P0A-CFG1` | N/A external | Codex global MCP catalog; no current `mem0` entry. | medium external config |
| Installed local server | `E0-P0A-SRC1` | N/A external | Stdio MCP owner for the local store. | high persistent-tool boundary |
| Local persistent store | `E0-P0A-LOCK1` | N/A external | Embedded Qdrant permits one process and is currently locked. | high concurrency warning |

## Status Rules

| Status | Meaning | Allowed next action |
|--------|---------|---------------------|
| `correct` | Already behaves as required. | Preserve. |
| `missing` | Required registration does not exist. | Add only the missing entry. |
| `blocked` | Required evidence cannot currently be produced safely. | Stop that path and retain visible blocker. |
| `unbound` | A valid server exists but Codex is not wired to it. | Bind without redesign. |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|------|---------------|----------------|--------|--------------------|----------|--------------------|
| Local Mem0 installation | Mem0 OSS 2.0.19 wrapper and venv exist. | Reuse unchanged. | correct | N/A external | `E0-P0A-SRC1`, `E0-P0A-SRC2` | preserve |
| Local MCP tool contract | Seven tools were discovered and exercised through direct stdio MCP against an isolated store. | Runtime discovery and round trip proven. | correct | N/A external | `E1-P1B-MCP1`, `E1-P1B-ROUND1` | preserve; real-store startup remains conditional |
| Codex MCP registration | One enabled `mem0` stdio server targets the installed local wrapper and existing scope. | One enabled local stdio registration. | correct | N/A external | `E1-P1A-CFG1`, `E1-P1A-CFG2` | preserve; validate P1-B |
| Persistent data/user scope | Store path and user ID are known. | Preserve exact values. | correct | N/A external | `E0-P0A-SCOPE1` | preserve in P1-A |
| Concurrent real-store launch | Active Mem0 process owns the embedded-Qdrant lock. | A new Codex-launched process opens the store after the holder exits, or shared architecture is separately approved. | blocked | N/A external | `E0-P0A-LOCK1`, `E0-P0A-FAIL1` | do not kill holder; disclose blocker |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---------|------|------------|----------------|----------------|----------|-------------------|
| R0 | 2026-08-30 | baseline before implementation | local wrapper, Codex catalog, persistent lock | initial classification | `E0-P0A-SRC1`, `E0-P0A-CFG1`, `E0-P0A-LOCK1` | register without modifying server/data; validate using isolated store |
| R1 | 2026-08-30 | after P1-A | Codex MCP catalog and active lock holder | registration `missing -> correct`; lock remains `blocked` | `E1-P1A-CFG1`, `E1-P1A-CFG2`, `E1-P1A-LOCK1` | proceed to isolated MCP validation; preserve real store |
| R2 | 2026-08-30 | after P1-B | isolated MCP contract and validation store | tool contract `partial -> correct`; real-store launch remains `blocked` | `E1-P1B-MCP1`, `E1-P1B-ROUND1`, `E1-P1B-CLEAN1` | proceed to supervisor with conditional real-store readiness |
| R3 | 2026-08-30 | after supervisor review | full Codex ready-to-use claim | config and isolated contract accepted; real-store readiness remains `blocked` | `E2-P2A-SUP1`, `E2-P2C-CFG1`, `E2-P2C-LOCK1` | close as conditionally ready; require restart and real-store call |

## Phase Touch Map

| Unit / File / Surface | Plan-Relevant Relationship File | Relationship to Target | Plan Item | Touch Mode | Evidence | Constraint |
|-----------------------|---------------------------------|------------------------|-----------|------------|----------|------------|
| Codex MCP catalog | `/home/arch/.codex/config.toml` | source-of-truth config | P1-A | edit | `E0-P0A-CFG1` | add only `mem0`; preserve other servers |
| Installed wrapper | `/home/arch/.local/share/mem0-mcp-server/server.py` | MCP command owner | P1-A, P1-B | preserve-only | `E0-P0A-SRC1` | no source edit |
| Persistent store | `/home/arch/.local/share/mem0` | real memory data | P1-A, P1-B | do-not-touch | `E0-P0A-LOCK1` | no writes, copy, migration, or lock disruption |
| Validation store | `.tmp/mem0-mcp-validation` | isolated test data | P1-B | edit then cleanup | pending | resolve inside repo and remove after test |
| RustSale app | `src/`, `src-tauri/` | unrelated consumers | all | preserve-only | `E0-P0A-GRAPH2` | no app edits |

## Detailed Findings

### Installed local server

Current state:

The executable wrapper is a custom self-hosted stdio server backed by local FastEmbed, Qdrant, and SQLite. It requires no Mem0 Cloud API key and defaults to exact storage when no real LLM is configured.

Required state:

```text
Codex launches the existing wrapper unchanged and receives its seven memory tools.
```

Evidence:

- `E0-P0A-SRC1`: exact installed source and storage configuration inspected.
- `E0-P0A-TOOL1`: all declared MCP tools inventoried.

Relationship and impact:

- Related file count: N/A, outside the indexed repo.
- Relationship summary: owns the MCP contract and persistent-memory boundary.
- Impact note: Tier 3 because delete operations and persistent data are exposed.

Classification:

`correct` installation, `partial` runtime proof.

Allowed next action:

Register and validate without editing the wrapper.

Forbidden next action:

Cloud substitution, wrapper rewrite, data migration, or destructive validation on existing memories.

### Real-store lock

Current state:

PID `15304`, a child of active `agy` PID `4152`, owns the embedded-Qdrant lock and is already running this wrapper.

Required state:

```text
No existing consumer is disrupted; concurrent startup failure remains visible until lock-free verification is possible.
```

Evidence:

- `E0-P0A-LOCK1`: process and file-lock ownership.
- `E0-P0A-FAIL1`: second startup failed closed before MCP initialization.

Relationship and impact:

- Related file count: N/A.
- Relationship summary: one-process storage constraint affects every stdio instance using the shared path.
- Impact note: high concurrency warning.

Classification:

`blocked` for concurrent real-store startup.

Allowed next action:

Validate the exact server contract using an isolated path and configure Codex for the existing path for a future lock-free session.

Forbidden next action:

Kill the holder, bypass the lock, or mutate/copy the live store without explicit authority.

## Next Phase Status Decisions

| Plan Item | Actual Status Finding | Required Status / Next-Action Update |
|-----------|-----------------------|--------------------------------------|
| P1-A | Server exists; only Codex binding is missing. | Add one local stdio entry with existing path/user values. |
| P1-B | Isolated MCP contract and round trip are correct; real store still cannot be opened concurrently. | Preserve the accepted registration and carry the lock condition into supervisor/final readiness. |

## Implementation Gate

- [x] Target scope is listed in Current Status Matrix.
- [x] Each target unit has a status and evidence IDs.
- [x] External targets are marked N/A for indexed relationship counts with rationale.
- [x] Phase Touch Map lists all plan-relevant surfaces and touch modes.
- [x] Correct parts are preserve-only.
- [x] Missing/partial/blocked parts have exact next actions.
- [x] The concurrency blocker is recorded.
- [x] P1 work steps were updated to use isolated validation and preserve the live holder.
- [x] Status Refresh Log contains R0.

## Final P0 Decision

- [ ] P0 actual-status incomplete. Implementation is blocked.
- [ ] P0 complete. Next phase can proceed unchanged.
- [x] P0 complete. Next phase status, next action, or work steps were updated before implementation.
- [ ] P0 complete. Target scope is preserve-only.
- [ ] P0 complete. Implementation is blocked by missing authority or evidence.

Decision note:

Proceed with Codex registration and isolated MCP validation. Do not claim full real-store readiness while the pre-existing lock holder remains active.
