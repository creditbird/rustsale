# Zalo Conversation Sync Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep enabled Zalo personal sessions connected, normalize direct/group events into SQLite, recover missed history, expose runtime status, and send durable direct/group text messages through the selected IMEI.

**Architecture:** A `ZaloSessionSupervisor` owns one long-lived `zca-rs::Zalo` client and ingestion task per IMEI. Raw listener events are normalized before repository mutations; receiver lag, reconnect, and startup trigger reconciliation. Outbound messages enter a SQLite outbox and are processed serially per account. A job left `running` after a crash is marked delivery-unknown and is not resent automatically.

**Tech Stack:** Rust, Tokio, tokio-util `CancellationToken`, async-trait, vendored `zca-rs`, rusqlite, Tauri managed state/events.

---

## Preconditions

- Complete the live spike plan and record any observed cursor differences in the design before implementation.
- Complete the storage foundation plan.
- Re-run impact analysis for `run` before startup integration. Expected risk: LOW.

## Task 1: Define Adapter, Runtime, and Mutation Contracts

**Files:**
- Create: `src-tauri/src/conversation/adapter.rs`
- Modify: `src-tauri/src/conversation/types.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

- [ ] **Step 1: Add failing contract serialization tests**

Append to the tests in `types.rs`:

```rust
#[test]
fn runtime_status_contract_uses_camel_case_fields() {
    let status = ConversationRuntimeStatus {
        accounts: vec![ConversationAccountRuntime {
            channel_account_id: 1,
            channel: Channel::Zalo,
            provider_account_key: "imei-a".to_string(),
            display_name: Some("Sale 01".to_string()),
            status: AccountConnectionStatus::Connected,
            last_connected_at: Some("2026-07-31T00:00:00.000Z".to_string()),
            last_error: None,
        }],
    };

    let json = serde_json::to_value(status).unwrap();
    assert_eq!(json["accounts"][0]["channelAccountId"], 1);
    assert_eq!(json["accounts"][0]["status"], "connected");
}
```

- [ ] **Step 2: Implement runtime and send request types**

Add to `types.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Degraded,
    Invalid,
}

impl AccountConnectionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Degraded => "degraded",
            Self::Invalid => "invalid",
        }
    }

    pub fn from_db(value: &str) -> rusqlite::Result<Self> {
        match value {
            "disconnected" => Ok(Self::Disconnected),
            "connecting" => Ok(Self::Connecting),
            "connected" => Ok(Self::Connected),
            "degraded" => Ok(Self::Degraded),
            "invalid" => Ok(Self::Invalid),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid account connection status: {other}"),
                )),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationAccountRuntime {
    pub channel_account_id: i64,
    pub channel: Channel,
    pub provider_account_key: String,
    pub display_name: Option<String>,
    pub status: AccountConnectionStatus,
    pub last_connected_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRuntimeStatus {
    pub accounts: Vec<ConversationAccountRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendConversationMessageRequest {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub text: String,
    pub reply_to_message_id: Option<i64>,
}
```

- [ ] **Step 3: Create adapter contracts**

Create `adapter.rs`:

```rust
use super::types::Channel;
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendRequest {
    pub external_thread_id: String,
    pub is_group: bool,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendResult {
    pub provider_message_id: String,
}

#[async_trait]
pub trait ChannelSender: Send + Sync {
    fn channel(&self) -> Channel;

    async fn send(
        &self,
        provider_account_key: &str,
        request: ProviderSendRequest,
    ) -> anyhow::Result<ProviderSendResult>;
}
```

Add to `conversation/mod.rs`:

```rust
pub mod adapter;
```

- [ ] **Step 4: Run type tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::types::tests
```

Expected: PASS.

## Task 2: Normalize Zalo Listener Events

**Files:**
- Create: `src-tauri/src/conversation/zalo/normalizer.rs`
- Create: `src-tauri/src/conversation/zalo/normalizer_test.rs`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`

- [ ] **Step 1: Write failing direct/group normalization tests**

Create `normalizer_test.rs` with JSON fixtures matching `zca-rs::models::TMessage` and `TGroupMessage`:

```rust
use super::normalizer::{normalize_listener_event, ZaloMutation};
use serde_json::json;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{GroupMessage, Message, TGroupMessage, TMessage, UserMessage};

fn base_message(uid_from: &str, id_to: &str, msg_id: &str, content: serde_json::Value) -> TMessage {
    serde_json::from_value(json!({
        "actionId": "action-1",
        "msgId": msg_id,
        "cliMsgId": format!("client-{msg_id}"),
        "msgType": "webchat",
        "uidFrom": uid_from,
        "idTo": id_to,
        "dName": "Probe Sender",
        "ts": "1785456000000",
        "status": 1,
        "content": content,
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": format!("real-{msg_id}"),
        "quote": null
    }))
    .unwrap()
}

#[test]
fn inbound_direct_message_links_peer_as_sender_and_participant() {
    let event = ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        base_message("peer-uid", "0", "message-1", json!("hello")),
    ))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.external_thread_id, "peer-uid");
    assert_eq!(message.direction, crate::conversation::types::MessageDirection::Inbound);
    assert!(message.sender_is_participant);
    assert_eq!(
        message.participant.as_ref().map(|identity| identity.external_id.as_str()),
        Some("peer-uid")
    );
}

#[test]
fn outbound_group_message_does_not_assign_customer_sender_identity() {
    let raw = TGroupMessage {
        base: base_message("0", "group-1", "message-2", json!("hello group")),
        mentions: None,
    };
    let event = ListenerEvent::Message(Box::new(Message::Group(GroupMessage::new(
        "self-uid",
        raw,
    ))));

    let mutations = normalize_listener_event("imei-a", Some("Sale 01"), event).unwrap();
    let ZaloMutation::Message(message) = &mutations[0] else {
        panic!("expected message mutation");
    };
    assert_eq!(message.external_thread_id, "group-1");
    assert_eq!(message.direction, crate::conversation::types::MessageDirection::Outbound);
    assert!(!message.sender_is_participant);
    assert!(message.participant.is_none());
}
```

- [ ] **Step 2: Register module and verify failure**

Add to `zalo/mod.rs`:

```rust
pub mod normalizer;

#[cfg(test)]
mod normalizer_test;
```

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::normalizer_test
```

Expected: FAIL because normalizer contracts do not exist.

- [ ] **Step 3: Implement normalized mutation types**

Create `normalizer.rs`:

```rust
use crate::conversation::repository::{NormalizedIdentity, NormalizedMessage};
use crate::conversation::types::{
    Channel, ConversationKind, MessageDirection, MessageStatus,
};
use anyhow::Result;
use chrono::{SecondsFormat, TimeZone, Utc};
use serde_json::Value;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{DeliveredMessage, Message, Reaction, SeenMessage, Undo};

#[derive(Debug, Clone)]
pub struct NormalizedReaction {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub actor_external_id: String,
    pub reaction: String,
    pub removed: bool,
    pub occurred_at: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct NormalizedUndo {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub occurred_at: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptKind {
    Delivered,
    Read,
}

#[derive(Debug, Clone)]
pub struct NormalizedReceipt {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub recipient_external_ids: Vec<String>,
    pub kind: ReceiptKind,
    pub occurred_at: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct NormalizedGroupParticipant {
    pub identity: NormalizedIdentity,
    pub participant_role: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedGroupSnapshot {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub title: String,
    pub avatar_url: Option<String>,
    pub participants: Vec<NormalizedGroupParticipant>,
    pub membership_complete: bool,
    pub occurred_at: String,
}

#[derive(Debug, Clone)]
pub enum ZaloMutation {
    Message(NormalizedMessage),
    Reaction(NormalizedReaction),
    Undo(NormalizedUndo),
    Receipt(NormalizedReceipt),
    GroupSnapshot(NormalizedGroupSnapshot),
    ReconcileRequired { scope: &'static str },
}

fn provider_time(value: &str) -> String {
    value
        .parse::<i64>()
        .ok()
        .and_then(|millis| Utc.timestamp_millis_opt(millis).single())
        .map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_else(crate::conversation::types::utc_now)
}

fn content_parts(message_type: &str, content: &Value) -> (String, Option<String>, Value) {
    if let Some(text) = content.as_str() {
        return ("text".to_string(), Some(text.to_string()), serde_json::json!({"text": text}));
    }
    let content_type = match message_type {
        "chat.photo" => "image",
        "chat.video.msg" => "video",
        "chat.voice" => "audio",
        "share.file" => "file",
        "chat.sticker" => "sticker",
        "chat.link" => "link",
        _ => "unsupported",
    };
    (content_type.to_string(), None, content.clone())
}

fn normalize_message(
    provider_account_key: &str,
    account_display_name: Option<&str>,
    message: Message,
) -> Result<NormalizedMessage> {
    let now = crate::conversation::types::utc_now();
    let (base, external_thread_id, is_self, kind, participant) = match message {
        Message::User(message) => {
            let participant = Some(NormalizedIdentity {
                identity_namespace: "zalo_uid".to_string(),
                external_id: message.thread_id.clone(),
                display_name: (!message.data.d_name.is_empty()).then(|| message.data.d_name.clone()),
                avatar_url: None,
                lead_id: None,
            });
            (message.data, message.thread_id, message.is_self, ConversationKind::Direct, participant)
        }
        Message::Group(message) => {
            let participant = (!message.is_self).then(|| NormalizedIdentity {
                identity_namespace: "zalo_uid".to_string(),
                external_id: message.data.base.uid_from.clone(),
                display_name: (!message.data.base.d_name.is_empty())
                    .then(|| message.data.base.d_name.clone()),
                avatar_url: None,
                lead_id: None,
            });
            (
                message.data.base,
                message.thread_id,
                message.is_self,
                ConversationKind::Group,
                participant,
            )
        }
    };
    let (content_type, text_content, content) = content_parts(&base.msg_type, &base.content);

    Ok(NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: provider_account_key.to_string(),
        account_display_name: account_display_name.map(str::to_string),
        participant,
        sender_is_participant: !is_self,
        external_thread_id,
        conversation_kind: kind,
        conversation_title: None,
        direction: if is_self {
            MessageDirection::Outbound
        } else {
            MessageDirection::Inbound
        },
        provider_message_id: (!base.msg_id.is_empty()).then_some(base.msg_id.clone()),
        provider_client_message_id: (!base.cli_msg_id.is_empty()).then_some(base.cli_msg_id.clone()),
        local_request_id: None,
        content_type,
        text_content,
        content,
        raw_payload: Some(serde_json::to_value(&base)?),
        provider_timestamp: Some(provider_time(&base.ts)),
        ingested_at: now,
        status: if is_self { MessageStatus::Sent } else { MessageStatus::Received },
    })
}
```

Complete `normalize_listener_event(account_key, account_name, event)` with these exact mappings:

- `Message`: one `ZaloMutation::Message`.
- `OldMessages`: one message mutation per item.
- `Reaction` and `OldReactions`: reaction mutations using `g_msg_id`, `c_msg_id`, `uid_from`, `r_icon`, and `r_type == 0` as removal semantics verified by the live spike.
- `Undo`: one undo mutation using `msg_id` and `cli_msg_id`.
- `DeliveredMessages`: one receipt mutation per event.
- `SeenMessages`: one receipt mutation per event.
- `Disconnected`, `Closed`, `Error`: one `ReconcileRequired` mutation for `direct_messages` and one for `group_messages`.
- `GroupEvent`: emit `ReconcileRequired { scope: "group_metadata" }`; the supervisor resolves the latest group snapshot through `get_group_info` before applying metadata.
- `Connected`, `Typing`, `UploadAttachment`, and `FriendEvent`: no database mutation in this task.

- [ ] **Step 4: Run normalizer tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::normalizer_test
```

Expected: PASS.

## Task 3: Persist Reactions, Undo, Receipts, Account Status, and Checkpoints

**Files:**
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/repository_test.rs`

- [ ] **Step 1: Write failing mutation tests**

Add tests for:

```rust
#[test]
fn reaction_upsert_targets_provider_or_client_message_id() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();
    let reaction = NormalizedReaction {
        provider_account_key: "imei-a".to_string(),
        external_thread_id: "peer-1".to_string(),
        is_group: false,
        provider_message_id: "message-1".to_string(),
        provider_client_message_id: "client-1".to_string(),
        actor_external_id: "peer-1".to_string(),
        reaction: "heart".to_string(),
        removed: false,
        occurred_at: "2026-07-31T01:00:00.000Z".to_string(),
        raw_payload: serde_json::json!({}),
    };
    harness.repository.apply_reaction(&reaction).unwrap();
    harness.repository.apply_reaction(&reaction).unwrap();
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 1);
}

#[test]
fn undo_marks_message_removed_without_deleting_it() {
    let harness = Harness::new();
    harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();
    harness
        .repository
        .apply_undo(&NormalizedUndo {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_message_id: "message-1".to_string(),
            provider_client_message_id: "client-1".to_string(),
            occurred_at: "2026-07-31T01:01:00.000Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM messages"), 1);
    assert_eq!(harness.text("SELECT status FROM messages LIMIT 1"), "removed");
    assert!(harness.optional_text("SELECT removed_at FROM messages LIMIT 1").is_some());
}

#[test]
fn read_receipt_advances_outbound_message_status() {
    let harness = Harness::new();
    let mut message = direct_message_with_client_id("message-1", "client-1");
    message.direction = MessageDirection::Outbound;
    message.status = MessageStatus::Sent;
    harness.repository.apply_normalized_message(&message).unwrap();
    harness
        .repository
        .apply_receipt(&NormalizedReceipt {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "peer-1".to_string(),
            is_group: false,
            provider_message_id: "message-1".to_string(),
            recipient_external_ids: vec!["peer-1".to_string()],
            kind: ReceiptKind::Read,
            occurred_at: "2026-07-31T01:02:00.000Z".to_string(),
            raw_payload: serde_json::json!({}),
        })
        .unwrap();
    assert_eq!(harness.text("SELECT status FROM messages LIMIT 1"), "read");
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_receipts"), 1);
}

#[test]
fn checkpoint_upsert_is_monotonic_per_account_scope() {
    let harness = Harness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message_with_client_id("message-1", "client-1"))
        .unwrap();
    harness
        .repository
        .update_checkpoint(
            applied.channel_account_id,
            "direct_messages",
            Some("cursor-1"),
            Some("2026-07-31T01:00:00.000Z"),
            None,
        )
        .unwrap();
    harness
        .repository
        .update_checkpoint(
            applied.channel_account_id,
            "direct_messages",
            Some("cursor-2"),
            Some("2026-07-31T01:05:00.000Z"),
            None,
        )
        .unwrap();
    assert_eq!(
        harness.text("SELECT cursor FROM channel_sync_checkpoints LIMIT 1"),
        "cursor-2"
    );
}

#[test]
fn group_snapshot_updates_title_avatar_and_participant_roles() {
    let harness = Harness::new();
    let conversation_id = harness
        .repository
        .apply_group_snapshot(&NormalizedGroupSnapshot {
            provider_account_key: "imei-a".to_string(),
            external_thread_id: "group-1".to_string(),
            title: "Project Alpha".to_string(),
            avatar_url: Some("https://example.invalid/group.png".to_string()),
            participants: vec![NormalizedGroupParticipant {
                identity: NormalizedIdentity {
                    identity_namespace: "zalo_uid".to_string(),
                    external_id: "member-1".to_string(),
                    display_name: Some("Member One".to_string()),
                    avatar_url: None,
                    lead_id: None,
                },
                participant_role: "admin".to_string(),
            }],
            membership_complete: true,
            occurred_at: "2026-07-31T01:05:00.000Z".to_string(),
        })
        .unwrap();
    assert!(conversation_id > 0);
    assert_eq!(
        harness.text("SELECT title FROM channel_conversations WHERE id = 1"),
        "Project Alpha"
    );
    assert_eq!(
        harness.text("SELECT participant_role FROM conversation_participants LIMIT 1"),
        "admin"
    );
}
```

Extend the repository test `Harness` with `scalar`, `text`, and `optional_text` helpers that open a fresh short connection. Add `direct_message_with_client_id` by copying the complete `NormalizedMessage` fixture from the storage plan and setting both provider IDs explicitly.

- [ ] **Step 2: Implement repository mutation methods**

Add exact public methods:

```rust
pub fn apply_reaction(&self, reaction: &NormalizedReaction) -> Result<Option<i64>>;
pub fn apply_undo(&self, undo: &NormalizedUndo) -> Result<Option<i64>>;
pub fn apply_receipt(&self, receipt: &NormalizedReceipt) -> Result<Option<i64>>;
pub fn apply_group_snapshot(&self, snapshot: &NormalizedGroupSnapshot) -> Result<i64>;
pub fn update_checkpoint(
    &self,
    channel_account_id: i64,
    scope: &str,
    cursor: Option<&str>,
    event_at: Option<&str>,
    error: Option<&str>,
) -> Result<()>;
pub fn set_account_status(
    &self,
    provider_account_key: &str,
    status: AccountConnectionStatus,
    error: Option<&str>,
) -> Result<i64>;
```

`apply_group_snapshot` runs in one transaction. It upserts the Zalo account and group conversation, updates `title` and `avatar_url`, upserts each `zalo_uid` identity with display name/avatar, and upserts `conversation_participants.participant_role` as `admin` or `member`. When `membership_complete` is true, participants absent from the snapshot receive `left_at = occurred_at`; partial snapshots never mark missing members as left.

Resolve messages using `(conversation, provider_message_id)` first and `provider_client_message_id` second. Unknown receipts/reactions/undo return `Ok(None)` and are logged for reconciliation; they are not treated as fatal ingestion errors.

Status precedence is monotonic:

```text
queued < sending < sent < delivered < read
```

`failed` and `removed` are terminal display states and are not replaced by late delivered/read receipts.

- [ ] **Step 3: Run repository mutation tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::repository_test
```

Expected: PASS.

## Task 4: Add Testable Zalo Client Abstraction

**Files:**
- Create: `src-tauri/src/conversation/zalo/client.rs`
- Create: `src-tauri/src/conversation/zalo/client_test.rs`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`

- [ ] **Step 1: Define the client and factory traits**

Create `client.rs`:

```rust
use async_trait::async_trait;
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;
use std::sync::Arc;
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;

#[async_trait]
pub trait ZaloClient: Send + Sync {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent>;
    fn request_old_messages(
        &self,
        thread_type: zca_rs::models::ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()>;
    fn request_old_reactions(
        &self,
        thread_type: zca_rs::models::ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>>;
    async fn get_group_snapshot(
        &self,
        group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>>;
    async fn send_text(
        &self,
        thread_id: &str,
        thread_type: zca_rs::models::ThreadType,
        text: &str,
    ) -> anyhow::Result<String>;
    fn stop(&self);
}

#[async_trait]
pub trait ZaloClientFactory: Send + Sync {
    async fn connect(&self, session: crate::models::ZaloSession) -> anyhow::Result<Arc<dyn ZaloClient>>;
}

pub struct RealZaloClient {
    inner: Arc<zca_rs::Zalo>,
    provider_account_key: String,
}

pub struct RealZaloClientFactory;
```

Implement `RealZaloClientFactory::connect` by parsing session cookies, setting `Options.self_listen = true`, and calling `Zalo::login`. Implement `send_text` with `MessageContent` and return `response.message.msg_id.to_string()`. Return an error when the response contains no text message ID.

Implement `get_all_group_snapshots` by calling `get_all_groups`, taking the keys from `grid_ver_map`, and calling `get_group_info` in batches of 50 IDs. Implement `get_group_snapshot` with one `get_group_info` call. Convert `GroupInfo.current_mems` to `NormalizedGroupParticipant`, set role `admin` when the member ID exists in `admin_ids`, prefer `full_avt` over `avt`, and set `membership_complete = info.has_more_member == 0`. Group identities always use namespace `zalo_uid` and `lead_id = None`.

- [ ] **Step 2: Add a fake client used by supervisor tests**

In `client_test.rs`, implement `FakeZaloClient` with a broadcast sender, captured history requests, captured group snapshot requests, captured send requests, and a deterministic provider message ID. Verify the trait forwards direct/group thread types without modification and maps complete/partial group membership explicitly.

- [ ] **Step 3: Run client tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::client_test
```

Expected: PASS without network access.

## Task 5: Implement Session Supervisor and Event Ingestion

**Files:**
- Create: `src-tauri/src/conversation/zalo/supervisor.rs`
- Create: `src-tauri/src/conversation/zalo/supervisor_test.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`

- [ ] **Step 1: Write failing supervisor tests**

Tests must cover:

```rust
#[tokio::test]
async fn supervisor_persists_fake_direct_message_and_publishes_invalidation() {
    let harness = SupervisorHarness::new().await;
    let mut events = harness.events.subscribe();
    harness.supervisor.start_all().await.unwrap();
    harness
        .factory
        .emit("imei-a", direct_listener_event("message-1"))
        .await;

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Message);
    assert_eq!(harness.repository.message_count(), 1);
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn lagged_receiver_requests_direct_and_group_reconciliation() {
    let harness = SupervisorHarness::with_listener_capacity(1).await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.emit_burst("imei-a", 8).await;
    harness.factory.wait_for_history_request_count("imei-a", 4).await;

    let requests = harness.factory.history_requests("imei-a").await;
    assert!(requests.iter().any(|request| request.thread_type == ThreadType::User));
    assert!(requests.iter().any(|request| request.thread_type == ThreadType::Group));
    assert_eq!(harness.repository.account_status("imei-a"), "degraded");
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn restart_account_cancels_old_task_before_connecting_again() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;

    harness.supervisor.restart_account("imei-a").await.unwrap();
    harness.factory.wait_for_stop_count("imei-a", 1).await;
    harness.factory.wait_for_connect_count("imei-a", 2).await;

    assert_eq!(harness.factory.max_simultaneous_clients("imei-a"), 1);
    harness.supervisor.stop_account("imei-a").await;
}
```

Create `SupervisorHarness` with a migrated temp database containing one enabled `zalo_sessions` row, a `FakeZaloClientFactory`, `ConversationRepository`, `ConversationService`, `ConversationEventPublisher`, and `ZaloSessionSupervisor`. The fake factory exposes deterministic emit/count/wait helpers used above; waits use one-second `tokio::time::timeout` bounds so failures cannot hang the suite.

- [ ] **Step 2: Add ingestion method to `ConversationService`**

Add:

```rust
pub fn ingest_zalo_event(
    &self,
    provider_account_key: &str,
    account_display_name: Option<&str>,
    event: zca_rs::listener::ListenerEvent,
) -> anyhow::Result<()> {
    for mutation in crate::conversation::zalo::normalizer::normalize_listener_event(
        provider_account_key,
        account_display_name,
        event,
    )? {
        match mutation {
            ZaloMutation::Message(message) => {
                let applied = self.repository.apply_normalized_message(&message)?;
                if !applied.duplicate {
                    let lead_id = applied
                        .identity_id
                        .and_then(|identity_id| self.repository.lead_id_for_identity(identity_id).ok().flatten());
                    self.events.publish(ConversationInvalidatedEvent {
                        conversation_id: applied.conversation_id,
                        lead_id,
                        channel: Channel::Zalo,
                        reason: ConversationInvalidationReason::Message,
                        message_id: Some(applied.message_id),
                    });
                }
            }
            ZaloMutation::Reaction(reaction) => {
                if let Some(message_id) = self.repository.apply_reaction(&reaction)? {
                    self.publish_message_invalidation(message_id, ConversationInvalidationReason::Reaction)?;
                }
            }
            ZaloMutation::Undo(undo) => {
                if let Some(message_id) = self.repository.apply_undo(&undo)? {
                    self.publish_message_invalidation(message_id, ConversationInvalidationReason::Message)?;
                }
            }
            ZaloMutation::Receipt(receipt) => {
                if let Some(message_id) = self.repository.apply_receipt(&receipt)? {
                    self.publish_message_invalidation(message_id, ConversationInvalidationReason::Receipt)?;
                }
            }
            ZaloMutation::GroupSnapshot(snapshot) => {
                let conversation_id = self.repository.apply_group_snapshot(&snapshot)?;
                self.events.publish(ConversationInvalidatedEvent {
                    conversation_id,
                    lead_id: None,
                    channel: Channel::Zalo,
                    reason: ConversationInvalidationReason::Metadata,
                    message_id: None,
                });
            }
            ZaloMutation::ReconcileRequired { .. } => {}
        }
    }
    Ok(())
}
```

Implement `repository.lead_id_for_identity` and `publish_message_invalidation` using one lookup that returns conversation, lead, and channel for a message ID.

Add `ConversationService::apply_group_snapshot(snapshot)` as a small wrapper around `repository.apply_group_snapshot` that publishes the same metadata invalidation. The supervisor uses this async-boundary-safe method after fetching snapshots from the client.

- [ ] **Step 3: Implement supervisor ownership**

Create `supervisor.rs` with:

```rust
pub struct ZaloSessionSupervisor {
    db_path: String,
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
    tasks: tokio::sync::Mutex<HashMap<String, SessionTask>>,
}

struct SessionTask {
    cancellation: CancellationToken,
    handle: tauri::async_runtime::JoinHandle<()>,
}
```

Required methods:

```rust
pub async fn start_all(&self) -> anyhow::Result<()>;
pub async fn restart_account(&self, provider_account_key: &str) -> anyhow::Result<()>;
pub async fn stop_account(&self, provider_account_key: &str);
pub async fn runtime_status(&self) -> anyhow::Result<ConversationRuntimeStatus>;
```

For each session task:

1. Mark account `connecting`.
2. Connect through the factory.
3. Subscribe immediately.
4. Mark account `connected`.
5. Fetch all group snapshots and apply them before history reconciliation.
6. Request direct/group old messages and reactions from stored checkpoints.
7. Drain receiver and poll the account outbox in one `tokio::select!` loop.
8. For `GroupEvent`, fetch and apply the referenced group snapshot before ingesting the event.
9. Call `service.ingest_zalo_event` before publishing UI events.
10. On `Lagged`, request direct/group reconciliation and mark account `degraded`.
11. On `Disconnected`, `Closed`, or recoverable connect failure, drop the client, sleep with bounded exponential backoff, and reconnect inside the same task.
12. On cancellation, call `client.stop()` and exit without reconnecting.

`restart_account` must never await a task while holding `tasks`. Remove the old `SessionTask` under the mutex, release the mutex, cancel and await the old handle, then spawn and insert the replacement under a new lock. The session task never calls `restart_account` on itself; reconnect stays inside its own outer loop, preventing self-cancellation and self-await deadlocks.

Never log cookies or full raw listener payloads.

- [ ] **Step 4: Run supervisor tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::zalo::supervisor_test
```

Expected: PASS.

## Task 6: Add Durable Zalo Text Outbox

**Files:**
- Modify: `src-tauri/src/conversation/repository.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/zalo/supervisor.rs`
- Create: `src-tauri/src/conversation/service_test.rs`

- [ ] **Step 1: Write failing outbox tests**

Required tests:

```rust
#[test]
fn enqueue_outbound_message_and_job_are_one_transaction() {
    let harness = ServiceHarness::new();
    let request = harness.direct_send_request("hello");
    let message = harness.service.enqueue_message(&request).unwrap();

    assert_eq!(message.status, MessageStatus::Queued);
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM messages"), 1);
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_outbox_jobs"), 1);
}

#[tokio::test]
async fn successful_worker_send_records_provider_id_and_completes_job() {
    let harness = ServiceHarness::new();
    let request = harness.direct_send_request("hello");
    let message = harness.service.enqueue_message(&request).unwrap();
    let client = FakeZaloClient::with_provider_message_id("provider-42");

    let processed = harness
        .service
        .process_next_outbox_job(request.channel_account_id, &client)
        .await
        .unwrap();

    assert!(processed);
    assert_eq!(harness.message_status(message.id), MessageStatus::Sent);
    assert_eq!(harness.provider_message_id(message.id).as_deref(), Some("provider-42"));
    assert_eq!(harness.outbox_status_for_message(message.id), "completed");
}

#[test]
fn startup_marks_running_job_delivery_unknown_without_requeue() {
    let harness = ServiceHarness::new();
    let request = harness.direct_send_request("hello");
    let message = harness.service.enqueue_message(&request).unwrap();
    harness.mark_job_running(message.id);

    assert_eq!(harness.service.recover_outbox_startup().unwrap(), 1);
    assert_eq!(harness.message_status(message.id), MessageStatus::Failed);
    assert_eq!(harness.outbox_status_for_message(message.id), "failed");
    assert_eq!(harness.pending_job_count(), 0);
}
```

Create `ServiceHarness` with one linked direct conversation and one enabled Zalo channel account. Its query helpers reopen the temp database and parse status through the same domain parser used by production code.

- [ ] **Step 2: Add repository outbox records**

Add:

```rust
pub struct ClaimedOutboxJob {
    pub job_id: String,
    pub message_id: i64,
    pub channel_account_id: i64,
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub text: String,
}

pub fn enqueue_outbound_message(
    &self,
    request: &SendConversationMessageRequest,
    local_request_id: &str,
    job_id: &str,
    now: &str,
) -> Result<ConversationMessage>;
pub fn claim_next_outbox_job(&self, channel_account_id: i64, now: &str) -> Result<Option<ClaimedOutboxJob>>;
pub fn complete_outbox_job(&self, job_id: &str, provider_message_id: &str, now: &str) -> Result<i64>;
pub fn fail_outbox_job(&self, job_id: &str, retry_at: Option<&str>, error: &str, now: &str) -> Result<()>;
pub fn mark_running_jobs_delivery_unknown(&self, now: &str) -> Result<u64>;
```

`enqueue_outbound_message` must verify that the supplied account owns the conversation. Insert the queued message and outbox job in one transaction.

`mark_running_jobs_delivery_unknown` sets job status `failed`, message status `failed`, and message error `Delivery state unknown after application restart; manual retry required.` It must not return those jobs to `pending`.

- [ ] **Step 3: Add service send method**

```rust
pub fn enqueue_message(
    &self,
    request: &SendConversationMessageRequest,
) -> anyhow::Result<ConversationMessage> {
    if request.text.trim().is_empty() {
        anyhow::bail!("message text must not be empty");
    }
    let local_request_id = format!("msg-{}", rand::random::<u128>());
    let job_id = format!("outbox-{}", rand::random::<u128>());
    let now = crate::conversation::types::utc_now();
    Ok(self.repository.enqueue_outbound_message(
        request,
        &local_request_id,
        &job_id,
        &now,
    )?)
}
```

Add a testable one-job worker method:

```rust
pub async fn process_next_outbox_job(
    &self,
    channel_account_id: i64,
    client: &dyn crate::conversation::zalo::client::ZaloClient,
) -> anyhow::Result<bool>;
```

It returns `false` when no job is ready and `true` after one job reaches a persisted terminal/retry state. The supervisor calls this method from its account `tokio::select!` interval branch, keeping provider I/O outside repository transactions.

- [ ] **Step 4: Add serial account outbox worker**

Each connected session task runs a second loop for its channel account:

1. Claim one pending/retry-ready job transactionally.
2. Call `client.send_text` with `ThreadType::User` or `ThreadType::Group` from conversation kind.
3. On success, complete the job and persist provider message ID.
4. On authentication/session failure, fail terminally and mark account invalid.
5. On bounded transient failures, set `retry_wait` with exponential backoff.
6. Publish message invalidation after every status change.

Use one worker per IMEI; never process two sends concurrently on the same personal session.

- [ ] **Step 5: Run outbox tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::service_test
```

Expected: PASS.

## Task 7: Register Runtime State and Commands

**Files:**
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/lib.rs:21-184`
- Modify: `src-tauri/src/conversation/zalo/mod.rs`

- [ ] **Step 1: Extend managed state without replacing the service**

Change:

```rust
pub struct ConversationState {
    pub service: Arc<ConversationService>,
    pub zalo_supervisor: Arc<crate::conversation::zalo::supervisor::ZaloSessionSupervisor>,
}
```

- [ ] **Step 2: Add commands**

```rust
#[tauri::command]
pub fn send_conversation_message(
    state: tauri::State<'_, ConversationState>,
    request: SendConversationMessageRequest,
) -> Result<ConversationMessage, String> {
    state.service.enqueue_message(&request).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_conversation_runtime_status(
    state: tauri::State<'_, ConversationState>,
) -> Result<ConversationRuntimeStatus, String> {
    state
        .zalo_supervisor
        .runtime_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn restart_conversation_account(
    state: tauri::State<'_, ConversationState>,
    provider_account_key: String,
) -> Result<(), String> {
    state
        .zalo_supervisor
        .restart_account(&provider_account_key)
        .await
        .map_err(|error| error.to_string())
}
```

- [ ] **Step 3: Construct and start the supervisor**

In `lib.rs` setup, after building `conversation_service`:

```rust
let zalo_supervisor = std::sync::Arc::new(
    conversation::zalo::supervisor::ZaloSessionSupervisor::new(
        db_path_str.to_string(),
        conversation_service.clone(),
        std::sync::Arc::new(conversation::zalo::client::RealZaloClientFactory),
    ),
);
app.manage(conversation::commands::ConversationState {
    service: conversation_service.clone(),
    zalo_supervisor: zalo_supervisor.clone(),
});
let startup_service = conversation_service.clone();
tauri::async_runtime::spawn(async move {
    if let Err(error) = startup_service.recover_outbox_startup() {
        tracing::error!(target: "conversation", %error, "outbox recovery failed");
    }
    if let Err(error) = zalo_supervisor.start_all().await {
        tracing::error!(target: "conversation", %error, "Zalo supervisor startup failed");
    }
});
```

- [ ] **Step 4: Register commands**

Add to `tauri::generate_handler!`:

```rust
conversation::commands::send_conversation_message,
conversation::commands::get_conversation_runtime_status,
conversation::commands::restart_conversation_account,
```

- [ ] **Step 5: Run startup and command tests**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
cargo test --manifest-path src-tauri/Cargo.toml test_verify_zalo_session_commands_with_state
```

Expected: PASS.

## Task 8: Sync Engine Verification

**Files:**
- No new files

- [ ] **Step 1: Run all targeted suites**

```bash
cargo test --manifest-path src-tauri/Cargo.toml conversation::
cargo test --manifest-path src-tauri/Cargo.toml test_send_zalo_
```

- [ ] **Step 2: Run full Rust verification**

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

- [ ] **Step 3: Run live smoke verification**

Start the Tauri application with a non-production Zalo session. Verify:

1. Account runtime becomes `connected`.
2. Direct inbound message creates one row.
3. Group inbound message creates one group conversation.
4. Reaction, recall, delivered, and seen updates apply without duplicate messages.
5. Restarting the account reconnects and backfills missed events.
6. Sending direct and group text from the conversation command records provider IDs.

- [ ] **Step 4: Verify graph scope**

```text
mcp__gitnexus.detect_changes({repo: "RustSale_v2", scope: "all"})
```

Expected: conversation modules and intentional `lib.rs` integration only. Existing Outreach internals remain untouched until the Outreach integration child plan.
