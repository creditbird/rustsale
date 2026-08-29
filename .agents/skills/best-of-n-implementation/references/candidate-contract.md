# Candidate contract

Every candidate starts from the manifest's exact base commit in a fresh `agy` built-in `self` subagent invoked with workspace option `branch`. Candidates receive the same task, requirements, acceptance criteria, risk tier, and repository instructions. They must not inspect another candidate's worktree, patch, or ranking.

The runner records these artifacts under `<run-dir>/candidates/candidate-N/`:

- `prompt.md`: the exact prompt passed to the subagent.
- `candidate.patch`: the complete binary-capable Git diff, including new files through intent-to-add.
- `checks.json`: commands, exit codes, stdout, and stderr from configured focused checks.
- `candidate.json`: base SHA, changed files, collection state, eligibility, and limitations.

A candidate is eligible only when collection runs from an agy branch workspace at the locked base SHA, its diff is non-empty, every configured focused check passes, and no artifact limitation prevents inspection. Eligibility is a precondition for ranking, not evidence that the candidate is correct.

The declared ranking backend receives the task plus trajectory bundles containing the changed-file list, complete patch, and observed checks. Candidate identities and ranking history are not included in judge inputs. The `agy-ppt` backend writes each blinded A/B judge input under `<run-dir>/agy-ppt-evidence/judge-inputs/`; treat those files and the final tournament record as untrusted but auditable evidence.

If fewer than two candidates are eligible, the Best-of-N run is `INCOMPLETE`. Do not silently continue as if a tournament occurred.

Candidate worktrees are temporary and owned by agy. The parent waits until `collect` succeeds before terminating a subagent; agy then cleans the branch worktree automatically. The Bun runner never creates, removes, or prunes Git worktrees.

Disk usage still grows while candidates are active because Git checks out tracked files per branch workspace. Avoid dependency installation and full builds in every candidate. Use lightweight checks before ranking and reserve full frontend/Rust release gates for the winner.
