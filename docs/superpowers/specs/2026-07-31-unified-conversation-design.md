# Unified Omnichannel Conversation Design

**Date:** 2026-07-31

## 1. Goal

Build a local-first Conversation module for RustSale_v2 that presents one customer timeline across Zalo personal sessions, WhatsApp, Facebook Messenger, and Email while preserving each provider's real conversation boundaries.

The first production milestone is Zalo-first:

- Fully support Zalo personal-session direct and group conversations through the vendored `zca-rs` library.
- Reuse the same database and UI contracts for future Email, WhatsApp, and Messenger adapters.
- Keep Zalo Outreach as one direct delivery per lead. Outreach must never create a group automatically.
- Run inside the current Tauri + SvelteKit + SQLite architecture without Redis, RabbitMQ, or a separate WebSocket server.

## 2. Confirmed Product Semantics

### 2.1 Direct customer timeline

A CRM lead is the canonical customer record for the MVP. A lead may have multiple channel identities and multiple provider conversations.

Example:

```text
Lead: Nguyen Van A
  Zalo UID 123 through Session IMEI A
  Zalo UID 123 through Session IMEI B
  WhatsApp +84901234567
  Messenger PSID abc
  Email a@example.com
```

The Conversation UI may aggregate all direct messages under Nguyen Van A, but every stored message remains attached to its original provider conversation and sending account.

### 2.2 Outreach behavior

The current outreach path sends with `ThreadType::User` in `src-tauri/src/commands.rs`. Therefore a campaign with N leads produces N direct deliveries.

Required behavior:

```text
Campaign
  -> CampaignRecipient A -> direct Zalo conversation A
  -> CampaignRecipient B -> direct Zalo conversation B
  -> CampaignRecipient C -> direct Zalo conversation C
```

Outreach must not:

- Create a Zalo group.
- Add recipients to an existing group.
- Represent the campaign itself as a chat conversation.
- Merge provider conversations that use different sending accounts.
- Send the same target lead once for every selected IMEI. Multiple selected sessions distribute or rotate recipients; each lead is attempted once per Outreach run unless the user explicitly starts another run.

### 2.3 Group behavior

A group is a provider conversation with multiple participants. It is not owned by one CRM lead and must not be included in any participant's private customer timeline.

Groups are displayed as separate sidebar items and retain a fixed channel and sending account.

## 3. Current Capability Assessment

### 3.1 Zalo personal sessions

The vendored `zca-rs` implementation already provides the foundation needed for the MVP:

- Login with stored IMEI, user agent, and cookies.
- Automatic WebSocket listener startup during login.
- Broadcast subscription through `Zalo::subscribe()`.
- Direct and group realtime messages.
- Old-message requests for user and group thread types.
- Group-specific history retrieval.
- Delivered and seen events.
- Reactions and old reactions.
- Undo/recall events.
- Typing events.
- Group membership and group-management events.
- Direct and group message sending.
- Text, quote, mention, sticker, image/file, link, voice, and video operations.

The RustSale application does not currently consume `Zalo::subscribe()`. Existing Zalo sessions are created temporarily for verification and outreach sending, then dropped.

### 3.2 Zalo risks requiring an implementation spike

The first implementation task must verify these behaviors against a real non-production Zalo account:

1. Whether `request_old_messages(ThreadType::User, cursor)` provides complete first-run direct history across conversations.
2. How old-message pagination signals completion.
3. Whether listener reconnect automatically replays missed messages or requires explicit backfill.
4. Whether `Options.self_listen = true` captures messages sent from another device using the same personal account.
5. Stable provider identifiers for text, attachments, reactions, undo, delivered, and seen events.
6. Maximum sustainable number of simultaneously connected Zalo sessions.

These checks do not change the data model. They determine checkpoint and recovery details.

### 3.3 Email

The current application supports SMTP configuration and outbound campaign sending only. It does not contain an inbound mailbox synchronizer.

Email conversation support requires a later adapter with:

- IMAP or provider-specific mailbox synchronization.
- `Message-ID`, `In-Reply-To`, and `References` threading.
- HTML and plain-text body handling.
- To, CC, BCC, subject, and attachments.
- Sent-folder synchronization or local outbound persistence.

### 3.4 WhatsApp and Messenger

No WhatsApp or Messenger adapter currently exists in the repository. Their provider integration mechanism must be selected before implementation. The shared schema and UI must not assume whether an adapter uses webhooks, polling, or personal-session automation.

Unconfigured channels do not appear as active badges and must not show fake placeholder data.

## 4. Architectural Decisions

### 4.1 Local-first architecture

The MVP runs inside the existing desktop process:

```text
Provider session or mailbox
  -> Rust channel adapter
  -> Normalization and deduplication
  -> SQLite transaction
  -> Tauri event
  -> Svelte Conversation UI
```

Use:

- SQLite WAL for durable state.
- Rust background tasks for session supervision and ingestion.
- Tauri events for UI invalidation.
- Durable SQLite outbox jobs for sending and crash recovery.

Do not add:

- Redis.
- RabbitMQ.
- Socket.io.
- A separate local WebSocket server.

### 4.2 Provider conversation is canonical

`channel_conversations` is the canonical provider thread. A unified customer timeline is a query/read model, not a row that replaces provider threads.

### 4.3 Existing `leads` table remains the customer root

The MVP reuses `leads` instead of introducing a parallel `contacts` table. Channel identities may initially be unlinked when an inbound sender is unknown.

### 4.4 Group conversations remain separate

Group messages are never merged into a lead's direct timeline. The UI may show a group in the same inbox list, but it is selected and rendered as its own conversation.

### 4.5 Every message records the channel account

The same Zalo UID contacted through two IMEIs represents two provider conversations. Messages must preserve the actual IMEI used for sending or receiving.

### 4.6 Canonical timestamps

Every Conversation timestamp and timeline cursor uses UTC RFC 3339 with fixed millisecond precision and a `Z` suffix:

```text
2026-07-31T01:02:03.000Z
```

Provider timestamps are converted before persistence. Mixed offsets or mixed second/millisecond formats are prohibited because SQLite orders these cursor fields as text.

## 5. Data Model

All timestamps use the canonical UTC format above. Provider timestamps are preserved separately from local ingestion timestamps.

### 5.1 `channel_accounts`

Registry for all configured sending and receiving accounts.

```sql
CREATE TABLE channel_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (
        channel IN ('zalo', 'whatsapp', 'messenger', 'email')
    ),
    provider_account_key TEXT NOT NULL,
    display_name TEXT,
    avatar_url TEXT,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    connection_status TEXT NOT NULL DEFAULT 'disconnected',
    last_connected_at TEXT,
    last_error TEXT,
    capabilities_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(channel, provider_account_key)
);
```

Provider keys:

- Zalo: `zalo_sessions.imei`.
- Email: mailbox/account identifier, not an email recipient address.
- WhatsApp: configured account or phone-number identifier.
- Messenger: configured Page/account identifier.

Credentials remain in provider-specific storage. `channel_accounts` contains no Zalo cookies or mailbox passwords.

### 5.2 `channel_identities`

Represents a participant identity on one channel.

```sql
CREATE TABLE channel_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER,
    channel TEXT NOT NULL CHECK (
        channel IN ('zalo', 'whatsapp', 'messenger', 'email')
    ),
    identity_namespace TEXT NOT NULL,
    external_id TEXT NOT NULL,
    normalized_address TEXT,
    display_name TEXT,
    avatar_url TEXT,
    link_source TEXT NOT NULL DEFAULT 'unlinked',
    link_confidence REAL,
    verified_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(channel, identity_namespace, external_id),
    FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE SET NULL
);
```

Identity namespaces prevent account-scoped identifiers from being merged incorrectly:

- Zalo UID: a stable Zalo namespace shared by personal sessions when confirmed by the adapter.
- WhatsApp phone identity: normalized E.164 namespace.
- Messenger PSID: Facebook Page ID namespace because a PSID is Page-scoped.
- Email: normalized mailbox-address namespace.

Identity linking rules:

- Zalo outreach may link an identity from an existing `leads.zalo_uid`.
- Email may link by normalized email address.
- WhatsApp may link by normalized phone number.
- Messenger PSID must not be matched by display name alone.
- Automatic linking by display name is prohibited.
- Unknown inbound senders remain unlinked and appear under an "Unknown contacts" state.
- Users may link an identity to an existing lead or create a new lead.

### 5.3 `channel_conversations`

```sql
CREATE TABLE channel_conversations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_account_id INTEGER NOT NULL,
    external_thread_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('direct', 'group', 'email_thread')),
    title TEXT,
    avatar_url TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    last_message_at TEXT,
    last_synced_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(channel_account_id, external_thread_id, kind),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
```

Zalo thread mapping:

- Direct: `external_thread_id = peer Zalo UID`, `kind = direct`.
- Group: `external_thread_id = Zalo group ID`, `kind = group`.

### 5.4 `conversation_participants`

```sql
CREATE TABLE conversation_participants (
    conversation_id INTEGER NOT NULL,
    identity_id INTEGER NOT NULL,
    participant_role TEXT NOT NULL DEFAULT 'member',
    joined_at TEXT,
    left_at TEXT,
    PRIMARY KEY(conversation_id, identity_id),
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE CASCADE
);
```

For direct conversations, the external customer identity is a participant. The local account is represented through `channel_account_id` and is not duplicated as a customer identity.

### 5.5 `messages`

```sql
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL,
    sender_identity_id INTEGER,
    direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound', 'system')),
    provider_message_id TEXT,
    provider_client_message_id TEXT,
    local_request_id TEXT,
    content_type TEXT NOT NULL,
    text_content TEXT,
    content_json TEXT NOT NULL DEFAULT '{}',
    raw_payload_json TEXT,
    provider_timestamp TEXT,
    ingested_at TEXT NOT NULL,
    sent_at TEXT,
    edited_at TEXT,
    removed_at TEXT,
    status TEXT NOT NULL DEFAULT 'received',
    error_code TEXT,
    error_message TEXT,
    reply_to_message_id INTEGER,
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL,
    FOREIGN KEY(reply_to_message_id) REFERENCES messages(id) ON DELETE SET NULL
);
```

Indexes:

```sql
CREATE UNIQUE INDEX idx_messages_provider_unique
ON messages(conversation_id, provider_message_id)
WHERE provider_message_id IS NOT NULL;

CREATE UNIQUE INDEX idx_messages_local_request_unique
ON messages(local_request_id)
WHERE local_request_id IS NOT NULL;

CREATE INDEX idx_messages_timeline
ON messages(conversation_id, provider_timestamp, id);
```

Status values:

```text
queued
sending
sent
delivered
read
received
failed
removed
```

The normalization layer maps provider payloads into common `content_type` values while preserving provider-specific data in `content_json` and `raw_payload_json`.

Initial content types:

```text
text
image
video
audio
file
sticker
link
location
contact_card
email
system
unsupported
```

### 5.6 `message_attachments`

```sql
CREATE TABLE message_attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id INTEGER NOT NULL,
    provider_attachment_id TEXT,
    attachment_type TEXT NOT NULL,
    file_name TEXT,
    mime_type TEXT,
    size_bytes INTEGER,
    remote_url TEXT,
    local_path TEXT,
    thumbnail_url TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);
```

Remote attachment URLs may expire. Downloading and retention policy are separate implementation decisions; the schema supports both remote and local references.

### 5.7 `message_reactions`

```sql
CREATE TABLE message_reactions (
    message_id INTEGER NOT NULL,
    actor_external_id TEXT NOT NULL,
    reaction TEXT NOT NULL,
    reacted_at TEXT,
    removed_at TEXT,
    PRIMARY KEY(message_id, actor_external_id, reaction),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);
```

### 5.8 `message_receipts`

```sql
CREATE TABLE message_receipts (
    message_id INTEGER NOT NULL,
    recipient_external_id TEXT NOT NULL DEFAULT '',
    receipt_type TEXT NOT NULL CHECK (
        receipt_type IN ('accepted', 'sent', 'delivered', 'read', 'failed')
    ),
    occurred_at TEXT NOT NULL,
    raw_payload_json TEXT,
    PRIMARY KEY(message_id, recipient_external_id, receipt_type),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);
```

Group receipts may contain multiple recipient identities. The aggregate `messages.status` is derived for convenient display but does not replace receipt rows.

### 5.9 `channel_sync_checkpoints`

```sql
CREATE TABLE channel_sync_checkpoints (
    channel_account_id INTEGER NOT NULL,
    scope TEXT NOT NULL,
    cursor TEXT,
    last_event_at TEXT,
    last_success_at TEXT,
    reconnect_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(channel_account_id, scope),
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
```

Expected Zalo scopes:

```text
direct_messages
group_messages
direct_reactions
group_reactions
groups
friends
```

### 5.10 `message_outbox_jobs`

```sql
CREATE TABLE message_outbox_jobs (
    id TEXT PRIMARY KEY,
    message_id INTEGER NOT NULL UNIQUE,
    channel_account_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('pending', 'running', 'retry_wait', 'completed', 'failed', 'cancelled')
    ),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);
```

This table provides idempotent sending and restart recovery. Only one worker may own an account's send queue at a time.

## 6. Backend Components

### 6.1 `ChannelAdapter`

All providers implement one conceptual interface:

```rust
trait ChannelAdapter {
    fn channel(&self) -> Channel;
    fn capabilities(&self) -> ChannelCapabilities;
    async fn start_sync(&self, account: ChannelAccount) -> Result<()>;
    async fn stop_sync(&self, account_id: i64) -> Result<()>;
    async fn backfill(&self, account_id: i64, checkpoint: SyncCheckpoint) -> Result<()>;
    async fn send_message(&self, request: SendMessageRequest) -> Result<ProviderSendResult>;
    async fn mark_seen(&self, request: MarkSeenRequest) -> Result<()>;
}
```

Provider adapters emit normalized events to the ingestion service. UI code never consumes raw `zca-rs` events directly.

### 6.2 `ZaloSessionSupervisor`

The supervisor owns long-lived Zalo clients keyed by IMEI.

Responsibilities:

- Load enabled Zalo sessions from SQLite.
- Log in and retain each `Zalo` client.
- Subscribe immediately after login.
- Drain the broadcast receiver continuously.
- Persist events before emitting Tauri invalidations.
- Handle `Lagged` receiver errors by scheduling backfill.
- Reflect Connected, Disconnected, Closed, and Error events in `channel_accounts`.
- Restart invalid or stopped sessions with bounded exponential backoff.
- Stop a client when its session is deleted or disabled.

The listener broadcast channel has finite capacity. A slow UI must never be the event consumer. A dedicated Rust ingestion task drains it and performs short SQLite transactions.

### 6.3 Self-message policy

Conversation synchronization should use `Options.self_listen = true` so messages sent from Zalo mobile or another device are captured.

Duplicate prevention uses:

1. Provider message ID when available.
2. Provider client message ID.
3. Local request ID for messages sent by this application.
4. A fallback event fingerprint only when the provider supplies no stable identifier.

### 6.4 Zalo event normalization

Required mappings:

| `ListenerEvent` | Database effect |
|---|---|
| `Message(User)` | Upsert direct conversation, identity, participant, and message |
| `Message(Group)` | Upsert group, participants when available, and message |
| `OldMessages` | Idempotent history insert and checkpoint advance |
| `DeliveredMessages` | Insert receipt and update aggregate status |
| `SeenMessages` | Insert receipt and update aggregate status |
| `Reaction` | Upsert or remove reaction |
| `OldReactions` | Backfill reactions idempotently |
| `Undo` | Set `removed_at` and retain tombstone metadata |
| `GroupEvent` | Update group metadata/participants and insert system event when visible |
| `Typing` | Ephemeral Tauri event only; do not persist |
| `Connected` | Mark account connected and trigger reconciliation |
| `Disconnected`/`Closed`/`Error` | Mark degraded/disconnected and schedule recovery |

### 6.5 Inbound transaction order

```text
Receive provider event
  -> Parse and normalize
  -> Begin SQLite transaction
  -> Upsert identity
  -> Upsert provider conversation
  -> Upsert participants
  -> Insert or update message/event
  -> Update conversation last_message_at
  -> Commit
  -> Emit conversation-invalidated Tauri event
```

No Tauri event is emitted before commit.

### 6.6 Outbound transaction order

```text
User presses Send
  -> Validate channel/account/conversation
  -> Insert queued message with local_request_id
  -> Insert outbox job in same transaction
  -> Commit
  -> Emit local optimistic update
  -> Account worker sends through adapter
  -> Persist provider message ID or failure
  -> Listener receipts update status
```

Retry rules:

- Authentication/session failures stop the account queue and mark the account invalid.
- Rate-limit failures use provider/account-specific backoff.
- Transient network failures retry with a bounded attempt count.
- Content/policy/recipient failures are terminal for that message.
- Retrying never creates a second local message.

### 6.7 Outreach integration

Existing outreach currently sends directly through `zca-rs`. The Conversation module must record the actual successful sending account.

Migration path:

1. On every outreach send, resolve or create `channel_accounts` from the selected IMEI.
2. Upsert a direct conversation using `(channel_account_id, lead.zalo_uid, direct)`.
3. Link the Zalo identity to the lead.
4. Insert the outbound message using the returned provider message ID.
5. Store campaign metadata in `content_json` or a future campaign-message junction.

The first integration may persist after the existing send succeeds. A later refactor may route composer and outreach through the same durable outbox service after impact analysis confirms the campaign blast radius.

## 7. Read Models and Commands

### 7.1 Inbox list

The sidebar has two logical sections:

```text
Customers
  Unified direct customer rows
  Unknown direct senders

Groups
  Provider group conversations
```

Customer rows aggregate direct conversations linked to one lead:

- Last message across all active channels.
- Channel badges present for that lead.
- Last activity timestamp.
- Sending account indicator when relevant.
- Reply targets containing the exact provider conversation ID, channel account ID, channel, account label, and most recent inbound timestamp for composer selection.

Group rows represent one `channel_conversations` record, show one channel badge, expose active participant count/preview, and expose exactly one locked reply target.

Unread counters are deferred until a local read-marker command is designed. The MVP must not derive unread state from provider delivery/read receipts because those receipts describe outbound delivery, not whether the local agent viewed an inbound message.

### 7.2 Timeline query

Direct customer timeline command:

```text
get_contact_timeline(
    lead_id,
    active_channels,
    before_timestamp,
    before_message_id,
    limit
)
```

The query selects messages from direct/email-thread conversations whose participants are identities linked to the lead.

Ordering:

```text
COALESCE(provider_timestamp, ingested_at), messages.id
```

The cursor contains both ordering values. Offset pagination is prohibited.

Group timeline command:

```text
get_conversation_timeline(
    conversation_id,
    before_timestamp,
    before_message_id,
    limit
)
```

### 7.3 Realtime invalidation

Backend emits compact events:

```ts
interface ConversationInvalidatedEvent {
  conversationId: number;
  leadId: number | null;
  channel: 'zalo' | 'whatsapp' | 'messenger' | 'email';
  reason: 'message' | 'receipt' | 'reaction' | 'metadata' | 'link';
  messageId?: number;
}
```

The MVP debounces invalidations into one lightweight inbox read-model refresh and refreshes only the currently open affected timeline. Inactive views are marked stale. A later optimization may add a single-row inbox command without changing the event contract.

## 8. UI Design

### 8.1 Layout

```text
+--------------------------+-------------------------------------------+
| Search                   | Nguyen Van A                              |
| Customers / Groups       | [Zalo] [WhatsApp] [Messenger] [Email]    |
|                          |-------------------------------------------|
| Nguyen Van A             | Unified chronological timeline            |
| Tran Thi B               |                                           |
|                          |                                           |
| Project Alpha [Group]    |-------------------------------------------|
|                          | Send via [Zalo - Sale 01]                 |
|                          | Composer                                  |
+--------------------------+-------------------------------------------+
```

### 8.2 Channel badges

For a direct customer:

- Render only channels with stored history or a configured linked identity.
- Select all available badges by default.
- Clicking an active badge deactivates it.
- Clicking an inactive badge activates it.
- If all badges are inactive, show an explicit empty-filter state.
- Persist the active set only for the currently opened customer during the session.

Filtering uses two layers:

1. Immediately hide/show already-loaded messages in Svelte state.
2. Pass `active_channels` to every older-page backend request.

The UI preserves the visible scroll anchor when filtering changes.

### 8.3 Message presentation

Every message shows:

- Channel icon.
- Sending/receiving account when multiple accounts of the same channel exist.
- Sender name for group messages.
- Provider timestamp.
- Delivery/read/failure state for outbound messages.
- Reply/quote context when available.
- Removed-message tombstone for recall events.

Email uses an expanded card with subject and recipient metadata rather than pretending to be a short chat bubble.

Unsupported provider content renders a safe fallback card and retains raw metadata for future support.

### 8.4 Composer

Direct customer composer:

- Allows channel and account selection.
- Defaults to the account that received the most recent eligible inbound message.
- Never switches accounts silently.
- Disables unavailable accounts with a visible reason.
- Adapts controls to provider capabilities.

Group composer:

- Locks channel and account to the selected group conversation.
- Does not offer Email, WhatsApp, or Messenger as alternatives for a Zalo group.
- Supports mentions only when the adapter exposes group participants.

### 8.5 Unknown sender flow

An unlinked direct sender displays:

- Provider display name and avatar.
- Channel and account.
- "Link to lead" action.
- "Create lead" action.

Linking immediately invalidates the unknown row and target lead timeline.

## 9. Performance and Reliability

### 9.1 Timeline bounds

- Initial page: 50 messages.
- Older pages: 50 messages.
- Keep a bounded DOM window around the viewport.
- Preserve cursor history and scroll anchor.
- Do not load all messages before rendering.

### 9.2 Session and event reliability

- One ingestion task per enabled Zalo account.
- One serial outbound worker per account.
- Short SQLite transactions.
- Receiver lag triggers reconciliation instead of silent message loss.
- App startup recovers pending outbox jobs.
- App startup resumes account synchronization from checkpoints.
- Duplicate events are harmless.
- Out-of-order receipts may update an existing message after message insertion.
- A receipt without a matching message is retained for later reconciliation or logged as an orphan event.

### 9.3 Security

- Never copy Zalo cookies into message or account metadata tables.
- Never emit cookies or credentials through Tauri events.
- Redact raw provider payload fields that contain credentials or session secrets before persistence.
- Attachment rendering must not execute provider HTML or scripts.
- Email HTML must be sanitized before display.

## 10. Rollout Plan

### Phase 0 — Zalo synchronization spike

- Start one non-production personal session as a long-lived client.
- Consume realtime direct and group messages.
- Request direct and group old messages.
- Verify reconnect, self-listen, pagination, and stable identifiers.
- Record findings in tests or a focused design addendum before production migration work.

### Phase 1 — Storage foundation

- Add normalized conversation tables and indexes.
- Add repositories and deterministic normalization tests.
- Register existing Zalo sessions as channel accounts.
- Add timeline and inbox read commands.

### Phase 2 — Zalo sync engine

- Add `ZaloSessionSupervisor`.
- Persist direct/group messages, reactions, recall, receipts, and group events.
- Add checkpoints, reconnect recovery, and duplicate handling.

### Phase 3 — Conversation UI

- Add customer/group sidebar.
- Add unified direct timeline and group timeline.
- Add channel badges and cursor pagination.
- Add Zalo composer and account selection.
- Add unknown-sender linking.

### Phase 4 — Outreach integration

- Persist successful outreach messages into direct conversations.
- Preserve actual IMEI after account rotation.
- Confirm N outreach recipients produce N direct conversations and zero groups.

### Phase 5 — Email adapter

- Add inbound mailbox configuration and synchronization.
- Add email threading and email composer mode.
- Reuse the unified timeline and channel badge contract.

### Phase 6 — WhatsApp and Messenger adapters

- Select provider integration mechanisms.
- Implement adapters without changing the canonical schema or UI contracts.

## 11. Acceptance Criteria

### Zalo direct and outreach

- Sending outreach to 100 leads creates or updates 100 direct provider conversations and creates no group.
- A successful outreach message records the actual IMEI selected after session rotation.
- The same customer contacted through two IMEIs has two provider conversations but one unified customer timeline.
- Retrying after an application restart does not duplicate a successfully persisted message.

### Zalo inbound synchronization

- Replayed realtime events create one message.
- Listener reconnect recovers messages missed during disconnection.
- A lagged broadcast receiver triggers backfill or reconciliation.
- Mobile-originated self messages are captured when self-listen is enabled.
- Delivered, seen, reaction, and undo events update the matching message.

### Groups

- Zalo groups appear separately from direct customers.
- Group participants and sender names are shown.
- Group messages never appear in a participant's private lead timeline.
- Group composer remains locked to its Zalo account and group ID.

### Badges and timeline

- Opening a direct customer selects all available channel badges.
- Deactivating a badge removes that channel from loaded and subsequently fetched history.
- Filtering preserves the visible scroll anchor.
- Timeline pagination never loads the entire history at once.
- All badges inactive produces a clear empty-filter state.

### Unknown identities

- An unknown inbound UID creates an unlinked identity and visible inbox row.
- Linking the UID to a lead moves its direct messages into that lead's unified timeline without copying message rows.
- Display-name collisions never auto-merge identities.

### Existing behavior

- Current Zalo login, verification, and outreach behavior remains functional.
- Existing Email Outreach remains functional before the inbound Email adapter is introduced.
- No Redis, RabbitMQ, Socket.io, or additional frontend framework is required for the MVP.

## 12. Non-Goals

- Automatically creating Zalo groups from outreach recipients.
- Merging groups across different channels.
- Treating a campaign as a chat thread.
- Building a multi-user centralized SaaS backend in the MVP.
- Implementing WhatsApp or Messenger before their provider integration is selected.
- Automatically linking identities by display name.
- Loading every historical message into frontend memory.

## 13. Implementation Gate

Implementation may begin only after:

1. The Zalo synchronization spike confirms first-run backfill and reconnect semantics.
2. The database schema is reviewed against the real event identifiers returned by `zca-rs`.
3. Impact analysis is run for every existing Rust or Svelte symbol selected for modification.
4. A task-level implementation plan identifies tests, migrations, commands, UI components, and rollout checkpoints.
