# Optimize Tauri Skill Evidence Ledger

## Metadata

- Date: `2026-09-01`
- Plan: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-plan.md`
- Evidence: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-evidence.md`
- Benchmark: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-benchmark.md`
- Actual status: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-actual-status.md`

## Evidence Rules

- Record only commands and facts observed in the current run.
- Use stable evidence IDs in companion files.
- Treat official documentation as the authority for version-sensitive technical claims.
- Treat structural checks as proof of structure only, not proof of Tauri runtime performance.
- Preserve the unavailable-mem0 limitation and existing unrelated worktree state.

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

- `E0-P0A-SRC1`: Full `/home/arch/.agents/skills/tauri/SKILL.md` read, lines 1-579.
- `E0-P0A-METRIC1`: Source measurement observed 579 lines, 27,125 bytes, 32 Markdown headings, and 44 code-fence markers.
- `E0-P0A-META1`: Existing metadata description measured 196 characters; it is technically below 200 but leaves almost no editing margin and reads as a broad article description.
- `E0-P0A-STRUCT1`: `/home/arch/.agents/skills/tauri/` contained only `SKILL.md`; no progressive-disclosure references existed.
- `E0-P0A-DUP1`: Build-profile advice appears at lines 86-115, 528-541, and 551-552, with `opt-level = "z"`, `"s"`, and prose recommending `"3"` for speed.
- `E0-P0A-DUP2`: IPC batching/large payload advice repeats pagination at lines 119-144 and IPC advice at lines 520-526.
- `E0-P0A-DUP3`: platform-specific handling repeats lines 170-205, 513-514, and 578-579.
- `E0-P0A-DUP4`: profiling/measurement repeats lines 430-455, 511-512, 554-555, and 572-576.
- `E0-P0A-CONFLICT1`: “strict CSP” at lines 308-323 includes `'unsafe-inline'` for scripts, weakening the stated policy.
- `E0-P0A-CONFLICT2`: error guidance says to hide implementation details at lines 356-381 but returns full filesystem paths and raw internal errors.
- `E0-P0A-SUSPECT1`: Tauri v1-style `devPath` appears at line 266 while the skill claims Tauri v2.
- `E0-P0A-SUSPECT2`: “Strict Allowlist” at line 540 uses v1 terminology despite v2 capability/permission guidance earlier.
- `E0-P0A-SUSPECT3`: `Rc` is recommended for sharing data across commands and threads at line 527 although `Rc` is not thread-safe.
- `E0-P0A-NOISE1`: lines 548-579 contain unattributed company/location anecdotes and numerical gains that are not reliable operational instructions.
- `E0-P0A-MEM1`: agentmemory recall succeeded and returned prior Tauri production lessons; no mem0 tool was exposed in the session tool catalog.
- `E0-P0A-GRAPH1`: `anvien analyze --force` completed with 542 scanned files, 382 parsed code files, 0 parse failures, 85,271 nodes, and 98,363 relationships.
- `E0-P0A-GRAPH2`: Initial query used the unregistered name `RustSale_v2`; `anvien list` established the registered repo name is `rustsale`, and the corrected query completed. No indexed external target file exists, so symbol/file impact is not applicable to the global skill.
- `E0-P0A-GIT1`: Baseline `git status --short --branch` showed `master...origin/master` and pre-existing untracked `Reports/`; it is preserve-only.

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`

- `E1-P1A-AUDIT1`: The 579 source lines were classified into eight canonical duplicate groups: release profiles, blocking/CPU work, capability/least privilege, large IPC data, memory/streaming, platform WebViews, measurement/profiling, and frontend bundle/rendering.
- `E1-P1A-AUDIT2`: Unattributed SEO/company anecdotes, generic framework/database/network advice without Tauri-specific action, unsupported percentages, stale syntax, and non-compiling examples were separated from retainable guidance.
- `E1-P1B-RESEARCH1`: `docs/research/2026-09-01-tauri-skill-official-validation.md` records an 85-line primary-source audit using official Tauri, Cargo/rustc, Tokio, WebView2, and Tauri distribution sources.
- `E1-P1B-FACT1`: Official Tauri v2 configuration uses `build.devUrl` and `build.frontendDist`; `devPath` is stale.
- `E1-P1B-FACT2`: Tauri v2 replaces the v1 allowlist with permissions/capabilities; capability files are auto-enabled by default unless configuration selects a subset.
- `E1-P1B-FACT3`: Large IPC guidance must distinguish structured JSON invokes, raw `Response`, ordered `Channel` streaming, small event payloads, and scoped asset protocol access.
- `E1-P1B-FACT4`: Official Cargo guidance treats `opt-level = "z"`, `"s"`, and `3` as different objectives and explicitly requires experimentation; no universal profile is both smallest and fastest.
- `E1-P1B-FACT5`: Official Tokio guidance requires blocking/CPU work isolation, bounded CPU concurrency, and lock selection based on whether a guard crosses `.await`; `Rc` is not valid thread-shared state.
- `E1-P1B-CONFLICT1`: Five genuine owner policies identified and resolved:
  1. Balanced release profile with explicit size-first and throughput-first options + mandatory benchmark gate.
  2. Remote origins denied by default, minimal recipe in security reference.
  3. Isolation pattern as conditional hardening for untrusted frontend code.
  4. UPX removed completely from standard optimization guidance.
  5. Linux distribution via package matrix (deb/rpm/AppImage/Flatpak) without false static promises.
- `E1-P1B-DECISION1`: User approved all 5 policy recommendations.

## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`, `P2-C`

- `E2-P2A-FILES1`: Final reference inventory in `/home/arch/.agents/skills/tauri/references/`: security 108 lines, IPC 115, runtime 122, build 94, platform 41, and frontend 50; every file is below 150 lines.
- `E2-P2A-REMOVE1`: Removed duplicate monolithic advice, marketing anecdotes, stale v1 configuration, unsupported performance guarantees, invalid cross-target linker guidance, and the rejected UPX section.
- `E2-P2B-SKILL1`: Final `/home/arch/.agents/skills/tauri/SKILL.md` is 68 lines and 3,832 bytes with a 147-character third-person description and six-task routing table.
- `E2-P2B-LINK1`: Verified all 6 markdown links from `SKILL.md` resolve to existing reference files.
- `E2-P2C-STRUCT1`: Fresh `bun` validation passed: all seven files below 150 lines, balanced fences, 6/6 internal links, valid frontmatter shape, 147-character description, zero forbidden package-manager commands, zero stale v1 config keys, and zero rejected terms. The remote-origin example is explicitly labeled for replacement rather than counted as an unresolved placeholder.
- `E2-P2C-SYNTAX1`: Language-aware checks passed for 7 JSON, 3 TOML, 7 Rust, 4 TypeScript, and 4 Bash blocks. JSON/TOML were parsed, TypeScript transpiled, Rust parsed/formatted by `rustfmt`, and Bash parsed with `bash -n`; this is syntax evidence, not dependency-level compilation.
- `E2-P2C-LINK1`: 17/17 external Markdown links to primary Tauri, Cargo, and Tokio references returned HTTP 200.
- `E2-P2C-REVIEW1`: Hostile review corrected unsupported Channel backpressure, asset CSP, ineffective semaphore usage, default-permission terminology, async blocking, isolation boundaries, profile/linker universality, platform claims, and frontend thresholds/guarantees.
- `E2-P2C-SUP1`: Initial supervisor review rejected the resubmission with nine blocker/major findings in `rp_supervisor_260901_223749_by_gpt-5-6-codex_tauri_skill_resubmission.md`.
- `E2-P2C-SUP2`: Fresh supervisor re-review passed after verifying every blocker in `rp_supervisor_260901_224501_by_gpt-5-6-codex_tauri_skill_rereview.md`.

## Closure Evidence

- Target files modified in `/home/arch/.agents/skills/tauri/`: `SKILL.md` and 6 reference markdown files.
- Companion plan, evidence, benchmark, and actual-status ledgers updated.
- `anvien analyze --force` refreshed the repo graph (548 files scanned, 382 code files parsed, 0 failures); `anvien detect-changes --repo rustsale --scope all` reported no graph-visible code changes because the target is an external Markdown skill and the plan artifacts are untracked documentation.
- Final Git status remains `master...origin/master` with untracked `Reports/`, this plan directory, and the research note; no commit was made.
