---
name: best-of-n-implementation
description: Orchestrate isolated implementation candidates with agy, rank eligible trajectories through the keyless agy-ppt backend or optional llm-as-a-verifier backend, and carry the selection through independent review and repository completion gates. Use for explicitly requested Best-of-N work or critical changes with multiple materially different valid implementations; do not use for routine fixes or single-diff review.
---

# Best-of-N implementation

Use this skill to select an implementation candidate, not to replace the repository's implementation process. `SELECTED` means only that one eligible candidate ranked above the others. Only the repository Completion Harness can establish readiness.

## 1. Lock the task

Read `AGENTS.md`, Agent Memory, and `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`. Build the requirement ledger, classify risk, and run required GitNexus impact analysis before any candidate edits an existing symbol.

Create a task JSON outside the repository matching [references/task.schema.json](references/task.schema.json). Start with `mode: "shadow"`; use `mode: "enforced"` only when applying the selected patch is authorized. Default to three candidates and `ranking_backend: "agy-ppt"`. Read [references/ranking-backends.md](references/ranking-backends.md) before choosing or configuring the ranking backend.

The Best-of-N runner requires a clean working tree and locks every candidate to the same `HEAD`. It never stashes, commits, resets, or overwrites existing work.

Completion criterion: requirements, acceptance criteria, risk, focused checks, candidate count, execution mode, and base commit are explicit.

## 2. Generate and rank candidates

Read [references/candidate-contract.md](references/candidate-contract.md), then initialize the locked run:

```bash
bun run best-of-n -- init --spec <task.json>
```

Invoke every prompt returned by `init` concurrently as a fresh built-in `self` subagent with agy's workspace option `branch`. This is mandatory: `--sandbox` constrains terminal operations but does not itself provide an independent code checkout. Agy owns and cleans these Git worktrees.

Each candidate implements only in its branch workspace and finishes by running the `collect` command embedded in its prompt. Collection captures the exact diff and runs configured lightweight checks without a shell. Avoid full release builds in candidate workspaces; run them once for the selected patch. Candidates with an empty diff, failed checks, or material limitations are ineligible.

After every `collect` succeeds, terminate the corresponding subagent so agy can remove its temporary branch worktree. Do not terminate a candidate before collection completes. If cleanup fails, report the retained workspace and disk impact instead of deleting it manually. Then run:

```bash
bun run best-of-n -- rank --run-dir <run-dir>
```

Eligible trajectories are ranked using [references/rustsale-criteria.md](references/rustsale-criteria.md). The default `agy-ppt` backend uses blinded pairwise comparisons, reverses A/B positions in fresh plan/sandbox sessions, and invokes an adaptive third judge when the first two disagree. The optional `llm-verifier` backend uses the pinned Python adapter and a separately configured compatible API backend. Both paths fail closed on unavailable execution, malformed output, unresolved votes, or invalid selection evidence. Never silently switch backends and never convert a ranking score into approval.

Completion criterion: at least two eligible candidates share the locked base SHA, every eligible candidate has inspectable diff/check evidence, branch subagents were terminated after collection (or retained cleanup is explicitly reported), and the declared ranking backend produced a persisted `SELECTED` result and auditable evidence without errors.

## 3. Inspect and apply the selection

Inspect `<run-dir>/selection.json`, the backend evidence named there, the selected candidate's `prompt.md`, `candidate.json`, checks, and patch. Also inspect the subagent's final report if the parent retained it. Reject the selection if it violates a requirement, hides relevant evidence, or changes unrelated scope.

In shadow mode, stop after reporting the ranking. In enforced mode, apply only after the inspection:

```bash
bun run best-of-n -- apply --run-dir <run-dir>
```

The apply command fails closed unless the repository path, `HEAD`, and clean working-tree state still match the locked manifest. It runs `git apply --check` before applying the exact stored patch.

Completion criterion: the chosen patch is either left unapplied in shadow mode or applied to the unchanged locked base with no unrelated work present.

## 4. Independent final review

After application, start a fresh `agy` review session in read-only/plan mode. Use the production review contract and schema already shared by `.agents/skills/codex-review/references/`. The reviewer must inspect the actual current diff and requirements, not trust the candidate transcript or verifier ranking.

Validate each finding against code evidence. For accepted findings, follow GitNexus pre-edit rules, fix the root cause, rerun focused checks, and ask the same review thread to re-review. Stop after five attempts, two invalid reviewer outputs, two consecutive repetitions of the same unresolved root defect, or any need for new authority.

Completion criterion: the final reviewer returns `APPROVED` with empty findings and limitations, or the workflow reports `CHANGES_REQUESTED`/`REVIEW_INCOMPLETE` without claiming readiness.

## 5. Close repository gates

Run fresh tests, typechecks, builds, Rust checks, `git diff --check`, and GitNexus `detect_changes` in proportion to the task's risk. Close every applicable Completion Lock item. Use [references/gate-output.schema.json](references/gate-output.schema.json) for the final workflow result.

- `PASS`: independent review approved and every applicable Completion Lock item is green.
- `FAIL`: an actionable defect or quality gate remains failing after the bounded repair loop.
- `INCOMPLETE`: required candidate, ranking, review, or runtime evidence is unavailable or invalid.

Commit, push, merge, release, or cleanup of unrelated work requires separate authorization.
