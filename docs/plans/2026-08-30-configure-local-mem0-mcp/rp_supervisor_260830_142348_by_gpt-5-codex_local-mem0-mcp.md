# Supervisor Report: Local Mem0 MCP Readiness

Verdict: REJECT

## Metadata

- Report file: `rp_supervisor_260830_142348_by_gpt-5-codex_local-mem0-mcp.md`
- Review time: `260830 142348 +07 (Asia/Ho_Chi_Minh)`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: user-level Codex MCP registration, installed local Mem0 wrapper, persistent-store access, plan/evidence artifacts, commits `db34691..d527124`
- Claim reviewed: the self-hosted local Mem0 integration is installed and ready for Codex to use with the existing persistent store.
- Authority used: user request; `AGENTS.md`; `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`; active plan requirements R-01 through R-05; current config, source, runtime, process-lock, build, and Anvien evidence.
- Related artifacts: the four-file plan set and commits `db34691`, `d527124`.

## Executive Summary

- Problem: Codex previously had no `mem0` MCP registration despite an existing local Mem0 stdio installation.
- Decision: REJECT the full ready-to-use claim. Registration and isolated MCP behavior are proven, but the exact configured command cannot currently open the existing persistent store because another active stdio server owns the embedded-Qdrant lock.
- Required outcome: release the current lock through the owning client's normal shutdown/restart, then perform a fresh Codex `search_memories` or `get_memories` call against `MEM0_DIR=/home/arch/.local/share/mem0` and user `arch`. If simultaneous clients are required, obtain explicit scope for a shared HTTP/Qdrant-server design instead.

## Blocking Findings

### [HIGH] Configured Codex process cannot currently open the real Mem0 store

File: `/home/arch/.local/share/mem0-mcp-server/server.py:106`

Issue: The server initializes an embedded Qdrant client against a single on-disk directory. PID `15304`, owned by active parent `agy` PID `4152`, already holds `/home/arch/.local/share/mem0/qdrant/.lock`. A fresh launch of the exact configured Codex command fails before MCP initialization with Qdrant `AlreadyLocked`.

Evidence:

- `/home/arch/.codex/config.toml:96` through `:102` correctly register the local Python/server command, user `arch`, and existing data directory.
- `lsof /home/arch/.local/share/mem0/qdrant/.lock` identifies PID `15304` as the live holder.
- A fresh exact-command startup against the real configured directory fails with `RuntimeError: Storage folder ... is already accessed by another instance of Qdrant client`.
- The isolated-store MCP test passed seven-tool discovery and add/search/get/update/delete, proving the wrapper works when it can own its storage.

Why this blocks acceptance: The user asked to install Mem0 for use, and the Completion Lock forbids a full readiness claim while the authoritative real-store runtime is unavailable. Isolated-store success is narrower than existing-store readiness.

Fix direction: Close or restart the client that normally owns the active Mem0 stdio process so the lock is released, then start a new Codex session and verify an actual Mem0 tool call. Do not kill the holder from this task. If both clients must operate concurrently, replace embedded multi-process access with a separately approved shared-server design.

Re-review evidence required: fresh lock-free startup of the configured command and one successful `search_memories` or `get_memories` call against the real store/user scope from a restarted Codex session.

## Source-Level Clearance Notes

- `/home/arch/.codex/config.toml:96`: clear for registration correctness — one enabled local stdio entry uses the installed absolute paths and existing non-secret scope.
- `/home/arch/.local/share/mem0-mcp-server/server.py:113`: clear for the isolated contract — seven declared memory tools match runtime discovery; file remained unmodified.
- `/home/arch/.local/share/mem0-mcp-server/server.py:106`: blocked for concurrent real-store readiness — initialization fails while another embedded-Qdrant process owns the directory.
- `docs/plans/2026-08-30-configure-local-mem0-mcp/`: clear for scope — Anvien reports docs-only, low-risk changes and no affected application processes.

## Evidence Checked

Passed:

- `codex mcp get mem0` — enabled stdio registration with exact local launcher and masked environment.
- Direct config inspection — `MEM0_DEFAULT_USER_ID="arch"` and `MEM0_DIR="/home/arch/.local/share/mem0"` are preserved.
- Isolated direct MCP runtime — seven tools discovered; add, search, list, get, update, delete, and post-delete absence passed.
- `bun run build` — completed successfully with pre-existing warnings only.
- Cleanup — the 56 KiB isolated validation store was moved to Trash and is absent from the repository source path.
- Fresh Anvien compare — only four documentation files changed, risk low, with no affected application process.
- Verification freshness: current — supervisor reran config, source, lock, real-startup, worktree, and Anvien checks after commit `d527124`.

Failed:

- Exact configured command against the real store — fails before MCP initialization due to the active embedded-Qdrant lock.
- Verification freshness: current — failure reproduced during this supervisor pass.

Not run:

- Real-store `search_memories`/`get_memories` from Codex — unavailable until the current lock holder exits and a new Codex session reloads the MCP catalog.
- Concurrent-client shared-server validation — outside the authorized scope.

## Invariant Closure

- Affected invariant: Codex must launch the registered local Mem0 server and access the existing persistent store/user without disrupting another consumer or falsely reporting readiness.
- Sibling surfaces checked: Codex config, installed wrapper tool surface, isolated stdio protocol, existing Qdrant lock owner, real-store startup, plan artifacts, app graph, and cleanup path.
- Residual unverified same-invariant surfaces: a lock-free Codex call against the real store and current user scope.

## Required Fix List For Resubmission

1. End the current Mem0-owning client/session through its normal lifecycle so `/home/arch/.local/share/mem0/qdrant/.lock` is released; do not force-kill it from this task.
2. Restart Codex so the new `mem0` registration enters the tool catalog.
3. Run `search_memories` or `get_memories` against the real store as user `arch`, and provide the fresh result plus lock/process evidence.

## Overall Evaluation

Implementation quality is acceptable for the registration and isolated contract. Evidence quality is strong for those narrower claims. Full acceptance is blocked only by the current embedded-Qdrant single-process constraint and the missing fresh real-store tool call; therefore the correct supervisor verdict is REJECT, corresponding to `CONDITIONALLY READY` rather than complete under the implementation harness.
