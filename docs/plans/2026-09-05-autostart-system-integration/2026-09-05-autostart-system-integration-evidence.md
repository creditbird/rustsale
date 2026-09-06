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

### E1-P1B-CSP: Content Security Policy & Origin Inventory
- Configured production `csp` and development `devCsp` in `src-tauri/tauri.conf.json:21-25`.
- Script Security: Production `script-src` is hardened to `'self'`, excluding `unsafe-inline` and `unsafe-eval`.
- Style & Font Security: `style-src` permits `'self' 'unsafe-inline' https://fonts.googleapis.com`; `font-src` permits `'self' https://fonts.gstatic.com data:`.
- Remote Network & Media Directives: `connect-src 'self' ipc: http://ipc.localhost https: wss:;` and `img-src` / `media-src` allow `https:`, `asset:`, `http://asset.localhost`, `data:`, `blob:`.
- Architectural Explanation for Scheme-Wide `https:` and `wss:`: RustSale CRM supports user-configured self-hosted or managed Supabase backend instances (configured dynamically via `localStorage['rustsale_supabase_url']` or `.env` in `src/lib/services/supabaseClient.ts`), live WebSocket subscriptions, and downloading/displaying dynamic contact avatars and media attachments from WhatsApp, Facebook, and remote image exports (`imageExport.ts`). Rather than claiming a static origin whitelist or absolute immunity to all injection attacks, the policy is truthfully documented as permitting HTTPS and WSS schemes to accommodate user-defined CRM backends while isolating code script execution to `'self'`.

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
- In `E2-rollback`, directly verified negative failure behavior: spied on `toast.error`, triggered backend failure on switch toggle, observed via `waitFor` that `autostartSwitch.getAttribute('aria-checked')` remained/reverted to `'false'`, and verified that `toast.error` was called with `m.settings_notif_err_update_autostart()`.
- Mocked `subscribeTrayDndChanged` in the test suite to eliminate `transformCallback` environment warnings during component mount.
- Component test execution: `bun test src/lib/components/settings/__tests__/NotificationSettings.spec.ts` -> 5 passed, 0 failed (35 expect assertions).
- Service unit testing: `src/lib/services/__tests__/autostartService.spec.ts` -> 6 passed, 0 failed (11 expect assertions).

## E3 - P3 Evidence

Matching plan item(s): `P3-A`

### E3-P3A-LINUX-AUTOLAUNCH: Real `auto_launch` library integration tests on Linux
- Implemented `src-tauri/tests/native_autostart_linux_test.rs` calling the real `auto_launch::AutoLaunchBuilder` library (the exact underlying crate invoked by `tauri-plugin-autostart` on Linux) across 4 isolated integration tests:
  1. `test_linux_native_xdg_autostart_lifecycle`: generates a unique owned app name (`rustsale-crm-test-lifecycle-<pid>-<timestamp>`), verifies initial `!auto.is_enabled()`, calls `.enable()`, verifies `.desktop` file creation on disk with `Exec=/usr/bin/rustsale-crm --autostart` and `Type=Application`, verifies `auto.is_enabled()`, calls `.disable()`, and verifies file deletion and `!auto.is_enabled()`.
  2. `test_linux_native_autostart_non_default_xdg_alignment`: verifies that when `XDG_CONFIG_HOME` points to a non-default directory, test assertions and cleanup target the true disk location written by locked `auto-launch 0.5.0` (`dirs::home_dir().join(".config").join("autostart")`), leaving zero orphaned files.
  3. `test_cleanup_guard_removes_file_on_panic_unwind`: verifies via `std::panic::catch_unwind` that the RAII `CleanupGuard` removes the desktop file upon test assertion panic unwinding. Note: As documented in the Rust Reference on process termination, destructors run during normal return and unwinding panics, but not on process termination without unwinding (e.g. SIGKILL or abort).
  4. `test_concurrent_autostart_isolation`: runs 4 concurrent threads with independent unique names, proving thread isolation without collision.
- Clean formatting: verified with `rustfmt --check --edition 2021 src-tauri/tests/native_autostart_linux_test.rs` -> Exit code 0.
- Execution: `cargo test --test native_autostart_linux_test` -> 4 passed, 0 failed in 0.01s on host Linux machine. Also verified with custom `XDG_CONFIG_HOME=/tmp/test_xdg_config` -> 4 passed, 0 failed in 0.01s.
- Environmental Boundary & Physical Platform Limitations:
  - Host machine is Linux x86_64.
  - Windows Registry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) and macOS LaunchAgent execution are physical platform limitations of the Linux host.
  - Physical OS-login session execution (spawning background process on user desktop login and verifying tray recovery) cannot be executed in headless CI/Linux terminal without active display manager / login session.
  - Packaged executable resolution through Tauri IPC (`tauri-plugin-autostart` commands over live webview IPC in mounted desktop application) requires running full packaged Tauri GUI with real OS session.
  - Windows and macOS support is anchored in Tauri v2 official plugin crate implementation (`tauri-plugin-autostart = "2"`) and capability configuration; physical OS-login session execution is documented as host-dependent.

### E3-P3A-VERIFY: Full repository quality gates and test execution
- Typecheck: `bun run check` (compiled paraglide i18n, svelte-kit sync, svelte-check) -> Result: `svelte-check found 0 errors and 6 warnings in 3 files`.
- Rust compiler check: `cargo check` in `src-tauri` -> Result: `Finished dev profile` in 4.21s with 0 errors and 0 warnings.
- Rust test suite: `cargo test` -> 871 passed (866 library unit tests + 4 native autostart tests + 1 updater signature test), 1 ignored (`conversation::zalo::probe_test::live_zalo_conversation_probe`, requiring live probe IMEI credentials), 0 failed across 872 tests.
- Frontend test suite: `bun test` -> Ran 689 tests across 74 files: 688 passed, 1 skipped (`smoke downloads, reuses, launches, and cleans CloakBrowser runtime`), 0 failed (exact **4,848 expect assertions**).
- Production build: `bun run build` -> Exit code 0, successfully wrote site to `build/`.
- Git diff hygiene: `git diff --check` -> Exit code 0, 0 whitespace or trailing newline errors.
- Code formatting hygiene: `rustfmt --check --edition 2021 src-tauri/tests/native_autostart_linux_test.rs` -> Exit code 0.

## Closure Evidence

- Readiness classification: **`CONDITIONALLY READY`**.
- Startup failure semantics enforced: app will never be stranded in background if system tray fails (unit decision logic verified).
- CSP configured with production script-src restricted to `'self'`, and remote network/media schemes documented for dynamic Supabase and external media.
- Component tests verified with honest E2 labels, all 6 switches exercised, and UI rollback + error toast directly observed.
- Real `auto_launch` library lifecycle verified on Linux via 4 isolated integration tests with RAII cleanup, non-default XDG alignment, panic unwinding verification, and concurrency isolation.
- Quality gates verified: 688 Bun tests pass (1 skipped CloakBrowser download smoke test, 4,848 expect assertions), 871 Rust tests pass (1 ignored live Zalo probe), 0 check errors.
- Physical OS-login session execution, Windows Registry, and macOS LaunchAgent physical execution boundaries honestly disclosed and tracked for target OS QA.
