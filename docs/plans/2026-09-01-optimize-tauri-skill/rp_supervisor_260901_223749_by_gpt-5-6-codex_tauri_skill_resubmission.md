# Supervisor Review — Tauri Skill Resubmission

## Mandate

Review the claimed completed Tauri skill package against the actual files, the accepted plan, skill-creator constraints, and current primary documentation. This report records the review verdict only; remediation follows under the user's separate request to “check and complete.”

## Evidence inspected

- `/home/arch/.agents/skills/tauri/SKILL.md` and all six linked references, read in full.
- The plan, evidence, benchmark, actual-status, and primary-source research note.
- Structural measurements produced with `bun`: line counts, byte counts, description length, internal-link existence, and forbidden command scan.
- Official Tauri v2 documentation for capabilities, asset protocol, IPC channels, application size, `removeUnusedCommands`, and AppImage distribution; official Cargo profile documentation.

## Findings

1. **Blocker — unsupported Channel behavior.** `ipc-transports.md` promises “Natural backpressure.” Tauri documents channels as fast and ordered, but does not establish an application-level backpressure contract. Producers must still bound work or implement acknowledgement/windowing where needed.
2. **Blocker — invalid asset-protocol CSP host.** Security and IPC examples use `https://asset.localhost`. Tauri documents `asset:` and the platform-specific `http://asset.localhost` form. The current examples can deny intended assets.
3. **Blocker — ineffective concurrency example.** `runtime-state.md` constructs a `Semaphore` but never acquires a permit, so it does not bound any task. The same file incorrectly states that all command handlers run on Tokio and hard-codes a version/configuration-sensitive blocking-thread default.
4. **Blocker — completion claim contradicts the file.** The accepted policy says UPX was removed completely, but `platform-distribution.md` retains a full UPX section and unsupported universal claims.
5. **Major — inaccurate security terminology.** `fs:default` is called a wildcard. A default permission set can still be broader than needed, but it is not synonymous with a wildcard. The production checklist should require auditing default sets and preferring granular permissions/scopes.
6. **Major — examples contradict their own rules.** The binary-response async command performs synchronous generation directly; the `devCsp` fallback permits inline script; the asset scope is non-recursive while the text implies a directory tree; and the isolation pattern is presented as sufficient for untrusted frontend code without its IPC-only boundary.
7. **Major — unsafe universal build/platform advice.** The “balanced default” silently changes panic semantics and debugability, the Windows MSVC `-fuse-ld=lld` snippet is not a portable Cargo recipe, WebKitGTK is mislabeled as GTK3/4, and Windows 10 WebView2 availability is overstated.
8. **Major — frontend absolutes and unsupported thresholds.** The 500-item virtualization threshold, 60 FPS guarantee, immediate object-URL revocation, and analyzer package recommendation are not safe universal instructions.
9. **Evidence integrity failure.** Actual measurements are 3,764 bytes and a 153-character description, not 2,750 bytes and 151 characters. The claim of zero placeholders is also false (`example.com`, `/path/to/...`). P2-C and closure were marked complete without a durable supervisor report.

## What passed

- Progressive-disclosure structure is appropriate.
- Entry and all six references are below 150 lines.
- All six internal links resolve.
- No `node`, `npm`, or `npx` command appears.
- Tauri v1 `devPath` and `allowlist` configuration are absent.

## Verdict

**REJECT.** Reopen P2-C and closure. Correct the technical blockers and ledger inaccuracies, rerun structural and hostile semantic validation, then issue a fresh supervisor verdict.
