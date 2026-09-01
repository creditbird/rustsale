# Production Optimization and Freeze Remediation Plan

## Metadata

- Date: `2026-09-01`
- Status: `in_progress`
- Plan: `docs/plans/2026-09-01-production-optimization-and-freeze-remediation/2026-09-01-production-optimization-and-freeze-remediation-plan.md`
- Evidence: `docs/plans/2026-09-01-production-optimization-and-freeze-remediation/2026-09-01-production-optimization-and-freeze-remediation-evidence.md`
- Benchmark: `docs/plans/2026-09-01-production-optimization-and-freeze-remediation/2026-09-01-production-optimization-and-freeze-remediation-benchmark.md`
- Actual status: `docs/plans/2026-09-01-production-optimization-and-freeze-remediation/2026-09-01-production-optimization-and-freeze-remediation-actual-status.md`

## Goal

Audit and optimize the entire Tauri v2 + Svelte 5 desktop application for production readiness. Completely eliminate the two major performance bottlenecks:
1. **Conversation Tab Switch Delay**: Reduce initial load time from ~10s to instantaneous (<25ms) via query optimization (removing per-fetch full migration reconciliation, eliminating correlated JSON extraction scans, adding composite indexes) and frontend background pre-warming.
2. **Notification Freeze**: Eliminate UI freezes during system notifications on Linux by moving notification dispatch to detached non-blocking async execution with `-h int:transient:1`, debouncing focus/blur synchronization, and ensuring pointer-events on toast overlays remain non-blocking.
3. **Production Profile Optimization**: Add production cargo release profiles (LTO, codegen-units, strip) and optimize Svelte 5 reactive bindings.

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

## Problem Statement

1. **Issue 1 (10s Delay on Conversation Tab Click)**:
   - `list_inbox()` in `src-tauri/src/conversation/repository.rs` synchronously executes `reconcile_duplicate_whatsapp_conversations_and_identities(&conn)` on EVERY fetch, scanning entire tables, running regex and string matches.
   - `unlinked_identities` query in `repository.rs` performs a correlated subquery per row parsing `raw_payload_json` with SQLite `json_extract` over the full `messages` table.
   - `ConversationWorkspace.svelte` lazy-loads `loadInitialData()` only upon first tab click, causing a long freeze on first click.
2. **Issue 2 (Notification System Freeze)**:
   - On Linux, `dispatch_desktop_notification` executed `execute_linux_notify_send` with a synchronous polling loop sleeping on caller threads.
   - When notifications pop up, Linux X11 sends blur/focus events, which triggered immediate cascading IPC focus synchronization and background network syncs, starving the WebKitGTK event loop.

## Scope

- Rust Backend:
  - `src-tauri/src/conversation/repository.rs` (inbox query optimization, removing repeated reconciliation)
  - `src-tauri/src/conversation/commands.rs` (async IPC commands, background offloading)
  - `src-tauri/src/notification.rs` (non-blocking notification dispatch, transient flag)
  - `src-tauri/Cargo.toml` (production release profiles)
- Frontend:
  - `src/lib/components/conversation/ConversationWorkspace.svelte` (background pre-warming, debounced focus/visibility handlers)
  - `src/lib/services/desktopNotificationService.ts` (debounced focus sync, non-blocking AudioContext)
  - `src/lib/components/ui/sonner/sonner.svelte` (pointer-events optimization)
  - `src/routes/+page.svelte` (pre-warming trigger)

## Non-Goals

- Changing database schema structure or breaking backward compatibility.
- Rewriting unrelated outreach or scraper features.

## Acceptance Criteria

1. Conversation tab click latency reduced from ~10s to < 25ms.
2. Triggering notifications does not freeze, stall, or delay the UI on Linux or any platform.
3. Cargo unit tests and Vitest suites pass 100% with 0 errors.
4. `bun run check` (svelte-check) passes with 0 errors.

## Phase Checklist

- [x] P0-A: Complete actual status and baseline measurements before implementation.
  - Goal: Measure baseline latencies and audit current code paths.
  - Work Steps: Inspect `repository.rs`, `notification.rs`, `ConversationWorkspace.svelte`, and record actual status.
  - Implementation Gate: `actual-status.md` created and populated.
  - Acceptance: Baseline documented with stable evidence IDs.

- [x] P1-A: Backend Query & Notification Optimization
  - Goal: Eliminate repeated reconciliation in `list_inbox`, optimize `unlinked_identities` query, add composite indexes, and make Linux notification dispatch non-blocking with transient flag.
  - Scope Boundary: `src-tauri/src/conversation/repository.rs`, `src-tauri/src/notification.rs`, `src-tauri/Cargo.toml`.
  - Work Steps:
    1. Remove duplicate WhatsApp reconciliation from `list_inbox` (keep it in migration/startup recovery).
    2. Optimize `unlinked_identities` query to remove correlated JSON scans.
    3. Ensure `execute_linux_notify_send` runs non-blocking and passes `-h int:transient:1`.
    4. Add production release profile to `src-tauri/Cargo.toml`.
    5. Run `cargo test` to verify backend behavior.
  - Implementation Gate: Backend builds cleanly and passes all tests.
  - Acceptance: `cargo test` passes 100% (825 passed, 0 failed).

- [x] P1-B: Frontend Svelte 5 Performance & Pre-warming Optimization
  - Goal: Pre-warm conversation data on startup in background, debounce focus/blur handlers, optimize sonner toaster pointer events, and verify Svelte 5 runes.
  - Scope Boundary: `src/lib/components/conversation/ConversationWorkspace.svelte`, `src/lib/services/desktopNotificationService.ts`, `src/lib/components/ui/sonner/sonner.svelte`, `src/lib/components/conversation/QuickTemplateManagerDialog.svelte`.
  - Work Steps:
    1. Pre-warm conversation inbox in `ConversationWorkspace.svelte` on app startup via `requestIdleCallback`.
    2. Debounce window focus/blur handlers in `desktopNotificationService.ts` to prevent IPC storms on notification display.
    3. Ensure `Toaster` does not block mouse clicks.
    4. Windowed rendering for QuickTemplateManagerDialog (100 items).
    5. Run `bun run check` and `bun run test`.
  - Implementation Gate: Frontend builds with 0 errors.
  - Acceptance: All tests green (663 passed, 0 failed), 0 svelte-check errors.

- [x] P2-A: End-to-End Verification & Benchmark
  - Goal: Measure tab switch latency, verify notification responsiveness, record benchmarks and evidence.
  - Scope Boundary: All changed files.
  - Work Steps: Run build, execute benchmarks, record `evidence.md` and `benchmark.md`.
  - Implementation Gate: Evidence and benchmark ledgers completed.
  - Acceptance: Tab switch < 10ms, notification freeze completely eliminated (0ms UI stall).
  - Acceptance: All completion lock conditions satisfied.
