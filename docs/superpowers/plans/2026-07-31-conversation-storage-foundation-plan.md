# Conversation Storage Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add idempotent normalized conversation storage, repository operations, cursor read models, and read/link Tauri commands without changing existing Outreach behavior.

**Architecture:** Keep all conversation SQL and repository logic in `src-tauri/src/conversation`. `db::init_db` makes one isolated migration call, while a dedicated repository opens short WAL connections by database path. A small service and event publisher provide stable state for later Zalo synchronization.

**Tech Stack:** Rust, rusqlite/SQLite WAL, Serde, Tauri state and events, Tokio broadcast, Cargo tests.

---

## Risk Warning

GitNexus reports `init_db` as **CRITICAL** because 34 direct callers/tests and multiple Google Maps flows create databases through it. Before editing `init_db`, re-run:

```text
mcp__gitnexus.impact({repo: "RustSale_v2", target: "init_db", direction: "upstream", maxDepth: 3, includeTests: true})
```

The permitted `init_db` change is one migration call inside the existing transaction. Stop and warn the user if broader edits become necessary.

## Task 1: Define Stable Conversation Domain Types

**Files:**
- Create: `src-tauri/src/conversation/types.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Write the failing serialization tests**

Create `src-tauri/src/conversation/types.rs` with tests first:

```rust
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_contract_is_snake_case() {
        assert_eq!(serde_json::to_string(&Channel::Zalo).unwrap(), "\"zalo\"");
        assert_eq!(
            serde_json::to_string(&Channel::WhatsApp).unwrap(),
            "\"whatsapp\""
        );
    }

    #[test]
    fn timeline_cursor_round_trips() {
        let cursor = TimelineCursor {
            occurred_at: "2026-07-31T01:02:03.000Z".to_string(),
            message_id: 42,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        assert_eq!(serde_json::from_str::<TimelineCursor>(&json).unwrap(), cursor);
    }

    #[test]
    fn utc_now_uses_fixed_millisecond_z_format() {
        let value = utc_now();
        assert!(value.ends_with('Z'));
        assert_eq!(value.len(), "2026-07-31T01:02:03.000Z".len());
    }
}
```

- [ ] **Step 2: Run the tests and verify failure**

Add to `src-tauri/src/conversation/mod.rs`:

```rust
pub mod types;
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::types::tests
```

Expected: FAIL because the domain types do not exist.

- [ ] **Step 3: Implement the domain contracts**

Add above the tests in `types.rs`:

```rust
pub fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Zalo,
    WhatsApp,
    Messenger,
    Email,
}

impl Channel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zalo => "zalo",
            Self::WhatsApp => "whatsapp",
            Self::Messenger => "messenger",
            Self::Email => "email",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Group,
    EmailThread,
}

impl ConversationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Group => "group",
            Self::EmailThread => "email_thread",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    Inbound,
    Outbound,
    System,
}

impl MessageDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Queued,
    Sending,
    Sent,
    Delivered,
    Read,
    Received,
    Failed,
    Removed,
}

impl MessageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Received => "received",
            Self::Failed => "failed",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineCursor {
    pub occurred_at: String,
    pub message_id: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelBadgeSummary {
    pub channel: Channel,
    pub message_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReplyTarget {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub channel: Channel,
    pub account_display_name: Option<String>,
    pub conversation_kind: ConversationKind,
    pub last_inbound_at: Option<String>,
    pub last_message_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationParticipantSummary {
    pub identity_id: i64,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub participant_role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxItemKind {
    Customer,
    Unknown,
    Group,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub key: String,
    pub kind: InboxItemKind,
    pub lead_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub identity_id: Option<i64>,
    pub title: String,
    pub avatar_url: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_at: Option<String>,
    pub channels: Vec<ChannelBadgeSummary>,
    pub reply_targets: Vec<ConversationReplyTarget>,
    pub participant_count: u64,
    pub participants: Vec<ConversationParticipantSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: i64,
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub channel: Channel,
    pub account_display_name: Option<String>,
    pub sender_identity_id: Option<i64>,
    pub sender_display_name: Option<String>,
    pub direction: MessageDirection,
    pub provider_message_id: Option<String>,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content: serde_json::Value,
    pub occurred_at: String,
    pub status: MessageStatus,
    pub removed_at: Option<String>,
    pub error_message: Option<String>,
    pub reply_to_message_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePage {
    pub items: Vec<ConversationMessage>,
    pub next_cursor: Option<TimelineCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkIdentityRequest {
    pub identity_id: i64,
    pub lead_id: i64,
}
```

- [ ] **Step 4: Run the type tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::types::tests
```

Expected: PASS.

## Task 2: Add Idempotent Conversation Schema

**Files:**
- Create: `src-tauri/src/conversation/schema.sql`
- Create: `src-tauri/src/conversation/migration.rs`
- Create: `src-tauri/src/conversation/migration_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`
- Modify: `src-tauri/src/db.rs:34-378`

- [ ] **Step 1: Write the failing migration tests**

Create `src-tauri/src/conversation/migration_test.rs`:

```rust
use super::migration::migrate;
use crate::db::open_db_connection;

fn table_names(conn: &rusqlite::Connection) -> Vec<String> {
    let mut statement = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn conversation_schema_is_idempotent() {
    let path = std::env::temp_dir().join(format!(
        "rustsale-conversation-migration-{}.db",
        rand::random::<u64>()
    ));
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();

    migrate(&conn).unwrap();
    migrate(&conn).unwrap();

    let tables = table_names(&conn);
    for expected in [
        "channel_accounts",
        "channel_identities",
        "channel_conversations",
        "conversation_participants",
        "messages",
        "message_attachments",
        "message_reactions",
        "message_receipts",
        "channel_sync_checkpoints",
        "message_outbox_jobs",
    ] {
        assert!(tables.contains(&expected.to_string()), "missing {expected}");
    }

    drop(conn);
    let _ = std::fs::remove_file(path);
}

#[test]
fn provider_message_id_is_unique_within_conversation() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-a', 'disconnected', '{}', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'peer-1', 'direct', 'active', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
        [],
    )
    .unwrap();
    let insert = || {
        conn.execute(
            "INSERT INTO messages
             (conversation_id, direction, provider_message_id, content_type, content_json, ingested_at, status)
             VALUES (1, 'inbound', 'message-1', 'text', '{}', '2026-07-31T00:00:00.000Z', 'received')",
            [],
        )
    };

    insert().unwrap();
    assert!(insert().is_err());
}
```

- [ ] **Step 2: Register migration modules and verify failure**

Add to `src-tauri/src/conversation/mod.rs`:

```rust
pub mod migration;

#[cfg(test)]
mod migration_test;
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::migration_test
```

Expected: FAIL because `migration.rs` and `schema.sql` do not exist.

- [ ] **Step 3: Create the schema**

Create `src-tauri/src/conversation/schema.sql`:

```sql
CREATE TABLE IF NOT EXISTS channel_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
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

CREATE TABLE IF NOT EXISTS channel_identities (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    lead_id INTEGER,
    channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
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

CREATE TABLE IF NOT EXISTS channel_conversations (
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

CREATE TABLE IF NOT EXISTS conversation_participants (
    conversation_id INTEGER NOT NULL,
    identity_id INTEGER NOT NULL,
    participant_role TEXT NOT NULL DEFAULT 'member',
    joined_at TEXT,
    left_at TEXT,
    PRIMARY KEY(conversation_id, identity_id),
    FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
    FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS messages (
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

CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_provider_unique
ON messages(conversation_id, provider_message_id)
WHERE provider_message_id IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_local_request_unique
ON messages(local_request_id)
WHERE local_request_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_messages_timeline
ON messages(conversation_id, provider_timestamp, ingested_at, id);

CREATE TABLE IF NOT EXISTS message_attachments (
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

CREATE TABLE IF NOT EXISTS message_reactions (
    message_id INTEGER NOT NULL,
    actor_external_id TEXT NOT NULL,
    reaction TEXT NOT NULL,
    reacted_at TEXT,
    removed_at TEXT,
    PRIMARY KEY(message_id, actor_external_id, reaction),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS message_receipts (
    message_id INTEGER NOT NULL,
    recipient_external_id TEXT NOT NULL DEFAULT '',
    receipt_type TEXT NOT NULL CHECK (receipt_type IN ('accepted', 'sent', 'delivered', 'read', 'failed')),
    occurred_at TEXT NOT NULL,
    raw_payload_json TEXT,
    PRIMARY KEY(message_id, recipient_external_id, receipt_type),
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS channel_sync_checkpoints (
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

CREATE TABLE IF NOT EXISTS message_outbox_jobs (
    id TEXT PRIMARY KEY,
    message_id INTEGER NOT NULL UNIQUE,
    channel_account_id INTEGER NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'retry_wait', 'completed', 'failed', 'cancelled')),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    completed_at TEXT,
    FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE,
    FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_channel_identities_lead
ON channel_identities(lead_id, channel);

CREATE INDEX IF NOT EXISTS idx_conversations_last_message
ON channel_conversations(last_message_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_outbox_claim
ON message_outbox_jobs(status, next_attempt_at, created_at);
```

- [ ] **Step 4: Implement the isolated migration**

Create `src-tauri/src/conversation/migration.rs`:

```rust
use rusqlite::{Connection, Result};

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema.sql"))
}
```

- [ ] **Step 5: Integrate the migration into `init_db`**

Inside the existing `init_db` transaction, after the `leads` table and its additive column migrations exist, add exactly:

```rust
crate::conversation::migration::migrate(&tx)?;
```

Do not move or rewrite existing Google Maps, Outreach, Email, category, lead, or activity schema statements.

- [ ] **Step 6: Run targeted and critical regression tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::migration_test
cargo test --manifest-path src-tauri/Cargo.toml db_test::tests::test_db_init_and_tables
cargo test --manifest-path src-tauri/Cargo.toml db_test::tests::google_maps_job_schema_is_idempotent_and_enforces_keys
```

Expected: PASS.

- [ ] **Step 7: Run the full Rust suite because `init_db` is CRITICAL**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

Expected: all conversation and existing database tests pass. Record unrelated environment-dependent failures without changing their code.

## Task 3: Implement Transactional Upsert and Deduplication Repository

**Files:**
- Create: `src-tauri/src/conversation/repository.rs`
- Create: `src-tauri/src/conversation/repository_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Define repository input records and failing tests**

Create `src-tauri/src/conversation/repository_test.rs` with a temporary database harness and these tests:

```rust
use super::migration::migrate;
use super::repository::{ConversationRepository, NormalizedIdentity, NormalizedMessage};
use super::types::{Channel, ConversationKind, MessageDirection, MessageStatus};
use crate::db::open_db_connection;

struct Harness {
    path: std::path::PathBuf,
    repository: ConversationRepository,
}

impl Harness {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustsale-conversation-repository-{}.db",
            rand::random::<u64>()
        ));
        let conn = open_db_connection(path.to_str().unwrap()).unwrap();
        conn.execute(
            "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        drop(conn);
        Self {
            repository: ConversationRepository::new(path.to_string_lossy().to_string()),
            path,
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

fn direct_message(account: &str, provider_message_id: &str) -> NormalizedMessage {
    NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: account.to_string(),
        account_display_name: Some(account.to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: "peer-1".to_string(),
            display_name: Some("Peer One".to_string()),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: "peer-1".to_string(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: Some("Peer One".to_string()),
        direction: MessageDirection::Inbound,
        provider_message_id: Some(provider_message_id.to_string()),
        provider_client_message_id: Some("client-1".to_string()),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("hello".to_string()),
        content: serde_json::json!({"text": "hello"}),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T00:00:00.000Z".to_string()),
        ingested_at: "2026-07-31T00:00:01.000Z".to_string(),
        status: MessageStatus::Received,
    }
}

#[test]
fn duplicate_provider_message_is_idempotent() {
    let harness = Harness::new();
    let first = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();

    assert_eq!(first.message_id, second.message_id);
    assert!(!first.duplicate);
    assert!(second.duplicate);
}

#[test]
fn same_peer_on_two_zalo_accounts_creates_two_provider_conversations() {
    let harness = Harness::new();
    let first = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let second = harness
        .repository
        .apply_normalized_message(&direct_message("imei-b", "message-2"))
        .unwrap();

    assert_ne!(first.channel_account_id, second.channel_account_id);
    assert_ne!(first.conversation_id, second.conversation_id);
    assert_eq!(first.identity_id, second.identity_id);
}
```

- [ ] **Step 2: Register the repository and verify failure**

Add to `src-tauri/src/conversation/mod.rs`:

```rust
pub mod repository;

#[cfg(test)]
mod repository_test;
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::repository_test
```

Expected: FAIL because repository types do not exist.

- [ ] **Step 3: Implement repository contracts and transactional upsert**

Create `src-tauri/src/conversation/repository.rs` with these public records and methods:

```rust
use crate::conversation::types::{
    Channel, ConversationKind, MessageDirection, MessageStatus,
};
use crate::db::open_db_connection;
use rusqlite::{params, OptionalExtension, Result};

#[derive(Debug, Clone)]
pub struct NormalizedIdentity {
    pub identity_namespace: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub lead_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NormalizedMessage {
    pub channel: Channel,
    pub provider_account_key: String,
    pub account_display_name: Option<String>,
    pub participant: Option<NormalizedIdentity>,
    pub sender_is_participant: bool,
    pub external_thread_id: String,
    pub conversation_kind: ConversationKind,
    pub conversation_title: Option<String>,
    pub direction: MessageDirection,
    pub provider_message_id: Option<String>,
    pub provider_client_message_id: Option<String>,
    pub local_request_id: Option<String>,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content: serde_json::Value,
    pub raw_payload: Option<serde_json::Value>,
    pub provider_timestamp: Option<String>,
    pub ingested_at: String,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMessage {
    pub channel_account_id: i64,
    pub identity_id: Option<i64>,
    pub conversation_id: i64,
    pub message_id: i64,
    pub duplicate: bool,
}

#[derive(Debug, Clone)]
pub struct ConversationRepository {
    db_path: String,
}

impl ConversationRepository {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    pub fn apply_normalized_message(&self, message: &NormalizedMessage) -> Result<AppliedMessage> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = &message.ingested_at;

        let channel_account_id: i64 = tx.query_row(
            "INSERT INTO channel_accounts
             (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'disconnected', '{}', ?4, ?4)
             ON CONFLICT(channel, provider_account_key) DO UPDATE SET
                 display_name = COALESCE(excluded.display_name, channel_accounts.display_name),
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                message.channel.as_str(),
                message.provider_account_key,
                message.account_display_name,
                now
            ],
            |row| row.get(0),
        )?;

        let identity_id = if let Some(identity) = &message.participant {
            Some(tx.query_row(
                "INSERT INTO channel_identities
                 (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(channel, identity_namespace, external_id) DO UPDATE SET
                     lead_id = COALESCE(channel_identities.lead_id, excluded.lead_id),
                     display_name = COALESCE(excluded.display_name, channel_identities.display_name),
                     avatar_url = COALESCE(excluded.avatar_url, channel_identities.avatar_url),
                     updated_at = excluded.updated_at
                 RETURNING id",
                params![
                    identity.lead_id,
                    message.channel.as_str(),
                    identity.identity_namespace,
                    identity.external_id,
                    identity.display_name,
                    identity.avatar_url,
                    if identity.lead_id.is_some() { "provider_match" } else { "unlinked" },
                    now
                ],
                |row| row.get::<_, i64>(0),
            )?)
        } else {
            None
        };

        let conversation_id: i64 = tx.query_row(
            "INSERT INTO channel_conversations
             (channel_account_id, external_thread_id, kind, title, status, last_message_at, last_synced_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?6, ?6)
             ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                 title = COALESCE(excluded.title, channel_conversations.title),
                 last_message_at = CASE
                     WHEN channel_conversations.last_message_at IS NULL
                          OR excluded.last_message_at > channel_conversations.last_message_at
                     THEN excluded.last_message_at
                     ELSE channel_conversations.last_message_at
                 END,
                 last_synced_at = excluded.last_synced_at,
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                channel_account_id,
                message.external_thread_id,
                message.conversation_kind.as_str(),
                message.conversation_title,
                message.provider_timestamp.as_deref().unwrap_or(now),
                now
            ],
            |row| row.get(0),
        )?;

        if let Some(identity_id) = identity_id {
            tx.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role)
                 VALUES (?1, ?2, 'member')
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![conversation_id, identity_id],
            )?;
        }

        let sender_identity_id = message.sender_is_participant.then_some(identity_id).flatten();

        let existing_message_id = if let Some(provider_message_id) = &message.provider_message_id {
            tx.query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![conversation_id, provider_message_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        } else if let Some(local_request_id) = &message.local_request_id {
            tx.query_row(
                "SELECT id FROM messages WHERE local_request_id = ?1",
                [local_request_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        } else {
            None
        };

        if let Some(message_id) = existing_message_id {
            tx.commit()?;
            return Ok(AppliedMessage {
                channel_account_id,
                identity_id,
                conversation_id,
                message_id,
                duplicate: true,
            });
        }

        tx.execute(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id,
              provider_client_message_id, local_request_id, content_type, text_content,
              content_json, raw_payload_json, provider_timestamp, ingested_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                conversation_id,
                sender_identity_id,
                message.direction.as_str(),
                message.provider_message_id,
                message.provider_client_message_id,
                message.local_request_id,
                message.content_type,
                message.text_content,
                message.content.to_string(),
                message.raw_payload.as_ref().map(serde_json::Value::to_string),
                message.provider_timestamp,
                message.ingested_at,
                message.status.as_str()
            ],
        )?;
        let message_id = tx.last_insert_rowid();
        tx.commit()?;

        Ok(AppliedMessage {
            channel_account_id,
            identity_id,
            conversation_id,
            message_id,
            duplicate: false,
        })
    }
}
```

- [ ] **Step 4: Run repository tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::repository_test
```

Expected: PASS.

## Task 4: Add Identity Linking and Cursor Timeline Queries

**Files:**
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`

- [ ] **Step 1: Write failing link and timeline tests**

Append tests that:

```rust
#[test]
fn linking_identity_moves_direct_messages_into_contact_timeline_without_copying() {
    let harness = Harness::new();
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (7, 'Lead Seven')", [])
        .unwrap();
    drop(conn);
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();

    harness
        .repository
        .link_identity(applied.identity_id.expect("direct message identity"), 7)
        .unwrap();
    let page = harness
        .repository
        .get_contact_timeline(7, &[Channel::Zalo], None, 50)
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].provider_message_id.as_deref(), Some("message-1"));
}

#[test]
fn group_messages_are_excluded_from_contact_timeline() {
    let harness = Harness::new();
    let mut message = direct_message("imei-a", "group-message-1");
    message.conversation_kind = ConversationKind::Group;
    message.external_thread_id = "group-1".to_string();
    message
        .participant
        .as_mut()
        .expect("group sender identity")
        .lead_id = Some(7);
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute("INSERT INTO leads (id, name) VALUES (7, 'Lead Seven')", [])
        .unwrap();
    drop(conn);

    harness.repository.apply_normalized_message(&message).unwrap();
    let page = harness
        .repository
        .get_contact_timeline(7, &[Channel::Zalo], None, 50)
        .unwrap();

    assert!(page.items.is_empty());
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml linking_identity_moves_direct_messages_into_contact_timeline_without_copying group_messages_are_excluded_from_contact_timeline
```

Expected: FAIL because repository methods do not exist.

- [ ] **Step 3: Implement identity linking**

Add:

```rust
pub fn link_identity(&self, identity_id: i64, lead_id: i64) -> Result<i64> {
    let mut conn = open_db_connection(&self.db_path)?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE channel_identities
         SET lead_id = ?1, link_source = 'manual', link_confidence = 1.0, verified_at = ?2, updated_at = ?2
         WHERE id = ?3",
        params![lead_id, crate::conversation::types::utc_now(), identity_id],
    )?;
    let conversation_id = tx
        .query_row(
            "SELECT cp.conversation_id
             FROM conversation_participants cp
             JOIN channel_conversations cc ON cc.id = cp.conversation_id
             WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')
             ORDER BY cc.last_message_at DESC, cc.id DESC
             LIMIT 1",
            [identity_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    tx.commit()?;
    Ok(conversation_id)
}
```

- [ ] **Step 4: Implement contact timeline cursor query**

Add a private row mapper plus:

```rust
pub fn get_contact_timeline(
    &self,
    lead_id: i64,
    channels: &[Channel],
    before: Option<&crate::conversation::types::TimelineCursor>,
    limit: u32,
) -> Result<crate::conversation::types::TimelinePage> {
    if channels.is_empty() {
        return Ok(crate::conversation::types::TimelinePage {
            items: Vec::new(),
            next_cursor: None,
        });
    }
    let conn = open_db_connection(&self.db_path)?;
    let channel_placeholders = std::iter::repeat("?")
        .take(channels.len())
        .collect::<Vec<_>>()
        .join(", ");
    let before_at = before.map(|cursor| cursor.occurred_at.as_str());
    let before_id = before.map(|cursor| cursor.message_id);
    let sql = format!(
        "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                ca.display_name, m.sender_identity_id, ci.display_name, m.direction,
                m.provider_message_id, m.content_type, m.text_content, m.content_json,
                COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                m.status, m.removed_at, m.error_message, m.reply_to_message_id
         FROM messages m
         JOIN channel_conversations cc ON cc.id = m.conversation_id
         JOIN channel_accounts ca ON ca.id = cc.channel_account_id
         LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
         WHERE EXISTS (
               SELECT 1
               FROM conversation_participants cp
               JOIN channel_identities participant ON participant.id = cp.identity_id
               WHERE cp.conversation_id = cc.id AND participant.lead_id = ?
           )
           AND cc.kind IN ('direct', 'email_thread')
           AND ca.channel IN ({channel_placeholders})
           AND (? IS NULL OR COALESCE(m.provider_timestamp, m.ingested_at) < ?
                OR (COALESCE(m.provider_timestamp, m.ingested_at) = ? AND m.id < ?))
         ORDER BY occurred_at DESC, m.id DESC
         LIMIT ?"
    );
    let mut values = Vec::<rusqlite::types::Value>::new();
    values.push(lead_id.into());
    values.extend(
        channels
            .iter()
            .map(|channel| rusqlite::types::Value::Text(channel.as_str().to_string())),
    );
    values.push(before_at.map_or(rusqlite::types::Value::Null, |value| value.to_string().into()));
    values.push(before_at.map_or(rusqlite::types::Value::Null, |value| value.to_string().into()));
    values.push(before_at.map_or(rusqlite::types::Value::Null, |value| value.to_string().into()));
    values.push(before_id.map_or(rusqlite::types::Value::Null, Into::into));
    values.push(i64::from(limit.clamp(1, 100)).into());
    let mut statement = conn.prepare(&sql)?;
    let rows = statement
        .query_map(rusqlite::params_from_iter(values.iter()), map_conversation_message)?
        .collect::<Result<Vec<_>>>()?;
    Ok(page_from_descending_rows(rows))
}
```

Implement `map_conversation_message` by parsing the channel, direction, status, and `content_json` into the exact `ConversationMessage` fields defined in Task 1. Implement `page_from_descending_rows` so returned items are chronological ascending and `next_cursor` points to the oldest returned item when the page is full.

- [ ] **Step 5: Implement group timeline query**

Add `get_conversation_timeline(conversation_id, before, limit)` using the same mapper/order but filtering by `cc.id = ?1` and without lead/channel joins.

- [ ] **Step 6: Run repository tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::repository_test
```

Expected: PASS.

## Task 5: Add Inbox Read Model, Service, Events, and Tauri Commands

**Files:**
- Create: `src-tauri/src/conversation/events.rs`
- Create: `src-tauri/src/conversation/service.rs`
- Create: `src-tauri/src/conversation/commands.rs`
- Create: `src-tauri/src/conversation/commands_test.rs`
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/mod.rs`
- Modify: `src-tauri/src/lib.rs:21-184`

- [ ] **Step 1: Add failing service/command tests**

Create `commands_test.rs` with a temp repository and assert:

```rust
#[test]
fn list_inbox_returns_customer_unknown_and_group_rows() {
    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let linked = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "linked-message"))
        .unwrap();
    harness
        .repository
        .link_identity(linked.identity_id.unwrap(), 7)
        .unwrap();
    let unknown = harness
        .repository
        .apply_normalized_message(&direct_message_for("imei-a", "peer-2", "unknown-message"))
        .unwrap();
    let group = harness
        .repository
        .apply_normalized_message(&group_message("imei-a", "group-1", "group-message"))
        .unwrap();

    let rows = harness.service.list_inbox().unwrap();
    let keys = rows.iter().map(|row| row.key.as_str()).collect::<Vec<_>>();
    let unknown_key = format!("identity:{}", unknown.identity_id.unwrap());
    let group_key = format!("conversation:{}", group.conversation_id);
    assert_eq!(keys.iter().filter(|key| **key == "lead:7").count(), 1);
    assert_eq!(
        keys.iter()
            .filter(|key| **key == unknown_key.as_str())
            .count(),
        1
    );
    assert_eq!(
        keys.iter()
            .filter(|key| **key == group_key.as_str())
            .count(),
        1
    );
}

#[test]
fn link_identity_publishes_link_invalidation() {
    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let mut receiver = harness.events.subscribe();

    harness
        .service
        .link_identity(applied.identity_id.unwrap(), 7)
        .unwrap();

    let event = receiver.try_recv().unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Link);
    assert_eq!(event.lead_id, Some(7));
    assert_eq!(event.conversation_id, applied.conversation_id);
}
```

Create `ServiceHarness` by copying the temp-path/migration setup from `repository_test.rs` and adding public test fields `repository`, `events`, and `service`. Add focused constructors `direct_message_for` and `group_message`; they must fill every `NormalizedMessage` field rather than using default/unsafe values.

- [ ] **Step 2: Create event contracts**

Create `events.rs`:

```rust
use crate::conversation::types::Channel;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationInvalidationReason {
    Message,
    Receipt,
    Reaction,
    Metadata,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInvalidatedEvent {
    pub conversation_id: i64,
    pub lead_id: Option<i64>,
    pub channel: Channel,
    pub reason: ConversationInvalidationReason,
    pub message_id: Option<i64>,
}

pub struct ConversationEventPublisher {
    sender: broadcast::Sender<ConversationInvalidatedEvent>,
}

impl ConversationEventPublisher {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConversationInvalidatedEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: ConversationInvalidatedEvent) {
        let _ = self.sender.send(event);
    }

    pub fn spawn_tauri_forwarder(
        self: std::sync::Arc<Self>,
        app_handle: tauri::AppHandle,
    ) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
            let mut receiver = self.subscribe();
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let Err(error) = app_handle.emit("conversation-invalidated", event) {
                            tracing::warn!(target: "conversation", %error, "tauri emit failed");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(target: "conversation", skipped, "event forwarder lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }
}
```

- [ ] **Step 3: Implement `ConversationService`**

Create `service.rs`:

```rust
use super::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
use super::repository::ConversationRepository;
use super::types::{Channel, InboxItem, TimelineCursor, TimelinePage};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConversationService {
    repository: Arc<ConversationRepository>,
    events: Arc<ConversationEventPublisher>,
}

impl ConversationService {
    pub fn new(
        repository: Arc<ConversationRepository>,
        events: Arc<ConversationEventPublisher>,
    ) -> Self {
        Self { repository, events }
    }

    pub fn list_inbox(&self) -> Result<Vec<InboxItem>> {
        Ok(self.repository.list_inbox()?)
    }

    pub fn get_contact_timeline(
        &self,
        lead_id: i64,
        channels: &[Channel],
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        Ok(self.repository.get_contact_timeline(lead_id, channels, before, limit)?)
    }

    pub fn get_conversation_timeline(
        &self,
        conversation_id: i64,
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        Ok(self
            .repository
            .get_conversation_timeline(conversation_id, before, limit)?)
    }

    pub fn link_identity(&self, identity_id: i64, lead_id: i64) -> Result<()> {
        let conversation_id = self.repository.link_identity(identity_id, lead_id)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id,
            lead_id: Some(lead_id),
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Link,
            message_id: None,
        });
        Ok(())
    }
}
```

`repository.link_identity` already returns the linked identity's most recent conversation ID. If no conversation exists, it returns `0`; the UI treats zero as a full inbox invalidation.

- [ ] **Step 4: Implement inbox aggregation**

Add `repository.list_inbox() -> Result<Vec<InboxItem>>` using three explicit queries:

1. Linked direct/email conversations grouped in Rust by `lead_id` with key `lead:{lead_id}`.
2. Unlinked direct conversations with key `identity:{identity_id}`.
3. Group conversations with key `conversation:{conversation_id}`.

Each query returns at most the latest 500 sidebar rows ordered by `last_message_at DESC`. Customer channel badges aggregate message counts by channel. Populate `reply_targets` with one row per provider conversation/account, including `last_inbound_at` so the frontend can select the most recently contacted eligible account without guessing from the currently loaded page. Group rows expose exactly one locked reply target, `participant_count`, and up to five active participant summaries ordered with admins first and then display name. Customer/unknown rows return `participant_count = 0` and an empty participant list. Do not include group conversations in linked customer rows. Do not invent unread counts from outbound provider receipts; unread markers are outside this MVP.

- [ ] **Step 5: Add Tauri command wrappers**

Create `commands.rs`:

```rust
use super::service::ConversationService;
use super::types::{
    Channel, InboxItem, LinkIdentityRequest, TimelineCursor, TimelinePage,
};
use std::sync::Arc;

pub struct ConversationState {
    pub service: Arc<ConversationService>,
}

#[tauri::command]
pub fn list_conversation_inbox(
    state: tauri::State<'_, ConversationState>,
) -> Result<Vec<InboxItem>, String> {
    state.service.list_inbox().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_contact_timeline(
    state: tauri::State<'_, ConversationState>,
    lead_id: i64,
    active_channels: Vec<Channel>,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    state
        .service
        .get_contact_timeline(lead_id, &active_channels, before.as_ref(), limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_conversation_timeline(
    state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
    before: Option<TimelineCursor>,
    limit: u32,
) -> Result<TimelinePage, String> {
    state
        .service
        .get_conversation_timeline(conversation_id, before.as_ref(), limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn link_conversation_identity(
    state: tauri::State<'_, ConversationState>,
    request: LinkIdentityRequest,
) -> Result<(), String> {
    state
        .service
        .link_identity(request.identity_id, request.lead_id)
        .map_err(|error| error.to_string())
}
```

- [ ] **Step 6: Register modules, state, event forwarder, and commands**

Add to `conversation/mod.rs`:

```rust
pub mod commands;
pub mod events;
pub mod service;
```

In `src-tauri/src/lib.rs` setup, after `db_path_str` is available and after database initialization:

```rust
let conversation_repository = std::sync::Arc::new(
    conversation::repository::ConversationRepository::new(db_path_str.to_string()),
);
let conversation_events = std::sync::Arc::new(
    conversation::events::ConversationEventPublisher::new(256),
);
let conversation_service = std::sync::Arc::new(
    conversation::service::ConversationService::new(
        conversation_repository,
        conversation_events.clone(),
    ),
);
app.manage(conversation::commands::ConversationState {
    service: conversation_service,
});
conversation_events.spawn_tauri_forwarder(app.handle().clone());
```

Register these commands in `tauri::generate_handler!`:

```rust
conversation::commands::list_conversation_inbox,
conversation::commands::get_contact_timeline,
conversation::commands::get_conversation_timeline,
conversation::commands::link_conversation_identity,
```

- [ ] **Step 7: Run command/service tests**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::commands_test
cargo test --manifest-path src-tauri/Cargo.toml conversation::repository_test
```

Expected: PASS.

## Task 6: Storage Foundation Verification

**Files:**
- No new files

- [ ] **Step 1: Run all conversation tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
```

- [ ] **Step 2: Run all database and Outreach regressions**

```bash
cargo test --manifest-path src-tauri/Cargo.toml db_test::
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_
```

- [ ] **Step 3: Run full verification**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

- [ ] **Step 4: Verify graph impact**

Run:

```text
mcp__gitnexus.detect_changes({repo: "RustSale_v2", scope: "all"})
```

Expected: conversation modules plus the intentional one-line `init_db` integration and `lib.rs` registration. Investigate any unrelated affected process before continuing.
