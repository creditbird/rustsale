# CI gates for NSH v3 (GitHub Actions)

Workflow: `.github/workflows/nsh-v3-gates.yml`. It runs the v3 gates on GitHub runners (Ubuntu 24.04), so you do not need
`heavy-job` on the shared dev machine just to know whether a branch is green.

## When it runs

- push to `feat/nsh-v3*` (and to `chore/nsh-v3-ci-gates`)
- pull request into `feat/nsh-v3*`
- manually: Actions tab > "NSH v3 gates" > Run workflow, or `gh workflow run nsh-v3-gates.yml --ref <branch>`

A newer push to the same branch cancels the older run. No secrets are used. `RUSTSALE_DEMO=1` is set only in the `app` job.

## What each job runs

| Job | Commands |
|---|---|
| `core` | `cargo fmt -p rustsale-core --check`, `cargo clippy -p rustsale-core --all-targets -- -D warnings`, `cargo test -p rustsale-core --lib` |
| `native` | in `native/`: `cargo clippy --all-targets -- -D warnings`, `cargo test`; also checks `gpui-fast =0.1.17` is still pinned in `native/Cargo.toml` |
| `app` | in `app/` with `RUSTSALE_DEMO=1`: `nsh check`, `nsh test` |
| `gate summary` | table of the three results and their durations (Summary page of the run); fails if any gate failed |

Toolchain: Rust 1.98.1 (same as the .deb container), nsh v0.6.1. Cargo caches (`Swatinem/rust-cache`) and the nsh
cache are keyed per job; the first run on a new branch family is cold and slower, later runs are warm.

## How to read the result

1. PR: the checks list at the bottom of the PR shows `core`, `native`, `app`, `gate summary`. Green on all four = gates pass.
2. Branch push: `gh run list -R creditbird/rustsale --workflow nsh-v3-gates.yml --branch <branch> --limit 3`
3. One run: `gh run view <run-id> -R creditbird/rustsale` (jobs and steps) or open the run page and read **Summary** (durations table).
4. A failed step: `gh run view <run-id> -R creditbird/rustsale --log-failed | tail -80`. The failed step name tells you which gate:
   - `cargo fmt`: run `cargo fmt -p rustsale-core` and commit.
   - `cargo clippy`: the warning is an error (`-D warnings`); fix the lint, do not `#[allow]` it without a reason.
   - `nsh check`: a TypeScript or `cargo check` error of the native crate.
   - `nsh test`: a failing `*.test.tsx` or native `cargo test`; the log names the test.
5. Re-run only the failed job: `gh run rerun <run-id> -R creditbird/rustsale --failed`.

## When to still run locally

- Reproducing a failure you cannot understand from the log: run the one failing command through `heavy-job`, not the whole gate.
- Screens needing a real display, scrolling, cold start, packaging: not covered here (issue #59, `package-linux.yml`).

## Notes

- The workflow trusts the pin in `native/Cargo.toml` (`gpui-fast =0.1.17`, under `cfg(any())`); remove both together when
  nsh-platform-gpui is fixed for newer gpui-fast.
- To add a gate, add a step to the right job; keep it free of secrets and network side effects.
