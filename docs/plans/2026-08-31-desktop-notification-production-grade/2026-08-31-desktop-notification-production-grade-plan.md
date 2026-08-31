# Desktop Notification Production-Grade Plan

## Metadata

- Date: `2026-08-31`
- Status: `draft`
- Plan: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-plan.md`
- Evidence: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-evidence.md`
- Benchmark: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-benchmark.md`
- Actual status: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-actual-status.md`

## Goal

Bring the RustSale desktop notification subsystem to true Telegram/Discord production grade across all supported desktop platforms (Linux, Windows, macOS). Eliminate all UI freezes/stalls during message arrival and scrolling, ensure observable non-blocking notification delivery with robust error handling and fallback, implement smart focus suppression and burst rate-limiting coalescing, unify DND state with system tray, and ensure persistent audio synthesis without audio context churn or memory leaks.

## Rules

- Complete P0 actual status before implementation work.
- Update each checklist item immediately when it is completed.
- Record evidence as work completes.
- Record benchmarkable counts or measurements when they are taken.
- Update later phase status assumptions, next actions, and work steps when actual-status evidence changes the repo state.
- After completing a phase or implementation slice and refreshing `actual-status.md`, update the next affected phase's work steps as needed to match the latest repo reality, while preserving that phase's original goal, scope, acceptance criteria, and major phase order.
- Run Anvien detect-changes before every implementation-slice commit when implementation work was performed.
- Hidden fallback is forbidden. Prefer a visible failure or observable log over a fallback that hides a broken primary path.
- Tests must prove product behavior.

## Problem

When notifications arrive on Linux (X11/Wayland with WebKitGTK), the app could previously experience a 3-5 second UI freeze due to DMA-BUF hardware acceleration stalls with desktop notification overlays, cascading focus/blur sync loops during scrolling, and repeated WebAudio AudioContext instantiation. Additionally, notification dispatch error handling, focus state cache invalidation on IPC failure, and notification burst coalescing needed production hardening.

## Scope

- Rust Backend: `src-tauri/src/notification.rs`, `src-tauri/src/notification_test.rs`, `src-tauri/src/lib.rs`, `src-tauri/src/main.rs`.
- Frontend Services & Components: `src/lib/services/desktopNotificationService.ts`, `src/lib/services/__tests__/desktopNotificationService.spec.ts`, `src/lib/components/conversation/ConversationWorkspace.svelte`.
- Comprehensive Rust & frontend test suites with failure-path verification.

## Non-Goals

- Replacing native OS notification centers with custom in-app OpenGL/Canvas toast windows.
- Redesigning unrelated settings or conversation timeline layout.

## Requirements

1. **R1 - Non-blocking Linux Notification Dispatch & Robust Fallback**:
   - `dispatch_desktop_notification` must execute without blocking caller threads.
   - Child process errors, non-zero exit codes, or missing binaries must be caught and gracefully fall back to `tauri-plugin-notification` with clear logging.
2. **R2 - Smart Focus Suppression & Cache Resilience**:
   - Suppress notifications when active conversation is focused.
   - Focus sync cache must be invalidated on IPC errors to prevent permanent stale suppression state.
3. **R3 - Unified DND & Settings State**:
   - Tray DND must suppress notifications across all channels and group chats.
4. **R4 - WebAudio Singleton & Zero Churn**:
   - Persistent `sharedAudioCtx` singleton with error handling.
5. **R5 - UI Scroll Smoothness & Debounced Event Handlers**:
   - Maintain `WEBKIT_DISABLE_DMABUF_RENDERER=1` on Linux to prevent composite stalls.
   - Debounce window focus listeners in `ConversationWorkspace.svelte` (300-500ms).

## Acceptance Criteria

1. Rust unit tests in `src-tauri/src/notification_test.rs` pass with 100% assertions covering default settings, privacy mode, media content, focus suppression, tray DND, rate limiting, and mock dispatch.
2. Frontend unit tests in `src/lib/services/__tests__/desktopNotificationService.spec.ts` pass covering i18n previews, focus tracking, IPC error recovery, and WebAudio chime.
3. Full test suites (`cargo test`, `bun run test`, `bun run check`) pass with 0 errors.

## Checklist

- [x] P0-A: Complete actual status before implementation work.
  - Goal: Establish the real current state across backend notification manager, frontend service, and test suites.
  - Work Steps: Inspect `src-tauri/src/notification.rs`, `src/lib/services/desktopNotificationService.ts`, and test files. Record baseline classifications in actual status.
  - Implementation Gate: `actual-status.md` completed.
  - Acceptance: All rows classified with evidence IDs.

### P1: Production Hardening of Notification Subsystem

- Phase Goal: Complete Telegram/Discord production-grade notification dispatch, focus resilience, and DND integration.
- Phase Boundary:
  - In scope: `src-tauri/src/notification.rs`, `src-tauri/src/notification_test.rs`, `src/lib/services/desktopNotificationService.ts`, `src/lib/services/__tests__/desktopNotificationService.spec.ts`.
  - Out of scope: Unrelated conversation business logic.
- Ordered Slice List:
  - P1-A: Backend Notification Dispatch & Coalescing Hardening
  - P1-B: Frontend Focus Synchronization & WebAudio Test Verification

- [x] P1-A: Backend Notification Dispatch & Coalescing Hardening
  - Goal: Harden Linux notification dispatch with exit code verification, plugin fallback, DND suppression, and unit tests.
  - Scope Boundary: `src-tauri/src/notification.rs`, `src-tauri/src/notification_test.rs`.
  - Pre-flight Questions: Are all imports and platform-specific cfg flags cleanly scoped?
  - Work Steps:
    1. Verify `dispatch_desktop_notification` exit code checking and fallback logic.
    2. Ensure `NotificationManager` integrates `tray_state.is_dnd_enabled()`.
    3. Run `cargo test notification_test` and record output.
  - Implementation Gate: Code compiles and tests pass.
  - Acceptance: `cargo test notification_test` passes with 0 failures (9/9 passed).
  - Evidence Targets: `E1-P1A-RUST1`.
  - Actual-status Update: Transition `notification.rs` to `correct`.
  - Commit Boundary: Backend notification production hardening.

- [x] P1-B: Frontend Focus Synchronization & WebAudio Test Verification
  - Goal: Verify focus state cache invalidation on IPC failure, singleton audio context, and debounced workspace focus handlers.
  - Scope Boundary: `src/lib/services/desktopNotificationService.ts`, `src/lib/services/__tests__/desktopNotificationService.spec.ts`, `src/lib/components/conversation/ConversationWorkspace.svelte`.
  - Pre-flight Questions: Are all frontend tests deterministic?
  - Work Steps:
    1. Verify `desktopNotificationService.ts` sequence tracking and error handling.
    2. Run `bun test src/lib/services/__tests__/desktopNotificationService.spec.ts`.
    3. Run full test suite `bun run test` and `bun run check`.
  - Implementation Gate: All tests pass.
  - Acceptance: 100% tests green (655 passed, 0 failed; svelte-check 0 errors).
  - Evidence Targets: `E1-P1B-FE1`.
  - Actual-status Update: Transition frontend services to `correct`.
  - Commit Boundary: Frontend notification production hardening.
