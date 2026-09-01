# Evidence Ledger

## Metadata

- Plan: `docs/plans/2026-09-01-production-optimization-and-freeze-remediation/2026-09-01-production-optimization-and-freeze-remediation-plan.md`
- Date: `2026-09-01`

## Baseline Evidence (E0)

- `E0-P0A-RUST1`: `src-tauri/src/conversation/repository.rs:1333` invokes `reconcile_duplicate_whatsapp_conversations_and_identities(&conn)` on every call to `list_inbox()`. Lines 1603-1619 perform correlated subqueries parsing JSON across `messages`.
- `E0-P0A-RUST2`: `src-tauri/src/notification.rs:252` runs `execute_linux_notify_send` with a polling loop and `thread::sleep(20ms)` for up to 1.5s synchronously.
- `E0-P0A-FE1`: `src/lib/components/conversation/ConversationWorkspace.svelte:666` only calls `loadInitialData()` after `activeTab === 'conversation'`, causing ~10s lag on first click.
- `E0-P0A-FE2`: `src/lib/services/desktopNotificationService.ts:298-311` attaches direct un-debounced focus/blur listeners calling IPC on every focus change.
- `E0-P0A-FE3`: `src/lib/components/ui/sonner/sonner.svelte` wraps `svelte-sonner`.
- `E0-P0A-RUST3`: `src-tauri/Cargo.toml` has default profile settings without LTO or symbol stripping.

## Implementation Evidence (E1)

- `E1-P1A-RUST1`: `src-tauri/src/conversation/repository.rs` modified to remove per-fetch WhatsApp reconciliation from `list_inbox()`. The correlated `messages` JSON table scan in `unlinked_identities` was replaced with targeted single-identity lookups for misattributed identities.
- `E1-P1A-RUST2`: `src-tauri/src/notification.rs` updated: `execute_linux_notify_send` now spawns with `-h int:transient:1` and `-h string:desktop-entry:rustsale-crm`, executing via fast `output()` without sleep loops. `test_desktop_notification` offloads to `spawn_blocking`.
- `E1-P1A-RUST3`: `src-tauri/Cargo.toml` configured with `[profile.release]` (`opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`).
- `E1-P1B-FE1`: `src/lib/components/conversation/ConversationWorkspace.svelte` pre-warms `loadInitialData()` in `onMount` via `requestIdleCallback`, eliminating cold-start latency when switching tabs.
- `E1-P1B-FE2`: `src/lib/services/desktopNotificationService.ts` debounces window `focus` and `blur` events by 150ms.
- `E1-P1B-FE3`: `src/lib/components/ui/sonner/sonner.svelte` toaster container styled with `pointer-events-none [&_[data-sonner-toast]]:pointer-events-auto` ensuring background clicks are never blocked.
- `E1-P1B-FE4`: `src/lib/components/conversation/QuickTemplateManagerDialog.svelte` windowed to render first 100 templates with remainder indicator, preventing DOM stalling on large template lists.

## Verification Evidence (E2)

- `E2-P2A-RUST_TEST`: `cargo test --manifest-path src-tauri/Cargo.toml` completed with `825 passed; 0 failed; finished in 33.43s`.
- `E2-P2A-FE_TEST`: `bun test` completed with `663 pass; 1 skip; 0 fail; 4728 expect() calls; Ran 664 tests across 70 files`.
- `E2-P2A-SVELTE_CHECK`: `bun run check` completed with `0 errors, 6 warnings in 3 files`.
- `E2-P2A-BUILD`: `bun run build` completed static adapter generation to `build/` in 19.09s.
- `E2-P2A-RELEASE_CHECK`: `cargo check --release --manifest-path src-tauri/Cargo.toml` finished release profile check with 0 warnings/errors.
