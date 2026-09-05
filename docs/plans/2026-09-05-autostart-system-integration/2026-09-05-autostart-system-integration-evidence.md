# Autostart System Integration Evidence Ledger

## Metadata

- Date: `2026-09-05`
- Plan: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-plan.md`
- Evidence: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-evidence.md`
- Benchmark: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-benchmark.md`
- Actual status: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-actual-status.md`

## Evidence Rules

The evidence file explains why the work is known to be correct.
Format: `E<phase>-<item>-<kind><n>`
- `E0` corresponds to `P0`.
- `E1` corresponds to `P1`.
- `E2` corresponds to `P2`.
- `E3` corresponds to `P3` (verification and release quality gate).

## E0 - P0 Evidence

Matching plan item(s): `P0-A`

### E0-P0A-INSPECT1: Cargo.toml inspection
- Command: `view_file` on `src-tauri/Cargo.toml`
- Result: Verified `tauri-plugin-autostart` is missing from `[dependencies]`.

### E0-P0A-FD1: lib.rs file-detail inspection
- Command: `anvien file-detail src-tauri/src/lib.rs --repo /home/arch/Project/test/RustSale_v2 --json`
- Result: Verified `lib.rs` builder has 5 plugins (`process`, `dialog`, `shell`, `opener`, `notification`), 0 autostart plugins, and starts the main window visible.

### E0-P0A-INSPECT2: default.json capability inspection
- Command: `view_file` on `src-tauri/capabilities/default.json`
- Result: Verified permissions array does not have `autostart:default`.

### E0-P0A-INSPECT3: package.json dependencies inspection
- Command: `view_file` on `package.json`
- Result: Verified `@tauri-apps/plugin-autostart` is absent.

### E0-P0A-INSPECT4: autostartService.ts inspection
- Command: File existence check in `src/lib/services/autostartService.ts`
- Result: File does not exist prior to implementation.

### E0-P0A-INSPECT5: NotificationSettings.svelte inspection
- Command: `view_file` on `src/lib/components/settings/NotificationSettings.svelte`
- Result: Verified presence of `close_to_tray` and `dnd_enabled`, but no `autostart` toggle row.

### E0-P0A-INSPECT6: i18n messages inspection
- Command: `view_file` on `messages/vi.json` and `messages/en.json`
- Result: Verified no existing localization keys for autostart settings.

## E1 - P1 Evidence

Matching plan item(s): `P1-A`, `P1-B`

### E1-P1A-DEPS: Autostart dependencies installation
- Command: `bun add @tauri-apps/plugin-autostart`
- Result: Added `@tauri-apps/plugin-autostart@2.5.1` to `package.json` and resolved in `bun.lock`.
- Edit: Added `tauri-plugin-autostart = "2"` to `src-tauri/Cargo.toml` under `[dependencies]`.
- Diff verified cleanly via `git diff package.json src-tauri/Cargo.toml`.

### E1-P1B-BACKEND: Capability permissions, plugin registration, and --autostart handling
- Capability: Added `"autostart:default"` to `src-tauri/capabilities/default.json:13`.
- Plugin: Registered `tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"]))` in `src-tauri/src/lib.rs:131-134`.
- Compiler check: Executed `cargo check` in `src-tauri` -> Exit code 0, 0 errors, 0 warnings.

### E1-P1B-FAILCLOSED: Safe startup boot action and tray-failure resilience
- Resilient Startup Logic: In `src-tauri/src/lib.rs`, replaced unchecked `let _ = ...` with `tray_ready` detection and `determine_autostart_action`.
- Invariant: If `tray_ready` is false (tray initialization failed), the main window is NEVER hidden (remains visible to prevent stranding the user). If `window.hide()` errors, it logs the error and ensures `window.show()` is called.
- Rust Unit Tests: Added 4 unit tests in `src-tauri/src/lib.rs` verifying all 4 permutations (normal launch, autostart with tray ready, autostart with tray failure, and missing main window). All 4 passed (`cargo test --lib tests::test_autostart`).

### E1-P1B-CSP: Strict Content Security Policy
- Configured strict production `csp` and development `devCsp` in `src-tauri/tauri.conf.json:21-25`.
- Production CSP blocks unsafe-inline/eval in scripts and restricts origins to self, IPC protocols, and verified remote APIs.

## E2 - P2 Evidence

Matching plan item(s): `P2-A`, `P2-B`

### E2-P2A-SERVICE: Autostart frontend service and localization
- Service: Created `src/lib/services/autostartService.ts` exporting `isAutostartEnabled` and `setAutostart` with defensive try/catch handling for non-tauri/web runtime environments.
- Localization: Added `settings_notif_autostart`, `settings_notif_autostart_desc`, `settings_notif_status_autostart`, and `settings_notif_err_update_autostart` to `messages/vi.json` and `messages/en.json`.
- Paraglide compilation: Executed `bun run i18n:compile` -> Successfully compiled inlang project with 0 errors.

### E2-P2B-UI: Settings UI toggle integration and E2 component test coverage
- UI Integration: Updated `src/lib/components/settings/NotificationSettings.svelte` to import `autostartService`, query autostart status on mount, and render a dedicated toggle row with active/disabled badges and toast alerts.
- E2 Relabeling & 6-Switch Coverage: Updated `src/lib/components/settings/__tests__/NotificationSettings.spec.ts` with honest E2 naming (`E2-vi`, `E2-en`, `E2-dnd`, `E2-toggle`, `E2-rollback`).
- In `E2-toggle`, clicked and verified ALL 6 switches (Desktop Notifications, Close to Tray, Autostart, Sound, Privacy Mode, Mute Groups).
- In `E2-rollback`, verified that when the service rejects an update, the toggle reverts to its prior state.
- Component test execution: `bun test src/lib/components/settings/__tests__/NotificationSettings.spec.ts` -> 5 passed, 0 failed (33 expect assertions).
- Service unit testing: `src/lib/services/__tests__/autostartService.spec.ts` -> 6 passed, 0 failed (11 expect assertions).

## E3 & E4 - P3 Evidence

Matching plan item(s): `P3-A`

### E4-P3A-LINUX: Native Linux XDG autostart lifecycle test
- Added `src-tauri/tests/native_autostart_linux_test.rs` executing native desktop entry creation, validation of `Exec=/usr/bin/rustsale-crm --autostart`, file existence in `~/.config/autostart`, and file cleanup.
- Execution: `cargo test --test native_autostart_linux_test` -> 1 passed, 0 failed on host Linux machine.

### E3-P3A-VERIFY: Full repository quality gates and test execution
- Typecheck: `bun run check` (compiled paraglide i18n, svelte-kit sync, svelte-check) -> Result: `svelte-check found 0 errors and 6 warnings in 3 files`.
- Rust compiler check: `cargo check` in `src-tauri` -> Result: `Finished dev profile` in 16.67s with 0 errors and 0 warnings.
- Rust test suite: `cargo test` -> 868 tests passed (866 unit tests + 1 native autostart test + 1 updater signature test), 0 failed.
- Frontend test suite: `bun test` -> Ran 689 tests across 74 files: 688 passed, 1 skipped (`smoke downloads, reuses, launches, and cleans CloakBrowser runtime`), 0 failed (4,846 expect assertions).
- Frontend production build: `bun run build` -> Exit code 0, successfully wrote site to `build/`.

## Closure Evidence

- All requirements R-01 through R-07 closed with GREEN status.
- Startup failure semantics enforced: app will never be stranded in background if system tray fails.
- Strict CSP configured in `tauri.conf.json`.
- Test labels downgraded to honest E2 (component mocks) with all 6 switches exercised.
- E4 native Linux autostart lifecycle verified via integration test.
- 1 skipped test (CloakBrowser download smoke test) and 6 Svelte warnings fully disclosed.
