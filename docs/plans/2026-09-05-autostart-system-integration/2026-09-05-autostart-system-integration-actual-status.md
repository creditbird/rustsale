# Autostart System Integration Actual Status

Title: Autostart System Integration
Date: 2026-09-05
Status: Complete
Companion plan: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-plan.md`
Companion evidence: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-evidence.md`
Companion benchmark: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-benchmark.md`

## Purpose

This file records the real current state before implementation.
Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

## Scope

Target scope:
- Cross-platform system autostart integration (Linux, Windows, macOS) for RustSale CRM
- Tauri v2 official plugin `@tauri-apps/plugin-autostart` and `tauri-plugin-autostart = "2"`
- Permission capabilities for autostart in `src-tauri/capabilities/default.json`
- Background startup handling with `--autostart` argument to start minimized to tray
- Frontend service wrapper `src/lib/services/autostartService.ts`
- Settings UI integration in `src/lib/components/settings/NotificationSettings.svelte`
- i18n messages in `messages/vi.json` and `messages/en.json`

Out of scope:
- Non-desktop platforms (iOS, Android mobile builds)
- Modifying other settings tabs (Zalo, Email, Facebook, WhatsApp, Language)

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|-----------------------|----------------------|--------------------|----------------------|-------------|
| `src-tauri/Cargo.toml` | `E0-P0A-INSPECT1` | 1 | Backend crate manifest defining Tauri plugins and deps | low scope risk |
| `src-tauri/src/lib.rs` | `E0-P0A-FD1` | 42 | Tauri application builder, plugin registry, and CLI argument handler | medium scope risk |
| `src-tauri/capabilities/default.json` | `E0-P0A-INSPECT2` | 1 | Security authorization capability file granting plugin permissions | low scope risk |
| `package.json` | `E0-P0A-INSPECT3` | 1 | Node/Bun dependencies for `@tauri-apps/plugin-autostart` | low scope risk |
| `src/lib/services/autostartService.ts` | `E0-P0A-INSPECT4` | 0 (new) | Frontend service wrapping Tauri autostart plugin API | low scope risk |
| `src/lib/components/settings/NotificationSettings.svelte` | `E0-P0A-INSPECT5` | 8 | Settings card presenting system tray and autostart toggle row | low scope risk |
| `messages/vi.json` & `messages/en.json` | `E0-P0A-INSPECT6` | 2 | Localization resource files compiled via Paraglide JS | low scope risk |

## Status Rules

| Status | Meaning | Allowed next action |
|--------|---------|---------------------|
| `correct` | Already behaves as required. | Preserve. Add evidence or tests only if needed. |
| `partial` | Some required behavior exists, but gaps remain. | Change only the missing parts. Preserve correct parts. |
| `wrong` | Current behavior, source, or contract is incorrect. | Replace with required behavior. Record the exact reason. |
| `missing` | Required behavior, source, or contract does not exist. | Implement the missing piece only. |
| `unbound` | Surface exists but is not wired to the real source, flow, or contract. | Bind to the real source only. Preserve approved surface. |
| `fake-or-stub` | Prototype, demo, mock, fallback, or placeholder data is being used as real behavior. | Remove fake behavior or replace it with an approved truthful state. |
| `blocked` | Source, authority, contract, or required evidence is unclear. | Stop. Do not implement until resolved. |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|------|---------------|----------------|--------|--------------------|----------|--------------------|
| Backend crate `tauri-plugin-autostart` | Added in `src-tauri/Cargo.toml` | Added with version `2` | `correct` | 1 related file | `E1-P1A-DEPS` | preserve |
| Frontend dependency `@tauri-apps/plugin-autostart` | Installed in `package.json` | Installed with version `^2` via bun | `correct` | 1 related file | `E1-P1A-DEPS` | preserve |
| Tauri capability permissions | Contains core, opener, dialog, updater, process, notification, shell:allow-spawn, autostart:default | Included `"autostart:default"` at line 13 | `correct` | 1 related file | `E1-P1B-BACKEND` | preserve |
| Rust lib plugin registration & startup arg | Registered autostart plugin with `LaunchAgent` & `Some(vec!["--autostart"])`; safe `determine_autostart_action` with fallback to visible window on tray failure | Registered plugin, safe startup handler, and unit tests | `correct` | 42 related files | `E1-P1B-FAILCLOSED` | preserve |
| Autostart frontend service | Created `src/lib/services/autostartService.ts` | `src/lib/services/autostartService.ts` exports `isAutostartEnabled` and `setAutostart` with error safety | `correct` | 0 related files | `E2-P2A-SERVICE` | preserve |
| Localization strings | Added autostart keys in `messages/vi.json` and `messages/en.json` | Vietnamese & English keys for label, description, status, and error | `correct` | 2 related files | `E2-P2A-SERVICE` | preserve |
| Settings UI toggle | Added Autostart toggle row with dynamic badge, active state sync, error rollback, and 6-switch test | Includes Autostart toggle row with dynamic badge, active state sync, and error toast | `correct` | 8 related files | `E2-P2B-REMEDIATION` | preserve |
| Tauri CSP configuration | Strict production CSP and devCsp configured in `src-tauri/tauri.conf.json` | Strict CSP policy defined without null | `correct` | 1 related file | `E1-P1B-CSP` | preserve |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---|---|---|---|---|---|---|
| R0 | 2026-09-05 | baseline commit 325c6e4 | autostart system integration | initial classification: all units missing | `E0-P0A-INSPECT1..6` | proceed to P1-A |
| R1 | 2026-09-05 | after P1-A deps add | `package.json`, `Cargo.toml` | deps: `missing -> correct` | `E1-P1A-DEPS` | proceed to P1-B |
| R2 | 2026-09-05 | after P1-B backend register | `default.json`, `lib.rs` | backend & capability: `missing -> correct` | `E1-P1B-BACKEND` | proceed to P2-A |
| R3 | 2026-09-05 | after P2-A service & i18n | `autostartService.ts`, `messages/*.json` | service & i18n: `missing -> correct` | `E2-P2A-SERVICE` | proceed to P2-B |
| R4 | 2026-09-05 | after P2-B UI & P3-A verify | `NotificationSettings.svelte`, tests | UI & tests: `missing -> correct` | `E2-P2B-UI`, `E3-P3A-VERIFY` | initial commit b1e5c5d |
| R5 | 2026-09-05 | post-review remediation | `lib.rs`, `tauri.conf.json`, `NotificationSettings.spec.ts`, `native_autostart_linux_test.rs` | fail-closed tray boot, strict CSP, all 6 switches exercised, native Linux test | `E1-P1B-FAILCLOSED`, `E1-P1B-CSP`, `E2-P2B-REMEDIATION` | supervisor re-review |
| R6 | 2026-09-05 | post-re-review remediation | `Cargo.toml`, `native_autostart_linux_test.rs`, `NotificationSettings.spec.ts`, plan docs | real `auto_launch` crate test, UI rollback + toast verification, honest CSP origin documentation, exact test counts (868 passed / 1 ignored; 688 passed / 1 skipped / 4849 expect) | `E3-P3A-LINUX-AUTOLAUNCH`, `E2-P2B-UI`, `E3-P3A-VERIFY` | resubmission ready |

## Phase Touch Map

| Unit / File / Surface | Plan-Relevant Relationship File | Relationship to Target | Plan Item | Touch Mode | Evidence | Constraint |
|-----------------------|---------------------------------|------------------------|-----------|------------|----------|------------|
| `package.json` | `bun.lock` | package dependencies | P1-A | edit | `E0-P0A-INSPECT3` | use `bun add` exclusively |
| `src-tauri/Cargo.toml` | `src-tauri/Cargo.lock` | rust dependencies | P1-A | edit | `E0-P0A-INSPECT1` | preserve existing crate versions |
| `src-tauri/capabilities/default.json` | `src-tauri/gen/schemas/desktop-schema.json` | capability schema | P1-B | edit | `E0-P0A-INSPECT2` | grant minimal necessary `"autostart:default"` |
| `src-tauri/src/lib.rs` | `src-tauri/src/tray.rs` | main window & system tray | P1-B | edit | `E0-P0A-FD1` | non-blocking, check `--autostart` flag |
| `src/lib/services/autostartService.ts` | `@tauri-apps/plugin-autostart` | plugin consumer | P2-A | create | `E0-P0A-INSPECT4` | robust try/catch fallback for dev mode |
| `messages/vi.json` & `messages/en.json` | `src/lib/paraglide/messages` | i18n source | P2-A | edit | `E0-P0A-INSPECT6` | run `bun run i18n:compile` after edit |
| `src/lib/components/settings/NotificationSettings.svelte` | `src/lib/services/autostartService.ts` | UI consumer | P2-B | edit | `E0-P0A-INSPECT5` | preserve existing toggles, test desktop notification |

## Baseline P0 Findings (Pre-Implementation Baseline)

1. `package.json` originally had `@tauri-apps/plugin-notification`, `@tauri-apps/plugin-process`, `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-opener`, `@tauri-apps/plugin-dialog`. The autostart plugin `@tauri-apps/plugin-autostart` was not installed.
2. `src-tauri/Cargo.toml` originally did not include `tauri-plugin-autostart`.
3. `src-tauri/capabilities/default.json` defines permissions for the "main" window; adding `"autostart:default"` enables standard autostart IPC commands (`enable`, `disable`, `is_enabled`).
4. `src-tauri/src/lib.rs` originally initialized the window with default visibility. When booting with `--autostart`, hiding the main window and keeping it in the system tray provides a smooth background experience.
5. In `NotificationSettings.svelte`, system tray integration (`close_to_tray`) was already present and matched the pattern required for an autostart toggle row.

## Post-Remediation Verification Findings (R6)

1. **Dependencies & Crate Integration**: `@tauri-apps/plugin-autostart` is installed in `package.json` and locked in `bun.lock`. `tauri-plugin-autostart = "2"` and `auto-launch = "0.5"` (dev-dependency) are configured in `src-tauri/Cargo.toml` and locked in `Cargo.lock`.
2. **Real Linux `auto_launch` Integration Test**: `src-tauri/tests/native_autostart_linux_test.rs` calls real `auto_launch::AutoLaunchBuilder` (the exact underlying crate used by `tauri-plugin-autostart` on Linux) to exercise `.enable()`, `.is_enabled()`, and `.disable()`, equipped with an RAII `CleanupGuard` implementing `Drop` to guarantee hermetic cleanup.
3. **Platform Scope & Environmental Boundaries**: The test host is Linux x86_64. Windows Registry (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) and macOS LaunchAgent execution are physical platform limitations of the Linux host. Their integration is grounded in the official upstream `tauri-plugin-autostart` v2 crate and Tauri capability definitions, while physical OS-login session validation is documented as host-dependent.
4. **Settings UI Rollback Verification**: `NotificationSettings.spec.ts` verifies that on backend failure, the switch reverts to `aria-checked="false"` and an error toast `toast.error(m.settings_notif_err_update_autostart())` is triggered and asserted.
5. **CSP Policy**: Production `script-src` is strictly hardened to `'self'`, eliminating unsafe script execution. `connect-src` and `img-src` permit `https:` and `wss:` to support user-configurable dynamic Supabase instances and remote media/avatars.
6. **Codebase Hygiene & Test Integrity**: `git diff --check` passes cleanly with 0 whitespace errors. Full test suites pass with exact counts: Frontend: 688 passed, 1 skipped, 0 failed, 4,849 assertions; Rust: 868 passed, 1 ignored, 0 failed.
