# System Tray & Desktop Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement full-featured System Tray and Desktop Notifications for RustSale CRM matching Telegram/Discord user experience (Close-to-Tray background execution, dynamic Tray menu, OS notification popups for inbound Zalo/Facebook messages, smart focus suppression, deep link navigation on click, audio chime, and configurable settings).

**Architecture:** 
- **Rust Backend (`src-tauri`)**: Uses Tauri v2 `TrayIconBuilder` and `tauri-plugin-notification`. A dedicated `TrayState` and tray module handles window minimize-to-tray interception and tray menu interactions. A `NotificationManager` listens to inbound message events from `ConversationService` and Facebook supervisor, applies rate-limiting/debouncing, checks window focus state, and triggers native OS desktop notifications.
- **Svelte Frontend (`src/`)**: `desktopNotificationService.ts` tracks active window focus and selected conversation, listens for notification clicks/deep-links from Rust to switch tabs to `conversation` and select the target chat, provides in-app synthesized dual-tone audio chime (D5 -> A5), and exposes `NotificationSettings.svelte` within the Settings module with i18n support.
- **Persistence**: User notification preferences (enable/disable, close-to-tray, privacy mode, sound, mute groups) stored in Rust state / local persistence.

**Tech Stack:** Rust (Tauri v2, `tauri-plugin-notification`, `rusqlite`, `tokio`), Svelte 5 (Runes `$state`, `$derived`, `$effect`), TypeScript, Bits UI / shadcn-svelte, Tailwind CSS, Lucide icons, Paraglide i18n.

## Global Constraints

- **ALWAYS use `bun` and `bunx` instead of `node`, `npm`, and `npx`** for all commands and scripts.
- Adhere strictly to [`docs/IMPLEMENTATION_COMPLETION_HARNESS.md`](docs/IMPLEMENTATION_COMPLETION_HARNESS.md).
- Do not break existing background tasks (Google Maps crawler, Zalo supervisor, Facebook sync).
- Ensure graceful fallback if OS notifications or tray icons are disabled/unsupported on certain Linux environments.

---

## Requirement Ledger

| ID | Exact requirement | Invariant | Positive case | Negative/failure case | Required evidence | Owning plan step | Status |
|---|---|---|---|---|---|---|---|
| R-01 | System Tray Icon & Menu | Tray icon is present in OS status bar with context menu (Open, Mark Read, Mute DND, Quit) | Clicking Tray item opens/toggles app window or quits cleanly | Missing icon file or headless environment handled gracefully | Rust unit/integration test & manual verify (`src-tauri/src/tray_test.rs`) | Task 1 & 2 | `VERIFIED` |
| R-02 | Close to Tray Behavior | Clicking window 'X' button hides window to tray when enabled instead of killing process | Window hidden, background sync (Zalo/FB) continues running | When 'close_to_tray' setting is false, app exits normally | Window event interceptor test & log evidence | Task 2 | `VERIFIED` |
| R-03 | Inbound Message Notification | New inbound message triggers native desktop notification with sender name & preview | Inbound Zalo/FB message displays OS notification | Outbound or duplicate message does NOT trigger notification | Rust unit test in `notification_test.rs` | Task 3 | `VERIFIED` |
| R-04 | Smart Focus Suppression | If user is actively focused on the specific conversation, suppress OS popup | In-app chime only, no duplicate OS notification popup | When window unfocused or in different tab, OS notification fires | Focus tracking unit test in `notification_test.rs` & `desktopNotificationService.spec.ts` | Task 3 & 4 | `VERIFIED` |
| R-05 | Notification Click Deep Linking | Clicking desktop notification brings window to front and opens target conversation | Window restored, tab switched to 'conversation', chat selected | Invalid or deleted conversation ID handled gracefully without crash | Frontend integration test & bridge wiring in `+page.svelte` | Task 4 | `VERIFIED` |
| R-06 | Notification & Tray Settings UI | User can toggle notifications, sound, privacy mode, close-to-tray, and test notification | Settings saved and applied immediately; test button fires sample notification | DB/storage error handled with error toast | Svelte component test (`NotificationSettings.spec.ts`) | Task 5 | `VERIFIED` |
| R-07 | Rate Limiting / Debounce | Rapid burst of messages from same contact grouped / debounced | 10 rapid messages produce 1 aggregated notification | Burst threshold resets after cooldown period | Unit test in `notification_test.rs` (`test_rate_limiting_debounces_rapid_burst`) | Task 3 | `VERIFIED` |

---

## File Structure

### Created Files
- `src-tauri/src/tray.rs`: System tray initialization, icon management, tray menu events, and window close interception.
- `src-tauri/src/tray_test.rs`: Unit tests for tray menu actions and configuration.
- `src-tauri/src/notification.rs`: Notification dispatcher, rate-limiter, privacy mask, and focus evaluation engine.
- `src-tauri/src/notification_test.rs`: Unit tests for inbound message notification logic, throttling, and privacy formatting.
- `src/lib/services/desktopNotificationService.ts`: Frontend bridge for focus state sync, notification click handler, and audio chime.
- `src/lib/services/__tests__/desktopNotificationService.spec.ts`: Unit tests for desktop notification frontend service.
- `src/lib/components/settings/NotificationSettings.svelte`: Settings UI card for Desktop Notification & System Tray options.
- `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`: Component tests for NotificationSettings.

### Modified Files
- `src-tauri/Cargo.toml`: Add `tauri-plugin-notification = "2"` dependency.
- `src-tauri/src/lib.rs`: Register notification plugin, setup tray icon, window event handler, and notification commands.
- `src-tauri/capabilities/default.json`: Add `"notification:default"` capability permission.
- `package.json`: Add `@tauri-apps/plugin-notification` dependency.
- `messages/vi.json` & `messages/en.json`: Add localized strings for notification settings and tray menu.
- `src/lib/components/Settings.svelte`: Integrate `NotificationSettings.svelte` into settings tabs/cards.
- `src/routes/+page.svelte`: Wire up `desktopNotificationService` initialization, focus tracking, and deep-link navigation.

---

## Tasks

### Task 1: Add Tauri Notification Plugin & Permissions
**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`
- Modify: `package.json`

**Interfaces:**
- Consumes: Tauri v2 plugin system
- Produces: `tauri-plugin-notification` available in Rust and `@tauri-apps/plugin-notification` in TypeScript.

- [x] **Step 1: Update Cargo.toml with `tauri-plugin-notification = "2"` and `tauri = { version = "2", features = ["tray-icon", "image-png"] }`**
- [x] **Step 2: Update package.json with `@tauri-apps/plugin-notification` using `bun add @tauri-apps/plugin-notification`**
- [x] **Step 3: Update `src-tauri/capabilities/default.json` to include `"notification:default"`**
- [x] **Step 4: Verify `cargo check` and `bun run check` succeed**

---

### Task 2: System Tray & Window Close-To-Tray Implementation
**Files:**
- Create: `src-tauri/src/tray.rs`
- Create: `src-tauri/src/tray_test.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `init_tray(app: &tauri::AppHandle) -> Result<()>`, `TrayState`, `TrayConfig`, `get_tray_state`, `set_close_to_tray`, `set_tray_dnd`.

- [x] **Step 1: Write tests in `src-tauri/src/tray_test.rs` for tray menu actions and event state**
- [x] **Step 2: Implement `src-tauri/src/tray.rs` with `TrayIconBuilder`, menu items (Show, Mute, Mark Read, Quit), and click events**
- [x] **Step 3: Hook `WindowEvent::CloseRequested` in `src-tauri/src/lib.rs` to hide window instead of exiting when `close_to_tray` is enabled**
- [x] **Step 4: Run `cargo test tray` and verify tests pass (3 passed)**

---

### Task 3: Notification Dispatcher & Smart Rate Limiting (Rust Backend)
**Files:**
- Create: `src-tauri/src/notification.rs`
- Create: `src-tauri/src/notification_test.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `NotificationManager`, commands `get_notification_settings`, `update_notification_settings`, `test_desktop_notification`, `set_active_conversation_focus`

- [x] **Step 1: Write tests in `src-tauri/src/notification_test.rs` for rate limiting, privacy text masking, and focus check**
- [x] **Step 2: Implement `NotificationManager` in `src-tauri/src/notification.rs` with debouncing and OS notification dispatch**
- [x] **Step 3: Hook inbound message arrival in `lib.rs` conversation subscriber to trigger `notification_manager.notify_inbound()`**
- [x] **Step 4: Register Tauri commands in `src-tauri/src/lib.rs`**
- [x] **Step 5: Run `cargo test notification` and verify all tests pass (8 passed)**

---

### Task 4: Frontend Notification Bridge & Deep-Linking (TypeScript/Svelte)
**Files:**
- Create: `src/lib/services/desktopNotificationService.ts`
- Create: `src/lib/services/__tests__/desktopNotificationService.spec.ts`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: Rust `notification-clicked` event, `set_active_conversation_focus` command
- Produces: `initDesktopNotificationBridge(handlers)`, `playNotificationChime()`, deep navigation to conversation.

- [x] **Step 1: Write tests in `src/lib/services/__tests__/desktopNotificationService.spec.ts`**
- [x] **Step 2: Implement `src/lib/services/desktopNotificationService.ts` with window focus listeners, chime audio synthesizer/player, and deep-link router**
- [x] **Step 3: Wire into `src/routes/+page.svelte` onMount to handle notification navigation and conversation tracking**
- [x] **Step 4: Run `bun test src/lib/services/__tests__/desktopNotificationService.spec.ts` (4 passed)**

---

### Task 5: Notification Settings UI & i18n Localization
**Files:**
- Create: `src/lib/components/settings/NotificationSettings.svelte`
- Create: `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`
- Modify: `messages/vi.json` & `messages/en.json`
- Modify: `src/lib/components/Settings.svelte`

**Interfaces:**
- Produces: `<NotificationSettings />` component with toggles for desktop notification, close-to-tray, privacy mode, chime sound, mute groups, and a "Test Notification" button.

- [x] **Step 1: Add i18n keys for notification and system tray settings in `messages/vi.json` and `messages/en.json`**
- [x] **Step 2: Run `bun run i18n:compile`**
- [x] **Step 3: Write tests in `src/lib/components/settings/__tests__/NotificationSettings.spec.ts`**
- [x] **Step 4: Implement `src/lib/components/settings/NotificationSettings.svelte` using shadcn-svelte cards, switches, and badges**
- [x] **Step 5: Mount `NotificationSettings` into `src/lib/components/Settings.svelte`**
- [x] **Step 6: Run `bun test` and `bun run check`**

---

### Task 6: Full Verification, Adversarial Testing & Harness Completion Lock
**Files:**
- Test: All Rust & frontend test suites

- [x] **Step 1: Run full Rust test suite (`cargo test tray notification`) (11 passed, 0 failed)**
- [x] **Step 2: Run full Frontend test suite (`bun test`) (567 passed, 0 failed, 1 skipped)**
- [x] **Step 3: Run `bun run check` (0 errors, full typecheck clean)**
- [x] **Step 4: Verify all Requirement Ledger rows are GREEN with fresh evidence**
- [x] **Step 5: Final review and report VERIFIED READY**
