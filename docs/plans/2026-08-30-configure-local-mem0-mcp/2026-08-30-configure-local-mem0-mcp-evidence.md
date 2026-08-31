# Configure Local Mem0 MCP Evidence Ledger

## Metadata

- Date: `2026-08-30`
- Plan: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-plan.md`
- Evidence: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-evidence.md`
- Benchmark: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-benchmark.md`
- Actual status: `docs/plans/2026-08-30-configure-local-mem0-mcp/2026-08-30-configure-local-mem0-mcp-actual-status.md`

## Evidence Rules

- Evidence IDs use `E<phase>-<item>-<kind><n>` and are never reused.
- Record only commands and results observed in this run.
- Keep existing memory contents and credentials out of evidence.
- A config-only pass does not prove MCP behavior; direct stdio tool invocation is required.
- A shared-store lock is a blocker, not a passing result.

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-RULE1`: Read `AGENTS.md`, `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`, and the full selected skill sources before implementation.
- `E0-P0A-DOC1`: Context7 resolved `/mem0ai/mem0-mcp`; cloud MCP requires `MEM0_API_KEY`, which does not apply to the user's self-hosted wrapper.
- `E0-P0A-SRC1`: `/home/arch/.local/share/mem0-mcp-server/server.py` uses Mem0 OSS, FastEmbed, on-disk Qdrant, SQLite history, and `FastMCP("mem0-local")` over stdio.
- `E0-P0A-SRC2`: Installed metadata reports `mem0ai` version `2.0.19`.
- `E0-P0A-TOOL1`: Source declares seven tools: `add_memory`, `search_memories`, `get_memories`, `get_memory`, `update_memory`, `delete_memory`, and `delete_all_memories`.
- `E0-P0A-CFG1`: `codex mcp get mem0` returned `No MCP server named 'mem0' found.`
- `E0-P0A-GRAPH1`: `anvien analyze --force` completed with 498 scanned files, 381 parsed code files, 82,212 nodes, and 94,988 relationships.
- `E0-P0A-GRAPH2`: Anvien query found no project-owned MCP/memory integration surface; returned app configuration and in-memory test false positives, so app code is preserve-only.
- `E0-P0A-LOCK1`: `/home/arch/.local/share/mem0/qdrant/.lock` is held by PID `15304`, running the installed server as a child of active process `agy` PID `4152`.
- `E0-P0A-SCOPE1`: The holder uses `MEM0_DIR=/home/arch/.local/share/mem0` and `MEM0_DEFAULT_USER_ID=arch`.
- `E0-P0A-FAIL1`: A direct MCP launch against the real store failed closed with Qdrant `AlreadyLocked`; no memory operation occurred.
- `E0-P0A-GIT1`: Baseline worktree was clean on `master...origin/master [ahead 1]` before plan artifacts were created.

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`

- `E1-P1A-CFG1`: `codex mcp add mem0 --env MEM0_DIR=/home/arch/.local/share/mem0 --env MEM0_DEFAULT_USER_ID=arch -- /home/arch/.local/share/mem0-mcp-server/.venv/bin/python /home/arch/.local/share/mem0-mcp-server/server.py` returned `Added global MCP server 'mem0'.`
- `E1-P1A-CFG2`: `codex mcp get mem0` reports enabled stdio transport, the exact absolute Python/server paths, and two masked environment values; `codex mcp list` contains exactly one enabled `mem0` entry.
- `E1-P1A-LOCK1`: After registration, PID `15304` remained the sole holder of the real Qdrant lock with parent `agy` PID `4152`; registration did not terminate or replace the existing consumer.
- `E1-P1A-BUILD1`: After Anvien reported no build/analyze lock, `bun run build` completed successfully. It emitted pre-existing Svelte state/a11y and large-chunk warnings but no build error; a fresh post-build `codex mcp get mem0` still reported the expected enabled local stdio registration.
- `E1-P1A-DETECT1`: After a fresh `anvien analyze --force`, `anvien detect-changes --repo rustsale --scope all` reported zero changed files, symbols, processes, and affected files in the application graph; the only repo changes are untracked plan documents.

- `E1-P1B-MCP1`: The direct stdio MCP client initialized successfully and discovered exactly seven tools: `add_memory`, `search_memories`, `get_memories`, `get_memory`, `update_memory`, `delete_memory`, and `delete_all_memories`.
- `E1-P1B-ROUND1`: Against isolated `MEM0_DIR=.tmp/mem0-mcp-validation.ye8wwe`, direct MCP calls passed add, semantic search, list, get-by-ID, update, delete, and post-delete absence assertions. This is the superficial-patch killer proving actual protocol and storage behavior rather than config text.
- `E1-P1B-WARN1`: The successful isolated run emitted a Mem0 deprecation warning for `update(data=...)` and a harmless Qdrant destructor warning during Python shutdown. Neither changed the exit result or assertions; the installed wrapper remains preserve-only in this task.
- `E1-P1B-CLEAN1`: The isolated store measured 56 KiB, was resolved under the repository `.tmp` directory, and was moved recoverably to the desktop Trash after validation. The source path no longer exists.
- `E1-P1B-LOCK1`: The real store remained preserve-only and its pre-existing lock holder was not terminated. Concurrent real-store startup remains a disclosed condition.

## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`, `P2-C`

- `E2-P2A-SUP1`: `rp_supervisor_260830_142348_by_gpt-5-codex_local-mem0-mcp.md` returned `REJECT` for the full ready-to-use claim because a fresh exact-command launch against the existing store still fails on the active Qdrant lock. Registration and isolated behavior were cleared.
- `E2-P2B-CLEAN1`: Final cleanup inspection confirmed `.tmp/mem0-mcp-validation.ye8wwe` is absent from the repository; no server source or real-memory file was changed by this plan.
- `E2-P2C-CFG1`: Final `codex mcp get mem0` still reports the exact enabled local stdio registration with masked environment values.
- `E2-P2C-LOCK1`: Final lock inspection still reports PID `15304` under active parent `agy` PID `4152`; the process was not disrupted.
- `E2-P2C-BUILD1`: The last implementation/configuration edit remains covered by successful `bun run build` evidence `E1-P1A-BUILD1`; subsequent repo edits are documentation-only.
- `E2-P2C-STATE1`: Final harness state is `CONDITIONALLY READY`, not complete: R-06 requires a restarted, lock-free Codex real-store call.

## Closure Evidence

- P1-A commit: `db34691` (`docs(plan): configure local Mem0 MCP`).
- P1-B commit: `d527124` (`docs(plan): verify local Mem0 MCP contract`).
- Supervisor report: `rp_supervisor_260830_142348_by_gpt-5-codex_local-mem0-mcp.md` with verdict `REJECT` for full real-store readiness.
- Final readiness: `CONDITIONALLY READY`; pending lock-free real-store verification after the owning client exits and Codex restarts.
