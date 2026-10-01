# Supervisor Report: Tauri Autostart System Integration

Verdict: REJECT

## Metadata

- Report file: `Reports/supervisor/rp_supervisor_260905_134704_by_gpt-5-codex_autostart_system_integration.md`
- Review time: `260905 134704 +07 (Asia/Ho_Chi_Minh)`
- Reviewer: `gpt-5-codex`
- Repo/project: `RustSale_v2`
- Scope reviewed: commit `b1e5c5d4aa011667b0acf17a1f9675714c8ed779` against its parent, current HEAD, associated plan/evidence ledgers, and the user's completion claim
- Claim reviewed: system autostart and the Settings toggle are complete and comprehensively verified on Linux, Windows, and macOS, including quiet startup in the system tray
- Authority used: user request; `AGENTS.md`; `.agents/skills/working-rules/SKILL.md`; `.agents/skills/supervisor/SKILL.md`; Tauri skill and its security/platform references; `docs/IMPLEMENTATION_COMPLETION_HARNESS.md`; committed plan; source; locked dependency source; fresh command output
- Related artifacts: `docs/plans/2026-09-05-autostart-system-integration/*`; no same-scope Playwright/runtime report or OS evidence was found

## Executive Summary

- Problem: determine whether the cross-platform native/OS integration and its UI can be accepted as fully implemented and comprehensively verified.
- Decision: the dependency, plugin registration, capability, service, UI row, rollback-on-error logic, and VI/EN keys are present, and fresh build/static/unit gates complete successfully. Acceptance is nevertheless blocked because this Tier 3 native/OS boundary has no E4 runtime proof, the boot-to-tray path can hide the window after tray setup failed and silently ignores hide failure, production CSP remains disabled, and committed ledgers/test labels materially overstate the evidence.
- Required outcome: repair the startup failure semantics and CSP gate, then produce honest fresh E4 evidence for the real native toggle and boot path on every claimed platform before resubmission.

## Blocking Findings

### [HIGH] No E4 evidence proves the claimed OS integration or quiet boot behavior

File: `src/lib/components/settings/__tests__/NotificationSettings.spec.ts:23`

Issue: both new test suites replace the authoritative boundary with mocks. The component suite is a jsdom component test, despite its `E4 Runtime` names, and the service suite mocks `@tauri-apps/plugin-autostart`. No evidence invokes the packaged Tauri IPC against a real Linux XDG entry, Windows Run key, or macOS LaunchAgent, and no reboot/login or real `--autostart` process proves that the window remains hidden and recoverable from the tray.

Evidence:

- `NotificationSettings.spec.ts:23-29` mocks `isAutostartEnabled`/`setAutostart`; lines 52, 74, 94, 114, and 127 label jsdom cases as E4.
- `autostartService.spec.ts:8-23` mocks all three plugin methods.
- Fresh targeted run: `10 pass`, `0 fail`, `40 expect()` calls, but every component case logged `window.__TAURI_INTERNALS__.transformCallback is not a function`, directly demonstrating that the real Tauri runtime was absent.
- The locked plugin source confirms only structural platform support: `tauri-plugin-autostart-2.5.1/src/lib.rs:5,172-249`; `auto-launch-0.5.0/src/linux.rs:25-76`; `windows.rs:31-83`; `macos.rs:54-180`. Library source is E1, not application runtime proof.
- Repository search found no same-scope reusable Playwright script or official JSON/Markdown runtime evidence under `Reports/qa/playwright`.

Why this blocks acceptance: the Implementation Completion Harness classifies native IPC/external OS integration as Tier 3 and requires production-like E4 verification, direct-entry/failure-path testing, compatibility evidence, and honest disclosure. E2 mocks cannot prove an E4 claim, let alone all three operating systems.

Fix direction: run the packaged app on Linux, Windows, and macOS; exercise enable/query/disable through the real Settings UI and native IPC; inspect the actual OS entry and `--autostart` argument; execute the registered entry and prove hidden-window plus functional tray recovery; cover permission/OS-write failure and rapid/reordered toggles.

Re-review evidence required: reusable runtime procedure/script and durable per-platform evidence showing the real OS artifact before/after toggle, app process arguments, window visibility, tray recovery, error propagation, and cleanup. If any platform is unavailable, report it as an explicit external limitation rather than “verified comprehensively.”

### [HIGH] Autostart can strand the user or visibly pop because tray and hide errors are discarded

File: `src-tauri/src/lib.rs:113`

Issue: `tray::init_tray(app.handle())` is fallible, but its result is discarded. The code then hides the main window on `--autostart` and also discards `window.hide()` failure. If tray creation fails but hide succeeds, the application can remain running without its intended recovery surface. If hide fails, the claimed quiet boot is false. Neither branch is surfaced or tested.

Evidence:

- `src-tauri/src/lib.rs:113-118` discards both results with `let _ = ...`.
- `src-tauri/src/tray.rs:59-145` returns `anyhow::Result<()>` and has multiple fallible menu/tray construction operations.
- The committed R-03 ledger nevertheless says `PROVEN` and lists only “lib.rs logic & cargo check” at `...-plan.md:55`; compiler success cannot exercise either native failure.

Why this blocks acceptance: the invariant is not merely “call hide”; it is “autostart is quiet and the app remains reachable through a working tray.” The authoritative startup path does not fail closed or recover visibly.

Fix direction: make tray readiness authoritative before hiding, propagate/log actionable errors, and keep/show the main window when the tray is unavailable. Treat hide failure as a startup failure that is observable rather than silently continuing.

Re-review evidence required: native tests or injected failure tests for tray initialization and hide failure, plus a real runtime proof that successful `--autostart` remains tray-recoverable.

### [HIGH] Strict production CSP gate is open

File: `src-tauri/tauri.conf.json:21`

Issue: production security configuration is `"csp": null`.

Evidence: `src-tauri/tauri.conf.json:21-23` disables CSP. `AGENTS.md` and the required Tauri skill explicitly require strict CSP before claiming completion at the QA gate.

Why this blocks acceptance: the change grants webview commands that mutate OS autostart state. The repository's Tauri completion rule requires a tight authorization boundary and strict CSP; an open mandatory QA condition forbids a completion claim even though `autostart:default` is correctly limited to window `main` at `src-tauri/capabilities/default.json:3-13`.

Fix direction: define and verify a production CSP compatible with the app and keep development-only allowances separate.

Re-review evidence required: committed strict CSP, production build, and a mounted Tauri runtime smoke proving required app behavior and autostart IPC still work under that policy.

### [MEDIUM] Completion ledger and user-facing evidence claims are inaccurate or misleading

File: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-plan.md:49`

Issue: requirements are marked `PROVEN` without the mandatory `GREEN` ledger state or required evidence strength; E4 naming is attached to mocked tests; test counts are misstated; one skipped test is omitted from the user summary; the component test does not toggle all six switches; and post-commit uncommitted change detection is presented as semantic proof of the commit.

Evidence:

- Plan lines 51-59 use `PROVEN`, whereas the harness permits `UNPROVEN`, `RED`, `GREEN`, or `BLOCKED`; R-03 uses code/compiler evidence for runtime behavior, and R-04 claims the OS registry is toggled without native evidence.
- Evidence ledger lines 90-100 declares all requirements proven and reports `4,842` assertions. Fresh full run observed `687 pass`, `1 skip`, `0 fail`, and `4,843 expect()` calls across 688 tests.
- `NotificationSettings.spec.ts:131-144` counts six switches but clicks/asserts only the first three, contradicting the claim that all six toggles were exercised.
- Fresh `bun run check` observed `0 errors and 6 warnings in 3 files`; fresh build exited 0 with the same six Svelte warnings plus a chunk larger than 500 kB. These are successful gates with warnings, not warning-free proof.
- Fresh Anvien compare from `b1e5c5d^` to HEAD found 16 changed files and 95 changed symbols with low graph risk. The ledger's `--scope all` result of `affected_count: 0` describes no uncommitted affected flow after commit; it does not prove that the committed feature has no impact or is correct.
- User citations are stale: current relevant lines are `src-tauri/capabilities/default.json:13` and `src-tauri/src/lib.rs:106-118`, not `default.json:40` and `lib.rs:85-95`.

Why this blocks acceptance: the Completion Lock requires every row to be GREEN, skipped checks to be disclosed, evidence labels to match their ladder level, and final claims to contain only exact observed results. Current artifacts assert stronger closure than their evidence supports.

Fix direction: downgrade mocked tests to E2, correct counts/skip/warnings/line references, use compare-scope change detection for the commit, reopen all unproven ledger rows, and update completion status only after the required runtime evidence exists.

Re-review evidence required: corrected plan/actual-status/evidence ledgers with harness-valid states, exact fresh command results, honest E-levels, and links to native runtime evidence.

## Source-Level Clearance Notes

- `package.json`, `bun.lock`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`: clear structurally — frontend and Rust plugin dependencies resolve to `2.5.1`.
- `src-tauri/capabilities/default.json`: clear for the requested plugin surface — `autostart:default` is scoped to the `main` window and grants enable, disable, and is-enabled; overall security clearance remains blocked by null CSP.
- `src-tauri/src/lib.rs` plus `src-tauri/src/tray.rs`: blocked — plugin and `--autostart` wiring exist, but tray/hide failure handling does not enforce quiet-and-recoverable startup.
- `src/lib/services/autostartService.ts`: clear at E1/E2 — query failure returns false and mutation failures are rethrown; the real plugin boundary remains unverified.
- `src/lib/components/settings/NotificationSettings.svelte`: clear at source level for initial query, optimistic toggle, rollback, error toast, switch/badge, and service call (`:46-47`, `:76-90`, `:130-139`, `:317-346`); rapid overlapping toggles and native behavior remain unverified.
- `messages/en.json`, `messages/vi.json`: clear structurally — four keys per locale compile. The exact committed description copy differs from the wording quoted in the user's claim but preserves the same meaning.
- Test files: blocked as acceptance evidence beyond E2 — plugin/service boundaries are mocked and E4 labels are false.
- Plan/status/evidence/benchmark files: blocked — they close the ledger without native evidence and include inaccurate/misleading results.

## Evidence Checked

Passed:

- Source and full commit diff inspection; `git diff --check b1e5c5d^ b1e5c5d` exited 0.
- `bun run build` exited 0; observed six Svelte warnings and a >500 kB chunk warning.
- `bun run check` exited 0; observed `0 errors and 6 warnings in 3 files`.
- `cargo check` exited 0 with no Rust warning in the fresh output.
- Focused suites with browser/Svelte conditions: `10 pass`, `0 fail`, `40 expect()` calls; real Tauri callback missing in all four component cases.
- Full frontend suite with browser/Svelte conditions: `687 pass`, `1 skip`, `0 fail`, `4,843 expect()` calls across 688 tests and 74 files.
- Anvien graph refreshed at exact HEAD `b1e5c5d`; file detail identified `src-tauri/src/lib.rs` as high file-level topology risk; compare-scope change detection reported low overall graph risk, 16 changed files, 95 changed symbols, and zero resolved affected flows.
- Local locked dependency source confirms official plugin command wiring and Linux/Windows/macOS implementation code.
- Verification freshness: current — all listed command observations were produced during this review after confirming HEAD.

Failed:

- Completion Lock: E4 native/OS behavior, direct-entry/failure behavior, compatibility, strict CSP, honest GREEN ledger, and full disclosure are open.
- `--autostart` tray availability invariant: source admits a tray-failure/hide-success path and a silent hide-failure path.
- Evidence labeling/count accuracy: mocked tests are labeled E4, assertion count differs by one, skip is omitted from the user claim, and six-toggle coverage is overstated.

Not run:

- Real packaged Tauri runtime, actual system-login launch, actual OS entry mutation, and UI/tray interaction on Linux, Windows, or macOS: no durable evidence/procedure was supplied, and this review did not mutate the review machine's startup configuration.
- `cargo test`, `cargo clippy`, signing/notarization, installer, and release publication: not needed to establish this rejection and not used as acceptance evidence.

## Invariant Closure

- Affected invariant: enabling autostart through the authorized Settings UI must persist the correct native OS entry; OS launch must pass `--autostart`, remain non-disruptive, and keep the application reachable through a functioning tray; failures must be reported without false state or user stranding.
- Sibling surfaces checked: dependencies/locks, plugin builder and locked platform implementation, generated permission set, main-window capability, Tauri config/CSP, startup setup, tray implementation, service wrapper, Settings component, VI/EN messages, focused tests, full frontend suite, plan/status/evidence ledgers, and Anvien compare/file topology.
- Residual unverified same-invariant surfaces: packaged behavior on Linux/Windows/macOS; boot/login manager semantics; tray visibility/recovery; OS permission and write failures; hide failure; concurrent/reordered toggle calls; signing/distribution compatibility.

## Required Fix List For Resubmission

1. Make `--autostart` hide conditional on successful tray initialization and handle/report both tray and hide failures so the user cannot be stranded or unexpectedly shown a boot window.
2. Enforce the repository-required strict production CSP and verify the mounted Tauri app still functions under it.
3. Add honest E2 failure/race coverage and real E4 per-platform evidence for enable/query/disable, OS artifact contents, launch arguments, quiet startup, tray recovery, and failure propagation.
4. Correct test names, counts, skipped-test disclosure, six-switch coverage statement, stale line references, Anvien evidence interpretation, and all plan/evidence statuses; do not mark completion until every applicable Completion Lock item is closed.

## Overall Evaluation

The implementation has a credible structural foundation and its compile/unit results are useful. It is not acceptable as “completed and comprehensively verified”: the most important native behavior is untested at the required evidence level, the startup recovery invariant is not enforced, a mandatory CSP gate is open, and the durable evidence overstates what was actually proved.
