# Autostart System Integration Plan

## Metadata

- Date: `2026-09-05`
- Status: `completed`
- Plan: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-plan.md`
- Evidence: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-evidence.md`
- Benchmark: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-benchmark.md`
- Actual status: `docs/plans/2026-09-05-autostart-system-integration/2026-09-05-autostart-system-integration-actual-status.md`

## Goal

Provide cross-platform system autostart integration for RustSale CRM across Linux, Windows, and macOS using Tauri v2 official plugin ecosystem (`@tauri-apps/plugin-autostart` and `tauri-plugin-autostart`), with background startup handling (launch minimized to tray via `--autostart`), two-way state synchronization, and a clean toggle row in the Settings UI under Notification & System Settings.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- NEVER use `node`, `npm`, or `npx`; ALWAYS use `bun` and `bunx`.
- Follow Tauri v2 optimization guidelines: non-blocking startup arguments, minimal capability permissions (`autostart:default`), and clean separation of concerns.
- Code first; tests/checks must be verified after implementation is in place.
- Each implementation phase must be decomposed into narrow executable slices.

## Problem

When the host OS (Linux, Windows, or macOS) starts up, RustSale CRM cannot launch automatically. Users who rely on RustSale for customer relationship management and instant messaging across Zalo, Facebook, and WhatsApp must manually remember to open the app each time they boot their computer. Furthermore, there is no option in the Settings UI allowing users to configure whether they want the app to start on system boot or remain strictly manual.

## Scope

- Official Tauri v2 plugin integration: `@tauri-apps/plugin-autostart` (frontend) and `tauri-plugin-autostart = "2"` (backend).
- Tauri v2 capability configuration in `src-tauri/capabilities/default.json`.
- Startup launch args with `--autostart`, and background launch handling in `src-tauri/src/lib.rs` (hides window when launched with `--autostart` so it stays quietly in the system tray).
- Frontend service wrapper `src/lib/services/autostartService.ts` with error handling and fallback.
- Settings UI integration in `src/lib/components/settings/NotificationSettings.svelte` with reactive switch, state badge, and toast feedback.
- Localization messages in `messages/vi.json` and `messages/en.json` compiled with Paraglide JS.

## Non-Goals

- Mobile OS startup hooks (Android / iOS).
- Modifying non-notification settings modules.
- Custom OS-level daemon or root/systemd service scripts outside user-level autostart specification.

## Requirements Ledger

| ID | Exact requirement | Invariant | Positive case | Negative/failure case | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|---|---|
| R-01 | Install official Tauri v2 autostart dependencies | Frontend uses `bun add`, backend uses `tauri-plugin-autostart = "2"` | Dependencies resolve and compile cleanly | Missing package or version mismatch | `Cargo.lock` & `bun.lock` | P1-A | `PROVEN` |
| R-02 | Grant minimal Tauri v2 autostart permissions | Default capability window `"main"` includes `"autostart:default"` | App frontend can call `enable()`, `disable()`, `isEnabled()` | Permission denied IPC error | `src-tauri/capabilities/default.json` diff | P1-B | `PROVEN` |
| R-03 | Initialize plugin and handle background `--autostart` | App hidden to tray if started with `--autostart`; normal launch opens window | System boot does not pop window; user launch opens window | Window fails to open or crashes on boot | `lib.rs` logic & `cargo check` | P1-B | `PROVEN` |
| R-04 | Provide frontend autostart service | `isAutostartEnabled` and `setAutostart` handle errors gracefully | Returns boolean state and toggles OS registry | Exception thrown in non-tauri or dev env caught safely | `autostartService.ts` & unit check | P2-A | `PROVEN` |
| R-05 | Add localization strings in Vietnamese and English | Both locales define autostart title, description, and status | Paraglide compiles without missing key warnings | Undefined translation runtime error | `messages/*.json` & `bun run i18n:compile` | P2-A | `PROVEN` |
| R-06 | Integrate Autostart switch in Settings UI | Rendered inside `NotificationSettings.svelte` with active badge and Sonner toast | Toggling switch updates state and shows feedback | Failure shows error toast and reverts switch | Component view & `bun run check` | P2-B | `PROVEN` |
| R-07 | End-to-end quality gates | Svelte checks, backend cargo checks, change detection, and build pass | Zero errors in `bun run check` and `cargo check` | Type or compile errors block release | Harness QA report | P3-A | `PROVEN` |

## Acceptance Criteria

1. `tauri-plugin-autostart` registered in `src-tauri/src/lib.rs` and `@tauri-apps/plugin-autostart` installed via `bun`.
2. Capability `src-tauri/capabilities/default.json` permits `"autostart:default"`.
3. If `--autostart` CLI argument is present, the main window is kept hidden in the system tray.
4. Settings UI displays a switch for "Khởi động cùng hệ thống", reflects true OS autostart state on load, and updates OS autostart when toggled.
5. All TypeScript type checks (`bun run check`), i18n compile, and Rust compiler checks (`cargo check`) pass with 0 errors.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: establish the real current state.
  - Work Steps: inspect `src-tauri/Cargo.toml`, `package.json`, `src-tauri/src/lib.rs`, `default.json`, `NotificationSettings.svelte`, record evidence `E0-P0A-INSPECT1..6`, and create `actual-status.md`.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: baseline documented; status confirmed.

### P1: Tauri Autostart Plugin & Backend Runtime

- Phase Goal: Install and configure the official Tauri v2 autostart plugin and handle `--autostart` CLI launch.
- Phase Boundary:
  - In scope: `package.json`, `src-tauri/Cargo.toml`, `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`.
  - Out of scope: Frontend Svelte components.
  - Dependencies: None.
- Phase Implementation Rule: Implement `P1-A`, verify, then `P1-B`.
- Ordered Slice List:
  - P1-A: Add `@tauri-apps/plugin-autostart` and `tauri-plugin-autostart = "2"`.
  - P1-B: Configure capability permissions, register plugin, and add `--autostart` background launch handler.

- [x] P1-A: Add autostart dependencies
  - Goal: Add `@tauri-apps/plugin-autostart` using `bun add` and `tauri-plugin-autostart = "2"` in `Cargo.toml`.
  - Scope Boundary:
    - Editable: `package.json`, `src-tauri/Cargo.toml`.
    - Inspect-only: `bun.lock`, `src-tauri/Cargo.lock`.
  - Work Steps:
    1. Run `bun add @tauri-apps/plugin-autostart`.
    2. Add `tauri-plugin-autostart = "2"` to `src-tauri/Cargo.toml` under `[dependencies]`.
  - Implementation Gate: `package.json` and `Cargo.toml` ready.
  - Acceptance: Dependencies added without conflicts.
  - Evidence Targets: `E1-P1A-DEPS`.
  - Actual-status Update: Transition dependencies from `missing` to `correct`.
  - Commit Boundary: Slice P1-A.

- [x] P1-B: Capability permissions and Rust registration
  - Goal: Grant `"autostart:default"` in `default.json`, register plugin in `src-tauri/src/lib.rs`, and hide window on `--autostart`.
  - Scope Boundary:
    - Editable: `src-tauri/capabilities/default.json`, `src-tauri/src/lib.rs`.
  - Work Steps:
    1. Add `"autostart:default"` to `permissions` in `src-tauri/capabilities/default.json`.
    2. Add `.plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--autostart"])))` in `lib.rs`.
    3. In `setup()`, check `std::env::args().any(|a| a == "--autostart")`; if true and main window exists, hide it.
    4. Run `cargo check` to verify compilation.
  - Implementation Gate: P1-A complete.
  - Acceptance: `cargo check` passes cleanly.
  - Evidence Targets: `E1-P1B-BACKEND`.
  - Actual-status Update: Transition backend plugin and capability from `missing` to `correct`.
  - Commit Boundary: Slice P1-B.

### P2: Frontend Service, i18n & Settings UI

- Phase Goal: Provide frontend autostart service, localizations, and Settings toggle UI.
- Phase Boundary:
  - In scope: `src/lib/services/autostartService.ts`, `messages/*.json`, `src/lib/components/settings/NotificationSettings.svelte`.
  - Out of scope: Backend code.
  - Dependencies: P1 completed.
- Ordered Slice List:
  - P2-A: Create `autostartService.ts` and update i18n messages.
  - P2-B: Add Autostart toggle row to `NotificationSettings.svelte`.

- [x] P2-A: Autostart service & localization
  - Goal: Create `src/lib/services/autostartService.ts` and add localization keys to `messages/vi.json` and `messages/en.json`.
  - Scope Boundary:
    - Editable: `src/lib/services/autostartService.ts`, `messages/vi.json`, `messages/en.json`.
  - Work Steps:
    1. Implement `isAutostartEnabled` and `setAutostart` with try/catch fallback.
    2. Add keys `settings_notif_autostart`, `settings_notif_autostart_desc`, `settings_notif_status_autostart`, `settings_notif_err_update_autostart` to `messages/vi.json` and `messages/en.json`.
    3. Run `bun run i18n:compile`.
  - Implementation Gate: P1 completed.
  - Acceptance: `bun run i18n:compile` succeeds; service exported.
  - Evidence Targets: `E2-P2A-SERVICE`.
  - Actual-status Update: Transition service and i18n from `missing` to `correct`.
  - Commit Boundary: Slice P2-A.

- [x] P2-B: Settings UI toggle integration
  - Goal: Add Autostart toggle row in `NotificationSettings.svelte` with reactive state, badge, and toast feedback.
  - Scope Boundary:
    - Editable: `src/lib/components/settings/NotificationSettings.svelte`.
  - Work Steps:
    1. Import `isAutostartEnabled` and `setAutostart` into `NotificationSettings.svelte`.
    2. Add `autostartEnabled` state and query initial state on mount.
    3. Add toggle row with switch, badge, and change handler.
    4. Run `bun run check`.
  - Implementation Gate: P2-A completed.
  - Acceptance: UI renders cleanly and `bun run check` reports 0 errors.
  - Evidence Targets: `E2-P2B-UI`.
  - Actual-status Update: Transition settings UI from `missing` to `correct`.
  - Commit Boundary: Slice P2-B.

### P3: Quality Gate & Verification

- [x] P3-A: Quality gate, test execution, and change detection
  - Goal: Run all verification commands (`bun run check`, `bun test`, `cargo check`), run Anvien `detect-changes`, and verify release readiness.
  - Work Steps:
    1. Run `bun run check`.
    2. Run `cargo check`.
    3. Run `anvien detect-changes --repo /home/arch/Project/test/RustSale_v2 --scope all`.
    4. Record closure evidence.
  - Implementation Gate: P1 and P2 complete.
  - Acceptance: 100% tests/checks pass; clean detect-changes.
  - Evidence Targets: `E3-P3A-VERIFY`.
