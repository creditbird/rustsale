# Optimize Tauri Skill Plan

## Metadata

- Date: `2026-09-01`
- Status: `complete`
- Plan: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-plan.md`
- Evidence: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-evidence.md`
- Benchmark: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-benchmark.md`
- Actual status: `docs/plans/2026-09-01-optimize-tauri-skill/2026-09-01-optimize-tauri-skill-actual-status.md`

## Goal

Turn `/home/arch/.agents/skills/tauri/SKILL.md` into a concise, current, actionable Tauri v2 skill with progressive-disclosure references, deduplicated guidance, explicit trade-offs, and no unverified internet claims.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- Preserve the user's source material only when it remains accurate, Tauri-specific, and operationally useful.
- Prefer official Tauri, Rust, Tokio, Cargo, and platform documentation over unattributed marketing claims.
- Ask the user before choosing between genuinely conflicting optimization objectives or policies.
- Keep `SKILL.md` and every reference below 150 lines, avoid duplication across files, and keep metadata below 200 characters.
- Use `bun`/`bunx`; never use `node`, `npm`, or `npx` in this repository.
- Run Anvien detect-changes before any implementation-slice commit when repository implementation work was performed.
- This text-only skill rewrite has no app runtime, UI, database, Docker, or Playwright surface.

## Problem

The current skill is a 579-line monolith that mixes Tauri security, API examples, performance advice, generic frontend/Rust/database guidance, and unattributed promotional claims. Several topics repeat, some examples appear version-stale, and multiple recommendations conflict unless the optimization goal is stated.

## Scope

- Audit every current recommendation for duplication, conflict, specificity, and current Tauri v2 correctness.
- Research disputed or version-sensitive claims against primary sources.
- Rewrite the Tauri skill entrypoint and create only the reference files required for progressive disclosure.
- Validate structure, links, line limits, command conventions, internal consistency, and source traceability.
- Record the user decision for any unresolved conflict before applying that policy.

## Non-Goals

- Change RustSale application code, Tauri configuration, dependencies, tests, or release artifacts.
- Benchmark the RustSale runtime or prescribe one universal release profile.
- Preserve unverifiable percentages, company anecdotes, or generic advice that is not Tauri-specific.
- Commit changes unless the user explicitly requests a commit.

## Requirements

| ID | Exact requirement | Invariant | Failure case | Required evidence |
|---|---|---|---|---|
| R-01 | Detect and consolidate duplicate ideas. | Each operational instruction has one canonical home. | Same recommendation appears in SKILL and references or in multiple references. | Dedup inventory and post-edit overlap review. |
| R-02 | Surface conflicts to the user. | No genuine policy/goal conflict is resolved silently. | Skill hard-codes size-first, speed-first, or security-weakened behavior without user choice or explicit conditional routing. | Conflict matrix and user decision. |
| R-03 | Produce a complete Tauri v2 skill. | Core workflow is actionable and detailed guidance is discoverable by task. | Skill is a long article, vague checklist, or missing validation flow. | Manual acceptance checklist and link validation. |
| R-04 | Keep technical advice current and trustworthy. | Version-sensitive claims are verified against primary sources or removed/qualified. | Stale v1 keys, unsafe defaults, invalid commands, or unsupported performance claims remain. | Primary-source research and hostile review. |
| R-05 | Meet skill packaging constraints. | Entry and each reference are under 150 lines; metadata is under 200 characters. | Any limit is exceeded or content is duplicated. | Line/character/link checks. |

## Acceptance Criteria

- Duplicate groups are listed and consolidated once.
- Conflicting recommendations are either decided by the user or represented as explicit goal-based branches.
- Stale, incorrect, unsafe, generic, and promotional material is removed or corrected.
- The entrypoint is below 150 lines and routes to focused reference files, each below 150 lines.
- All internal links exist, YAML frontmatter is valid, commands obey repository package-manager rules, and no placeholder remains.
- A hostile final review finds no sentence that is only mostly correct.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: read the full source skill and governing skill-creator rules; measure file size, metadata, headings, code fences, and bundled resources; classify correct, partial, wrong, duplicated, and blocked surfaces; update later phase decisions.
  - Implementation Gate: no target skill edit starts until the actual-status file has a final P0 decision.
  - Acceptance: actual status identifies the monolith, missing progressive disclosure, duplicate clusters, suspicious version-sensitive claims, and the user-decision boundary.

### P1: Audit and Primary-Source Research

- Phase Goal: convert the collected tips into a verified deduplication and conflict matrix.
- Phase Boundary:
  - In scope: existing skill text, official Tauri/Rust/Tokio/Cargo documentation, repo-specific bun rule.
  - Out of scope: application source edits and secondary marketing claims as proof.
  - Dependencies: P0 baseline; research lane; user answer for genuine policy conflicts.
- Phase Implementation Rule: complete P1-A before P1-B; do not edit the target skill in this phase.
- Ordered Slice List:
  - P1-A: Classify duplicate and suspect guidance.
  - P1-B: Verify version-sensitive claims and ask for conflict decisions.

- [x] P1-A: Classify duplicate and suspect guidance.
  - Goal: map every current tip to one canonical topic and identify repeats, stale syntax, unsafe examples, generic filler, and unverifiable claims.
  - Scope Boundary:
    - Editable: plan evidence and research note only.
    - Inspect-only: `/home/arch/.agents/skills/tauri/SKILL.md` and official docs.
    - Preserve-only: application source and existing `Reports/` worktree content.
    - Out of scope: target skill rewrite.
  - Non-Goals: decide unresolved product priorities.
  - Pre-flight Questions:
    - Data source: full source skill plus primary documentation.
    - Display permission: N/A; no UI.
    - DB read flow: N/A; no database.
    - DB write flow: N/A; no database.
    - Render location: N/A; Markdown skill only.
    - UI behavior flow: N/A; no UI.
    - Docker runtime: N/A; text-only documentation policy.
    - Playwright target: N/A; no UI runtime.
    - Behavior test: structural and semantic skill validation.
    - Cleanup/quarantine: remove only plan-created dead artifacts.
    - External side effects: official documentation browsing only.
    - N/A notes: no executable product behavior changes.
  - Work Steps:
    1. Build duplicate and conflict groups from the complete 579-line source.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: N/A.
       - Mini QA: manually trace each source section to exactly one planned destination.
       - Evidence target: `E1-P1A-AUDIT1`.
    2. Separate Tauri-specific operational guidance from generic Rust/frontend/database advice and promotional content.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: N/A.
       - Mini QA: sample every removal category against the user goal.
       - Evidence target: `E1-P1A-AUDIT2`.
  - Implementation Gate: source skill has been read completely; no graph impact is applicable because the target is an external Markdown skill, not an indexed code symbol.
  - Acceptance:
    - Source: all 579 lines classified.
    - Runtime/UI: N/A.
    - DB/data: N/A.
    - Behavior test: each retained idea has one canonical destination.
    - Cleanup/quarantine: no temp artifacts.
    - Evidence IDs: `E1-P1A-AUDIT1`, `E1-P1A-AUDIT2`.
    - Actual-status rows refreshed: Tauri skill content and structure.
  - Evidence Targets: complete audit matrix and removal rationale.
  - Actual-status Update: record exact duplicate groups and blocked conflicts.
  - Commit Boundary: no commit; research-only slice.

- [x] P1-B: Verify version-sensitive claims and ask for conflict decisions.
  - Goal: establish current technical facts and obtain user direction where more than one valid optimization policy remains.
  - Scope Boundary:
    - Editable: `docs/research/2026-09-01-tauri-skill-official-validation.md` and plan ledgers.
    - Inspect-only: official documentation and current skill.
    - Preserve-only: product source.
    - Out of scope: target skill rewrite before user decision.
  - Non-Goals: retain claims merely because they appeared in the original collection.
  - Pre-flight Questions:
    - Data source: primary sources only.
    - Display permission: N/A.
    - DB read flow: N/A.
    - DB write flow: N/A.
    - Render location: N/A.
    - UI behavior flow: N/A.
    - Docker runtime: N/A.
    - Playwright target: N/A.
    - Behavior test: source-to-claim traceability and contradiction review.
    - Cleanup/quarantine: delete abandoned research drafts.
    - External side effects: read-only web access.
    - N/A notes: no app runtime.
  - Work Steps:
    1. [Complete] Verify capabilities, CSP, IPC, async work, state, build configuration, bundling, and platform claims with official sources.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: N/A.
       - Mini QA: every version-sensitive retained claim receives a source or an explicit qualification.
       - Evidence target: `E1-P1B-RESEARCH1`.
    2. [Complete] Present only genuine conflicts to the user and record the chosen policy.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: N/A.
       - Mini QA: verify no conflict was silently resolved.
       - Evidence target: `E1-P1B-DECISION1`.
  - Implementation Gate: official-source research is available; target edit remains blocked until user decisions are recorded.
  - Acceptance:
    - Source: research note cites primary sources.
    - Runtime/UI: N/A.
    - DB/data: N/A.
    - Behavior test: disputed claims are verified, qualified, or rejected.
    - Cleanup/quarantine: no duplicate research artifact.
    - Evidence IDs: `E1-P1B-RESEARCH1`, `E1-P1B-DECISION1`.
    - Actual-status rows refreshed: version-sensitive claims and policy conflicts.
  - Evidence Targets: primary-source matrix and user decision record.
  - Actual-status Update: transition unresolved claims to verified/rejected; conflicts to decided or blocked.
  - Commit Boundary: no commit unless the user requests it.

### P2: Rewrite and Validate the Skill Package

- Phase Goal: implement the accepted, deduplicated skill architecture and prove it meets the skill-creator gates.
- Phase Boundary:
  - In scope: `/home/arch/.agents/skills/tauri/SKILL.md` and required `references/*.md`.
  - Out of scope: application source, generated AGENTS content, and release behavior.
  - Dependencies: P1 complete and user conflict decisions recorded.
- Phase Implementation Rule: create reusable references first in P2-A, then replace the entrypoint in P2-B, then validate in P2-C.
- Ordered Slice List:
  - P2-A: Create focused reference files.
  - P2-B: Replace the skill entrypoint.
  - P2-C: Run structural, semantic, and hostile validation.

- [x] P2-A: Create focused reference files.
  - Goal: place detailed, non-overlapping guidance into task-routed files below 150 lines each.
  - Scope Boundary:
    - Editable: `/home/arch/.agents/skills/tauri/references/*.md`.
    - Inspect-only: official research note and current skill.
    - Preserve-only: repo source and user-created `Reports/`.
    - Out of scope: entrypoint until references stabilize.
  - Non-Goals: create placeholder references.
  - Pre-flight Questions: data source is accepted research; all UI/DB/Docker/Playwright fields are N/A because this is a Markdown skill package; cleanup removes only plan-created rejected drafts; no external write side effects.
  - Work Steps:
    1. Write security/IPC, runtime/frontend, and build/platform references with one canonical home per instruction.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: reference routing from SKILL.
       - Mini QA: line limit and overlap review per file.
       - Evidence target: `E2-P2A-FILES1`.
    2. Remove stale examples, generic filler, unsupported claims, and unsafe defaults rather than carrying them forward.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: N/A.
       - Mini QA: compare every removed category to audit evidence.
       - Evidence target: `E2-P2A-REMOVE1`.
  - Implementation Gate: `E1-P1B-DECISION1` exists; no Anvien impact applies to external Markdown files.
  - Acceptance: every reference is useful, linked, below 150 lines, and non-overlapping.
  - Evidence Targets: file inventory, line counts, retained-source map.
  - Actual-status Update: structure `missing -> correct`; content `partial/wrong -> correct` where proven.
  - Commit Boundary: no commit unless requested.

- [x] P2-B: Replace the skill entrypoint.
  - Goal: produce a concise operational workflow with strong activation metadata and progressive-disclosure routing.
  - Scope Boundary:
    - Editable: `/home/arch/.agents/skills/tauri/SKILL.md`.
    - Inspect-only: new references and skill-creator checklist.
    - Preserve-only: all unrelated global skills and repo files.
    - Out of scope: new assets or scripts unless proven necessary.
  - Non-Goals: duplicate detailed advice from references.
  - Pre-flight Questions: data source is accepted P1/P2-A output; all UI/DB/Docker/Playwright fields are N/A; cleanup removes the obsolete monolithic content through replacement; no external side effect beyond the requested file edit.
  - Work Steps:
    1. Write metadata, trigger scope, repository-first discovery, measure-first workflow, decision routing, implementation order, and verification gates.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: progressive-disclosure links.
       - Mini QA: activation and acceptance checklist.
       - Evidence target: `E2-P2B-SKILL1`.
    2. Verify every detailed instruction appears only in its reference and every reference is reachable from the entrypoint.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: link traversal.
       - Mini QA: no-duplication review.
       - Evidence target: `E2-P2B-LINK1`.
  - Implementation Gate: P2-A accepted; metadata and routing design fixed.
  - Acceptance: SKILL is below 150 lines, metadata below 200 characters, imperative, actionable, and link-complete.
  - Evidence Targets: metadata count, line count, link check, manual checklist.
  - Actual-status Update: entrypoint `partial -> correct`.
  - Commit Boundary: no commit unless requested.

- [x] P2-C: Run structural, semantic, and hostile validation.
  - Goal: prove the rewritten package is internally consistent and useful for real Tauri optimization work.
  - Scope Boundary:
    - Editable: only defects found in the target skill package and plan ledgers.
    - Inspect-only: official research, skill-creator checklist, final diff.
    - Preserve-only: all unrelated files.
    - Out of scope: app runtime validation.
  - Non-Goals: claim runtime performance gains without benchmarks.
  - Pre-flight Questions: data source is final package; all UI/DB/Docker/Playwright fields are N/A; cleanup removes dead plan-created drafts; no external side effects.
  - Work Steps:
    1. Run YAML, line-count, description-length, internal-link, placeholder, forbidden-command, and duplicate-phrase checks.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: all links resolve.
       - Mini QA: inspect actual output, not only exit status.
       - Evidence target: `E2-P2C-STRUCT1`.
    2. Perform a hostile sentence-level review and use the supervisor acceptance workflow before closure.
       - UI flow check: N/A.
       - DB/data flow check: N/A.
       - Render location check: workflow can route representative Tauri tasks.
       - Mini QA: challenge unsafe, universal, stale, and unsupported statements.
       - Evidence target: `E2-P2C-REVIEW1`.
  - Implementation Gate: target rewrite is complete and all evidence is fresh after the last edit.
  - Acceptance: all skill-creator critical checks pass; no unresolved conflict, stale syntax, unsupported claim, broken link, or duplicate instruction remains.
  - Evidence Targets: command output, hostile review notes, supervisor result.
  - Actual-status Update: all target rows transition to `correct` or remain explicitly blocked.
  - Commit Boundary: no commit unless requested.

- [x] Pn-A: Call supervisor for the implemented-plan acceptance loop.
  - Goal: verify the completed plan work against the accepted plan, evidence, benchmark, changed files, and validation results before closure.
  - Work Steps: read the supervisor skill; review the complete package from actual files; repair only rejected scope; repeat until pass or a blocker is recorded.
  - Implementation Gate: all planned implementation slices are complete or explicitly blocked.
  - Acceptance: supervisor review passes or records a blocker with evidence.

- [x] Pn-B: Remove dead work created during this plan.
  - Goal: ensure the final diff contains only artifacts that still serve the accepted plan.
  - Work Steps: inspect plan-created files and target package; remove rejected drafts/placeholders; preserve unrelated work; rerun supervisor review.
  - Implementation Gate: remove only artifacts created by this plan unless the user explicitly approves broader cleanup.
  - Acceptance: no dead plan-created artifact remains and supervisor passes cleanup.

- [x] Pn-C: Close the plan.
  - Goal: finish validation, evidence, benchmark, and final status without inventing runtime proof.
  - Work Steps: run final documentation checks; record why build/Docker/Playwright are N/A; run Anvien detect-changes for repo plan changes; record worktree state; do not commit without user instruction.
  - Implementation Gate: Pn-A and Pn-B pass or record blockers.
  - Acceptance: final evidence is recorded and the worktree state is known.

## Risk Notes

- The requested target is outside the RustSale Git repository, so repository graph impact does not model it; manual file-boundary evidence is authoritative for that file.
- The skill is global and may affect future Tauri tasks across projects; unsafe universal language has a wider behavioral blast radius than a normal local documentation edit.
- Current content includes conflicting size/speed profile choices and security examples that may weaken their own stated policy; user direction is required for genuine preference conflicts, while objectively stale/unsafe syntax will be corrected.
- The unavailable `mem0` tool prevents satisfying the dual-memory rule fully in this session; agentmemory evidence is available and the limitation must remain disclosed.
