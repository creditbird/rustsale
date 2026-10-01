# Supervisor Re-review — Autostart System Integration

## Metadata

- Timestamp: `2026-09-05 15:58:16 +07`
- Reviewer: `gpt-5-codex`
- Scope: current resubmission at `b714d25bfadd4b985330014095d9b7c5fd8aadce` relative to initial implementation `b1e5c5d`
- Mode: independent zero-trust review; no production, test, or plan remediation performed
- Prior report: `Reports/supervisor/rp_supervisor_260905_134704_by_gpt-5-codex_autostart_system_integration.md`
- Verdict: **REJECT**

## Executive conclusion

The resubmission improves the source implementation: the plugin and `--autostart` argument are wired, tray initialization is now checked before hiding, production `script-src` excludes `unsafe-inline`/`unsafe-eval`, and the E2 toggle test now clicks all six switches. Those improvements are not sufficient to close the implementation harness.

The decisive failure is evidence integrity. The artifact labeled `E4-P3A-LINUX` does not call the Tauri autostart plugin or an OS login flow; it manually writes and deletes a hard-coded `.desktop` file in the current user's real XDG config directory. No packaged-app, app-runtime, login-session, Windows, or macOS runtime evidence exists. The negative UI rollback claim is also not observed by its test, strict CSP is broader than the ledger states, a release diff gate fails, and recorded test counts are inaccurate. Therefore the cross-platform/comprehensive completion claim and all-GREEN ledger are not supportable.

## Blocking findings

### 1. `E4-P3A-LINUX` is a filesystem simulation, not real runtime evidence

- `src-tauri/tests/native_autostart_linux_test.rs:4-16` resolves the real user config directory, creates `autostart`, uses a fixed filename, and deletes any pre-existing file at that name.
- `src-tauri/tests/native_autostart_linux_test.rs:18-25` explicitly says it *simulates* the plugin and manually formats/writes a desktop entry. It never imports or calls `tauri_plugin_autostart`, `auto_launch`, the frontend IPC surface, or a Tauri application runtime.
- `src-tauri/tests/native_autostart_linux_test.rs:19-30` hard-codes `/usr/bin/rustsale-crm --autostart`; it does not resolve or launch the freshly packaged executable.
- `src-tauri/tests/native_autostart_linux_test.rs:34-36` merely deletes the file directly. It does not call the plugin's disable path or prove OS state transition.
- The passing test therefore proves only host filesystem write/read/delete capability. It does not prove OS login consumes the entry, the packaged process receives `--autostart`, the application hides only after tray creation, tray recovery works, or Settings state round-trips through the native plugin.
- It is also non-hermetic and potentially destructive: a pre-existing fixed-name entry is deleted and cleanup is not guarded against an assertion panic.
- The evidence ledger nevertheless calls this `E4` at `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-evidence.md:97-100,114`, and R-07 is marked GREEN at `...-plan.md:59`. This violates the harness evidence ladder and Completion Lock.

No corresponding Windows Registry Run-key runtime evidence or macOS LaunchAgent runtime evidence was supplied. That absence directly conflicts with the cross-platform goal at `...-plan.md:14` and scope at `...-actual-status.md:18`. Source support in an upstream plugin is not application-level runtime proof on those platforms.

### 2. Rollback test does not observe UI rollback

- The mock throws before changing `mockAutostartEnabled` at `src/lib/components/settings/__tests__/NotificationSettings.spec.ts:23-28`.
- The rollback case at `src/lib/components/settings/__tests__/NotificationSettings.spec.ts:159-169` clicks the switch and only asserts that the untouched backend mock variable remains `false`.
- It never asserts the rendered switch's `aria-checked` value after the rejected promise, never waits for the component state transition, and never asserts the error toast. The assertion would pass even if the component failed to execute `autostartEnabled = !next` at `src/lib/components/settings/NotificationSettings.svelte:130-138`.
- The source handler contains rollback logic, but the ledger's statement that rollback was verified (`...-evidence.md:89-90`) is unsupported. R-06's required negative/failure evidence at `...-plan.md:58` remains open.
- The targeted component run passed 5 tests, but every mount also emitted `window.__TAURI_INTERNALS__.transformCallback is not a function`; this is a mocked jsdom/E2 environment, not native runtime evidence.

### 3. Production CSP is build-compatible but not as narrowly restricted as claimed

- `src-tauri/tauri.conf.json:22` correctly removes `unsafe-inline` and `unsafe-eval` from production `script-src`; `src-tauri/tauri.conf.json:23` confines those script allowances to `devCsp`.
- A fresh production Tauri build completed successfully, so configuration/build compatibility is established.
- However, production `img-src`, `connect-src`, and `media-src` contain scheme-wide `https:` and/or `wss:` allowances at `src-tauri/tauri.conf.json:22`. These permit every origin on those schemes, not only enumerated "verified remote APIs" as claimed at `...-evidence.md:72-75`.
- Because AGENTS requires a strict CSP and the ledger asserts least-privilege remote origins, exact required endpoints must be inventoried or the evidence/status wording must truthfully acknowledge the broader policy. The current all-GREEN security closure overstates the configuration.

### 4. Completion Lock and evidence synchronization are still open

- `git diff --check b1e5c5d..HEAD` exits `2` due to `src-tauri/src/lib.rs:714: new blank line at EOF.`
- Fresh full frontend result is `688 pass, 1 skip, 0 fail, 4847 expect() calls across 689 tests/74 files`, not `4846` as recorded at `...-evidence.md:105`.
- Fresh Rust result is `868 passed, 1 ignored, 0 failed` in aggregate (`866 passed + 1 ignored` library; 1 Linux simulation; 1 updater test). The evidence records the passed count but omits the ignored test at `...-evidence.md:104`.
- `...-actual-status.md:5,76` declares Complete/resubmission ready and calls the Linux test E4, while `...-actual-status.md:92-95` still describes the pre-implementation state (dependencies absent). The same ledger is internally contradictory.
- `...-plan.md:53-59` marks R-01 through R-07 GREEN despite the missing E4 boundary and unverified rollback negative path. The Completion Lock is therefore not synchronized with repository reality.

## Source-level checks and prior-blocker disposition

| Prior review area | Re-review result |
|---|---|
| Plugin/capability/argument wiring | Source wiring present: `src-tauri/src/lib.rs:131-146`; capability remains present. |
| Tray-init/hide failure semantics | Source logic materially improved at `src-tauri/src/lib.rs:137-176`: tray failure does not select hide, hide failure is logged, and visible fallback is attempted. The two `window.show()` results remain discarded at lines 159 and 168, and no mounted Tauri test exercises actual `tray::init_tray`, `window.hide`, or `window.show` failures. |
| Four Rust tests | Fresh run: 4/4 pass. They test only the pure `determine_autostart_action` decision function at `src-tauri/src/lib.rs:684-713`; they do not prove actual tray/window calls. |
| CSP null / unsafe production script | Narrow blocker closed: CSP is non-null; production `script-src` has no unsafe allowances; full production build passes. Scheme-wide remote directives still prevent the stronger least-privilege claim from closing. |
| E2 naming and six switches | Honest E2 labels are present; all six switches are clicked and their mocks checked at `NotificationSettings.spec.ts:127-157`. Rollback evidence remains invalid as described above. |
| Native E4 | Not closed. The new test is a direct filesystem simulation and does not cross the application/native runtime boundary required for E4. |

## Evidence independently checked

- Git: HEAD `b714d25`; branch `master` is two commits ahead of `origin/master`. Worktree has pre-existing untracked `Reports/`, `docs/plans/2026-09-01-optimize-tauri-skill/`, and `docs/research/2026-09-01-tauri-skill-official-validation.md`; no production/test/plan edits were made by this review.
- Diff: 7 files changed from `b1e5c5d` to HEAD. `git diff --check` failed as recorded above.
- Anvien: refreshed at HEAD with `anvien analyze --force`; 598 files scanned, 410 code files parsed, 0 parse failures, 92,108 nodes, 106,110 relationships. The index was exact/current at `b714d25`. Compare scope reported 7 files and 74 changed symbols with low graph-level risk; `src-tauri/src/lib.rs` retained HIGH file-level impact. A final `anvien detect-changes --scope all` reported no uncommitted graph changes. This supports scope only, not runtime correctness.
- Full production build: `bun tauri build --no-bundle` exit 0; release artifact built at `src-tauri/target/release/rustsale-crm` in 6m50s. Frontend stage emitted 6 existing Svelte warnings and a >500 kB chunk warning.
- `bun run check`: exit 0, 0 errors, 6 warnings in 3 files.
- `cargo check`: exit 0, 0 warnings/errors, 3.87s.
- Rust autostart decision tests: 4 passed, 0 failed, 863 filtered.
- Full Rust suite: 868 passed, 1 ignored, 0 failed.
- Targeted frontend service/component suites: 11 passed, 0 failed, 44 assertions; emitted the Tauri `transformCallback` environment error during component mounts.
- Full frontend suite: 688 passed, 1 skipped, 0 failed, 4847 assertions across 689 tests/74 files.
- Linux simulation cleaned its created file on this run; `/home/arch/.config/autostart/rustsale-crm-test-autostart.desktop` was absent afterward. That cleanup does not change its evidence tier.

## Required next step

Keep the task incomplete. Replace the mislabeled Linux simulation with truthful evidence and close the actual native boundary: use the real plugin/API and packaged application, verify enable/disable against OS state, launch through the OS-login mechanism, prove `--autostart` reaches the running app, and visibly verify tray accessibility plus failure fallback. Provide equivalent Windows and macOS runtime evidence before retaining a cross-platform/comprehensive claim; otherwise narrow the claim explicitly to the platforms genuinely exercised. Also make the E2 rollback test assert rendered state/toast after rejection, narrow or accurately document CSP origins, fix the diff-check failure, refresh all four ledgers with exact counts/ignored/skipped results, and rerun independent Supervisor review before marking Complete or GREEN.

## Verdict

**REJECT**
