# Desktop Notification Production-Grade Actual Status

Title: Desktop Notification Production-Grade Actual Status
Date: 2026-08-31
Status: P0 Complete
Companion plan: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-plan.md`
Companion evidence: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-evidence.md`
Companion benchmark: `docs/plans/2026-08-31-desktop-notification-production-grade/2026-08-31-desktop-notification-production-grade-benchmark.md`

## Purpose

This file records the real current state before implementation.
Implementation must not start until the target scope has a completed status row, evidence IDs, and a downstream plan decision.

## Scope

Target scope:
- `src-tauri/src/notification.rs`
- `src-tauri/src/notification_test.rs`
- `src-tauri/src/main.rs`
- `src-tauri/src/lib.rs`
- `src/lib/services/desktopNotificationService.ts`
- `src/lib/services/__tests__/desktopNotificationService.spec.ts`
- `src/lib/components/conversation/ConversationWorkspace.svelte`

Out of scope:
- Unrelated customer lead forms, Zalo QR pairing, or settings themes.

## Relationship / Impact Evidence

| Unit / File / Surface | File Detail Evidence | Related File Count | Relationship Summary | Impact Note |
|---|---|---|---|---|
| `src-tauri/src/notification.rs` | `E0-P0A-SRC1` | 4 related files | Rust notification manager, DND state, rate limiting, dispatch | High file-level risk (core notification surface) |
| `src-tauri/src/main.rs` | `E0-P0A-SRC2` | 2 related files | Linux WebKitGTK DMA-BUF renderer environment toggle | Low risk |
| `src/lib/services/desktopNotificationService.ts` | `E0-P0A-SRC3` | 6 related files | Frontend focus tracking, AudioContext singleton, Tray DND bridge | Medium risk |
| `src/lib/components/conversation/ConversationWorkspace.svelte` | `E0-P0A-SRC4` | 18 related files | Conversation workspace debounced focus listeners | High risk |

## Current Status Matrix

| Unit | Current State | Required State | Status | Relationship Count | Evidence | Next Plan Decision |
|---|---|---|---|---|---|---|
| `src-tauri/src/notification.rs` | Linux dispatch with exit-code verification, plugin fallback, rate limit | Telegram/Discord production-grade non-blocking dispatch | `correct` | 4 related files | `E0-P0A-SRC1`, `E1-P1A-RUST1` | edit P1-A / verify |
| `src-tauri/src/notification_test.rs` | Unit tests for settings, formatting, DND, focus, rate limit, mock dispatch | 100% passing assertive tests | `correct` | 2 related files | `E0-P0A-TEST1`, `E1-P1A-RUST1` | edit P1-A / verify |
| `src-tauri/src/main.rs` | `WEBKIT_DISABLE_DMABUF_RENDERER=1` toggle set on Linux | Disable DMA-BUF buffer swap stalls on Linux | `correct` | 2 related files | `E0-P0A-SRC2` | preserve |
| `src/lib/services/desktopNotificationService.ts` | Shared AudioContext singleton, IPC failure cache invalidation, Tray DND bridge | Robust focus sync and audio chime | `correct` | 6 related files | `E0-P0A-SRC3`, `E1-P1B-FE1` | edit P1-B / verify |
| `src/lib/services/__tests__/desktopNotificationService.spec.ts` | 6 unit tests covering i18n, focus sync, IPC recovery, WebAudio | 100% passing tests | `correct` | 2 related files | `E0-P0A-TEST2`, `E1-P1B-FE1` | edit P1-B / verify |
| `src/lib/components/conversation/ConversationWorkspace.svelte` | Debounced focus listeners (300ms/500ms) | No burst re-sync on transient notification focus | `correct` | 18 related files | `E0-P0A-SRC4` | preserve |

## Status Refresh Log

| Refresh | Date | Repo Basis | Changed Scope | Status Changes | Evidence | Next Phase Update |
|---|---|---|---|---|---|---|
| R0 | 2026-08-31 | baseline before P0 | Desktop notification subsystem | initial classification | `E0-P0A-SRC1..E0-P0A-SRC4` | Proceed to P1-A |

## Phase Touch Map

| Unit / File / Surface | Plan-Relevant Relationship File | Relationship to Target | Plan Item | Touch Mode | Evidence | Constraint |
|---|---|---|---|---|---|---|
| `src-tauri/src/notification.rs` | `src-tauri/src/lib.rs` | consumer | P1-A | edit | `E1-P1A-RUST1` | Do not change public IPC signatures |
| `src-tauri/src/notification_test.rs` | `src-tauri/src/notification.rs` | test | P1-A | edit | `E1-P1A-RUST1` | Must test failure and fallback |
| `src/lib/services/desktopNotificationService.ts` | `ConversationWorkspace.svelte` | service | P1-B | edit | `E1-P1B-FE1` | Preserve AudioContext singleton |
| `src/lib/services/__tests__/desktopNotificationService.spec.ts` | `desktopNotificationService.ts` | test | P1-B | edit | `E1-P1B-FE1` | Must test IPC failure retry |
