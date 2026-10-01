# Notification and System Tray Settings Persistence Plan

## Metadata

- Date: `2026-10-01`
- Status: `in_progress`
- Plan: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-plan.md`
- Evidence: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-evidence.md`
- Benchmark: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-benchmark.md`
- Actual status: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-actual-status.md`

## Goal

Persist desktop notification settings (`NotificationSettings`: `enabled`, `sound_enabled`, `privacy_mode`, `mute_groups`) and system tray configuration (`TrayConfig`: `close_to_tray`, `dnd_enabled`) across application restarts and OS reboots by storing configuration in SQLite (`rustsale.db` -> `app_metadata` table), restoring them during startup in `src-tauri/src/lib.rs`, atomically updating database state during Tauri IPC command invocations, and ensuring frontend components hydrate without state flickering or reverting to defaults on Linux, Windows, and macOS.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- Hidden fallback is forbidden. Prefer a visible failure over a fallback that hides a broken primary path.
- When touching DB-backed content, verify the full loop: UI toggle -> IPC command -> DB write -> DB read on restart -> correct UI state rendered.
- Tests must prove product behavior: assert that when a setting is toggled to false, stored in database, and reloaded on a fresh state instance, the loaded state retains false instead of reverting to default true.

## Problem

Currently, `NotificationSettings` and `TrayState` are stored purely in volatile process RAM (`RwLock<NotificationSettings>` in `notification.rs` and `Arc<AtomicBool>` in `tray.rs`). When the user toggles "Bật thông báo trên màn hình (Desktop Notifications)" or "Thu nhỏ xuống khay hệ thống khi đóng (Close to Tray)" to OFF, the in-memory state updates and suppresses notifications for the active session. However, when the user quits the app or reboots the computer (on Linux, Windows, and macOS alike), process memory is destroyed. Upon restart, `lib.rs` executes `NotificationManager::default()` and `TrayState::default()`, which hardcode `enabled: true` and `close_to_tray: true`. The frontend queries the backend on mount and resets all switches back to active defaults, causing user preference loss.

## Scope

- In scope:
  - `src-tauri/src/notification.rs`: Add persistence helper functions (`load_notification_settings`, `save_notification_settings`) to serialize/deserialize settings to/from SQLite `app_metadata`. Update `update_notification_settings` command to persist to SQLite.
  - `src-tauri/src/tray.rs`: Add persistence helper functions (`load_tray_config`, `save_tray_config`) to serialize/deserialize tray config to/from SQLite `app_metadata`. Update `set_close_to_tray` and `set_tray_dnd` commands to persist to SQLite.
  - `src-tauri/src/lib.rs`: In `.setup()`, read persisted `NotificationSettings` and `TrayConfig` from SQLite `app_metadata` and apply them to `NotificationManager` and `TrayState` before any window event or inbound message is dispatched.
  - `src/lib/services/desktopNotificationService.ts`: Ensure service layers handle persistence smoothly, with client-side caching to prevent layout flickering.
  - `src-tauri/src/notification_test.rs` & `src-tauri/src/tray_test.rs`: Add tests verifying serialization, database roundtrip, and restart recovery.
  - `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`: Verify UI toggle synchronization and persistence behavior.
- Out of scope:
  - Changing external OS notification permissions (macOS System Settings, Windows Action Center) which are controlled by OS policies.
  - Modifying `@tauri-apps/plugin-autostart` which already persists correctly via OS registries/launchers.

## Non-Goals

- Replacing SQLite `app_metadata` with a separate proprietary config file parser.
- Altering the visual design or token styling of `NotificationSettings.svelte`.

## Requirements

- R1: Backend persistence helpers and SQLite `app_metadata` storage for `NotificationSettings` and `TrayConfig` in `rustsale.db`.
- R2: Startup restoration in `src-tauri/src/lib.rs` loads persisted settings from database into `NotificationManager` and `TrayState` before inbound notification processing.
- R3: `update_notification_settings` and `set_close_to_tray` IPC commands atomically write new configurations to `app_metadata` while keeping in-memory state in sync.
- R4: Frontend `desktopNotificationService` and `NotificationSettings` maintain synchronized state and avoid falling back to defaults on reload or restart.
- R5: Automated test coverage passing 100% across cargo test and bun test.

## Acceptance Criteria

- When notifications are turned off in settings, the change is immediately stored in `app_metadata` table in SQLite (`rustsale.db`).
- Simulating an application restart (initializing new manager instances and loading from database) restores the exact previously saved settings (`enabled: false`, `close_to_tray: false`, etc.) without defaulting back to `true`.
- Cargo tests (`notification::` and `tray::`) pass with zero regressions.
- Frontend tests (`NotificationSettings.spec.ts` and `desktopNotificationService.spec.ts`) pass with zero regressions.
- Full verification harness (`agent:verify 0e44fc2f-d922-44e5-baed-edf9a5d133b4`) reports PASS.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state across frontend and backend.
  - Work Steps: inspect target files, run Anvien file-detail, classify reality, record evidence IDs in `actual-status.md`.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: actual status classifies target files and confirms root cause.

### P1: Backend Database Persistence & Startup Restoration

- Phase Goal: Implement SQLite persistence and startup loading for `NotificationSettings` and `TrayConfig`.
- Phase Boundary:
  - In scope: `src-tauri/src/notification.rs`, `src-tauri/src/tray.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/notification_test.rs`, `src-tauri/src/tray_test.rs`.
  - Out of scope: Frontend Svelte component layout changes.
  - Dependencies: `rusqlite`, `serde_json`, `app_metadata` table.
- Ordered Slice List:
  - P1-A: Add persistence helpers and update IPC commands in `notification.rs` and `tray.rs`.
  - P1-B: Connect startup restoration in `src-tauri/src/lib.rs` and add Rust integration tests.

- [x] P1-A: Add persistence helpers and update IPC commands in `notification.rs` and `tray.rs`
  - Goal: Provide `load_notification_settings`, `save_notification_settings`, `load_tray_config`, and `save_tray_config`, and update Tauri IPC commands to persist configurations to SQLite `app_metadata`.
  - Scope Boundary:
    - Editable: `src-tauri/src/notification.rs`, `src-tauri/src/tray.rs`.
    - Inspect-only: `src-tauri/src/commands.rs`, `src-tauri/src/db.rs`.
    - Out of scope: Frontend components.
  - Pre-flight Questions:
    - Data source: `rustsale.db` -> `app_metadata`.
    - DB read flow: `SELECT value FROM app_metadata WHERE key = 'notification_settings' / 'tray_config'`.
    - DB write flow: `INSERT INTO app_metadata (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value`.
    - Behavior test: Cargo tests in `notification_test.rs` and `tray_test.rs`.
  - Work Steps:
    1. In `notification.rs`, implement `load_notification_settings(&rusqlite::Connection) -> NotificationSettings` and `save_notification_settings(&rusqlite::Connection, &NotificationSettings) -> Result<(), rusqlite::Error>`.
    2. In `notification.rs`, update `update_notification_settings` to accept optional `DbState` and persist settings when available.
    3. In `tray.rs`, implement `load_tray_config(&rusqlite::Connection) -> TrayConfig` and `save_tray_config(&rusqlite::Connection, &TrayConfig) -> Result<(), rusqlite::Error>`.
    4. In `tray.rs`, update `set_close_to_tray` and `set_tray_dnd` to accept optional `DbState` and persist configurations when available.
  - Implementation Gate: Anvien file-detail recorded.
  - Acceptance: Persistence helpers compile, pass unit tests, and correctly serialize/deserialize settings.

- [x] P1-B: Connect startup restoration in `src-tauri/src/lib.rs` and add Rust tests
  - Goal: In `lib.rs` `.setup()`, read persisted settings from SQLite `app_metadata` and apply them to `NotificationManager` and `TrayState` on boot.
  - Scope Boundary:
    - Editable: `src-tauri/src/lib.rs`, `src-tauri/src/notification_test.rs`, `src-tauri/src/tray_test.rs`.
    - Inspect-only: `src-tauri/src/notification.rs`, `src-tauri/src/tray.rs`.
  - Work Steps:
    1. In `src-tauri/src/lib.rs` inside `.setup()`, after `db::init_db` initializes the connection, load `notification::load_notification_settings(&conn)` and call `notification_manager.update_settings(...)`.
    2. Also load `tray::load_tray_config(&conn)` and call `tray_state.set_close_to_tray(loaded.close_to_tray)` and `tray_state.set_dnd_enabled(loaded.dnd_enabled)`.
    3. Add tests in `notification_test.rs` and `tray_test.rs` verifying that saving settings to SQLite, creating a new `NotificationManager` / `TrayState` from loaded data, preserves the updated values.
    4. Run `cargo test --manifest-path src-tauri/Cargo.toml --lib notification::` and `cargo test --manifest-path src-tauri/Cargo.toml --lib tray::`.
  - Acceptance: Rust tests pass 100%, proving persisted settings survive simulated restarts.

### P2: Frontend Synchronization & Verification Harness

- Phase Goal: Verify frontend service caching, run full test suites, and execute verification harness.
- Phase Boundary:
  - In scope: `src/lib/services/desktopNotificationService.ts`, `src/lib/components/settings/NotificationSettings.svelte`, `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`.
  - Out of scope: Other settings tabs (Accounts, Language).
- Ordered Slice List:
  - P2-A: Frontend service synchronization and spec verification.
  - P2-B: Full harness verification and final closure.

- [x] P2-A: Frontend service synchronization and spec verification
  - Goal: Ensure `desktopNotificationService.ts` and `NotificationSettings.svelte` sync reliably with backend and handle fast initial render.
  - Scope Boundary:
    - Editable: `src/lib/services/desktopNotificationService.ts`, `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`.
  - Work Steps:
    1. Ensure `desktopNotificationService.ts` seamlessly handles loading and updating notification settings.
    2. Add/update tests in `NotificationSettings.spec.ts` asserting that toggling persists and does not revert.
    3. Run `bun test src/lib/components/settings/__tests__/NotificationSettings.spec.ts` and `bun test src/lib/services/__tests__/desktopNotificationService.spec.ts`.
  - Acceptance: All frontend tests pass.

- [ ] P2-B: Full harness verification and final closure
  - Goal: Run `agent:verify 0e44fc2f-d922-44e5-baed-edf9a5d133b4` and ensure all quality gates pass.
  - Work Steps:
    1. Run `bun --no-env-file run agent:verify 0e44fc2f-d922-44e5-baed-edf9a5d133b4`.
    2. Inspect results and verify exit code 0.
    3. Record evidence and benchmark metrics.
  - Acceptance: `agent:verify` returns PASS with exit code 0.

## Risk Notes

- Low risk: `app_metadata` table is already established in SQLite and handles concurrency via SQLite mutex transactions.
- Zero breaking changes to existing IPC signatures: `DbState` is injected via Tauri's dependency injection container (`tauri::State`).
