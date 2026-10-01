# Notification and System Tray Settings Persistence Evidence Ledger

## Metadata

- Date: `2026-10-01`
- Plan: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-plan.md`
- Evidence: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-evidence.md`
- Benchmark: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-benchmark.md`
- Actual status: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-actual-status.md`

## Evidence Rules

- Stable evidence IDs format: `E<phase>-<item>-<kind><n>`.
- Proof before assertions. Record exact commands, outputs, exit codes, and diffs.

## E0 - P0 Baseline Evidence

### E0-P0A-USER: User problem report
- Observation: User reported that on Linux, turning off desktop notifications works during the session, but after rebooting the machine, settings revert to default `true` (notification turns back on). Also asked if Windows and macOS have the same bug.
- User uploaded screenshot showing "Thông báo & Khay hệ thống" with "Bật thông báo trên màn hình" highlighted.

### E0-P0A-INSPECT-NOTIF: Code inspection of notification.rs
- Target: `src-tauri/src/notification.rs`
- Finding: `NotificationSettings` is stored in `RwLock<NotificationSettings>` in `NotificationManager`. `update_notification_settings` only updates the `RwLock` in RAM. No persistence layer exists.
- Result: On app shutdown/reboot, process RAM is freed. On restart, `NotificationManager::default()` resets `enabled` to `true`.

### E0-P0A-INSPECT-TRAY: Code inspection of tray.rs
- Target: `src-tauri/src/tray.rs`
- Finding: `TrayState` stores `close_to_tray` and `dnd_enabled` in `Arc<AtomicBool>`. `set_close_to_tray` only stores into the atomic bool in RAM.
- Result: On app shutdown/reboot, resets to `close_to_tray: true`.

### E0-P0A-INSPECT-LIB: Code inspection of lib.rs
- Target: `src-tauri/src/lib.rs` lines 122-125
- Finding: `lib.rs` executes:
  `let tray_state = tray::TrayState::default();`
  `let notification_manager = std::sync::Arc::new(notification::NotificationManager::default());`
  Neither reads from SQLite or config files during `.setup()`.

### E0-P0A-CROSS-OS: Cross-platform verification
- Finding: The Rust code in `notification.rs`, `tray.rs`, and `lib.rs` has no platform conditionals for settings storage. Process memory is ephemeral across Linux, Windows, and macOS.
- Conclusion: Linux, Windows, and macOS all suffer from the exact same setting loss bug.

### E0-P0A-FD-NOTIF & E0-P0A-FD-TRAY: Anvien graph evidence
- Commands: `anvien file-detail src-tauri/src/notification.rs --repo rustsale --json` and `anvien file-detail src-tauri/src/tray.rs --repo rustsale --json`
- Anvien index fresh: commit `ac00f20`, 97256 nodes, 111812 relationships.

## E1 - P1 Implementation Evidence

### E1-P1A-NOTIF-SQLITE: Notification persistence in SQLite
- File: `src-tauri/src/notification.rs`
- Changes:
  - Added `load_notification_settings(&conn) -> NotificationSettings` querying `app_metadata` (`key='notification_settings'`).
  - Added `save_notification_settings(&conn, &settings) -> Result<(), rusqlite::Error>` upserting into `app_metadata`.
  - Updated `update_notification_settings` command to accept `app: AppHandle<R>`, update in-memory state, and persist to SQLite if `DbState` is managed.

### E1-P1A-TRAY-SQLITE: Tray configuration persistence in SQLite
- File: `src-tauri/src/tray.rs`
- Changes:
  - Added `load_tray_config(&conn) -> TrayConfig` querying `app_metadata` (`key='tray_config'`).
  - Added `save_tray_config(&conn, &config) -> Result<(), rusqlite::Error>` upserting into `app_metadata`.
  - Updated `set_close_to_tray`, `set_tray_dnd`, and tray menu "toggle_dnd" event handler to persist changes immediately to SQLite.

### E1-P1B-RESTORE-BOOT: Startup restoration on application boot
- File: `src-tauri/src/lib.rs`
- Changes:
  - Cloned `notification_manager` and `tray_state` handles for the `.setup()` hook.
  - In `.setup()` after `db::init_db`, loaded `notification::load_notification_settings(&conn)` and `tray::load_tray_config(&conn)`.
  - Updated `notification_manager` and `tray_state` with restored values before any window event or message processing occurs.

### E1-P1B-TESTS-PASS: Rust unit and integration test evidence
- Commands:
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib tray::`: 5 passed, 0 failed.
    - `test_tray_config_sqlite_roundtrip_and_restart ... ok`
    - `test_set_close_to_tray_command_persists_to_db ... ok`
  - `cargo test --manifest-path src-tauri/Cargo.toml --lib notification::`: 15 passed, 0 failed.
    - `test_notification_settings_sqlite_roundtrip_and_restart ... ok`
    - `test_update_notification_settings_command_persists_to_db ... ok`

## E2 - P2 Frontend Evidence

### E2-P2A-FRONTEND-SYNC: Frontend service caching & test verification
- File: `src/lib/services/desktopNotificationService.ts`
- Changes: Added client-side localStorage caching (`rustsale_notification_settings_cache`, `rustsale_tray_config_cache`) to prevent UI state flickering upon mount while maintaining SQLite backend as canonical source of truth.
- Tests: `bun test src/lib/components/settings/__tests__/NotificationSettings.spec.ts src/lib/services/__tests__/desktopNotificationService.spec.ts`: 16 passed, 0 failed.
- Types & Build: `bun run check` (0 errors), `bun run build` (built in 20.88s).

