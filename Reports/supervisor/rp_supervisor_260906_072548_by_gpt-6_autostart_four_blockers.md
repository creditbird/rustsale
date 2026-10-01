# Supervisor Review: Autostart — Four-Blocker Resubmission

Verdict: **REJECT**

## Metadata

- Review completed: 2026-09-06, Asia/Ho_Chi_Minh (+07).
- Reviewer: gpt-6.
- Repository: RustSale_v2.
- Reviewed HEAD: `896a7559c633499b46d72b37c563f24d95893004`.
- Scope: the user's four-blocker closure claim; latest remediation against `b714d25`; cumulative remediation against `b1e5c5d`; all four companion ledgers.
- Authority: user request, AGENTS.md, working-rules, supervisor, Tauri skill, and `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`, especially evidence ladder and Completion Lock.
- History checked: supervisor reports `rp_supervisor_260905_134704_by_gpt-5-codex_autostart_system_integration.md` and `rp_supervisor_260905_155816_by_gpt-5-codex_autostart_system_integration_rereview.md`.
- Evidence directory: [autostart_896a755_review](evidence/autostart_896a755_review).
- Review only: no application, test, plan, or capability edits; no commit or publication. Review generated this report and evidence. Pre-existing untracked work was preserved.

## Executive Summary

The real Linux crate lifecycle and the improved E2 rollback test are present. The CSP origin explanation now acknowledges scheme-wide access. Normal-environment build/check/test results are broadly reproducible.

Acceptance remains blocked by a reproduced cleanup defect, missing application/native evidence despite Complete/all-GREEN ledgers, and formatting failure in the newly added native test. The test is not hermetic: with a valid non-default `XDG_CONFIG_HOME`, it panics and leaves an enabled test entry in the user's actual startup directory.

## Disposition of the Four Submitted Blockers

| Blocker | Current disposition | Evidence |
|---|---|---|
| 1 — Native Linux/platform boundary/RAII | Partially addressed; remains open | Calls locked `auto-launch 0.5.0` and passes on the default config path. Non-default XDG reproduction fails and leaves a startup entry. No real app IPC/login/tray evidence closes the broader requirement. |
| 2 — Rendered rollback/error toast | Addressed at E2 | Actual switch `aria-checked` and localized toast are asserted; focused suites pass; no `transformCallback` error. The rollback case specifically uses Vietnamese; VI/EN rendering is covered separately. |
| 3 — CSP/origin explanation | Prior documentation blocker addressed | Production scripts exclude unsafe-inline/eval. Evidence explicitly acknowledges broad HTTPS/WSS. This does not establish immunity to every injection attack or prove mounted-app compatibility. |
| 4 — Diff/counts/ledgers | Partially addressed | Diff whitespace check passes; test totals and inventory match except fresh full Bun assertion count is 4,848. Complete/all-GREEN statuses still overstate readiness; new test fails rustfmt. |

## Blocking Findings

### [HIGH] Cleanup targets a different directory from the real crate under non-default XDG

File: `src-tauri/tests/native_autostart_linux_test.rs:5`.

- Lines 5–8 resolve the asserted/cleanup path through `dirs::config_dir()`.
- Locked `auto-launch-0.5.0/src/linux.rs:81` resolves its actual directory through `dirs::home_dir().unwrap().join(".config").join("autostart")`; it does not use XDG_CONFIG_HOME.
- `dirs` honors an absolute XDG_CONFIG_HOME. Therefore these paths diverge on a supported environment configuration.
- Lines 13–14 remove only the asserted path and ignore removal errors.
- Lines 19–21 also delete a pre-existing fixed-name file. The test retains a shared, user-scoped name rather than owning an isolated fixture; concurrent invocations can interfere.

Fresh counterexample after the full build and ordinary Rust suite:

1. Confirmed the real fixed-name startup entry did not exist, preserving any pre-existing entry by refusing the probe otherwise.
2. Ran the unchanged test with XDG_CONFIG_HOME pointing to a fresh directory inside the repository.
3. Observed exit **101**, panic at line **37**, and **one failed test**.
4. Observed `~/.config/autostart/rustsale-crm-test-autostart-integration.desktop` still present after the test process exited. It contained `Exec=/usr/bin/rustsale-crm --autostart`.
5. The review's separate cleanup removed only that newly created, content-checked entry. Confirmed it was absent afterward; the temporary directory was also removed.

Evidence: [native-xdg.log](evidence/autostart_896a755_review/native-xdg.log), [native-xdg.json](evidence/autostart_896a755_review/native-xdg.json). The wrapper's successful exit records completion of the investigation; the child test's failure is explicitly recorded as 101.

Why this blocks: the assertion that panic-safe RAII guarantees hermetic cleanup is directly contradicted by execution. The standard full test command can leave a startup entry behind on a non-default XDG host.

Required fix: isolate the test's actual OS side effects, use a unique owned entry, align assertions and cleanup with the locked crate's real path behavior, and preserve pre-existing state. Verify ordinary completion, injected panic, non-default XDG, and concurrent runs. Do not claim guaranteed cleanup on arbitrary process interruption: Rust does not run destructors on termination without unwinding. See [Rust Reference: process termination](https://doc.rust-lang.org/reference/destructors.html#process-termination-without-unwinding).

### [HIGH] Environmental disclosure does not close the application/native requirement

Files: `...-plan.md:6`, `...-plan.md:55`, `...-actual-status.md:5`, `...-evidence.md:118` in `docs/plans/2026-09-05-autostart-system-integration/`.

The plan still says `completed`, actual status says `Complete`, and all R-01–R-07 are GREEN. The goal remains Linux/Windows/macOS autostart with real OS state reflected in Settings and recoverable background startup.

The new integration test constructs an independent AutoLaunch instance with a fixed test name and hard-coded `/usr/bin/rustsale-crm` path (`native_autostart_linux_test.rs:25`). It never invokes RustSale's registered Tauri plugin, frontend IPC, packaged executable resolution, OS-login manager, or tray/window runtime. This is useful library integration evidence; changing its ID to E3 correctly narrows the label but does not supply the missing evidence.

The real plugin's setup derives the application name/current executable and has a distinct Linux AppImage path branch. Its three commands and OS implementation are structurally supported, as confirmed in locked source and [official autostart documentation](https://v2.tauri.app/plugin/autostart/). That support does not test RustSale's configuration or installed behavior on Windows/macOS.

The four Rust decision tests cover a pure enum-selection function. They do not exercise actual `tray::init_tray`, `window.hide`, recovery by tray click, or fallback window calls at `src-tauri/src/lib.rs:137`.

Why this blocks: the prior native/runtime finding and Tier 3 Completion Lock remain open. Acknowledging unavailable platforms is appropriate, but must affect readiness status. Redefining R-07 around a narrower Linux library test cannot waive the remaining native/UI acceptance requirements.

Required evidence: real application enable/query/disable through native IPC, the actual registered executable/arguments, launch through the registered entry/login mechanism, visible verification of tray recovery and startup failure handling. Supply per-platform evidence for verified claims. Where only an external platform limitation remains, record affected requirement IDs and the exact follow-up, and use `CONDITIONALLY READY` rather than Complete. Current failing cleanup/formatting gates still require remediation first.

### [MEDIUM] New native test does not pass the formatting gate

File: `src-tauri/tests/native_autostart_linux_test.rs:32`.

- `rustfmt --check --edition 2021 src-tauri/tests/native_autostart_linux_test.rs` exits **1**, with formatting differences around the new assertions and enable/disable calls.
- `cargo fmt --check` also exits **1**. Its broader log contains unrelated files; those differences are not attributed to this remediation. The isolated new-file check proves an in-scope failure independently.
- `git diff --check b1e5c5d..HEAD` exits **0**. Whitespace hygiene and Rust formatting are different checks.

Evidence: [native-rustfmt.log](evidence/autostart_896a755_review/native-rustfmt.log), [cargo-fmt.log](evidence/autostart_896a755_review/cargo-fmt.log).

Required fix: format the in-scope test and rerun its formatting check. Record broader baseline formatting debt separately without rewriting unrelated files.

## Source-Level Clearance Notes

- Dependencies/locks: the added dev dependency is real `auto-launch = "0.5"`; Cargo.lock resolves the same 0.5.0 used by tauri-plugin-autostart 2.5.1. Structural claim confirmed.
- `src-tauri/src/lib.rs`: plugin/argument wiring and tray-readiness decision remain present; the latest commit removes the extra EOF line. Runtime assurance remains unproven as described above.
- `src-tauri/capabilities/default.json`: autostart permission is scoped to `main`; eight permission entries, with seven registered plugins overall. No new remote capability is granted by the CSP change.
- `src-tauri/tauri.conf.json:22`: production `script-src 'self'`; development allowances are separated. HTTPS/WSS scheme-wide network allowances are accurately described in the revised evidence. `supabaseClient.ts:12` confirms configurable URLs. `imageExport.ts` uses native IPC in desktop mode and fetch in its browser fallback, so it alone is not proof that desktop image export requires broad webview connect-src.
- `autostartService.ts` and `NotificationSettings.svelte:130`: query fallback, optimistic state, mutation rejection, rollback and toast are present. E2 tests now observe the relevant rendered state. Native read failure, concurrent toggles, and mounted runtime are not established by this test.
- VI/EN messages: all four keys are present per locale; compilation succeeds. Settings has six switch controls.
- Native test: real library calls are confirmed; isolation/cleanup remains defective.
- Ledgers: historical P0 is now separated from R6, and ignored/skipped tests are disclosed. Readiness remains overstated. Evidence line 103 still names `rustsale-crm-test-autostart.desktop`, while source uses `rustsale-crm-test-autostart-integration.desktop`. Benchmark inventory values match reality, although that file was not changed by the latest commit.

## Evidence Checked

| Command/check | Observed result | What it proves |
|---|---|---|
| `bun tauri build --no-bundle` | Build log completes with release binary; Rust stage 14m09s | Frontend, sidecar and native production build. Tool session expired across the user pause, so final process exit code was not recovered; completed log and artifact were checked on resume. |
| `bun run check` | Exit 0; 0 errors, 6 warnings in 3 files | Current type/compiler checks and i18n compile |
| `cargo check` | Exit 0; no warnings/errors; 6.22s | Current native compilation |
| `cargo test` | Exit 0; 868 passed, 1 ignored, 0 failed | 866 library passes plus native/updater integration passes |
| `cargo test --test native_autostart_linux_test` | Exit 0; 1 passed on default XDG path | Real crate lifecycle on this host's default directory |
| Same native test with non-default XDG | Exit 101; 1 failed; leftover real startup file | Cleanup/path counterexample |
| Focused component/service Bun suites | Exit 0; 11 passed, 46 assertions; no transformCallback error | E2 state/error behavior |
| `bun test` | Exit 0; 688 passed, 1 skipped, 0 failed; **4,848 assertions**, 689 tests/74 files | Fresh full-suite result; the claimed 4,849 was not reproduced in this run |
| `git diff --check b1e5c5d..HEAD` | Exit 0, including after resumption | Cumulative whitespace defect closed |
| Isolated rustfmt / full cargo fmt | Both exit 1 | Formatting gate remains open |
| Inventory | 7 plugins, 8 capability entries, 6 switches, 4 keys per locale | Counts match benchmark ledger |

Skipped: CloakBrowser download/reuse/launch/cleanup smoke. Ignored: `conversation::zalo::probe_test::live_zalo_conversation_probe`, requiring explicit live credentials. The assertion-count difference is reported as an observation from this run, not proof that an earlier run could not have had 4,849.

Graph evidence gathered at this unchanged HEAD: Anvien refresh scanned 599 files, parsed 410 code files with zero parse failures, and produced 92,133 nodes/106,136 relationships. Compare scope against b1e5c5d reported nine files, 89 changed symbols, low aggregate risk, and no resolved affected processes; `lib.rs` file risk was HIGH with 272 unresolved references. Complete graph details are retained in the evidence directory; zero resolved flows is not proof of runtime correctness. Scope-all reported no uncommitted indexed changes. Git status still contains untracked Reports and unrelated documentation, so “clean worktree” is not literally established by scope-all. GitNexus registry was stale and was not used as current code evidence; no implementation edit/commit required a new pre-change impact gate.

Freshness: HEAD was rechecked after the overnight pause and remained 896a755; no tracked source diff appeared. Build evidence was generated earlier in this same review; all reported check/test and XDG results were obtained after resumption. Agentmemory was queried for prior findings; no mem0 tool was exposed in this session.

Not run: mounted browser/Tauri UI, actual OS-login, Windows/macOS execution, tray interaction/failure injection, installer/signing/publication, and a mutation test of UI rollback. None is represented by the green E2/E3 results. No new E4 screenshot or Playwright artifact was produced.

## Invariant Closure and Required Resubmission

The invariant spans Settings state, native authorization, OS registration, login arguments, background visibility, and tray recovery. Source, mocks, locked plugin implementation, native crate lifecycle, companion ledgers and graph scope were checked. Real app/OS boundaries and their failure paths remain unverified.

1. Fix native test isolation and path alignment; prove non-default XDG, panic cleanup, ownership preservation and concurrency behavior.
2. Supply native application/runtime evidence and synchronize readiness with genuinely unavailable platforms; keep unresolved rows open.
3. Format the new test; refresh relevant gates after its final edit.
4. Correct the evidence filename and record test counts with their run/log; remove unsupported Complete/all-GREEN and absolute cleanup/security claims.

The resubmission improves the original work and closes the narrow E2 rollback and CSP documentation findings. It does not yet close all four blockers or support implementation completion.
