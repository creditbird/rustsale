# Supervisor Review: Autostart Cleanup Resubmission

Verdict: **REJECT**

## Metadata

- Reviewed commit: `89e20d232b913df9cd00f24ede9bdea804728c8c` against `896a755`.
- Review date: 2026-09-06, Asia/Ho_Chi_Minh (+07).
- Reviewer: gpt-6.
- Scope: claimed closure of the four prior blockers, including automatic XDG/panic/concurrency coverage and conditional readiness. This is not a review of a claim to have physically tested Windows/macOS.
- Authority: user request, AGENTS.md, working-rules, supervisor, Tauri skill, and Implementation Completion Harness sections 6, 9, 11–13.
- Prior report: [896a755 review](rp_supervisor_260906_072548_by_gpt-6_autostart_four_blockers.md).
- Evidence: [autostart_89e20d2_review](evidence/autostart_89e20d2_review).
- No application, committed test, or plan file was changed. The review added only its report, logs and an isolated diagnostic runner. No commit or release was performed.

## Executive Summary

The original XDG path/cleanup failure is fixed in the reviewed implementation. Fresh execution with an externally configured non-default XDG path passes all four tests, preserves a pre-existing fixture, and leaves no test entries. Formatting is fixed, E2 rollback remains passing, CSP wording is improved, and full build/check/test totals reproduce the latest report.

The remaining rejection concerns two local test-evidence gaps: the named XDG regression test does not establish a non-default XDG environment itself, and the panic test can pass when fixture creation fails before the intended panic. These can be corrected on the current host; they are not unavailable-platform limitations. Missing OS-login/Windows/macOS evidence is now disclosed and is not reused here as a demand for unconditional platform verification before a conditional handoff.

## Blocking Findings

### [MEDIUM] The automatic XDG test does not establish its claimed environmental precondition

File: `src-tauri/tests/native_autostart_linux_test.rs:102`.

`test_linux_native_autostart_non_default_xdg_alignment` creates `temp_xdg` at line 107, then uses it only in `assert_ne!` at lines 115–118. It never configures XDG_CONFIG_HOME, launches a child with that environment, or verifies that the process's actual XDG setting differs from the default. The temporary directory has no effect on the AutoLaunch invocation.

Consequently, ordinary `cargo test` can pass this named regression case without exercising the original divergence. On a default-config host, reverting the helper to `dirs::config_dir().join("autostart")` would not be challenged by this test's purported non-default setup.

The externally configured command documented by the implementer is meaningful, and this review independently confirms it works. This closes the current path defect, but does not make the standalone test self-contained or validate the ledger's description of what that automatic case sets up (`...-evidence.md`, E3-P3A-LINUX-AUTOLAUNCH item 2; `...-actual-status.md`, R7 native findings).

Required fix: establish the environment in a child process or a maintained test runner that standard validation invokes. Assert the actual non-default environment before exercising the crate. Avoid process-global environment mutation racing other Rust tests. Alternatively, accurately separate the path-alignment unit case from the externally configured integration check and make that external check a durable required gate.

Re-review evidence: the automated gate establishes non-default XDG, passes the corrected helper, fails a deliberate return to the old config-dir helper, and preserves existing entries without leftovers.

### [MEDIUM] Panic cleanup test passes when it never creates the file or reaches the intended panic

File: `src-tauri/tests/native_autostart_linux_test.rs:151`.

Both fixture setup operations are inside `catch_unwind`: `create_dir_all(...).unwrap()` at line 153 and `write(...).unwrap()` at line 154. The outer assertions at lines 162–165 accept any caught panic plus an absent file. They do not distinguish the intentional panic at line 159 from a setup failure.

Fresh adversarial reproduction used the freshly built test binary in a bubblewrap mount namespace. A repository-local fixture was mounted at the child's normal config path, first writable and then read-only. The host's real config was not altered by these probes.

| Case | Result | Intended panic reached? |
|---|---|---|
| Writable config, exact panic test | 1 passed, exit 0 | Yes; expected panic message logged |
| Read-only config, exact panic test | **1 passed, exit 0** | **No; write failed at line 154 with OS error 30, ReadOnlyFilesystem** |

The read-only case has no newly created file to delete, yet the test reports cleanup success. This does not demonstrate a new production cleanup failure; it demonstrates a false-positive path in the submitted proof.

Evidence: [read-only log](evidence/autostart_89e20d2_review/probe-panic_readonly.log), [writable log](evidence/autostart_89e20d2_review/probe-panic_writable.log), [probe matrix](evidence/autostart_89e20d2_review/native-probes.json), [reusable probe](evidence/autostart_89e20d2_review/native_probe.py).

Required fix: make setup failure fail the test. Establish/check the fixture outside the caught-panic boundary, or independently assert that fixture creation and the intentional panic point were reached and inspect the caught payload. Retain safe cleanup if setup or assertions fail.

Re-review evidence: writable setup plus intended panic passes and removes the owned entry; failed setup cannot be reported as successful panic-cleanup verification; removing cleanup makes the valid-fixture case fail.

## Four-Blocker Disposition

| Prior area | Current finding |
|---|---|
| Native path and entry isolation | Original defect closed in fresh default/non-default runs. PID/prefix/timestamp names replace the old fixed name, and the explicit pre-existing-file deletion is removed. Submitted automatic evidence still needs the two fixes above. |
| Rollback and toast | Preserved at E2: DOM aria-checked and localized toast assertion pass; no transformCallback error in focused output. |
| CSP explanation | Preserved/improved: source is unchanged, production script-src remains self-only, broad HTTPS/WSS and lack of absolute injection immunity are acknowledged. |
| Gates, formatting and status | New-file rustfmt and diff checks pass; suite totals match. Conditional readiness is a better representation of platform gaps, but does not excuse local test-evidence gaps. |

## Source-Level Clearance Notes

- Latest commit changes exactly the native test and four ledger files. No production Rust, frontend, dependency, capability, or CSP change is included.
- `get_auto_launch_autostart_dir`, lines 5–13: matches locked auto-launch 0.5.0's home/.config/autostart resolution. Confirmed against the locked crate's `src/linux.rs:81`.
- `unique_test_app_name`, lines 17–27: separates the exercised test/process/thread entries. The observed isolation tests pass. Timestamp/PID naming should be described as the implemented isolation strategy rather than an absolute guarantee against every possible pre-existing name.
- `CleanupGuard`, lines 31–41: points to the correct path. The writable panic probe verifies removal after the intended unwind; arbitrary abort/SIGKILL limitations are now acknowledged. The false-positive finding is in the test's precondition handling.
- Lifecycle and four-thread case: real AutoLaunch API calls remain present. Both default and externally configured XDG suite runs pass with a preserved sentinel and no residual test entries.
- Frontend component/service, plugin/capability and CSP: unchanged from the prior source review. Fresh focused/full suites confirm their tested behavior is preserved, not that native IPC/OS login has been exercised.
- Ledgers: plan metadata, actual-status and evidence now communicate conditional readiness. Benchmark has no explicit readiness field, so the literal claim that all four files contain that status is overstated. R-03 still marks the combined runtime invariant GREEN with a unit-only qualifier; split local unit proof from pending native evidence using the harness's GREEN/BLOCKED row statuses. R-07 uses overall readiness text as a row status. These notes should be corrected with the evidence updates.
- Environmental explanation: this review observed DISPLAY=:0.0, XDG_SESSION_TYPE=x11 and a successful XOpenDisplay call. A blanket assertion that this host has no active display is not established. This does not prove OS-login test access or replace native QA; record actual missing tooling/access separately from unexecuted work.

## Evidence Checked

All commands below were observed at unchanged HEAD 89e20d2 after source inspection. Full build preceded executable test validation.

| Check | Fresh result | Scope proved |
|---|---|---|
| `bun --bun tauri build --no-bundle` | Exit 0; release Rust stage 7m01s | Full frontend/sidecar/native production build |
| `bun run check` | Exit 0; 0 errors, 6 warnings in 3 files | Typecheck and i18n compilation |
| `cargo check` | Exit 0; no warnings/errors; 4.99s | Native compilation |
| `cargo test` | Exit 0; **871 passed, 1 ignored, 0 failed** | 866 library passes + 4 native passes + 1 updater pass |
| `cargo test --test native_autostart_linux_test` | Exit 0; **4 passed** | Current default-environment suite |
| Fresh native binary, isolated default config | Exit 0; **4 passed**, sentinel unchanged, no test entries left | Normal path/isolation |
| Fresh native binary, isolated non-default XDG | Exit 0; **4 passed**, sentinel unchanged, no test entries left | Current XDG fix works when the caller really supplies that environment |
| Exact panic test, writable config | Exit 0; **1 passed**, intentional panic observed | Guard cleanup in valid-fixture case |
| Exact panic test, read-only config | Exit 0; **1 passed despite fixture-write failure** | False-positive counterexample |
| Focused Bun component/service suites | Exit 0; **11 passed, 46 assertions** | E2 preservation; no transformCallback error |
| `bun test` | Exit 0; **688 passed, 1 skipped, 0 failed; 4,848 assertions; 689 tests/74 files** | Full frontend result matches current claim |
| Isolated native test rustfmt | Exit 0 | Previous in-scope formatting defect closed |
| `git diff --check 896a755..HEAD` | Exit 0 | Latest remediation whitespace hygiene |

Ignored Rust test: live Zalo conversation probe requiring credentials. Skipped Bun test: CloakBrowser download/reuse/launch/cleanup smoke. Expected panic messages in the writable probe are not test failures. The read-only probe's green exit is recorded as defective evidence, not a successful cleanup proof.

User-config preservation: the three existing host .desktop files were unchanged by name/content hash after the full Rust suite; no additional entry was present. Probe-only fixtures and temporary directories were inside the repository and removed after execution. Raw results are retained in the evidence directory.

Anvien: refreshed at current HEAD; 626 scanned files, 410 parsed code files, zero parse failures, 92,352 nodes and 106,382 relationships. Compare scope reports 5 files/124 changed symbols, low aggregate risk, affected_count 0; native-test file topology risk is HIGH with 77 unresolved references. Scope-all reports risk **none**, not low, because there are no tracked/indexed pending changes. The implementer's earlier low value may have been pre-commit; it is not the current scope-all result. Full graph artifacts are retained. This is scope evidence, not runtime correctness proof.

Memory: agentmemory contains the remediation note. No mem0 tool was exposed, so the separate mem0 update claim was not independently verified.

Not run: actual app IPC toggle, actual OS-login/logout, tray recovery/failure injection, Windows/macOS runtime, signing/installer/release, full repository rustfmt, or a modified-source mutation run. The isolated new-file formatting gate was rerun; unrelated formatting debt from the prior review was not attributed to this commit.

## Invariant Closure and Required Next Step

The old Linux assertion/cleanup mismatch is closed in current execution. E2 rollback and CSP documentation remain acceptable. Per-platform app/OS integration remains explicitly deferred; it is not presented here as verified.

Before accepting the submitted native-test closure claim:

1. Make the XDG regression environment an enforced part of reusable automatic validation and accurately document the owning gate.
2. Make fixture/setup failure fail the panic-cleanup test and verify the intended panic path.
3. Refresh the associated evidence and distinguish locally proven rows from pending native runtime rows. Retain conditional overall readiness for specific external gaps once local requirements are satisfied.

This review rejects the remaining evidence gaps, while explicitly recognizing that the original XDG defect, formatting, ordinary cleanup and reported suite counts now verify successfully.
