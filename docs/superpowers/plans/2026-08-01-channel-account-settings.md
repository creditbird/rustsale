# Channel & Account Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. This repository has pre-existing uncommitted work, so implementation remains in the current worktree and must preserve unrelated diffs. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Centralize Zalo and SMTP account management in Settings, make the Zalo supervisor the sole realtime listener owner, expose redacted account contracts, and leave Outreach/Conversations as account consumers.

**Architecture:** Provider credentials remain in `zalo_sessions` and `smtp_servers`. A redacted account-settings contract combines credential health with `channel_accounts` runtime state. All non-supervisor Zalo logins use API-only options with `auto_start_listener = false`; active frontend views refresh runtime state through bounded polling.

**Tech Stack:** Rust, Tauri 2 commands/events, SQLite/rusqlite, Svelte 5, TypeScript, shadcn-svelte, Bun test, Cargo test.

**Approved Spec:** `docs/superpowers/specs/2026-08-01-channel-account-settings-design.md`

**Commit Policy:** Do not create commits unless the user explicitly requests them.

---

## Requirement Map

- `REQ-001`: One realtime Zalo listener owner per IMEI.
- `REQ-002`: Authentication and runtime status remain distinct.
- `REQ-003`: Account list contracts expose no credentials.
- `REQ-004`: Settings owns Zalo and SMTP management.
- `REQ-005`: Outreach only selects configured accounts.
- `REQ-006`: Conversations refreshes runtime state and offers recovery actions.
- `REQ-007`: Existing campaign behavior remains compatible.
- `REQ-008`: Facebook and WhatsApp appear only as non-functional future channels.

## File Structure

### Backend

- Modify `src-tauri/src/commands.rs`: API-only Zalo options, redacted account commands, SMTP password-preserving updates.
- Modify `src-tauri/src/commands_test.rs`: listener ownership, redaction, and password-preservation regression tests.
- Modify `src-tauri/src/db.rs`: additive Zalo authentication metadata and SMTP update semantics.
- Modify `src-tauri/src/db_test.rs`: migration and persistence tests.
- Modify `src-tauri/src/models.rs`: redacted Settings DTOs.
- Modify `src-tauri/src/lib.rs`: register new Settings commands.

### Frontend

- Create `src/lib/services/accountSettingsService.ts`: redacted Settings IPC and types.
- Create `src/lib/services/__tests__/accountSettingsService.spec.ts`: IPC contract tests.
- Create `src/lib/components/settings/ZaloAccountsSettings.svelte`: QR, verify, reconnect, delete, health cards.
- Create `src/lib/components/settings/EmailAccountsSettings.svelte`: SMTP add/edit/test/delete cards.
- Create `src/lib/components/Settings.svelte`: channel tabs and future-channel states.
- Create `src/lib/components/__tests__/Settings.spec.ts`: Settings behavior tests.
- Modify `src/lib/components/ZaloOutreach.svelte`: remove account CRUD and use redacted selector data.
- Modify `src/lib/components/EmailOutreach.svelte`: remove SMTP CRUD and use redacted selector data.
- Modify `src/lib/components/__tests__/ZaloOutreach.spec.ts`: selector and Settings CTA expectations.
- Modify `src/lib/components/__tests__/EmailOutreach.spec.ts`: selector and Settings CTA expectations.
- Modify `src/lib/components/conversation/ConversationWorkspace.svelte`: bounded runtime refresh while active.
- Modify `src/lib/components/conversation/MessageComposer.svelte`: reconnect and Settings actions.
- Modify `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`: runtime refresh behavior.
- Modify `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`: reconnect behavior.
- Modify `src/lib/components/Sidebar.svelte`: Settings navigation.
- Modify `src/lib/components/__tests__/Sidebar.spec.ts`: Settings navigation expectation.
- Modify `src/routes/+page.svelte`: render Settings and route callbacks.

---

### Task 1: Fix Zalo Listener Ownership

**Requirements:** `REQ-001`, `REQ-007`

**Files:**
- Modify: `src-tauri/src/commands_test.rs`
- Modify: `src-tauri/src/commands.rs`

- [x] **Step 1: Write the failing options test**

```rust
#[test]
fn zalo_api_options_do_not_start_realtime_listener() {
    let options = commands::zalo_api_options();
    assert!(!options.auto_start_listener);
}
```

- [x] **Step 2: Run RED test**

Run:

```bash
df -h .
cd src-tauri && cargo test commands_test::tests::zalo_api_options_do_not_start_realtime_listener --lib -- --exact
```

Expected: compile failure because `zalo_api_options` does not exist.

- [x] **Step 3: Add API-only options helper**

```rust
pub(crate) fn zalo_api_options() -> zca_rs::Options {
    let mut options = zca_rs::Options::default();
    options.auto_start_listener = false;
    options
}
```

- [x] **Step 4: Replace non-supervisor default options**

Use `zalo_api_options()` in:

- `start_zalo_qr_login`
- `verify_zalo_session_internal`
- `send_zalo_outreach_internal`
- `send_zalo_campaign_internal`

Do not change `RealZaloClientFactory`; it already disables auto-start and explicitly starts its listener through the supervisor.

- [x] **Step 5: Run GREEN tests**

```bash
df -h .
cd src-tauri && cargo test commands_test::tests::zalo_api_options_do_not_start_realtime_listener --lib -- --exact
cd src-tauri && cargo test commands_test::tests::test_zalo_session_status_serialization_and_verification_internal --lib -- --exact
```

Expected: both pass.

---

### Task 2: Add Redacted Account Contracts

**Requirements:** `REQ-002`, `REQ-003`

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/db_test.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands_test.rs`
- Modify: `src-tauri/src/lib.rs`

- [x] **Step 1: Write failing redaction and migration tests**

Add tests proving:

```text
managed Zalo JSON contains no cookies_json or user_agent
managed SMTP JSON contains no password
existing SMTP password survives an update whose password is empty
zalo_sessions contains auth_status, auth_checked_at, and auth_error columns
```

- [x] **Step 2: Run RED tests**

```bash
df -h .
cd src-tauri && cargo test managed_account --lib
cd src-tauri && cargo test smtp_empty_password --lib
```

Expected: tests fail because DTOs, columns, and preservation behavior do not exist.

- [x] **Step 3: Add DTOs**

```rust
pub struct ManagedZaloAccount {
    pub account_key: String,
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub auth_status: String,
    pub auth_checked_at: Option<String>,
    pub auth_error: Option<String>,
    pub runtime_status: String,
    pub last_connected_at: Option<String>,
    pub last_error: Option<String>,
}

pub struct ManagedSmtpAccount {
    pub id: i32,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub from_email: String,
    pub from_name: String,
    pub security: String,
}
```

- [x] **Step 4: Add additive migration**

Add idempotent columns to `zalo_sessions`:

```sql
auth_status TEXT NOT NULL DEFAULT 'unknown'
auth_checked_at TEXT
auth_error TEXT
```

- [x] **Step 5: Persist verification results**

Successful verification stores `valid`; failed verification stores `expired` or `error` with sanitized text and timestamp. It must not alter `channel_accounts.connection_status`.

- [x] **Step 6: Preserve SMTP password on empty update**

Use SQL that selects the current password when an edit request provides an empty password.

- [x] **Step 7: Add and register commands**

```text
get_managed_zalo_accounts
get_managed_smtp_accounts
```

- [x] **Step 8: Run GREEN tests**

```bash
df -h .
cd src-tauri && cargo test managed_account --lib
cd src-tauri && cargo test smtp_empty_password --lib
cd src-tauri && cargo test commands_test::tests::test_smtp_and_email_template_commands --lib -- --exact
```

---

### Task 3: Add Frontend Account Settings Service

**Requirements:** `REQ-002`, `REQ-003`, `REQ-004`

**Files:**
- Create: `src/lib/services/accountSettingsService.ts`
- Create: `src/lib/services/__tests__/accountSettingsService.spec.ts`

- [x] **Step 1: Write failing IPC tests**

Test commands and camelCase payloads for:

```text
get_managed_zalo_accounts
get_managed_smtp_accounts
verify_zalo_session
restart_conversation_account
delete_zalo_session
save_smtp_server
delete_smtp_server
test_smtp_connection
```

- [x] **Step 2: Run RED test**

```bash
bun test src/lib/services/__tests__/accountSettingsService.spec.ts
```

Expected: module-not-found failure.

- [x] **Step 3: Implement typed wrappers**

Expose no credential fields in `ManagedZaloAccount` or `ManagedSmtpAccount`.

- [x] **Step 4: Run GREEN test**

```bash
bun test src/lib/services/__tests__/accountSettingsService.spec.ts
```

---

### Task 4: Build Settings UI

**Requirements:** `REQ-004`, `REQ-008`

**Files:**
- Create: `src/lib/components/settings/ZaloAccountsSettings.svelte`
- Create: `src/lib/components/settings/EmailAccountsSettings.svelte`
- Create: `src/lib/components/Settings.svelte`
- Create: `src/lib/components/__tests__/Settings.spec.ts`

- [x] **Step 1: Capture visual baseline**

Open `http://localhost:1420` and record sidebar width, header height, content padding, card radius, and tab height with browser computed measurements before editing navigation or page DOM.

- [x] **Step 2: Write failing Settings tests**

Tests cover:

```text
Zalo and Email tabs render
Facebook and WhatsApp are disabled Coming soon states
Zalo auth and runtime badges render independently
Reconnect invokes restart command
SMTP edit sends an empty password unless the user enters a replacement
```

- [x] **Step 3: Run RED test**

```bash
bun test src/lib/components/__tests__/Settings.spec.ts
```

- [x] **Step 4: Implement Settings components**

Use existing shadcn-svelte `Tabs`, `Card`, `Badge`, `Button`, `Dialog`, `Input`, and `Select` components. Move QR and SMTP management behavior into the provider-specific Settings components.

- [x] **Step 5: Run GREEN test**

```bash
bun test src/lib/components/__tests__/Settings.spec.ts
```

---

### Task 5: Convert Outreach Pages to Consumers

**Requirements:** `REQ-005`, `REQ-007`

**Files:**
- Modify: `src/lib/components/ZaloOutreach.svelte`
- Modify: `src/lib/components/EmailOutreach.svelte`
- Modify: `src/lib/components/__tests__/ZaloOutreach.spec.ts`
- Modify: `src/lib/components/__tests__/EmailOutreach.spec.ts`

- [x] **Step 1: Update tests to require consumer-only UI**

Assert:

```text
no Add Session button in Zalo Outreach
no SMTP configuration dialog in Email Outreach
configured account selectors remain
empty account state calls onOpenSettings
campaign commands receive the same selected identifiers
```

- [x] **Step 2: Run RED tests**

```bash
bun test src/lib/components/__tests__/ZaloOutreach.spec.ts
bun test src/lib/components/__tests__/EmailOutreach.spec.ts
```

- [x] **Step 3: Remove account-management ownership**

Delete QR/session CRUD UI and SMTP CRUD UI from Outreach. Replace account loads with `accountSettingsService` redacted lists. Keep templates, lead selection, and campaign execution unchanged.

- [x] **Step 4: Run GREEN tests**

```bash
bun test src/lib/components/__tests__/ZaloOutreach.spec.ts
bun test src/lib/components/__tests__/EmailOutreach.spec.ts
```

---

### Task 6: Add Navigation and Conversation Recovery

**Requirements:** `REQ-006`, `REQ-008`

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/components/__tests__/Sidebar.spec.ts`
- Modify: `src/routes/+page.svelte`
- Modify: `src/lib/components/conversation/ConversationWorkspace.svelte`
- Modify: `src/lib/components/conversation/MessageComposer.svelte`
- Modify: `src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts`
- Modify: `src/lib/components/conversation/__tests__/MessageComposer.spec.ts`

- [x] **Step 1: Write failing navigation and recovery tests**

Tests cover Settings navigation, active-only runtime polling cleanup, reconnect command invocation, and Settings callback invocation.

- [x] **Step 2: Run RED tests**

```bash
bun test src/lib/components/__tests__/Sidebar.spec.ts
bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun test src/lib/components/conversation/__tests__/MessageComposer.spec.ts
```

- [x] **Step 3: Implement navigation and bounded polling**

Add the Settings tab and pass `onOpenSettings={() => activeTab = 'settings'}` into consumer components. Poll Conversation runtime only while active and clear the timer on deactivation/unmount.

- [x] **Step 4: Add composer recovery actions**

Reconnect uses the selected account's provider key and refreshes runtime afterward. `Open account settings` invokes the provided callback.

- [x] **Step 5: Run GREEN tests**

Run the three targeted test files again.

---

### Task 7: Integration and Release QA

**Requirements:** `REQ-001` through `REQ-008`

**Files:**
- Update: `docs/superpowers/ledgers/2026-08-01-channel-account-settings.md`

- [x] **Step 1: Run formatting and static checks**

```bash
df -h .
cd src-tauri && cargo fmt --check
cd src-tauri && cargo check
bun run check
```

- [x] **Step 2: Run targeted backend and frontend suites**

```bash
df -h .
cd src-tauri && cargo test commands_test::tests --lib
bun test src/lib/services/__tests__/accountSettingsService.spec.ts
bun test src/lib/components/__tests__/Settings.spec.ts
bun test src/lib/components/__tests__/ZaloOutreach.spec.ts
bun test src/lib/components/__tests__/EmailOutreach.spec.ts
bun test src/lib/components/conversation/__tests__/ConversationWorkspace.spec.ts
bun test src/lib/components/conversation/__tests__/MessageComposer.spec.ts
```

- [x] **Step 3: Run broader build gates**

```bash
df -h .
cd src-tauri && cargo test --lib
bun run build
```

- [x] **Step 4: Validate runtime behavior**

With the development app running:

```text
Settings lists Zalo without exposing credentials
Check authentication does not cause runtime flapping
Reconnect transitions through connecting to connected
Zalo Outreach still selects and sends with account IMEI
Email Outreach still selects SMTP account IDs
```

- [x] **Step 5: Measure visual results**

Repeat baseline browser measurements and record before/after differences. Any intended layout change must be documented; unchanged shell metrics must remain at zero drift.

- [x] **Step 6: Run GitNexus final analysis**

```text
gitnexus_detect_changes(scope="all")
```

Review every direct dependency and affected execution flow.

- [x] **Step 7: Inspect final diff**

```bash
git diff --check
git status --short
```

Confirm pre-existing changes remain outside this plan's ownership boundary.
