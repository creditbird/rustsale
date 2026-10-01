# Notification and System Tray Settings Persistence Actual Status

Title: Notification and System Tray Settings Persistence Actual Status
Date: 2026-10-01
Status: P0 Complete
Companion plan: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-plan.md`
Companion evidence: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-evidence.md`
Companion benchmark: `docs/plans/2026-10-01-notification-settings-persistence/2026-10-01-notification-settings-persistence-benchmark.md`

## Purpose

This file records the real current state before implementation.
Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

## Freshness / Refresh Rules

- P0 records the baseline before implementation.
- Update after each completed slice or phase transition.
- Use explicit transitions: `in-memory-only -> sqlite-persisted`, `missing-boot-load -> bound-on-startup`.

## Scope

Target scope:
- `src-tauri/src/notification.rs`
- `src-tauri/src/tray.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/notification_test.rs`
- `src-tauri/src/tray_test.rs`
- `src/lib/services/desktopNotificationService.ts`
- `src/lib/components/settings/NotificationSettings.svelte`
- `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`

Out of scope:
- OS-level notification daemons, macOS System Settings, Windows Action Center settings.
- Autostart registry integration (already persisted via `@tauri-apps/plugin-autostart`).

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
| :--- | :--- | :--- | :--- | :--- |
| `src-tauri/src/notification.rs` | `E0-P0A-FD-NOTIF` | 3 inbound / 3 process flows | Provides `NotificationManager` and notification IPC commands | Core notification module, medium risk |
| `src-tauri/src/tray.rs` | `E0-P0A-FD-TRAY` | 1 inbound / 0 process flows | Provides `TrayState` and tray IPC commands | Tray close-to-tray & DND state, low risk |
| `src-tauri/src/lib.rs` | `E0-P0A-FD-LIB` | App entry point and setup | Instantiates managers and registers IPC commands | App lifecycle & SQLite setup, medium risk |
| `src/lib/services/desktopNotificationService.ts` | `E0-P0A-FD-SVC` | Frontend IPC wrapper | Calls `get_notification_settings` and `update_notification_settings` | Frontend service layer, low risk |
| `src/lib/components/settings/NotificationSettings.svelte` | `E0-P0A-FD-UI` | Settings UI component | Renders 6 toggle switches and binds state | User-facing UI, low risk |

## Current Status Matrix

| Unit / Surface | Baseline Reality (P0) | Evidence ID | Target State | Touch Mode | Plan Consequence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `NotificationSettings` storage | In-memory only (`RwLock<NotificationSettings>`); resets to default `true` on restart | `E1-P1A-NOTIF-SQLITE` | Persisted to SQLite `app_metadata` (`key='notification_settings'`) | `in-memory -> sqlite-persisted` | Complete (P1-A) |
| `TrayConfig` storage | In-memory only (`Arc<AtomicBool>`); resets to default `true` on restart | `E1-P1A-TRAY-SQLITE` | Persisted to SQLite `app_metadata` (`key='tray_config'`) | `in-memory -> sqlite-persisted` | Complete (P1-A) |
| Startup restore in `lib.rs` | Missing; creates default managers without reading DB | `E1-P1B-RESTORE-BOOT` | Reads `app_metadata` in `.setup()` and restores state before event dispatch | `missing -> bound-on-startup` | Complete (P1-B) |
| Rust test coverage | Tests exist for in-memory defaults and suppression, but zero tests for restart persistence | `E1-P1B-TESTS-PASS` | Tests for DB roundtrip and restart simulation | `13 -> 15 notif tests, 3 -> 5 tray tests` | Complete (P1-B) |
| Frontend service & UI | Works during session, but reflects backend reset on app reload | `E2-P2A-FRONTEND-SYNC` | Retains user selection across app and machine restarts with cached hydration | `cache-fallback -> verified` | Complete (P2-A) |

## Status Refresh Log

| Refresh ID | Date | Trigger | Changed Rows | Previous State | New State | Next Phase Impact |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| R0 | 2026-10-01 | P0 baseline classification | All target rows | Unclassified | P0 Complete | Proceed to P1-A |
| R1 | 2026-10-01 | P1-A persistence implementation | Notification & Tray storage | in-memory only | sqlite-persisted | Proceed to P1-B |
| R2 | 2026-10-01 | P1-B startup boot load & Rust tests | Startup restore, Rust tests | missing boot load | bound-on-startup & 20 tests pass | Proceed to P2-A |
| R3 | 2026-10-01 | P2-A frontend sync & test verification | Frontend service & UI | unverified cache | cache-fallback & verified | Proceed to P2-B |
