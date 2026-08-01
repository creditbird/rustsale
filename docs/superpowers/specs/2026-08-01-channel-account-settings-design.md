# Centralized Channel & Account Settings Design

**Date:** 2026-08-01
**Status:** Proposed for implementation

## 1. Goal

Create a centralized `Settings -> Channels & Accounts` workspace for managing communication accounts independently from the features that consume them.

The design covers:

- Zalo personal-session authentication and realtime listener health.
- Email SMTP account configuration and connection testing.
- Extensible navigation for future Facebook Messenger and WhatsApp adapters.
- Removal of account-creation responsibilities from Outreach pages.
- A single-owner rule for realtime provider listeners.

## 2. Product Boundaries

### 2.1 Settings owns account management

Settings is responsible for:

- Adding and removing provider accounts.
- Authenticating accounts.
- Enabling or disabling account use.
- Displaying authentication and runtime health separately.
- Reconnecting realtime adapters.
- Testing non-realtime transports such as SMTP.
- Showing actionable provider errors.

### 2.2 Conversations owns messaging

Unified Conversations is responsible for:

- Displaying stored conversation history.
- Selecting an eligible sending account.
- Sending through an already configured account.
- Explaining why an account is not eligible.
- Linking users to Settings when account management is required.

Conversations must not:

- Perform QR login.
- Create provider credentials.
- Treat valid credentials as proof of an active realtime listener.
- Mark an account connected before the provider protocol reports readiness.

### 2.3 Outreach owns campaign execution

Zalo Outreach and Email Outreach are responsible for:

- Selecting configured sending accounts.
- Selecting templates and recipients.
- Executing campaign workflows.
- Showing delivery progress and history.

Outreach must not:

- Own credential CRUD.
- Start a second realtime listener.
- Change runtime status based only on an HTTP authentication check.
- Duplicate Settings account-management dialogs.

## 3. Current Problem

RustSale currently has two legitimate but different Zalo states:

1. `zalo_sessions` stores reusable authentication material.
2. `channel_accounts.connection_status` stores Conversation runtime health.

The current UI labels an HTTP verification result as `Active`, while Conversations requires a healthy realtime listener. These states can disagree without either database row being inherently wrong.

A concrete listener ownership defect also exists:

- `zca_rs::Options::default()` enables `auto_start_listener`.
- Verification and Outreach paths use default options.
- `ZaloSessionSupervisor` also creates and starts its own listener.
- Temporary or long-running Outreach clients can therefore compete with the supervisor for the same Zalo account.
- Zalo may close one connection as a duplicate, causing rapid `connected` and `disconnected` transitions.

Moving UI controls alone does not fix this defect. Listener ownership must be fixed first.

## 4. Considered Approaches

### 4.1 UI-only relocation

Move the existing Zalo and SMTP cards into a Settings component without changing backend ownership or status semantics.

Advantages:

- Small frontend change.
- Minimal migration risk.

Disadvantages:

- Preserves duplicate-listener behavior.
- Preserves ambiguous `Active` status.
- Moves the bug instead of fixing it.

**Decision:** Rejected.

### 4.2 Immediate unified provider-account table

Replace `zalo_sessions`, `smtp_servers`, and `channel_accounts` with one generic credential and runtime table.

Advantages:

- One conceptual account registry.
- Consistent generic queries.

Disadvantages:

- Provider credentials have incompatible schemas and security requirements.
- Large migration and rollback risk.
- Encourages generic abstractions before Facebook and WhatsApp adapters exist.

**Decision:** Rejected for this phase.

### 4.3 Centralized management with provider-specific credentials

Keep provider credentials in their existing provider-specific tables. Add a shared application service and DTO that combines credential presence, authentication health, enablement, and runtime health for Settings.

Advantages:

- Fixes ownership and UX without a destructive credential migration.
- Supports provider-specific behavior.
- Provides a stable frontend contract for future channels.
- Allows later storage consolidation if real adapters justify it.

**Decision:** Selected.

## 5. Status Model

The UI and backend contracts must distinguish three concepts.

### 5.1 Enablement

Whether RustSale is allowed to use the account.

```text
enabled | disabled
```

Disabled accounts remain stored but are excluded from Conversations and Outreach selectors. Phase one may derive all existing accounts as enabled until persistent toggling is added.

### 5.2 Authentication status

Whether stored credentials can authenticate provider API requests.

```text
unknown | checking | valid | expired | error
```

Authentication verification may use HTTP APIs. A valid result must not set realtime status to connected.

### 5.3 Runtime status

Whether a long-lived adapter is currently operational.

```text
not_applicable | disconnected | connecting | connected | degraded | invalid
```

Rules:

- Zalo runtime is owned by `ZaloSessionSupervisor`.
- SMTP has `not_applicable` runtime status because it has no background listener.
- Zalo becomes `connected` only after `ListenerEvent::Connected` and protocol readiness.
- Authentication expiry may transition runtime to `invalid` only when confirmed by the adapter.
- A valid authentication check cannot override a disconnected runtime.

## 6. Listener Ownership

### 6.1 Single-owner invariant

For each Zalo IMEI, exactly one RustSale component may own a realtime listener:

```text
ZaloSessionSupervisor -> RealZaloClient -> zca-rs Listener
```

All other Zalo operations use API-only clients:

```text
QR authentication completion
Session verification
Single outreach send
Batch outreach send
Campaign session rotation
Profile lookup
```

API-only clients must set:

```rust
options.auto_start_listener = false;
```

### 6.2 Connection lifecycle

```text
Application startup
  -> load enabled Zalo sessions
  -> start one supervisor task per IMEI
  -> connecting
  -> wait for ListenerEvent::Connected
  -> connected
  -> ingest messages and process outbox
  -> terminal listener event
  -> persist reason
  -> disconnected/degraded
  -> backoff and reconnect
```

### 6.3 QR lifecycle

```text
Settings: Add Zalo account
  -> QR authentication using API-only options
  -> persist zalo_sessions row
  -> update public profile metadata
  -> supervisor.restart_account(imei)
  -> Settings observes connecting/connected runtime
```

The QR client must not keep a competing listener alive.

## 7. Backend Contracts

### 7.1 Managed account DTO

Settings consumes a redacted provider-neutral DTO:

```text
ManagedChannelAccount
  accountKey
  channel
  displayName
  avatarUrl
  enabled
  authStatus
  authCheckedAt
  runtimeStatus
  lastConnectedAt
  lastError
  capabilities
```

The DTO must never include:

- Zalo cookie JSON.
- SMTP password.
- OAuth access or refresh tokens.
- Provider encryption keys.

### 7.2 Commands

The Settings service exposes channel-management commands with provider-specific actions behind a consistent frontend API.

Required Zalo actions:

- List redacted Zalo accounts with runtime status.
- Start QR authentication.
- Verify authentication without starting a listener.
- Reconnect through `ZaloSessionSupervisor`.
- Delete the stored session and stop its supervisor task.

Required Email actions:

- List redacted SMTP accounts.
- Add or update SMTP configuration.
- Test SMTP connection.
- Delete SMTP configuration.

Existing provider commands may remain internally during migration, but new Settings components must not receive secrets in list responses.

### 7.3 Runtime updates

Account runtime changes must reach active Settings and Conversations views without requiring application restart.

Preferred contract:

```text
conversation-account-status-changed
  channelAccountId
  providerAccountKey
  channel
  status
  lastConnectedAt
  lastError
```

The backend emits the event after a committed status transition. Frontend stores update the affected account in place.

A short polling fallback may be used only if the Tauri event bridge cannot be extended safely in the first implementation slice.

## 8. Data Storage

### 8.1 Existing provider tables remain authoritative for credentials

- `zalo_sessions`: Zalo authentication material and profile metadata.
- `smtp_servers`: SMTP transport configuration.
- Future provider tables: OAuth or API-specific credentials.

### 8.2 `channel_accounts` remains authoritative for runtime identity

`channel_accounts` continues to store:

- Channel type.
- Provider account key.
- Display metadata used by Conversations.
- Runtime connection status.
- Last connected time.
- Last runtime error.

### 8.3 Phase-one migration policy

No credential-table consolidation is required.

If persistent enablement and authentication metadata are introduced, they must be additive columns with safe defaults and idempotent migrations. Existing sessions must remain usable after migration.

## 9. Settings UI

### 9.1 Navigation

Add `Settings` to the sidebar using the existing navigation pattern and design tokens.

The page title is:

```text
Channel & Account Settings
Manage authentication, connection health, and sending identities
```

### 9.2 Page structure

Use channel tabs or a segmented channel selector:

- Zalo.
- Email.
- Facebook Messenger.
- WhatsApp.

Only Zalo and Email are operational in this phase. Facebook Messenger and WhatsApp appear as disabled `Coming soon` states and must not display fake health data.

### 9.3 Zalo account card

Each Zalo account card shows:

- Avatar and display name.
- Redacted account identifier.
- Authentication badge.
- Realtime badge.
- Last successful connection time.
- Last actionable error.
- `Check authentication` action.
- `Reconnect` action.
- `Remove` action.

The page includes `Add Zalo account`, which opens the existing QR flow moved from Zalo Outreach.

### 9.4 Email account card

Each SMTP account card shows:

- Account name.
- From name and redacted email address.
- Host, port, and security mode.
- Last connection-test result when available.
- `Test connection` action.
- `Edit` action.
- `Remove` action.

The existing SMTP dialog moves from Email Outreach into Settings.

### 9.5 Error semantics

Errors must be specific and actionable:

- `Credentials valid; realtime listener disconnected.`
- `Duplicate realtime connection detected.`
- `Session expired; scan QR again.`
- `SMTP authentication failed.`
- `Connection timed out; retry or check network.`

The UI must not reduce all failures to `Active` or `Disconnected` without context.

## 10. Outreach UI Changes

### 10.1 Zalo Outreach

Remove:

- Add Session button.
- QR dialog ownership.
- Session deletion controls.
- Authentication verification controls.

Keep:

- Redacted sending-account selector.
- Account display name and avatar.
- Eligibility state.

Empty or unhealthy state:

```text
No connected Zalo account is available.
Open Settings -> Channels & Accounts to connect or repair an account.
```

### 10.2 Email Outreach

Remove:

- SMTP creation and editing cards.
- SMTP configuration dialog.
- SMTP deletion controls.

Keep:

- SMTP sending-account selector.
- Template management.
- Campaign execution.

Empty state links to Settings.

### 10.3 Migration sequence

Do not remove account controls from Outreach until the equivalent Settings flow is working and tested. During the transition, Settings becomes available first; duplicated controls are removed only in a later implementation slice.

## 11. Conversations UI Changes

The composer continues to require `runtimeStatus == connected` for Zalo sending.

For unavailable accounts it shows:

- Runtime status.
- Last error.
- `Reconnect` action when supported.
- `Open account settings` action.

Conversations must not use authentication validity as a fallback that bypasses runtime status.

## 12. Security

- Account list APIs return redacted DTOs.
- Secrets remain backend-only after initial entry.
- Editing SMTP with an empty password preserves the existing secret.
- Logs and Tauri events must not contain cookies, passwords, tokens, full IMEIs, or provider encryption material.
- Error messages shown to users must be sanitized.
- Deleting an account stops background tasks before deleting credentials.

## 13. Failure Handling

### 13.1 Zalo verification succeeds but runtime is disconnected

- Display `Authenticated` and `Disconnected` simultaneously.
- Do not enable sending.
- Offer reconnect.
- Preserve the listener disconnect reason.

### 13.2 Reconnect fails

- Supervisor remains responsible for backoff.
- Settings shows the latest sanitized error.
- Repeated clicks must not create multiple tasks for one IMEI.

### 13.3 Account is removed during campaign execution

- Existing campaign code handles the missing or failed account according to its rotation policy.
- New campaign starts must exclude removed accounts.
- Deletion must not leave an orphan listener.

### 13.4 Settings is inactive

- Runtime events may continue updating a shared account store.
- Expensive list refreshes occur only when Settings becomes active or data is stale.

## 14. Rollout Plan

### Slice 1: Listener ownership correction

- Introduce one API-only Zalo options helper.
- Use it for QR, verification, Outreach, campaign, and profile-only logins.
- Add regression tests proving non-supervisor clients do not auto-start listeners.
- Preserve the existing supervisor as the only realtime owner.

### Slice 2: Account status contract

- Add redacted managed-account DTOs.
- Split authentication and runtime status in frontend contracts.
- Add runtime status-change delivery or bounded polling fallback.
- Preserve disconnect reasons.

### Slice 3: Settings shell and Zalo management

- Add Settings navigation and page shell.
- Move QR, verification, reconnect, and deletion workflows.
- Add Zalo account health cards.

### Slice 4: Email account management

- Move SMTP CRUD and connection testing into Settings.
- Keep templates and campaigns in Email Outreach.
- Add redacted SMTP list contract.

### Slice 5: Consumer cleanup

- Remove account-management controls from Outreach pages.
- Add Settings links and empty states.
- Add reconnect and Settings actions to Conversations.
- Add disabled future-channel tabs.

## 15. Testing Strategy

### 15.1 Backend

- API-only Zalo options disable automatic listeners.
- Supervisor clients explicitly start one listener.
- Verification does not alter runtime status to connected.
- QR success persists the session and restarts exactly one supervisor task.
- Reconnect replaces the existing task without overlap.
- Account list DTOs do not serialize secrets.
- Deletion stops the task before deleting the credential row.
- Runtime status events contain sanitized fields.

### 15.2 Frontend

- Settings loads only while active and refreshes stale state.
- Zalo cards render authentication and runtime badges independently.
- Reconnect invokes the supervisor command with the provider account key.
- Outreach selectors exclude unavailable or disabled accounts.
- Outreach empty states navigate to Settings.
- SMTP forms preserve an existing password when the password field is empty.
- Future provider tabs show no fake accounts.

### 15.3 Integration

- Starting RustSale with one Zalo session creates one realtime connection owner.
- Running `Check authentication` does not disconnect Conversations.
- Starting a Zalo Outreach campaign does not create a competing listener.
- Removing an account stops inbound ingestion and removes it from selectors.
- A temporary disconnect updates Settings and Conversations, then returns to connected after recovery.

## 16. Success Criteria

- One RustSale realtime listener exists per enabled Zalo IMEI.
- HTTP verification and campaign sending never start a realtime listener.
- Zalo authentication and runtime statuses are shown separately.
- `connected` is assigned only after provider listener readiness.
- Settings is the only account-management surface.
- Outreach contains no credential creation, editing, verification, or deletion controls after migration.
- Conversations never bypasses runtime eligibility.
- Account list payloads expose no credentials.
- Existing Zalo and Email campaign behavior remains functionally compatible.

## 17. Non-Goals

- Implementing Facebook Messenger integration.
- Implementing WhatsApp integration.
- Implementing IMAP inbound email synchronization.
- Migrating all provider credentials into a generic encrypted table.
- Supporting multiple workspaces or role-based account permissions in this phase.
- Replacing the existing Conversation storage schema.
