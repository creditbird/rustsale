# Supervisor Report: Desktop Notification Freeze and Production Readiness

Verdict: REJECT

## Metadata
- Report file: `rp_supervisor_260831_214927_by_gpt-5.6-codex_desktop_notification_freeze.md`
- Review time: 260831 214927 +07 (Asia/Ho_Chi_Minh)
- Reviewer: gpt-5.6-codex
- Repo/project: RustSale_v2
- Scope reviewed: current uncommitted worktree affecting desktop notifications and focus handling
- Claim reviewed: the Linux desktop-notification freeze has a confirmed D-Bus/X11 plus WebAudio root cause, the applied changes fix it, and RustSale notifications are production-ready
- Authority used: latest user request, `AGENTS.md`, current source/diff, locked dependency source, Tauri notification documentation from Context7, fresh build/test/check output, Anvien and GitNexus change analysis
- Related artifacts: current worktree diff; no fresh runtime latency trace or official desktop interaction evidence was supplied

## Executive Summary
- Problem: determine whether the stated RCA is technically supported and whether the current Tauri notification implementation is ready for production.
- Decision: REJECT. The build and reported test counts are reproducible, and the shared `AudioContext` change is directionally sensible, but the central D-Bus/X11 RCA conflicts with the actual call path, notification delivery errors are hidden, focus deduplication can permanently cache a failed IPC state, DND is not connected to notification suppression, and no runtime evidence measures the original 3-5 second UI freeze.
- Required outcome: correct the delivery/focus/DND contracts, replace the pass-by-default timing test with fault and runtime interaction evidence, and re-run acceptance on the real desktop runtime.

## Blocking Findings

### [HIGH] The stated D-Bus/X11 root cause is not established by the actual call path
File: `src-tauri/src/lib.rs:281`, `src-tauri/src/lib.rs:290`, `src-tauri/src/notification.rs:171`

Issue: the RCA says `notify-rust` executes directly on RustSale's UI/event-loop thread and blocks WebKitGTK/X11 until the popup expires. In the current pre-fix caller, notification work already originates inside `tauri::async_runtime::spawn` and then `tokio::task::spawn_blocking`. In locked `tauri-plugin-notification` 2.3.3, `NotificationBuilder::show()` additionally uses `tauri::async_runtime::spawn` before calling `notify_rust::Notification::show()` (`~/.cargo/registry/src/.../tauri-plugin-notification-2.3.3/src/desktop.rs:216`). The locked `notify-rust` D-Bus payload contains app name, ID, icon, summary, body, actions, hints, and timeout; no X11 window ID or app PID is passed (`~/.cargo/registry/src/.../notify-rust-4.18.0/src/xdg/zbus_rs.rs:141`).

Evidence: source inspection confirms `zbus::block_on` exists in `notify-rust`, but it runs behind background scheduling rather than on the GTK/WebView main loop. No runtime trace, focus-grab trace, D-Bus capture, or profiler output demonstrates the claimed PID/window association. Context7 confirms the official plugin supports Linux and the documented capability setup, but does not document this freeze mechanism.

Why this blocks acceptance: process separation may still be an empirical workaround, but the stated causal explanation cannot be accepted as root cause. Production rollout based on an unproven mechanism risks masking the real focus, renderer, daemon, or event-loop trigger.

Fix direction: capture a reproducible before/after trace on XFCE/X11: main-loop/input latency, focus/blur events, D-Bus request timing, audio initialization/close timing, and notification daemon behavior. Describe the result as a workaround until the causal link is proven.

Re-review evidence required: reproducible runtime trace showing which operation blocks input for 3-5 seconds and evidence that the selected change removes that block without relying only on popup timing.

### [HIGH] Linux dispatch reports success even when no notification was delivered
File: `src-tauri/src/notification.rs:183`

Issue: `dispatch_desktop_notification` returns `()`. The result of `std::thread::Builder::spawn` is discarded. If `notify-send` starts but exits non-zero because D-Bus is unavailable, an option is unsupported, or the daemon rejects the request, the exit status from `child.wait()` is ignored and fallback is never invoked. Fallback only executes when the executable cannot be spawned. `test_desktop_notification` therefore always returns `Ok(())` and emits `desktop-notification-sent`, causing the frontend to play a chime and show a success toast even when delivery failed.

Evidence: `src-tauri/src/notification.rs:194-229`, `src-tauri/src/notification.rs:273-294`, and `src/lib/components/settings/NotificationSettings.svelte:125-132`.

Why this blocks acceptance: silent notification loss and false success are incompatible with a production notification contract.

Fix direction: return a typed asynchronous result; check both thread/task creation and child exit status; log stderr/status safely; trigger fallback on any delivery failure; emit success only after an accepted delivery result. Use a bounded worker/task strategy rather than one untracked thread and child per notification.

Re-review evidence required: tests for missing binary, non-zero child exit, daemon unavailable, thread/task failure, fallback success/failure, and truthful UI status.

### [HIGH] Focus deduplication can preserve stale backend state after one IPC failure
File: `src/lib/services/desktopNotificationService.ts:82`

Issue: `lastSyncedFocus` is updated before `invoke('set_active_conversation_focus')` succeeds. If that invoke fails, a later call with the same state returns early and never retries, while the backend retains its previous focus state.

Evidence: cache assignment occurs at line 100; IPC begins at line 102; the catch only logs at lines 107-109. The frontend service tests do not inject an IPC failure or assert deduplication/retry behavior.

Why this blocks acceptance: stale focus state can either suppress notifications that should be shown or show notifications for the active conversation. This is directly inside the invariant the change claims to improve.

Fix direction: commit the cache only after successful IPC, or clear/rollback it on failure; serialize or version concurrent focus updates so an older completion cannot overwrite newer state.

Re-review evidence required: deterministic tests for failure then identical retry, rapid focus/blur races, and out-of-order IPC completion.

### [HIGH] DND is displayed as active but is not consulted by notification suppression
File: `src-tauri/src/tray.rs:50`, `src-tauri/src/notification.rs:115`

Issue: `TrayState::dnd_enabled` and `NotificationManager` are separate state owners. `should_suppress` checks enabled, group mute, and focus only; it never checks tray DND. The test named `test_should_suppress_notification_when_dnd_or_disabled` does not enable DND at all; it tests disabled notifications and group mute. The frontend declares `unlistenTrayDndChanged` but never registers the listener.

Evidence: `src-tauri/src/tray.rs:112-116`, `src-tauri/src/tray.rs:163-165`, `src-tauri/src/notification.rs:115-130`, `src-tauri/src/notification_test.rs:80-105`, and `src/lib/services/desktopNotificationService.ts:21,255-277`.

Why this blocks acceptance: a user-visible "Do Not Disturb" control currently does not enforce its advertised behavior.

Fix direction: define one authoritative DND owner and make notification suppression consult it; wire state-change events and persistence consistently.

Re-review evidence required: backend and mounted UI tests showing DND suppresses both popup and chime from settings and tray entry points, then restores delivery when disabled.

### [MEDIUM] The 9/9 notification result does not test timing, delivery, or UI responsiveness
File: `src-tauri/src/notification_test.rs:121`

Issue: `test_live_desktop_notification_timing` has no assertion. It does not assert the command result, elapsed threshold, child result, popup presence, focus behavior, click/keyboard responsiveness, or audio latency. Because dispatch returns immediately after spawning a thread, the measured elapsed time excludes the relevant work.

Evidence: the focused Rust suite completes all nine tests in `0.00s`, which itself shows that it did not wait for the 3.5-second notification lifecycle. Frontend tests have no coverage for `playNotificationChime`, failed focus sync, or Linux process fallback.

Why this blocks acceptance: green suites prove compilation and existing logic, not closure of the reported runtime freeze.

Fix direction: replace the test with assertive unit/fault tests and add reusable real-runtime QA that fires notifications while continuously measuring input/event-loop responsiveness for longer than the popup timeout.

Re-review evidence required: fresh XFCE/X11 runtime evidence plus at least one other supported Linux environment; include latency distribution and no-click-loss assertions.

### [MEDIUM] The reviewed diff contains app-wide changes omitted from the fix summary
File: `src-tauri/src/main.rs:4`, `src/lib/components/conversation/ConversationWorkspace.svelte:683`

Issue: the worktree globally forces `WEBKIT_DISABLE_DMABUF_RENDERER=1` for all Linux launches and changes two conversation focus/visibility refresh paths with 300/500 ms timers. Neither change is described in the supplied fix summary or covered by focused regression evidence. The two debounced callbacks still execute independently, so this is not evidence of a single deduplicated path.

Evidence: Git diff and Anvien change detection identify five changed files, one affected notification execution flow, and high file-level risk for `notification.rs`; the summary only describes two of the four production-code areas.

Why this blocks acceptance: the completion claim is narrower than the actual behavior change, and the renderer flag changes app-wide Linux rendering/performance characteristics.

Fix direction: separate or explicitly justify these changes; test conversation recovery timing and rendering/input performance with and without the renderer override.

Re-review evidence required: scoped rationale, benchmark/runtime evidence, and sibling-flow regression coverage.

## Source-Level Clearance Notes
- `src-tauri/src/notification.rs`: blocked - delivery result/fallback contract is not reliable and the RCA is not supported by the prior execution boundaries.
- `src/lib/services/desktopNotificationService.ts`: blocked - singleton reuse is reasonable, but focus cache failure semantics are unsafe and the audio path lacks focused tests.
- `src-tauri/src/notification_test.rs`: blocked - new timing test is pass-by-default.
- `src-tauri/src/main.rs`: blocked - global renderer override lacks scoped evidence.
- `src/lib/components/conversation/ConversationWorkspace.svelte`: blocked - additional debounce behavior is unreported and unverified.
- Tauri plugin registration/capability: clear - plugin is registered in `src-tauri/src/lib.rs:101` and `notification:default` exists in `src-tauri/capabilities/default.json:12`, matching Context7's official setup guidance.

## Evidence Checked

Passed:
- `bun run tauri build --no-bundle`: PASS; optimized release application built at `src-tauri/target/release/rustsale-crm`.
- `cargo test notification_test --manifest-path src-tauri/Cargo.toml`: PASS; 9 passed, 0 failed, 811 filtered out.
- `bun run test`: PASS; 654 passed, 1 skipped, 0 failed across 70 files.
- `bun run check`: PASS with 0 errors and 6 warnings in 3 unrelated files.
- `git diff --check`: PASS.
- Context7 `/tauri-apps/plugins-workspace`: confirms plugin registration, `notification:default`, and Linux support.
- Anvien refreshed with `anvien analyze --force`; detect-changes found 5 changed files and the `Notify_inbound -> New` flow. Symbol impact for `dispatch_desktop_notification` is LOW with 2 direct callers in one file and one affected process; file-level notification risk is HIGH.
- GitNexus detect-changes independently found the same 5 changed files.
- Verification freshness: fresh/current for this worktree on 2026-08-31.

Failed:
- Production-readiness review: source-level blockers above remain open.

Not run:
- Automated interaction/latency measurement against the visible XFCE/X11 RustSale runtime.
- Fault-injection matrix for missing/non-zero/hanging `notify-send` and unavailable notification daemon.
- Wayland, other Linux desktop environments, Windows, and macOS runtime matrix.
- Notification click/action behavior and application activation.

## Invariant Closure
- affected invariant: an inbound message must produce at most one truthful, non-blocking popup/chime according to focus, DND, group mute, privacy, and user settings; failure must be observable and must not poison future focus state.
- sibling surfaces checked: inbound event forwarder, notification manager/dispatch, Tauri plugin implementation, locked `notify-rust` implementation, frontend focus/audio bridge, tray DND state, settings UI, focused Rust tests, frontend service tests, capability registration, and full worktree diff.
- residual unverified same-invariant surfaces: real desktop input latency, daemon/process failure modes, platform matrix, click activation, and persisted settings across restart.

## Required Fix List For Resubmission
1. Make Linux dispatch result-bearing, bounded, observable, and fallback on non-zero delivery failures.
2. Repair focus dedupe so failed/out-of-order IPC cannot leave stale backend state.
3. Connect DND to popup and chime suppression through one authoritative state path.
4. Replace the timing smoke test with assertions and fault tests; add real-runtime input-latency evidence that reproduces and closes the original freeze.
5. Separate or justify the global DMABUF and conversation refresh changes with regression/performance evidence.
6. Reframe the RCA to match proven evidence; do not claim PID/window pointer-grab causality without a trace.

## Overall Evaluation
The patch compiles and broad regression suites remain green. It also removes repeated `AudioContext.close()` churn, which is a useful optimization. Those facts are insufficient for production acceptance: the central RCA is contradicted by the existing background execution boundaries, the new Linux adapter can silently lose notifications while reporting success, and focus/DND correctness remains open. The implementation is a promising experimental workaround, not a verified production-grade notification subsystem.
