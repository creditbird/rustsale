---
name: codex-review
description: Run an independent Codex CLI review for a defined Git diff, triage actionable findings, and drive a bounded fix-and-re-review loop with fresh quality evidence. Use when the user asks Codex to review uncommitted changes, a commit, or a branch diff, including auto-fix until approval.
---

# Codex production review

Use Codex CLI as an independent, read-only reviewer. The current agent remains the implementer and evidence owner.

`VERDICT: APPROVED` means the reviewer found no actionable defect in the reviewed scope. It is not permission to commit, push, merge, release, or call the change production-ready. Those claims still require repository instructions, fresh quality gates, and any completion harness in force.

## 1. Lock the review scope

Before invoking Codex:

1. Read repository instructions and any mandatory completion or review harness.
2. Inspect `git status --short --branch` and the relevant diff. Preserve unrelated work.
3. Record the requirement or acceptance criteria the diff is meant to satisfy.
4. Choose exactly one review target:
   - `uncommitted`: all staged, unstaged, and untracked changes in the working tree.
   - `base <branch>`: the branch diff against a known base.
   - `commit <sha>`: one committed change.
5. Resolve a base or commit target with `git rev-parse --verify` before use and record the resulting commit ID. Quote refs in shell commands; do not interpolate an unresolved ref as executable shell text.
6. If the selected target includes unrelated work, use a clean worktree/branch or a narrower commit-based target. `--uncommitted` has no path filter, so never describe it as reviewing only selected files.
7. Run the repository's baseline checks individually. Preserve stderr and exit codes; do not combine alternatives with `||` in a way that turns one failed gate into success.

Completion criterion: the target, requirements, pre-existing failures, and changed-file scope are explicit.

## 2. Dispatch a requirements-aware structured review

Read [references/reviewer-prompt.md](references/reviewer-prompt.md) and pass its full contents plus the task-specific requirements to Codex. Resolve the schema path from this skill's directory.

Create a unique run directory outside the repository. Assemble the review contract, locked scope, requirements, and relevant baseline facts in `<run-dir>/prompt.md` using the current agent's normal file-editing mechanism. Feed the prompt through stdin to avoid shell-quoting bugs, then capture both the JSONL event stream and final structured response:

```bash
codex exec -s read-only --strict-config --json \
  --output-schema "<skill-root>/references/review-output.schema.json" \
  -o "<run-dir>/result.json" \
  - < "<run-dir>/prompt.md" \
  > "<run-dir>/events.jsonl"
```

The prompt must name the exact target and tell Codex how to inspect it: working-tree status plus staged/unstaged/untracked content, `<base>...HEAD`, or `git show <sha>`. Put `-m <model>` before the prompt only when the user or repository policy selects a model. Keep the sandbox `read-only`; the reviewer may inspect files and history but must not modify the workspace.

The built-in review command is useful for a quick default review:

```bash
codex exec -s read-only --strict-config --json \
  -o "<run-dir>/raw-review.txt" \
  review --uncommitted \
  > "<run-dir>/events.jsonl"
```

Do not append a custom prompt to `review --uncommitted`, `review --base`, or `review --commit`: the official CLI contract makes those targets mutually exclusive with `PROMPT`. Do not use built-in review output as the machine-readable production contract; inspect it manually or use generic `codex exec` with the schema above.

Use `codex exec --help` and `codex exec review --help` when the installed CLI differs from the documented command surface. Do not use `--ephemeral`, because the loop needs a resumable thread.

Completion criterion: the CLI exits successfully, `result.json` validates against the schema, `events.jsonl` contains the new thread ID, and no material `error` or `turn.failed` event undermines the review.

## 3. Validate the verdict

Treat the run as `REVIEW_INCOMPLETE`, never as approval, when the command fails, output is missing or invalid, the thread ID cannot be identified, an event reports a material execution failure, or the reviewer reports a material limitation.

Enforce these response invariants:

- `APPROVED`: `findings` and `limitations` are both empty.
- `CHANGES_REQUESTED`: at least one actionable finding exists.
- `REVIEW_INCOMPLETE`: at least one limitation explains what prevented a reliable review.

Independently verify every finding before editing:

- The cited file and line exist and are in the locked diff, or the finding clearly identifies unchanged code directly broken by the diff.
- The described trigger can reach the faulty behavior.
- The impact is concrete, not a style preference or speculative concern.
- The suggested verification would fail for the defect and pass for a root-cause fix.

Deduplicate findings that share one root cause. If a finding is wrong or ambiguous, respond in the same thread with code evidence and request re-evaluation; do not blindly implement the suggestion.

Completion criterion: every accepted finding maps to a requirement/invariant and a verification action; rejected findings have explicit evidence.

## 4. Fix and re-review

For each accepted finding:

1. Follow repository pre-edit rules, including impact analysis for every symbol when required. Warn before high-risk edits.
2. Reproduce the failure or add a focused RED check when practical.
3. Apply the smallest root-cause fix and update direct dependents.
4. Run focused tests and relevant static checks separately. Record actual outcomes and skipped checks.
5. Inspect the new diff for scope drift and unrelated edits.

Resume the exact thread ID captured from `events.jsonl`:

```bash
codex exec -s read-only --strict-config --json \
  --output-schema "<skill-root>/references/review-output.schema.json" \
  -o "<run-dir>/result-N.json" \
  resume "<thread-id>" - < "<run-dir>/re-review-N.md" \
  > "<run-dir>/events-N.jsonl"
```

Ask Codex to inspect the latest diff independently, verify each previous finding, and look for regressions introduced by the fix. Pass test evidence as evidence to assess, not as proof the reviewer must trust.

Use `resume --last` only when the environment guarantees no other Codex session could have started. An explicit thread ID is the production default.

## 5. Circuit breakers

Stop and report `NOT READY` or the repository's equivalent state when any condition occurs:

- Five total review attempts have completed without approval.
- A per-attempt wall-clock budget chosen for the repository is exceeded; interrupt the run and classify it as `REVIEW_INCOMPLETE` instead of waiting indefinitely.
- The same unresolved root finding returns in two consecutive re-reviews without meaningful diff progress.
- Codex produces invalid or incomplete output twice.
- The review scope changes materially and has not been re-locked.
- A fix needs user authority, credentials, destructive action, or scope expansion.
- Required evidence remains failing, skipped, unavailable, or stale after the latest edit.

Never weaken tests, suppress diagnostics, broaden permissions, or delete user work merely to obtain approval.

## 6. Completion gate

After `APPROVED`:

1. Run fresh post-edit gates required by the repository and risk tier.
2. Run GitNexus change detection when available and confirm the affected scope is expected.
3. Check `git diff --check`, final status, and skipped/unavailable tests.
4. Report the Codex verdict separately from implementation readiness.

The final handoff must include:

- Reviewed target and changed-file scope.
- Iteration count and final Codex verdict.
- Findings fixed, rebutted, or still open.
- Commands actually run and observed outcomes.
- Residual limitations and readiness state.

Commit, push, merge, release, or other external mutation only when separately authorized.
