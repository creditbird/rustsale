use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{
    EventReactions, Message, Reaction, TMessage, TReaction, TReactionContent, TReactionMsg,
    ThreadType, UserMessage,
};

use crate::conversation::events::ConversationEventPublisher;
use crate::conversation::migration::migrate;
use crate::conversation::repository::{
    ConversationRepository, NormalizedIdentity, NormalizedMessage,
};
use crate::conversation::service::ConversationService;
use crate::conversation::types::{
    utc_now, Channel, ConversationCapabilities, ForwardConversationMessageRequest,
    ForwardTargetStatus, MessageDirection, MessageStatus, SendConversationMessageRequest,
};
use crate::conversation::zalo::client::{NormalizedProfile, ZaloClient};
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;
use crate::conversation::zalo::webhook::{WebhookTuning, ZaloWebhook};
use crate::db::open_db_connection;

fn direct_message_event(message_id: &str) -> ListenerEvent {
    let raw_message: TMessage = serde_json::from_value(serde_json::json!({
        "actionId": "action-1",
        "msgId": message_id,
        "cliMsgId": format!("client-{message_id}"),
        "msgType": "webchat",
        "uidFrom": "peer-uid",
        "idTo": "0",
        "dName": "Peer",
        "ts": "1785492000000",
        "status": 1,
        "content": "hello",
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": format!("real-{message_id}"),
        "quote": null
    }))
    .unwrap();

    ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid",
        raw_message,
    ))))
}

fn direct_reaction_event(target_message_id: &str) -> ListenerEvent {
    direct_reaction_event_with_client_id(target_message_id, "old-client-message")
}

fn direct_reaction_event_with_client_id(
    target_message_id: &str,
    target_client_message_id: &str,
) -> ListenerEvent {
    ListenerEvent::Reaction(Reaction::new(
        "self-uid",
        TReaction {
            action_id: "reaction-action".to_string(),
            msg_id: "reaction-event".to_string(),
            cli_msg_id: "reaction-client".to_string(),
            msg_type: "1".to_string(),
            uid_from: "peer-uid".to_string(),
            id_to: "0".to_string(),
            d_name: Some("Peer".to_string()),
            content: TReactionContent {
                r_msg: vec![TReactionMsg {
                    g_msg_id: target_message_id.to_string(),
                    c_msg_id: target_client_message_id.to_string(),
                    msg_type: 1,
                }],
                r_icon: EventReactions::Heart,
                r_type: 1,
                source: 1,
            },
            ts: "1785495600000".to_string(),
            ttl: 0,
        },
        false,
    ))
}

fn self_reaction_event(
    target_message_id: &str,
    target_client_message_id: &str,
    reaction: EventReactions,
) -> ListenerEvent {
    ListenerEvent::Reaction(Reaction::new(
        "self-uid",
        TReaction {
            action_id: "self-reaction-action".to_string(),
            msg_id: "self-reaction-event".to_string(),
            cli_msg_id: "self-reaction-client".to_string(),
            msg_type: "1".to_string(),
            uid_from: "self-uid".to_string(),
            id_to: "peer-1".to_string(),
            d_name: Some("Self".to_string()),
            content: TReactionContent {
                r_msg: vec![TReactionMsg {
                    g_msg_id: target_message_id.to_string(),
                    c_msg_id: target_client_message_id.to_string(),
                    msg_type: 1,
                }],
                r_icon: reaction,
                r_type: 1,
                source: 6,
            },
            ts: "1785495600000".to_string(),
            ttl: 0,
        },
        false,
    ))
}

pub struct FakeZaloClient {
    pub provider_message_id: Option<String>,
    pub should_fail: bool,
    pub fail_error_kind: Option<crate::conversation::zalo::client::ProviderSendErrorKind>,
    pub fail_error: Option<String>,
    pub captured_part_requests:
        std::sync::Mutex<Vec<crate::conversation::zalo::client::ProviderSendRequest>>,
    pub captured_forward_requests:
        std::sync::Mutex<Vec<crate::conversation::zalo::client::ProviderForwardRequest>>,
    pub captured_reaction_requests:
        std::sync::Mutex<Vec<crate::conversation::zalo::client::ProviderReactionRequest>>,
    pub captured_old_reaction_polls: std::sync::Mutex<Vec<(ThreadType, Option<String>)>>,
    pub own_external_id: Option<String>,
    pub reaction_fail_on_call: Option<usize>,
}

impl FakeZaloClient {
    pub fn with_provider_message_id(id: &str) -> Self {
        Self {
            provider_message_id: Some(id.to_string()),
            should_fail: false,
            fail_error_kind: None,
            fail_error: None,
            captured_part_requests: std::sync::Mutex::new(Vec::new()),
            captured_forward_requests: std::sync::Mutex::new(Vec::new()),
            captured_reaction_requests: std::sync::Mutex::new(Vec::new()),
            captured_old_reaction_polls: std::sync::Mutex::new(Vec::new()),
            own_external_id: Some("self-uid".to_string()),
            reaction_fail_on_call: None,
        }
    }

    pub fn failing(err: &str) -> Self {
        Self {
            provider_message_id: None,
            should_fail: true,
            fail_error_kind: None,
            fail_error: Some(err.to_string()),
            captured_part_requests: std::sync::Mutex::new(Vec::new()),
            captured_forward_requests: std::sync::Mutex::new(Vec::new()),
            captured_reaction_requests: std::sync::Mutex::new(Vec::new()),
            captured_old_reaction_polls: std::sync::Mutex::new(Vec::new()),
            own_external_id: Some("self-uid".to_string()),
            reaction_fail_on_call: None,
        }
    }

    pub fn failing_with_kind(
        kind: crate::conversation::zalo::client::ProviderSendErrorKind,
        err: &str,
    ) -> Self {
        Self {
            provider_message_id: None,
            should_fail: true,
            fail_error_kind: Some(kind),
            fail_error: Some(err.to_string()),
            captured_part_requests: std::sync::Mutex::new(Vec::new()),
            captured_forward_requests: std::sync::Mutex::new(Vec::new()),
            captured_reaction_requests: std::sync::Mutex::new(Vec::new()),
            captured_old_reaction_polls: std::sync::Mutex::new(Vec::new()),
            own_external_id: Some("self-uid".to_string()),
            reaction_fail_on_call: None,
        }
    }

    pub fn failing_reaction_on_call(
        call_number: usize,
        kind: crate::conversation::zalo::client::ProviderSendErrorKind,
        err: &str,
    ) -> Self {
        let mut client = Self::with_provider_message_id("provider-default");
        client.reaction_fail_on_call = Some(call_number);
        client.fail_error_kind = Some(kind);
        client.fail_error = Some(err.to_string());
        client
    }
}

#[async_trait]
impl ZaloClient for FakeZaloClient {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        let (_tx, rx) = broadcast::channel(1);
        rx
    }

    fn request_old_messages(
        &self,
        _thread_type: ThreadType,
        _cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn request_old_reactions(
        &self,
        thread_type: ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.captured_old_reaction_polls
            .lock()
            .unwrap()
            .push((thread_type, cursor.map(|s| s.to_string())));
        Ok(())
    }

    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>> {
        Ok(Vec::new())
    }

    async fn get_group_snapshot(
        &self,
        _group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>> {
        Ok(None)
    }

    async fn send_text(
        &self,
        _thread_id: &str,
        _thread_type: ThreadType,
        _text: &str,
    ) -> anyhow::Result<String> {
        if self.should_fail {
            let err = self.fail_error.as_deref().unwrap_or("send failed");
            anyhow::bail!("{err}");
        }
        Ok(self
            .provider_message_id
            .clone()
            .unwrap_or_else(|| "provider-default".to_string()))
    }

    async fn send_part(
        &self,
        request: crate::conversation::zalo::client::ProviderSendRequest,
    ) -> Result<
        crate::conversation::zalo::client::ProviderSendResult,
        crate::conversation::zalo::client::ProviderSendError,
    > {
        self.captured_part_requests
            .lock()
            .unwrap()
            .push(request.clone());
        if self.should_fail {
            let err = self.fail_error.as_deref().unwrap_or("send failed");
            return Err(crate::conversation::zalo::client::ProviderSendError {
                kind: crate::conversation::zalo::client::ProviderSendErrorKind::Retryable,
                message: err.to_string(),
            });
        }
        Ok(crate::conversation::zalo::client::ProviderSendResult {
            provider_message_id: self
                .provider_message_id
                .clone()
                .unwrap_or_else(|| "provider-default".to_string()),
            provider_client_message_id: request.provider_client_message_id,
        })
    }

    async fn forward_message(
        &self,
        request: crate::conversation::zalo::client::ProviderForwardRequest,
    ) -> Result<
        crate::conversation::zalo::client::ProviderSendResult,
        crate::conversation::zalo::client::ProviderSendError,
    > {
        self.captured_forward_requests
            .lock()
            .unwrap()
            .push(request.clone());
        if self.should_fail {
            let kind = self
                .fail_error_kind
                .unwrap_or(crate::conversation::zalo::client::ProviderSendErrorKind::Permanent);
            let err = self.fail_error.as_deref().unwrap_or("forward failed");
            return Err(crate::conversation::zalo::client::ProviderSendError {
                kind,
                message: err.to_string(),
            });
        }
        Ok(crate::conversation::zalo::client::ProviderSendResult {
            provider_message_id: self
                .provider_message_id
                .clone()
                .unwrap_or_else(|| "provider-fwd-default".to_string()),
            provider_client_message_id: request.provider_client_message_id,
        })
    }

    async fn set_reaction(
        &self,
        request: crate::conversation::zalo::client::ProviderReactionRequest,
    ) -> Result<(), crate::conversation::zalo::client::ProviderSendError> {
        let call_number = {
            let mut requests = self.captured_reaction_requests.lock().unwrap();
            requests.push(request);
            requests.len()
        };
        if self.should_fail || self.reaction_fail_on_call == Some(call_number) {
            let kind = self
                .fail_error_kind
                .unwrap_or(crate::conversation::zalo::client::ProviderSendErrorKind::Retryable);
            let err = self.fail_error.as_deref().unwrap_or("reaction failed");
            return Err(crate::conversation::zalo::client::ProviderSendError {
                kind,
                message: err.to_string(),
            });
        }
        Ok(())
    }

    fn own_external_id(&self) -> Option<String> {
        self.own_external_id.clone()
    }

    fn stop(&self) {}
}

pub struct PhoneLookupZaloClient {
    profile: NormalizedProfile,
    sent_thread_ids: std::sync::Mutex<Vec<String>>,
}

impl PhoneLookupZaloClient {
    fn new(profile: NormalizedProfile) -> Self {
        Self {
            profile,
            sent_thread_ids: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ZaloClient for PhoneLookupZaloClient {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        let (_tx, rx) = broadcast::channel(1);
        rx
    }

    fn request_old_messages(
        &self,
        _thread_type: ThreadType,
        _cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn request_old_reactions(
        &self,
        _thread_type: ThreadType,
        _cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>> {
        Ok(Vec::new())
    }

    async fn get_group_snapshot(
        &self,
        _group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>> {
        Ok(None)
    }

    async fn find_user_by_phone(&self, _phone: &str) -> anyhow::Result<Option<NormalizedProfile>> {
        Ok(Some(self.profile.clone()))
    }

    async fn send_text(
        &self,
        thread_id: &str,
        _thread_type: ThreadType,
        _text: &str,
    ) -> anyhow::Result<String> {
        self.sent_thread_ids
            .lock()
            .unwrap()
            .push(thread_id.to_string());
        Ok("provider-phone-1".to_string())
    }

    async fn send_part(
        &self,
        request: crate::conversation::zalo::client::ProviderSendRequest,
    ) -> Result<
        crate::conversation::zalo::client::ProviderSendResult,
        crate::conversation::zalo::client::ProviderSendError,
    > {
        self.sent_thread_ids
            .lock()
            .unwrap()
            .push(request.external_thread_id.clone());
        Ok(crate::conversation::zalo::client::ProviderSendResult {
            provider_message_id: "provider-phone-1".to_string(),
            provider_client_message_id: request.provider_client_message_id,
        })
    }

    fn stop(&self) {}
}

pub struct ServiceHarness {
    pub db_path: String,
    pub repository: Arc<ConversationRepository>,
    pub service: Arc<ConversationService>,
    pub events: Arc<ConversationEventPublisher>,
    _clean_path: std::path::PathBuf,
}

impl Default for ServiceHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceHarness {
    pub fn new() -> Self {
        Self::new_with_webhook(ZaloWebhook::disabled())
    }

    pub fn new_with_webhook(webhook: ZaloWebhook) -> Self {
        let clean_path = std::env::temp_dir().join(format!(
            "rustsale-service-test-{}.db",
            rand::random::<u64>()
        ));
        let db_path = clean_path.to_str().unwrap().to_string();

        let conn = crate::db::init_db(&db_path).unwrap();
        migrate(&conn).unwrap();

        // 1. Enabled Zalo channel account (id=1)
        conn.execute(
            "INSERT INTO channel_accounts
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (1, 'zalo', 'imei-1', 'Sale 01', 1, 'connected', '{}', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
            [],
        )
        .unwrap();

        // 2. Identity (id=1)
        conn.execute(
            "INSERT INTO channel_identities
             (id, lead_id, channel, identity_namespace, external_id, display_name, link_source, created_at, updated_at)
             VALUES (1, NULL, 'zalo', 'zalo_uid', 'user-1', 'Customer 1', 'unlinked', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
            [],
        )
        .unwrap();

        // 3. Direct conversation (id=1) linked to channel_account_id=1
        conn.execute(
            "INSERT INTO channel_conversations
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (1, 1, 'user-1', 'direct', 'Customer 1', 'active', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
            [],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO conversation_participants
             (conversation_id, identity_id, participant_role, joined_at)
             VALUES (1, 1, 'member', '2026-07-31T00:00:00.000Z')",
            [],
        )
        .unwrap();

        let repository = Arc::new(ConversationRepository::new(db_path.clone()));
        let events = Arc::new(ConversationEventPublisher::new(100));
        let spool_root = std::path::Path::new(&db_path)
            .parent()
            .unwrap()
            .join("spool-service");
        let service = Arc::new(
            ConversationService::new(repository.clone(), events.clone(), spool_root)
                .with_zalo_webhook(webhook),
        );

        Self {
            db_path,
            repository,
            service,
            events,
            _clean_path: clean_path,
        }
    }

    pub fn direct_send_request(&self, text: &str) -> SendConversationMessageRequest {
        SendConversationMessageRequest {
            conversation_id: 1,
            channel_account_id: 1,
            text: text.to_string(),
            reply_to_message_id: None,
        }
    }

    pub fn open_conn(&self) -> rusqlite::Connection {
        open_db_connection(&self.db_path).unwrap()
    }

    pub fn seed_zalo_account(
        &self,
        provider_account_key: &str,
        caps: Option<ConversationCapabilities>,
    ) -> i64 {
        let conn = self.open_conn();
        let now = utc_now();
        let caps_json =
            serde_json::to_string(&caps.unwrap_or_else(ConversationCapabilities::zalo_phase_one))
                .unwrap();
        conn.query_row(
            "INSERT INTO channel_accounts
             (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
             VALUES ('zalo', ?1, 'connected', ?2, ?3, ?3)
             RETURNING id",
            rusqlite::params![provider_account_key, caps_json, now],
            |row| row.get(0),
        )
        .unwrap()
    }

    pub fn seed_conversation(&self, account_id: i64, thread_id: &str) -> i64 {
        let conn = self.open_conn();
        let now = utc_now();
        conn.query_row(
            "INSERT INTO channel_conversations
             (channel_account_id, external_thread_id, kind, title, status, last_synced_at, created_at, updated_at)
             VALUES (?1, ?2, 'direct', ?3, 'active', ?4, ?4, ?4)
             RETURNING id",
            rusqlite::params![account_id, thread_id, thread_id, now],
            |row| row.get(0),
        )
        .unwrap()
    }

    pub fn seed_source_message(
        &self,
        conversation_id: i64,
        provider_message_id: Option<&str>,
        provider_client_message_id: Option<&str>,
    ) -> i64 {
        let conn = self.open_conn();
        let now = utc_now();
        conn.query_row(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id, provider_client_message_id,
              local_request_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp,
              ingested_at, sent_at, edited_at, removed_at, status, error_code, error_message, reply_to_message_id,
              send_batch_id, batch_index)
             VALUES (?1, NULL, 'inbound', ?2, ?3, NULL, 'text', 'Hello World', '{\"text\":\"Hello World\"}', NULL, ?4, ?4, NULL, NULL, NULL, 'sent', NULL, NULL, NULL, NULL, 0)
             RETURNING id",
            rusqlite::params![conversation_id, provider_message_id, provider_client_message_id, now],
            |row| row.get(0),
        )
        .unwrap()
    }

    pub fn scalar(&self, sql: &str) -> i64 {
        let conn = open_db_connection(&self.db_path).unwrap();
        conn.query_row(sql, [], |row| row.get(0)).unwrap()
    }

    pub fn message_status(&self, message_id: i64) -> MessageStatus {
        let conn = open_db_connection(&self.db_path).unwrap();
        let status_str: String = conn
            .query_row(
                "SELECT status FROM messages WHERE id = ?1",
                [message_id],
                |row| row.get(0),
            )
            .unwrap();
        MessageStatus::from_str(&status_str).unwrap()
    }

    pub fn provider_message_id(&self, message_id: i64) -> Option<String> {
        let conn = open_db_connection(&self.db_path).unwrap();
        conn.query_row(
            "SELECT provider_message_id FROM messages WHERE id = ?1",
            [message_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    pub fn outbox_status_for_message(&self, message_id: i64) -> String {
        let conn = open_db_connection(&self.db_path).unwrap();
        conn.query_row(
            "SELECT status FROM message_outbox_jobs WHERE message_id = ?1",
            [message_id],
            |row| row.get(0),
        )
        .unwrap()
    }

    pub fn mark_job_running(&self, message_id: i64) {
        let conn = open_db_connection(&self.db_path).unwrap();
        conn.execute(
            "UPDATE message_outbox_jobs SET status = 'running' WHERE message_id = ?1",
            [message_id],
        )
        .unwrap();
    }

    pub fn pending_job_count(&self) -> i64 {
        let conn = open_db_connection(&self.db_path).unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM message_outbox_jobs WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .unwrap()
    }
}

impl Drop for ServiceHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self._clean_path);
    }
}

#[test]
fn enqueue_outbound_message_and_job_are_one_transaction() {
    let harness = ServiceHarness::new();
    let request = harness.direct_send_request("hello");
    let message = harness.service.enqueue_message(&request).unwrap();

    assert_eq!(message.status, MessageStatus::Queued);
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM messages"), 1);
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM message_outbox_jobs"),
        1
    );
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
    assert_eq!(
        harness.provider_message_id(message.id).as_deref(),
        Some("provider-42")
    );
    assert_eq!(harness.outbox_status_for_message(message.id), "completed");
}

#[tokio::test]
async fn worker_resolves_phone_to_zalo_uid_before_sending_and_merges_thread() {
    let harness = ServiceHarness::new();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "INSERT INTO leads
         (id, name, company, status, value, last_activity, owner, email, phone, zalo_uid)
         VALUES (114, 'Thao', 'lemy', 'New', 0, '2026-08-02T00:00:00Z', 'Self', 'info@lemyfinest.com', '0949743369', NULL)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_identities
         (id, lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
         VALUES (801, NULL, 'zalo', 'zalo_uid', 'zalo-uid-thao-live', 'Hồ Ngọc Phương Thảo', 'https://example.com/thao.jpg', 'unlinked', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (id, channel_account_id, external_thread_id, kind, title, avatar_url, status, created_at, updated_at)
         VALUES (6002, 1, 'zalo-uid-thao-live', 'direct', 'Hồ Ngọc Phương Thảo', 'https://example.com/thao.jpg', 'active', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO conversation_participants
         (conversation_id, identity_id, participant_role, joined_at)
         VALUES (6002, 801, 'member', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    drop(conn);

    let request = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 1,
        phone_or_external_id: "0949743369".to_string(),
        display_name: None,
        initial_message: "Xin chào Thảo".to_string(),
    };
    let (message, _) = harness
        .repository
        .start_direct_conversation(&request, "2026-08-02T00:45:54.120Z")
        .unwrap();
    assert_ne!(message.conversation_id, 6002);

    let client = PhoneLookupZaloClient::new(NormalizedProfile {
        external_id: "zalo-uid-thao-live".to_string(),
        display_name: Some("Hồ Ngọc Phương Thảo".to_string()),
        avatar_url: Some("https://example.com/thao.jpg".to_string()),
    });

    let processed = harness
        .service
        .process_next_outbox_job(1, &client)
        .await
        .unwrap();

    assert!(processed);
    assert_eq!(
        client.sent_thread_ids.lock().unwrap().as_slice(),
        ["zalo-uid-thao-live"]
    );
    assert_eq!(harness.message_status(message.id), MessageStatus::Sent);

    let conn = open_db_connection(&harness.db_path).unwrap();
    let conversation_id: i64 = conn
        .query_row(
            "SELECT conversation_id FROM messages WHERE id = ?1",
            [message.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(conversation_id, 6002);
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

#[tokio::test]
async fn publish_outreach_success_emits_event_when_not_duplicate() {
    let harness = ServiceHarness::new();
    let mut rx = harness.events.subscribe();

    let applied = crate::conversation::outreach::AppliedOutreachMessage {
        applied: crate::conversation::repository::AppliedMessage {
            channel_account_id: 1,
            identity_id: Some(1),
            conversation_id: 10,
            message_id: 100,
            duplicate: false,
            replayed_reaction_count: 0,
            delta_event: None,
        },
        lead_id: 42,
    };

    harness.service.publish_outreach_success(&applied);

    let event = rx.recv().await.unwrap();
    assert_eq!(event.conversation_id, applied.applied.conversation_id);
    assert_eq!(event.lead_id, Some(applied.lead_id));
    assert_eq!(event.channel, crate::conversation::types::Channel::Zalo);
    assert_eq!(
        event.reason,
        crate::conversation::events::ConversationInvalidationReason::Message
    );
    assert_eq!(event.message_id, Some(applied.applied.message_id));
}

#[tokio::test]
async fn publish_outreach_success_skips_duplicate() {
    let harness = ServiceHarness::new();
    let mut rx = harness.events.subscribe();

    let applied = crate::conversation::outreach::AppliedOutreachMessage {
        applied: crate::conversation::repository::AppliedMessage {
            channel_account_id: 1,
            identity_id: Some(1),
            conversation_id: 10,
            message_id: 100,
            duplicate: true,
            replayed_reaction_count: 0,
            delta_event: None,
        },
        lead_id: 42,
    };

    harness.service.publish_outreach_success(&applied);

    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn reconcile_required_updates_checkpoint_with_error_and_emits_invalidation_event() {
    let harness = ServiceHarness::new();
    let mut rx = harness.events.subscribe();

    let account_id = harness
        .service
        .repository
        .set_account_status(
            "imei-rec",
            crate::conversation::types::AccountConnectionStatus::Connected,
            None,
        )
        .unwrap();

    let event = ListenerEvent::Disconnected {
        reason: "connection dropped".to_string(),
        code: 1000,
    };

    harness
        .service
        .ingest_zalo_event("imei-rec", Some("Sale 1"), event)
        .unwrap();

    let direct_detail = harness
        .service
        .repository
        .get_checkpoint_detail(account_id, "direct_messages")
        .unwrap()
        .unwrap();
    assert_eq!(
        direct_detail.last_error,
        Some("Reconcile required".to_string())
    );

    let group_detail = harness
        .service
        .repository
        .get_checkpoint_detail(account_id, "group_messages")
        .unwrap()
        .unwrap();
    assert_eq!(
        group_detail.last_error,
        Some("Reconcile required".to_string())
    );

    for scope in ["direct_reactions", "group_reactions"] {
        let detail = harness
            .service
            .repository
            .get_checkpoint_detail(account_id, scope)
            .unwrap()
            .unwrap();
        assert_eq!(detail.last_error, Some("Reconcile required".to_string()));
    }

    let event_received = rx.try_recv().unwrap();
    assert_eq!(
        event_received.channel,
        crate::conversation::types::Channel::Zalo
    );
    assert_eq!(
        event_received.reason,
        crate::conversation::events::ConversationInvalidationReason::Metadata
    );
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn monotonic_checkpoint_prevents_cursor_regression_from_older_or_out_of_order_events() {
    let harness = ServiceHarness::new();
    let account_id = harness
        .service
        .repository
        .set_account_status(
            "imei-mono",
            crate::conversation::types::AccountConnectionStatus::Connected,
            None,
        )
        .unwrap();

    // 1. Ingest newer event (2026-07-31T10:00:00.000Z) with cursor msg-newer
    harness
        .service
        .repository
        .update_checkpoint(
            account_id,
            "direct_messages",
            Some("msg-newer"),
            Some("2026-07-31T10:00:00.000Z"),
            None,
        )
        .unwrap();

    let detail_1 = harness
        .service
        .repository
        .get_checkpoint_detail(account_id, "direct_messages")
        .unwrap()
        .unwrap();
    assert_eq!(detail_1.cursor, Some("msg-newer".to_string()));
    assert_eq!(
        detail_1.last_event_at,
        Some("2026-07-31T10:00:00.000Z".to_string())
    );

    // 2. Ingest older event (2026-07-31T09:00:00.000Z) with cursor msg-older
    harness
        .service
        .repository
        .update_checkpoint(
            account_id,
            "direct_messages",
            Some("msg-older"),
            Some("2026-07-31T09:00:00.000Z"),
            None,
        )
        .unwrap();

    // Checkpoint MUST keep msg-newer and 10:00:00 timestamp
    let detail_2 = harness
        .service
        .repository
        .get_checkpoint_detail(account_id, "direct_messages")
        .unwrap()
        .unwrap();
    assert_eq!(detail_2.cursor, Some("msg-newer".to_string()));
    assert_eq!(
        detail_2.last_event_at,
        Some("2026-07-31T10:00:00.000Z".to_string())
    );
}

#[tokio::test]
async fn reaction_checkpoint_is_isolated_from_message_watermark() {
    let harness = ServiceHarness::new();
    let account_id = harness
        .service
        .repository
        .set_account_status(
            "imei-test",
            crate::conversation::types::AccountConnectionStatus::Connected,
            None,
        )
        .unwrap();
    harness
        .service
        .repository
        .update_checkpoint(
            account_id,
            "direct_messages",
            Some("message-newest"),
            Some("2026-07-31T10:00:00.000Z"),
            None,
        )
        .unwrap();

    harness
        .service
        .ingest_zalo_event(
            "imei-test",
            Some("Sale 1"),
            direct_reaction_event("message-old"),
        )
        .unwrap();

    assert_eq!(
        harness
            .service
            .repository
            .get_checkpoint(account_id, "direct_messages")
            .unwrap(),
        Some("message-newest".to_string())
    );
    assert_eq!(
        harness
            .service
            .repository
            .get_checkpoint(account_id, "direct_reactions")
            .unwrap(),
        Some("reaction-event".to_string())
    );
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        1
    );
}

#[tokio::test]
async fn duplicate_message_replay_emits_reaction_invalidation() {
    let harness = ServiceHarness::new();
    let account_id = harness
        .service
        .repository
        .set_account_status(
            "imei-test",
            crate::conversation::types::AccountConnectionStatus::Connected,
            None,
        )
        .unwrap();
    let conversation_id = harness.seed_conversation(account_id, "peer-uid");
    let message_id = harness.seed_source_message(
        conversation_id,
        Some("message-replay"),
        Some("client-before-replay"),
    );

    harness
        .service
        .ingest_zalo_event(
            "imei-test",
            Some("Sale 1"),
            direct_reaction_event_with_client_id("0", "client-message-replay"),
        )
        .unwrap();
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        1
    );

    let mut receiver = harness.events.subscribe();
    harness
        .service
        .ingest_zalo_event(
            "imei-test",
            Some("Sale 1"),
            direct_message_event("message-replay"),
        )
        .unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_millis(100), receiver.recv())
        .await
        .expect("duplicate replay must emit an invalidation")
        .unwrap();
    assert_eq!(event.conversation_id, conversation_id);
    assert_eq!(event.message_id, Some(message_id));
    assert_eq!(
        event.reason,
        crate::conversation::events::ConversationInvalidationReason::Reaction
    );
    assert_eq!(
        harness.scalar("SELECT COUNT(*) FROM pending_message_reactions"),
        0
    );
    assert_eq!(harness.scalar("SELECT COUNT(*) FROM message_reactions"), 1);
}

#[tokio::test]
async fn message_checkpoint_failure_is_returned_to_the_supervisor() {
    let harness = ServiceHarness::new();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "CREATE TRIGGER fail_message_checkpoint BEFORE INSERT ON channel_sync_checkpoints
         BEGIN SELECT RAISE(ABORT, 'checkpoint unavailable'); END;",
        [],
    )
    .unwrap();

    let error = harness
        .service
        .ingest_zalo_event(
            "imei-test",
            Some("Sale 1"),
            direct_message_event("checkpoint-failure-message"),
        )
        .unwrap_err();

    assert!(error.to_string().contains("checkpoint"));
}

#[tokio::test]
async fn outbox_job_reaching_max_attempts_fails_permanently() {
    let harness = ServiceHarness::new();
    let account_id = harness
        .service
        .repository
        .set_account_status(
            "imei-test",
            crate::conversation::types::AccountConnectionStatus::Connected,
            None,
        )
        .unwrap();

    let conversation_id = {
        let conn = open_db_connection(&harness.db_path).unwrap();
        conn.query_row(
            "INSERT INTO channel_conversations (channel_account_id, external_thread_id, kind, title, status, created_at, updated_at) VALUES (?1, 'peer-1', 'direct', 'Direct', 'active', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z') RETURNING id",
            [account_id],
            |row| row.get::<_, i64>(0),
        )
        .unwrap()
    };

    let req = SendConversationMessageRequest {
        conversation_id,
        channel_account_id: account_id,
        text: "fail test".to_string(),
        reply_to_message_id: None,
    };

    let _msg_id = harness.service.enqueue_message(&req).unwrap();

    let client = FakeZaloClient::failing("transient network error");

    // Simulate 5 claims/attempts by resetting next_attempt_at between loops
    for _ in 0..4 {
        let _ = harness
            .service
            .process_next_outbox_job(account_id, &client)
            .await;
        let conn = open_db_connection(&harness.db_path).unwrap();
        conn.execute(
            "UPDATE message_outbox_jobs SET next_attempt_at = NULL WHERE channel_account_id = ?1",
            [account_id],
        )
        .unwrap();
    }

    // 5th attempt should give up and fail permanently
    let processed = harness
        .service
        .process_next_outbox_job(account_id, &client)
        .await
        .unwrap();
    assert!(processed);

    let conn = open_db_connection(&harness.db_path).unwrap();
    let job_status: String = conn
        .query_row(
            "SELECT status FROM message_outbox_jobs WHERE channel_account_id = ?1",
            [account_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(job_status, "failed");

    let msg_status: String = conn
        .query_row(
            "SELECT status FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(msg_status, "failed");
}

#[tokio::test]
async fn outbound_group_mentions_enqueued_and_sent_with_exact_ranges() {
    use crate::conversation::types::{
        ConversationMentionInput, ConversationSendPartContent, ConversationSendPartRequest,
        SendConversationBatchRequest,
    };
    use crate::conversation::zalo::client::{ProviderMention, ProviderOutboundContent};

    let harness = ServiceHarness::new();
    let conn = open_db_connection(&harness.db_path).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, connection_status, created_at, updated_at)
         VALUES (10, 'zalo', 'acc-10', 'connected', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
         VALUES (500, 10, 'group-500', 'group', 'Dev Team', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (601, 'zalo', 'zalo_uid', 'user-601', 'Nguyễn Văn A', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (500, 601, 'member', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();

    let text = "Xin chào @Nguyễn Văn A"; // UTF-16: "Xin chào " is 9 units, "@Nguyễn Văn A" starts at 9, len 13
    let batch_req = SendConversationBatchRequest {
        conversation_id: 500,
        client_batch_id: "batch-mention-1".into(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "part-0".into(),
            content: ConversationSendPartContent::Text {
                text: text.to_string(),
                mentions: vec![ConversationMentionInput {
                    pos: 9,
                    len: 13,
                    target_identity_id: Some(601),
                    is_all: false,
                }],
            },
        }],
        message_tag: None,
    };

    let res = harness.service.enqueue_batch(&batch_req).unwrap();
    assert_eq!(res.messages.len(), 1);
    let msg_id = res.messages[0].id;

    let mention_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_mentions WHERE message_id = ?1",
            [msg_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(mention_count, 1);

    let (pos, len, ext_id): (i64, i64, Option<String>) = conn
        .query_row(
            "SELECT start_utf16, length_utf16, external_id FROM message_mentions WHERE message_id = ?1",
            [msg_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(pos, 9);
    assert_eq!(len, 13);
    assert_eq!(ext_id.as_deref(), Some("user-601"));

    let client = FakeZaloClient::with_provider_message_id("provider-mention-100");
    let processed = harness
        .service
        .process_next_outbox_job(10, &client)
        .await
        .unwrap();
    assert!(processed);

    let captured = client.captured_part_requests.lock().unwrap();
    assert_eq!(captured.len(), 1);
    if let ProviderOutboundContent::Text { text, mentions } = &captured[0].content {
        assert_eq!(text, "Xin chào @Nguyễn Văn A");
        assert_eq!(mentions.len(), 1);
        assert_eq!(
            mentions[0],
            ProviderMention {
                pos: 9,
                len: 13,
                target_external_id: Some("user-601".into()),
                is_all: false,
            }
        );
    } else {
        panic!("Expected ProviderOutboundContent::Text");
    }
}

#[test]
fn direct_conversation_mentions_rejected_with_typed_error() {
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::types::{
        ConversationMentionInput, ConversationSendPartContent, ConversationSendPartRequest,
        SendConversationBatchRequest,
    };

    let harness = ServiceHarness::new();
    let conn = open_db_connection(&harness.db_path).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
         VALUES (20, 'zalo', 'acc-20', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
         VALUES (600, 20, 'direct-user-1', 'direct', 'Direct Chat', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z')",
        [],
    )
    .unwrap();

    let batch_req = SendConversationBatchRequest {
        conversation_id: 600,
        client_batch_id: "batch-direct-mention".into(),
        reply_to_message_id: None,
        parts: vec![ConversationSendPartRequest {
            client_part_id: "part-0".into(),
            content: ConversationSendPartContent::Text {
                text: "@User hello".to_string(),
                mentions: vec![ConversationMentionInput {
                    pos: 0,
                    len: 5,
                    target_identity_id: Some(999),
                    is_all: false,
                }],
            },
        }],
        message_tag: None,
    };

    let err = harness.service.enqueue_batch(&batch_req).unwrap_err();
    assert_eq!(
        err.code,
        ConversationErrorCode::MentionDirectConversationUnsupported
    );
}

#[tokio::test]
async fn fake_client_captures_outbound_group_mentions() {
    use crate::conversation::zalo::client::{
        ProviderMention, ProviderOutboundContent, ProviderSendRequest,
    };

    let client = FakeZaloClient::with_provider_message_id("msg-captured");
    let mentions = vec![ProviderMention {
        pos: 0,
        len: 4,
        target_external_id: None,
        is_all: true,
    }];

    let req = ProviderSendRequest {
        external_thread_id: "group-999".into(),
        thread_type: zca_rs::models::ThreadType::Group,
        provider_client_message_id: "123456".into(),
        content: ProviderOutboundContent::Text {
            text: "@all announcement".into(),
            mentions: mentions.clone(),
        },
        quote: None,
    };

    client.send_part(req).await.unwrap();

    let captured = client.captured_part_requests.lock().unwrap();
    assert_eq!(captured.len(), 1);
    if let ProviderOutboundContent::Text {
        text,
        mentions: cap_m,
    } = &captured[0].content
    {
        assert_eq!(text, "@all announcement");
        assert_eq!(cap_m, &mentions);
    } else {
        panic!("Expected ProviderOutboundContent::Text");
    }
}

#[tokio::test]
async fn set_reaction_success_local_apply() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-react-1"))
        .unwrap();

    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self' WHERE id = ?1",
        rusqlite::params![applied.channel_account_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-react-1");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: None,
        client_mutation_id: "mut-101-1".to_string(),
    };

    let res = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();

    assert_eq!(res.status, ReactionMutationStatus::Succeeded);
    assert_eq!(res.message_id, applied.message_id);
    assert_eq!(res.reaction, ConversationReactionCode::Heart);
    assert!(res.requested_active);
    assert_eq!(res.client_mutation_id, "mut-101-1");
    assert!(!res.reactions.is_empty());
    assert!(res.reactions.iter().any(|r| r.includes_me));

    let conn = open_db_connection(&harness.db_path).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = ?1",
            rusqlite::params!["mut-101-1"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "succeeded");
}

#[tokio::test]
async fn set_reaction_batches_provider_adds_and_accumulates_locally() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-react-batch"))
        .unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-react-batch");
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: Some(3),
        client_mutation_id: "mut-react-batch".to_string(),
    };

    let result = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap();

    assert_eq!(result.status, ReactionMutationStatus::Succeeded);
    assert_eq!(result.reactions[0].count, 3);
    let requests = client.captured_reaction_requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(requests
        .windows(2)
        .all(|window| window[0].client_mutation_id != window[1].client_mutation_id));
}

#[tokio::test]
async fn set_reaction_partial_failure_preserves_successful_increments() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };
    use crate::conversation::zalo::client::ProviderSendErrorKind;

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-react-partial"))
        .unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();

    let client = FakeZaloClient::failing_reaction_on_call(
        3,
        ProviderSendErrorKind::Permanent,
        "third reaction rejected",
    );
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: Some(3),
        client_mutation_id: "mut-react-partial".to_string(),
    };

    let result = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap();

    assert_eq!(result.status, ReactionMutationStatus::Failed);
    assert_eq!(result.reactions[0].count, 2);
    assert_eq!(client.captured_reaction_requests.lock().unwrap().len(), 3);
    let mutation_status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = 'mut-react-partial'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(mutation_status, "failed");
}

#[tokio::test]
async fn set_reaction_active_false_removes_locally_without_provider_call() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-react-remove"))
        .unwrap();

    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self' WHERE id = ?1",
        rusqlite::params![applied.channel_account_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (?1, 'self', 'heart', '2026-08-02T12:00:00Z', NULL)",
        [applied.message_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-react-remove");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: false,
        count: None,
        client_mutation_id: "mut-remove-1".to_string(),
    };

    let res = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();

    assert_eq!(res.status, ReactionMutationStatus::Succeeded);
    assert!(!res.requested_active);

    let captured_set = client.captured_reaction_requests.lock().unwrap();
    assert_eq!(captured_set.len(), 1);
    assert!(captured_set[0].reaction_code.is_none());
    let remaining: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_reactions
             WHERE message_id = ?1 AND actor_external_id = 'self' AND reaction = 'heart'",
            [applied.message_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn remove_one_local_reaction_preserves_other_emoji() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-selective-remove"))
        .unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self-uid' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (?1, 'self-uid', 'heart', '2026-08-02T12:00:00Z', NULL),
                (?1, 'self-uid', 'haha', '2026-08-02T12:01:00Z', NULL)",
        [applied.message_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-selective-remove");
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: false,
        count: None,
        client_mutation_id: "mut-selective-remove".to_string(),
    };

    let result = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap();

    let captured_set = client.captured_reaction_requests.lock().unwrap();
    assert_eq!(captured_set.len(), 1);
    assert!(captured_set[0].reaction_code.is_none());
    assert_eq!(result.reactions.len(), 1);
    assert_eq!(result.reactions[0].reaction_icon, "😆");
    assert!(result.reactions[0].includes_me);
    let stored: Vec<String> = conn
        .prepare(
            "SELECT reaction FROM message_reactions
             WHERE message_id = ?1 AND actor_external_id = 'self-uid'
             ORDER BY reaction",
        )
        .unwrap()
        .query_map([applied.message_id], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert_eq!(stored, vec!["haha".to_string()]);
}

#[tokio::test]
async fn remove_reaction_requires_provider_message_ids() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message(
            "imei-a",
            "msg-local-remove-no-provider-ids",
        ))
        .unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self-uid' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();
    conn.execute(
        "UPDATE messages
         SET provider_message_id = NULL,
             provider_client_message_id = NULL,
             content_json = '{}'
         WHERE id = ?1",
        [applied.message_id],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (?1, 'self-uid', 'heart', '2026-08-02T12:00:00Z', NULL)",
        [applied.message_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("unused-provider-id");
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: false,
        count: None,
        client_mutation_id: "mut-local-remove-no-provider-ids".to_string(),
    };

    let err = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap_err();

    assert_eq!(
        err.code,
        crate::conversation::error::ConversationErrorCode::ReactionNotSupported
    );
}

#[tokio::test]
async fn successful_add_self_echo_is_suppressed_without_second_invalidation() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-self-echo"))
        .unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self-uid' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();
    let client = FakeZaloClient::with_provider_message_id("msg-self-echo");
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: None,
        client_mutation_id: "mut-self-echo".to_string(),
    };
    let mut receiver = harness.events.subscribe();

    harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap();
    receiver.try_recv().unwrap();
    harness
        .service
        .ingest_zalo_event(
            "imei-a",
            Some("Sale 1"),
            self_reaction_event(
                "msg-self-echo",
                "client-msg-self-echo",
                EventReactions::Heart,
            ),
        )
        .unwrap();

    assert!(receiver.try_recv().is_err());
    let reaction_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_reactions WHERE message_id = ?1",
            [applied.message_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reaction_count, 1);
}

#[tokio::test]
async fn provider_failure_clears_expected_echo_so_later_event_is_applied() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};
    use crate::conversation::zalo::client::ProviderSendErrorKind;

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-failed-echo"))
        .unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET self_external_id = 'self-uid' WHERE id = ?1",
        [applied.channel_account_id],
    )
    .unwrap();
    let client = FakeZaloClient::failing_with_kind(
        ProviderSendErrorKind::Permanent,
        "provider rejected reaction",
    );
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: None,
        client_mutation_id: "mut-failed-echo".to_string(),
    };
    let mut receiver = harness.events.subscribe();

    harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap();
    harness
        .service
        .ingest_zalo_event(
            "imei-a",
            Some("Sale 1"),
            self_reaction_event(
                "msg-failed-echo",
                "client-msg-failed-echo",
                EventReactions::Heart,
            ),
        )
        .unwrap();

    let invalidation = receiver.try_recv().unwrap();
    assert_eq!(invalidation.message_id, Some(applied.message_id));
    let reaction_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_reactions WHERE message_id = ?1",
            [applied.message_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(reaction_count, 1);
}

#[tokio::test]
async fn client_self_identity_is_persisted_for_includes_me_projection() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-client-self-id"))
        .unwrap();
    let client = FakeZaloClient::with_provider_message_id("msg-client-self-id");
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Like,
        active: true,
        count: None,
        client_mutation_id: "mut-client-self-id".to_string(),
    };

    let result = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap();

    assert_eq!(result.reactions.len(), 1);
    assert!(result.reactions[0].includes_me);
    let conn = open_db_connection(&harness.db_path).unwrap();
    let persisted: Option<String> = conn
        .query_row(
            "SELECT self_external_id FROM channel_accounts WHERE id = ?1",
            [applied.channel_account_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(persisted.as_deref(), Some("self-uid"));
}

#[tokio::test]
async fn missing_self_identity_rejects_reaction_before_provider_call() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-missing-self-id"))
        .unwrap();
    let mut client = FakeZaloClient::with_provider_message_id("msg-missing-self-id");
    client.own_external_id = None;
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Like,
        active: true,
        count: None,
        client_mutation_id: "mut-missing-self-id".to_string(),
    };

    let error = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap_err();

    assert_eq!(error.code, ConversationErrorCode::InvalidRequest);
    assert!(client.captured_reaction_requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn set_reaction_missing_ids_fails() {
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let client = FakeZaloClient::with_provider_message_id("msg-missing");
    let req = SetConversationReactionRequest {
        message_id: 999999,
        reaction: ConversationReactionCode::Like,
        active: true,
        count: None,
        client_mutation_id: "mut-missing-ids".to_string(),
    };

    let err = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap_err();

    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[tokio::test]
async fn set_reaction_write_disabled_fails() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::types::{
        ConversationCapabilities, ConversationReactionCode, SetConversationReactionRequest,
    };

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-no-react"))
        .unwrap();

    let mut disabled_caps = ConversationCapabilities::zalo_phase_one();
    disabled_caps.reactions.write = false;
    let caps_json = serde_json::to_string(&disabled_caps).unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET capabilities_json = ?1 WHERE id = ?2",
        rusqlite::params![caps_json, applied.channel_account_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-no-react");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Haha,
        active: true,
        count: None,
        client_mutation_id: "mut-no-react".to_string(),
    };

    let err = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap_err();

    assert_eq!(err.code, ConversationErrorCode::ReactionNotSupported);
    assert_eq!(client.captured_reaction_requests.lock().unwrap().len(), 0);
}

#[tokio::test]
async fn set_reaction_outside_capability_allowlist_fails() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::types::{
        ConversationCapabilities, ConversationReactionCode, SetConversationReactionRequest,
    };

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-reaction-allowlist"))
        .unwrap();

    let mut capabilities = ConversationCapabilities::zalo_phase_one();
    capabilities.reactions.supported = vec![ConversationReactionCode::Heart];
    let capabilities_json = serde_json::to_string(&capabilities).unwrap();
    let conn = open_db_connection(&harness.db_path).unwrap();
    conn.execute(
        "UPDATE channel_accounts SET capabilities_json = ?1 WHERE id = ?2",
        rusqlite::params![capabilities_json, applied.channel_account_id],
    )
    .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-reaction-allowlist");
    let request = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Angry,
        active: true,
        count: None,
        client_mutation_id: "mut-reaction-allowlist".to_string(),
    };

    let error = harness
        .service
        .set_conversation_reaction(&request, &client)
        .await
        .unwrap_err();

    assert_eq!(error.code, ConversationErrorCode::ReactionNotSupported);
    assert_eq!(client.captured_reaction_requests.lock().unwrap().len(), 0);
}

#[tokio::test]
async fn set_reaction_duplicate_client_id_idempotent() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-dup-react"))
        .unwrap();

    let client = FakeZaloClient::with_provider_message_id("msg-dup-react");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Wow,
        active: true,
        count: None,
        client_mutation_id: "mut-dup-100".to_string(),
    };

    let res1 = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();
    assert_eq!(res1.status, ReactionMutationStatus::Succeeded);

    let res2 = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();
    assert_eq!(res2.status, ReactionMutationStatus::Succeeded);

    let captured = client.captured_reaction_requests.lock().unwrap();
    assert_eq!(captured.len(), 1);
}

#[tokio::test]
async fn set_reaction_permanent_failure_recorded() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };
    use crate::conversation::zalo::client::ProviderSendErrorKind;

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-perm-fail"))
        .unwrap();

    let client =
        FakeZaloClient::failing_with_kind(ProviderSendErrorKind::Permanent, "Zalo API error 400");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Angry,
        active: true,
        count: None,
        client_mutation_id: "mut-perm-fail-1".to_string(),
    };

    let res = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();

    assert_eq!(res.status, ReactionMutationStatus::Failed);

    let conn = open_db_connection(&harness.db_path).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = ?1",
            rusqlite::params!["mut-perm-fail-1"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "failed");
}

#[tokio::test]
async fn set_reaction_timeout_delivery_unknown_triggers_poll_no_auto_resend() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationReactionCode, ReactionMutationStatus, SetConversationReactionRequest,
    };
    use crate::conversation::zalo::client::ProviderSendErrorKind;

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-timeout"))
        .unwrap();

    let client =
        FakeZaloClient::failing_with_kind(ProviderSendErrorKind::Retryable, "Network timeout");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Cry,
        active: true,
        count: None,
        client_mutation_id: "mut-timeout-1".to_string(),
    };

    let res = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();

    assert_eq!(res.status, ReactionMutationStatus::DeliveryUnknown);

    let conn = open_db_connection(&harness.db_path).unwrap();
    let status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = ?1",
            rusqlite::params!["mut-timeout-1"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(status, "delivery_unknown");

    {
        let polls = client.captured_old_reaction_polls.lock().unwrap();
        assert_eq!(polls.len(), 1);
    }

    // Call service a second time with the same client_mutation_id
    let res2 = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();

    assert_eq!(res2.status, ReactionMutationStatus::DeliveryUnknown);
    assert_eq!(client.captured_reaction_requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn set_reaction_running_status_duplicate_call_does_not_resend() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::reactions::ReactionMutationRepository;
    use crate::conversation::types::{ConversationReactionCode, SetConversationReactionRequest};

    let harness = ServiceHarness::new();
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-running-test"))
        .unwrap();

    let client = FakeZaloClient::with_provider_message_id("pmsg-running");
    let req = SetConversationReactionRequest {
        message_id: applied.message_id,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: None,
        client_mutation_id: "mut-running-1".to_string(),
    };

    // Pre-insert a mutation in 'running' status
    let repo = ReactionMutationRepository::new(&harness.db_path);
    let begin_res = repo.begin(&req, 1).unwrap();
    assert_eq!(begin_res.status, "running");
    assert!(!begin_res.is_existing);

    // Now call service with the same request
    let res = harness
        .service
        .set_conversation_reaction(&req, &client)
        .await
        .unwrap();

    assert_eq!(
        res.status,
        crate::conversation::types::ReactionMutationStatus::Running
    );
    assert_eq!(res.client_mutation_id, "mut-running-1");
    assert_eq!(res.reaction, ConversationReactionCode::Heart);
    assert!(res.requested_active);
    assert_eq!(client.captured_reaction_requests.lock().unwrap().len(), 0);
}

#[tokio::test]
async fn forward_conversation_message_service_delegates_to_repository() {
    use crate::conversation::commands_test::direct_message;
    use crate::conversation::types::{
        ConversationCapabilities, ForwardConversationMessageRequest, ForwardOperationStatus,
    };
    use crate::db::open_db_connection;

    let harness = ServiceHarness::new();
    let conn = open_db_connection(&harness.db_path).unwrap();
    let caps_json = serde_json::to_string(&ConversationCapabilities::zalo_phase_one()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-a', 'connected', ?1, '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z')",
        [caps_json],
    )
    .unwrap();

    let applied1 = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-svc-fwd-src"))
        .unwrap();

    let applied2 = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "msg-svc-fwd-target"))
        .unwrap();

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-svc-fwd-1".to_string(),
        source_message_id: applied1.message_id,
        target_conversation_ids: vec![applied2.conversation_id],
    };

    let res = harness.service.forward_conversation_message(req).unwrap();
    assert_eq!(res.status, ForwardOperationStatus::Queued);
    assert_eq!(res.targets.len(), 1);
}

#[tokio::test]
async fn forward_worker_calls_one_target_with_stable_client_id() {
    let harness = ServiceHarness::new();
    let account_id = harness.seed_zalo_account("acc-worker-1", None);
    let conv_src = harness.seed_conversation(account_id, "thread-src-w1");
    let conv_t1 = harness.seed_conversation(account_id, "thread-target-w1");
    let src_msg_id =
        harness.seed_source_message(conv_src, Some("src-pmsg-w1"), Some("src-pcli-w1"));
    harness
        .open_conn()
        .execute(
            "UPDATE messages
             SET text_content = ?1, content_json = ?2, provider_timestamp = ?3
             WHERE id = ?4",
            rusqlite::params![
                "Nội dung gốc cần chuyển tiếp",
                serde_json::json!({"text": "Nội dung gốc cần chuyển tiếp"}).to_string(),
                "2026-08-02T08:28:08.951Z",
                src_msg_id,
            ],
        )
        .unwrap();

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-worker-1".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1],
    };

    let fwd_res = harness.service.forward_conversation_message(req).unwrap();
    let fwd_msg_id: i64 = harness
        .open_conn()
        .query_row(
            "SELECT m.id FROM messages m
             JOIN message_send_batches b ON b.id = m.send_batch_id
             WHERE b.client_batch_id = ?1",
            rusqlite::params![&fwd_res.targets[0].client_batch_id],
            |r| r.get(0),
        )
        .unwrap();

    let conn = harness.open_conn();
    let expected_cli_id: String = conn
        .query_row(
            "SELECT provider_client_message_id FROM messages WHERE id = ?1",
            rusqlite::params![fwd_msg_id],
            |r| r.get(0),
        )
        .unwrap();
    let queued_text: String = conn
        .query_row(
            "SELECT text_content FROM messages WHERE id = ?1",
            rusqlite::params![fwd_msg_id],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);

    assert_eq!(queued_text, "Nội dung gốc cần chuyển tiếp");

    let client = FakeZaloClient::with_provider_message_id("pmsg-fwd-res-1");
    let processed = harness
        .service
        .process_next_outbox_job(account_id, &client)
        .await
        .unwrap();

    assert!(processed);

    let captured = client.captured_forward_requests.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(captured[0].external_thread_id, "thread-target-w1");
    assert_eq!(captured[0].source_provider_message_id, "src-pmsg-w1");
    assert_eq!(captured[0].source_message, "Nội dung gốc cần chuyển tiếp");
    assert_eq!(
        captured[0].source_timestamp_ms,
        chrono::DateTime::parse_from_rfc3339("2026-08-02T08:28:08.951Z")
            .unwrap()
            .timestamp_millis() as u64
    );
    assert_eq!(captured[0].provider_client_message_id, expected_cli_id);
}

#[tokio::test]
async fn forward_success_persists_provider_ids() {
    let harness = ServiceHarness::new();
    let account_id = harness.seed_zalo_account("acc-worker-2", None);
    let conv_src = harness.seed_conversation(account_id, "thread-src-w2");
    let conv_t1 = harness.seed_conversation(account_id, "thread-target-w2");
    let src_msg_id =
        harness.seed_source_message(conv_src, Some("src-pmsg-w2"), Some("src-pcli-w2"));

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-worker-2".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1],
    };

    let fwd_res = harness.service.forward_conversation_message(req).unwrap();
    let fwd_msg_id: i64 = harness
        .open_conn()
        .query_row(
            "SELECT m.id FROM messages m
             JOIN message_send_batches b ON b.id = m.send_batch_id
             WHERE b.client_batch_id = ?1",
            rusqlite::params![&fwd_res.targets[0].client_batch_id],
            |r| r.get(0),
        )
        .unwrap();

    let client = FakeZaloClient::with_provider_message_id("pmsg-persisted-99");
    harness
        .service
        .process_next_outbox_job(account_id, &client)
        .await
        .unwrap();

    let conn = harness.open_conn();
    let (msg_status, provider_msg_id): (String, String) = conn
        .query_row(
            "SELECT status, provider_message_id FROM messages WHERE id = ?1",
            rusqlite::params![fwd_msg_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(msg_status, "sent");
    assert_eq!(provider_msg_id, "pmsg-persisted-99");

    let (target_status, op_status): (String, String) = conn
        .query_row(
            "SELECT t.status, o.status FROM message_forward_targets t
             JOIN message_forward_operations o ON o.client_operation_id = t.client_operation_id
             WHERE t.client_operation_id = 'op-worker-2' AND t.target_conversation_id = ?1",
            rusqlite::params![conv_t1],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(target_status, "sent");
    assert_eq!(op_status, "sent");
}

#[tokio::test]
async fn forward_echo_updates_queued_target_without_duplicate() {
    let harness = ServiceHarness::new();
    let account_id = harness.seed_zalo_account("acc-worker-3", None);
    let conv_src = harness.seed_conversation(account_id, "thread-src-w3");
    let conv_t1 = harness.seed_conversation(account_id, "thread-target-w3");
    let src_msg_id =
        harness.seed_source_message(conv_src, Some("src-pmsg-w3"), Some("src-pcli-w3"));

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-worker-3".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1],
    };

    let fwd_res = harness.service.forward_conversation_message(req).unwrap();
    let fwd_msg_id: i64 = harness
        .open_conn()
        .query_row(
            "SELECT m.id FROM messages m
             JOIN message_send_batches b ON b.id = m.send_batch_id
             WHERE b.client_batch_id = ?1",
            rusqlite::params![&fwd_res.targets[0].client_batch_id],
            |r| r.get(0),
        )
        .unwrap();

    let conn = harness.open_conn();
    let cli_msg_id: String = conn
        .query_row(
            "SELECT provider_client_message_id FROM messages WHERE id = ?1",
            rusqlite::params![fwd_msg_id],
            |r| r.get(0),
        )
        .unwrap();
    drop(conn);

    let now = utc_now();
    let echo = NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: "acc-worker-3".to_string(),
        account_display_name: None,
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: "thread-target-w3".to_string(),
            display_name: None,
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: false,
        external_thread_id: "thread-target-w3".to_string(),
        conversation_kind: crate::conversation::types::ConversationKind::Direct,
        conversation_title: None,
        direction: MessageDirection::Outbound,
        provider_message_id: Some("echo-pmsg-3".to_string()),
        provider_client_message_id: Some(cli_msg_id),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("Đã chuyển tiếp một tin nhắn".to_string()),
        content: serde_json::json!({}),
        attachments: vec![],
        reply: None,
        mentions: vec![],
        raw_payload: None,
        provider_timestamp: Some(now.clone()),
        ingested_at: now,
        status: MessageStatus::Sent,
    };

    let applied = harness.repository.apply_normalized_message(&echo).unwrap();
    assert!(applied.duplicate);
    assert_eq!(applied.message_id, fwd_msg_id);

    let conn = harness.open_conn();
    let (target_status, op_status): (String, String) = conn
        .query_row(
            "SELECT t.status, o.status FROM message_forward_targets t
             JOIN message_forward_operations o ON o.client_operation_id = t.client_operation_id
             WHERE t.client_operation_id = 'op-worker-3' AND t.target_conversation_id = ?1",
            rusqlite::params![conv_t1],
            |r: &rusqlite::Row<'_>| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(target_status, "sent");
    assert_eq!(op_status, "sent");

    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE conversation_id = ?1",
            rusqlite::params![conv_t1],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn successful_target_is_not_requeued_when_sibling_fails() {
    let harness = ServiceHarness::new();
    let account_id = harness.seed_zalo_account("acc-worker-4", None);
    let conv_src = harness.seed_conversation(account_id, "thread-src-w4");
    let conv_t1 = harness.seed_conversation(account_id, "thread-target-w4-1");
    let conv_t2 = harness.seed_conversation(account_id, "thread-target-w4-2");
    let src_msg_id =
        harness.seed_source_message(conv_src, Some("src-pmsg-w4"), Some("src-pcli-w4"));

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-worker-4".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1, conv_t2],
    };

    let _ = harness.service.forward_conversation_message(req).unwrap();

    let client_success = FakeZaloClient::with_provider_message_id("pmsg-t1-ok");
    let p1 = harness
        .service
        .process_next_outbox_job(account_id, &client_success)
        .await
        .unwrap();
    assert!(p1);

    let client_fail = FakeZaloClient::failing_with_kind(
        crate::conversation::zalo::client::ProviderSendErrorKind::Permanent,
        "Failed sibling",
    );
    let p2 = harness
        .service
        .process_next_outbox_job(account_id, &client_fail)
        .await
        .unwrap();
    assert!(p2);

    let conn = harness.open_conn();
    let t1_status: String = conn
        .query_row(
            "SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-4' AND target_conversation_id = ?1",
            rusqlite::params![conv_t1],
            |r| r.get(0),
        )
        .unwrap();
    let t2_status: String = conn
        .query_row(
            "SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-4' AND target_conversation_id = ?1",
            rusqlite::params![conv_t2],
            |r| r.get(0),
        )
        .unwrap();
    let op_status: String = conn
        .query_row(
            "SELECT status FROM message_forward_operations WHERE client_operation_id = 'op-worker-4'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(t1_status, "sent");
    assert_eq!(t2_status, "failed");
    assert_eq!(op_status, "partial");

    drop(conn);

    let retry_res = harness
        .service
        .retry_forward_target("op-worker-4", conv_t2)
        .unwrap();
    assert_eq!(retry_res.status, ForwardTargetStatus::Queued);

    let conn = harness.open_conn();
    let t1_status_after: String = conn
        .query_row(
            "SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-4' AND target_conversation_id = ?1",
            rusqlite::params![conv_t1],
            |r| r.get(0),
        )
        .unwrap();
    let t2_status_after: String = conn
        .query_row(
            "SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-4' AND target_conversation_id = ?1",
            rusqlite::params![conv_t2],
            |r| r.get(0),
        )
        .unwrap();

    assert_eq!(t1_status_after, "sent");
    assert_eq!(t2_status_after, "queued");
}

#[tokio::test]
async fn forward_running_after_restart_becomes_delivery_unknown() {
    let harness = ServiceHarness::new();
    let account_id = harness.seed_zalo_account("acc-worker-5", None);
    let conv_src = harness.seed_conversation(account_id, "thread-src-w5");
    let conv_t1 = harness.seed_conversation(account_id, "thread-target-w5");
    let src_msg_id =
        harness.seed_source_message(conv_src, Some("src-pmsg-w5"), Some("src-pcli-w5"));

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-worker-5".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1],
    };

    let fwd_res = harness.service.forward_conversation_message(req).unwrap();
    let fwd_msg_id: i64 = harness
        .open_conn()
        .query_row(
            "SELECT m.id FROM messages m
             JOIN message_send_batches b ON b.id = m.send_batch_id
             WHERE b.client_batch_id = ?1",
            rusqlite::params![&fwd_res.targets[0].client_batch_id],
            |r| r.get(0),
        )
        .unwrap();

    let conn = harness.open_conn();
    conn.execute(
        "UPDATE message_outbox_jobs SET status = 'running' WHERE message_id = ?1",
        rusqlite::params![fwd_msg_id],
    )
    .unwrap();
    conn.execute(
        "UPDATE message_forward_targets SET status = 'sending' WHERE client_operation_id = 'op-worker-5'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE message_forward_operations SET status = 'sending' WHERE client_operation_id = 'op-worker-5'",
        [],
    )
    .unwrap();
    drop(conn);

    let recovered = harness.service.recover_outbox_startup().unwrap();
    assert_eq!(recovered, 1);

    let conn = harness.open_conn();
    let (t_status, op_status): (String, String) = conn
        .query_row(
            "SELECT t.status, o.status FROM message_forward_targets t
             JOIN message_forward_operations o ON o.client_operation_id = t.client_operation_id
             WHERE t.client_operation_id = 'op-worker-5'",
            [],
            |r: &rusqlite::Row<'_>| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(t_status, "delivery_unknown");
    assert_eq!(op_status, "delivery_unknown");
}

#[test]
fn forward_operation_status_rules_cover_mixed_states() {
    use crate::conversation::forwarding::compute_aggregate_operation_status;

    assert_eq!(
        compute_aggregate_operation_status(&["queued", "queued"]),
        "queued"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["sending", "queued"]),
        "sending"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["sending", "sending"]),
        "sending"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["sent", "sent"]),
        "sent"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["failed", "failed"]),
        "failed"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["delivery_unknown", "delivery_unknown"]),
        "delivery_unknown"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["sent", "failed"]),
        "partial"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["sent", "queued"]),
        "partial"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["sent", "delivery_unknown"]),
        "partial"
    );
    assert_eq!(
        compute_aggregate_operation_status(&["failed", "delivery_unknown"]),
        "partial"
    );
}

#[tokio::test]
async fn manual_retry_requeues_only_failed_target() {
    let harness = ServiceHarness::new();
    let account_id = harness.seed_zalo_account("acc-worker-7", None);
    let conv_src = harness.seed_conversation(account_id, "thread-src-w7");
    let conv_t1 = harness.seed_conversation(account_id, "thread-target-w7-1");
    let conv_t2 = harness.seed_conversation(account_id, "thread-target-w7-2");
    let conv_t3 = harness.seed_conversation(account_id, "thread-target-w7-3");
    let src_msg_id =
        harness.seed_source_message(conv_src, Some("src-pmsg-w7"), Some("src-pcli-w7"));

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-worker-7".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1, conv_t2, conv_t3],
    };

    let _ = harness.service.forward_conversation_message(req).unwrap();

    let conn = harness.open_conn();
    conn.execute(
        "UPDATE message_forward_targets SET status = 'sent' WHERE client_operation_id = 'op-worker-7' AND target_conversation_id = ?1",
        rusqlite::params![conv_t1],
    )
    .unwrap();
    conn.execute(
        "UPDATE message_forward_targets SET status = 'failed' WHERE client_operation_id = 'op-worker-7' AND target_conversation_id = ?1",
        rusqlite::params![conv_t2],
    )
    .unwrap();
    conn.execute(
        "UPDATE message_forward_targets SET status = 'delivery_unknown' WHERE client_operation_id = 'op-worker-7' AND target_conversation_id = ?1",
        rusqlite::params![conv_t3],
    )
    .unwrap();
    drop(conn);

    let res_t1 = harness.service.retry_forward_target("op-worker-7", conv_t1);
    assert!(res_t1.is_err());

    let res_t3 = harness.service.retry_forward_target("op-worker-7", conv_t3);
    assert!(res_t3.is_err());

    let res_t2 = harness
        .service
        .retry_forward_target("op-worker-7", conv_t2)
        .unwrap();
    assert_eq!(res_t2.status, ForwardTargetStatus::Queued);

    let conn = harness.open_conn();
    let (s1, s2, s3): (String, String, String) = conn
        .query_row(
            "SELECT
             (SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-7' AND target_conversation_id = ?1),
             (SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-7' AND target_conversation_id = ?2),
             (SELECT status FROM message_forward_targets WHERE client_operation_id = 'op-worker-7' AND target_conversation_id = ?3)",
            rusqlite::params![conv_t1, conv_t2, conv_t3],
            |r: &rusqlite::Row<'_>| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_eq!(s1, "sent");
    assert_eq!(s2, "queued");
    assert_eq!(s3, "delivery_unknown");
}

mod group_member_import_events {
    use super::*;
    use crate::conversation::repository::{
        GroupMemberLeadImportItem, GroupMemberLeadImportRequest,
    };

    struct ImportServiceHarness {
        path: std::path::PathBuf,
        service: ConversationService,
        events: Arc<ConversationEventPublisher>,
    }

    impl ImportServiceHarness {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "rustsale-group-import-service-{}",
                rand::random::<u64>()
            ));
            std::fs::create_dir_all(&root).unwrap();
            let path = root.join("app.db");
            let conn = crate::db::init_db(path.to_str().unwrap()).unwrap();
            let now = "2026-08-10T00:00:00Z";
            conn.execute(
                "INSERT INTO channel_accounts
                 (id, channel, provider_account_key, display_name, created_at, updated_at)
                 VALUES (1, 'zalo', 'account-a', 'Account A', ?1, ?1)",
                [now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO channel_conversations
                 (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
                 VALUES (100, 1, 'group-a', 'group', 'Group A', ?1, ?1)",
                [now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO channel_identities
                 (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
                 VALUES (1001, 'zalo', 'zalo_uid', 'uid-a', 'Member A', ?1, ?1)",
                [now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (100, 1001, 'member', ?1)",
                [now],
            )
            .unwrap();
            drop(conn);

            let repository = Arc::new(ConversationRepository::new(
                path.to_string_lossy().to_string(),
            ));
            let events = Arc::new(ConversationEventPublisher::new(8));
            let service = ConversationService::new(repository, events.clone(), root.join("spool"));
            Self {
                path,
                service,
                events,
            }
        }

        fn request() -> GroupMemberLeadImportRequest {
            GroupMemberLeadImportRequest {
                conversation_id: 100,
                members: vec![GroupMemberLeadImportItem {
                    identity_id: Some(1001),
                    external_id: Some("uid-a".to_string()),
                    phone: None,
                }],
                category_ids: Vec::new(),
                new_category_name: None,
                skip_existing: false,
            }
        }
    }

    impl Drop for ImportServiceHarness {
        fn drop(&mut self) {
            if let Some(root) = self.path.parent() {
                let _ = std::fs::remove_dir_all(root);
            }
        }
    }

    #[test]
    fn publishes_one_link_invalidation_after_successful_commit() {
        let harness = ImportServiceHarness::new();
        let mut receiver = harness.events.subscribe();

        harness
            .service
            .import_group_members_to_leads(&ImportServiceHarness::request())
            .unwrap();

        let event = receiver.try_recv().unwrap();
        assert_eq!(event.conversation_id, 100);
        assert_eq!(event.lead_id, None);
        assert_eq!(
            event.reason,
            crate::conversation::events::ConversationInvalidationReason::Link
        );
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn rollback_emits_no_invalidation() {
        let harness = ImportServiceHarness::new();
        let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_group_import_lead
             BEFORE INSERT ON leads
             BEGIN
                 SELECT RAISE(ABORT, 'blocked lead insert');
             END;",
        )
        .unwrap();
        drop(conn);
        let mut receiver = harness.events.subscribe();

        let result = harness
            .service
            .import_group_members_to_leads(&ImportServiceHarness::request());

        assert!(result.is_err());
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn test_process_next_messenger_outbox_job_resolves_correct_customer_psid() {
        use crate::conversation::facebook::client::FacebookGraphClient;
        use crate::conversation::facebook::client::SendFacebookMessageResponse;

        struct MockGraphClient {
            sent_recipient: std::sync::Mutex<Option<String>>,
            sent_message_tag: std::sync::Mutex<Option<Option<String>>>,
        }

        #[async_trait::async_trait]
        impl FacebookGraphClient for MockGraphClient {
            async fn fetch_page_conversations(
                &self,
                _page_id: &str,
                _page_token: &str,
                _limit: usize,
                _after: Option<&str>,
            ) -> anyhow::Result<crate::conversation::facebook::client::FacebookConversationsPage>
            {
                unimplemented!()
            }

            async fn fetch_thread_messages(
                &self,
                _thread_id: &str,
                _page_token: &str,
                _limit: usize,
                _after: Option<&str>,
            ) -> anyhow::Result<crate::conversation::facebook::client::FacebookMessagesPage>
            {
                unimplemented!()
            }

            async fn send_messenger_message(
                &self,
                _page_id: &str,
                _page_token: &str,
                recipient_psid: &str,
                _text: &str,
                message_tag: Option<&str>,
            ) -> anyhow::Result<SendFacebookMessageResponse> {
                *self.sent_recipient.lock().unwrap() = Some(recipient_psid.to_string());
                *self.sent_message_tag.lock().unwrap() = Some(message_tag.map(|s| s.to_string()));
                Ok(SendFacebookMessageResponse {
                    recipient_id: recipient_psid.to_string(),
                    message_id: format!("m_test_{}", rand::random::<u64>()),
                })
            }
        }

        let harness = ServiceHarness::new();
        let conn = rusqlite::Connection::open(harness.repository.db_path()).unwrap();

        // 1. Create channel account for Page
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, enabled, created_at, updated_at)
             VALUES (999, 'messenger', 'PAGE_123', 'My Page', 'connected', 1, datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // 2. Create customer identity (PSID) and page identity
        conn.execute(
            "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (101, 'messenger', 999, 'psid', 'CUSTOMER_PSID_777', 'Customer Name', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (102, 'messenger', 999, 'psid', 'PAGE_123', 'My Page', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // 3. Create conversation and link participants
        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (500, 999, 't_1383466303165641', 'direct', 'Customer', 'active', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
             VALUES (500, 102, 'member', '2026-08-24 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
             VALUES (500, 101, 'member', '2026-08-24 10:01:00')",
            [],
        ).unwrap();

        // 4. Create an outbound message (no tag) and outbox job
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (888, 500, 'outbound', 'client_msg_1', 'text', 'Hello customer', '{}', 'queued', datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_888', 999, 888, 'pending', 0, datetime('now'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        let mock_client = MockGraphClient {
            sent_recipient: std::sync::Mutex::new(None),
            sent_message_tag: std::sync::Mutex::new(None),
        };

        let dispatched = harness
            .service
            .process_next_messenger_outbox_job(999, &mock_client, "PAGE_123", "TOKEN_XYZ")
            .await
            .unwrap();

        assert!(dispatched);
        let sent_psid = mock_client.sent_recipient.lock().unwrap().clone();
        assert_eq!(sent_psid, Some("CUSTOMER_PSID_777".to_string()));
        let sent_tag = mock_client.sent_message_tag.lock().unwrap().clone();
        assert_eq!(
            sent_tag,
            Some(None),
            "Standard message within 24h must NOT send any message tag"
        );

        // 5. Test message with HUMAN_AGENT tag
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (889, 500, 'outbound', 'client_msg_2', 'text', 'Hello after 24h', '{\"messageTag\":\"HUMAN_AGENT\"}', 'queued', datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_889', 999, 889, 'pending', 0, datetime('now'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        let mock_client_tag = MockGraphClient {
            sent_recipient: std::sync::Mutex::new(None),
            sent_message_tag: std::sync::Mutex::new(None),
        };

        let dispatched_tag = harness
            .service
            .process_next_messenger_outbox_job(999, &mock_client_tag, "PAGE_123", "TOKEN_XYZ")
            .await
            .unwrap();

        assert!(dispatched_tag);
        let sent_tag_2 = mock_client_tag.sent_message_tag.lock().unwrap().clone();
        assert_eq!(
            sent_tag_2,
            Some(Some("HUMAN_AGENT".to_string())),
            "Explicit HUMAN_AGENT tag message must pass HUMAN_AGENT tag"
        );
    }

    #[tokio::test]
    async fn test_process_next_messenger_outbox_job_circuit_breaker_on_auth_failure() {
        use crate::conversation::facebook::client::{
            FacebookConversationsPage, FacebookGraphClient, FacebookMessagesPage,
            SendFacebookMessageResponse,
        };
        use crate::conversation::facebook::error::{MetaErrorKind, MetaGraphError};
        use crate::facebook::vault::FacebookTokenVault;
        use rusqlite::Connection;

        let harness = ServiceHarness::new();
        let conn = Connection::open(harness.service.repository.db_path()).unwrap();

        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
             VALUES (999, 'messenger', 'PAGE_123', 'My Page', 'connected', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        FacebookTokenVault::encrypt_and_upsert_page_token(
            &conn,
            999,
            "PAGE_123",
            "TOKEN_EXPIRED",
            &["pages_messaging".to_string()],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (101, 'messenger', 999, 'psid', 'CUSTOMER_PSID_777', 'Customer Name', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (102, 'messenger', 999, 'psid', 'PAGE_123', 'My Page', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (500, 999, 't_12345', 'direct', 'Customer Chat', 'active', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
             VALUES (500, 102, 'member', '2026-08-24 10:00:00')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
             VALUES (500, 101, 'member', '2026-08-24 10:01:00')",
            [],
        ).unwrap();

        // Queue Job 1
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (888, 500, 'outbound', 'client_msg_1', 'text', 'Job 1', '{}', 'queued', datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO message_outbox_jobs (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_888', 999, 888, 'pending', 0, datetime('now'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        // Queue Job 2
        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (889, 500, 'outbound', 'client_msg_2', 'text', 'Job 2', '{}', 'queued', datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO message_outbox_jobs (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_889', 999, 889, 'pending', 0, datetime('now'), datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        struct FailingAuthClient;
        #[async_trait::async_trait]
        impl FacebookGraphClient for FailingAuthClient {
            async fn fetch_page_conversations(
                &self,
                _page_id: &str,
                _page_token: &str,
                _limit: usize,
                _after: Option<&str>,
            ) -> anyhow::Result<FacebookConversationsPage> {
                unimplemented!()
            }
            async fn fetch_thread_messages(
                &self,
                _thread_id: &str,
                _page_token: &str,
                _limit: usize,
                _after: Option<&str>,
            ) -> anyhow::Result<FacebookMessagesPage> {
                unimplemented!()
            }
            async fn send_messenger_message(
                &self,
                _page_id: &str,
                _page_token: &str,
                _recipient_psid: &str,
                _text: &str,
                _message_tag: Option<&str>,
            ) -> anyhow::Result<SendFacebookMessageResponse> {
                Err(anyhow::anyhow!(MetaGraphError {
                    http_status: 400,
                    code: Some(190),
                    subcode: Some(463),
                    message: "Error validating access token: Session has expired".to_string(),
                    error_type: Some("OAuthException".to_string()),
                    fbtrace_id: None,
                    is_transient: false,
                    kind: MetaErrorKind::AuthFailure,
                }))
            }
        }

        let failing_client = FailingAuthClient;

        // Run outbox drain
        let continues = harness
            .service
            .process_next_messenger_outbox_job(999, &failing_client, "PAGE_123", "TOKEN_EXPIRED")
            .await
            .unwrap();

        // Circuit breaker: must return false on auth failure
        assert!(
            !continues,
            "process_next_messenger_outbox_job must return false on auth error to halt queue drain"
        );

        // Verify Job 1 failed with provider_auth
        let (job1_status, msg1_err_code): (String, String) = conn.query_row(
            "SELECT oj.status, m.error_code FROM message_outbox_jobs oj JOIN messages m ON m.id = oj.message_id WHERE oj.id = 'job_888'",
            [],
            |r: &rusqlite::Row| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(job1_status, "failed");
        assert_eq!(msg1_err_code, "provider_auth");

        // Verify Job 2 remained pending (not drained into failure)
        let job2_status: String = conn
            .query_row(
                "SELECT status FROM message_outbox_jobs WHERE id = 'job_889'",
                [],
                |r: &rusqlite::Row| r.get(0),
            )
            .unwrap();
        assert_eq!(
            job2_status, "pending",
            "Second job must stay pending due to circuit breaker"
        );

        // Verify channel_accounts was marked invalid
        let connection_status: String = conn
            .query_row(
                "SELECT connection_status FROM channel_accounts WHERE id = 999",
                [],
                |r: &rusqlite::Row| r.get(0),
            )
            .unwrap();
        assert_eq!(connection_status, "invalid");

        // Verify token_status was marked expired
        let token_status: String = conn
            .query_row(
                "SELECT token_status FROM facebook_page_tokens WHERE channel_account_id = 999",
                [],
                |r: &rusqlite::Row| r.get(0),
            )
            .unwrap();
        assert_eq!(token_status, "expired");
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_success() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        // 1. Create WhatsApp account
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (701, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO whatsapp_sessions 
             (channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at)
             VALUES (701, '84901234567', '84901234567', 'session_701.db', 1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        // 2. Create conversation
        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (702, 701, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        // 3. Create outbound message & outbox job
        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (703, 702, 'outbound', 'wa_client_msg_1', 'text', 'Hello from WhatsApp outbox test', '{}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_703', 701, 703, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();

        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(701, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        // Verify outbox job is completed
        let job_status: String = conn
            .query_row(
                "SELECT status FROM message_outbox_jobs WHERE id = 'job_wa_703'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(job_status, "completed");

        // Verify message status is sent
        let msg_status: String = conn
            .query_row("SELECT status FROM messages WHERE id = 703", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(msg_status, "sent");

        // Verify client dispatched message
        let sent = mock_client.get_sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].recipient_jid, "84909999999@s.whatsapp.net");
        assert_eq!(sent[0].text, "Hello from WhatsApp outbox test");
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_with_quote_reply() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (710, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (711, 710, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        // Previous message from customer
        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (712, 711, 'inbound', 'parent_stanza_123', 'text', 'Original question?', '{}', 'delivered', ?1)",
            rusqlite::params![now],
        ).unwrap();

        // Reply outbound message quoting parent (id=712)
        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, reply_to_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (713, 711, 'outbound', 'wa_reply_msg', 712, 'text', 'Quoted answer!', '{}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_713', 710, 713, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(710, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        let sent = mock_client.get_sent_messages();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].text, "Quoted answer!");
        let quote = sent[0].quote.as_ref().expect("Quote must be populated");
        assert_eq!(quote.stanza_id, "parent_stanza_123");
        assert_eq!(quote.text.as_deref(), Some("Original question?"));
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_media() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (720, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (721, 720, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (722, 721, 'outbound', 'wa_media_msg', 'image', 'Check this photo', '{\"url\":\"https://example.com/photo.jpg\"}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_722', 720, 722, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(720, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        let sent_media = mock_client.get_sent_media();
        assert_eq!(sent_media.len(), 1);
        assert_eq!(sent_media[0].media_type, "image");
        assert_eq!(
            sent_media[0].media_data_or_url,
            "https://example.com/photo.jpg"
        );
        assert_eq!(sent_media[0].caption.as_deref(), Some("Check this photo"));
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_attachment() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (725, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (726, 725, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (727, 726, 'outbound', 'wa_att_msg', 'attachment', 'Sent an image attachment', '{\"caption\":\"Sent an image attachment\"}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_attachments
             (id, message_id, attachment_type, file_name, mime_type, size_bytes, local_path, created_at)
             VALUES (728, 727, 'image', 'clipboard.png', 'image/png', 54000, '/tmp/test_clipboard.png', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_727', 725, 727, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(725, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        let sent_media = mock_client.get_sent_media();
        assert_eq!(sent_media.len(), 1);
        assert_eq!(sent_media[0].media_type, "image");
        assert_eq!(sent_media[0].media_data_or_url, "/tmp/test_clipboard.png");
        assert_eq!(
            sent_media[0].caption.as_deref(),
            Some("Sent an image attachment")
        );

        let (remote_url, metadata_json): (Option<String>, String) = conn
            .query_row(
                "SELECT remote_url, metadata_json FROM message_attachments WHERE id = 728",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            remote_url.as_deref(),
            Some("https://mmg.whatsapp.net/mock/direct_path")
        );
        assert!(metadata_json.contains("/mock/direct_path"));
        assert!(metadata_json.contains("bW9ja19tZWRpYV9rZXlfMzJieXRlc19mb3JfdGVzdA=="));
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_docx_attachment() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let temp_dir = std::env::temp_dir();
        let docx_file = temp_dir.join(format!("test_wa_{}.docx", rand::random::<u64>()));
        std::fs::write(&docx_file, b"PK\x03\x04test_docx_payload").unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (825, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (826, 825, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (827, 826, 'outbound', 'wa_docx_msg', 'attachment', 'Attached report', '{\"caption\":\"Attached report\"}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_attachments
             (id, message_id, attachment_type, file_name, mime_type, size_bytes, local_path, created_at)
             VALUES (828, 827, 'file', 'quarterly_report.docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', 2048, ?1, ?2)",
            rusqlite::params![docx_file.to_string_lossy().to_string(), now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_827', 825, 827, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(825, &mock_client)
            .await
            .unwrap();

        let _ = std::fs::remove_file(&docx_file);

        assert!(processed);

        let sent_media = mock_client.get_sent_media();
        assert_eq!(sent_media.len(), 1);
        assert_eq!(sent_media[0].media_type, "file");
        assert_eq!(
            sent_media[0].file_name.as_deref(),
            Some("quarterly_report.docx")
        );
        assert_eq!(
            sent_media[0].mime_type.as_deref(),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
    }

    #[tokio::test]
    async fn test_process_messenger_outbox_job_docx_attachment() {
        use crate::conversation::facebook::client::{
            FacebookGraphClient, SendFacebookMessageResponse,
        };

        struct MockMessengerAttachmentClient {
            sent_file_name: std::sync::Mutex<Option<String>>,
            sent_mime_type: std::sync::Mutex<Option<String>>,
            sent_media_type: std::sync::Mutex<Option<String>>,
            sent_bytes_len: std::sync::Mutex<Option<usize>>,
        }

        #[async_trait::async_trait]
        impl FacebookGraphClient for MockMessengerAttachmentClient {
            async fn fetch_page_conversations(
                &self,
                _: &str,
                _: &str,
                _: usize,
                _: Option<&str>,
            ) -> anyhow::Result<crate::conversation::facebook::client::FacebookConversationsPage>
            {
                unimplemented!()
            }
            async fn fetch_thread_messages(
                &self,
                _: &str,
                _: &str,
                _: usize,
                _: Option<&str>,
            ) -> anyhow::Result<crate::conversation::facebook::client::FacebookMessagesPage>
            {
                unimplemented!()
            }
            async fn send_messenger_message(
                &self,
                _: &str,
                _: &str,
                _: &str,
                _: &str,
                _: Option<&str>,
            ) -> anyhow::Result<SendFacebookMessageResponse> {
                unimplemented!()
            }

            async fn send_messenger_attachment(
                &self,
                _page_id: &str,
                _page_token: &str,
                _recipient_psid: &str,
                file_bytes: Vec<u8>,
                file_name: Option<&str>,
                mime_type: Option<&str>,
                media_type: &str,
                _message_tag: Option<&str>,
            ) -> anyhow::Result<SendFacebookMessageResponse> {
                *self.sent_file_name.lock().unwrap() = file_name.map(str::to_string);
                *self.sent_mime_type.lock().unwrap() = mime_type.map(str::to_string);
                *self.sent_media_type.lock().unwrap() = Some(media_type.to_string());
                *self.sent_bytes_len.lock().unwrap() = Some(file_bytes.len());
                Ok(SendFacebookMessageResponse {
                    recipient_id: "PSID_123".to_string(),
                    message_id: "mid_att_123".to_string(),
                })
            }
        }

        let harness = ServiceHarness::new();
        let conn = harness.open_conn();

        let temp_dir = std::env::temp_dir();
        let docx_file = temp_dir.join(format!("test_fb_{}.docx", rand::random::<u64>()));
        std::fs::write(&docx_file, b"PK\x03\x04fb_docx_payload").unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
             VALUES (950, 'messenger', 'PAGE_950', 'FB Page', 'connected', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (951, 950, 't_PSID_999', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
             VALUES (952, 'messenger', 950, 'psid', 'PSID_999', 'Customer', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
             VALUES (951, 952, 'member', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (954, 951, 'outbound', 'fb_docx_msg', 'attachment', 'Sent contract', '{\"caption\":\"Sent contract\"}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_attachments
             (id, message_id, attachment_type, file_name, mime_type, size_bytes, local_path, created_at)
             VALUES (955, 954, 'file', 'contract.docx', 'application/vnd.openxmlformats-officedocument.wordprocessingml.document', 2048, ?1, ?2)",
            rusqlite::params![docx_file.to_string_lossy().to_string(), now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_fb_954', 950, 954, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = MockMessengerAttachmentClient {
            sent_file_name: std::sync::Mutex::new(None),
            sent_mime_type: std::sync::Mutex::new(None),
            sent_media_type: std::sync::Mutex::new(None),
            sent_bytes_len: std::sync::Mutex::new(None),
        };

        let processed = harness
            .service
            .process_next_messenger_outbox_job(950, &mock_client, "PAGE_950", "PAGE_TOKEN_VALID")
            .await
            .unwrap();

        let _ = std::fs::remove_file(&docx_file);

        assert!(processed);
        assert_eq!(
            mock_client.sent_file_name.lock().unwrap().as_deref(),
            Some("contract.docx")
        );
        assert_eq!(
            mock_client.sent_mime_type.lock().unwrap().as_deref(),
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        );
        assert_eq!(
            mock_client.sent_media_type.lock().unwrap().as_deref(),
            Some("file")
        );
        assert_eq!(
            mock_client.sent_bytes_len.lock().unwrap().as_ref(),
            Some(&b"PK\x03\x04fb_docx_payload".len())
        );
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_reaction() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (730, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (731, 730, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (732, 731, 'outbound', 'wa_react_msg', 'reaction', '❤️', '{\"emoji\":\"❤️\",\"target_message_id\":\"target_msg_100\"}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_732', 730, 732, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(730, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        let reactions = mock_client.get_sent_reactions();
        assert_eq!(reactions.len(), 1);
        assert_eq!(reactions[0].emoji, "❤️");
        assert_eq!(reactions[0].target_message_id, "target_msg_100");
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_retryable_error() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (740, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (741, 740, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (742, 741, 'outbound', 'wa_msg_retry', 'text', 'Will fail transiently', '{}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_742', 740, 742, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        mock_client.set_next_send_error(
            crate::conversation::whatsapp::client::WhatsAppErrorKind::RateLimited,
            "Rate limited: 429 Too Many Requests",
        );

        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(740, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        // Verify status is retry_wait and next_attempt_at is set
        let (status, next_attempt, attempt_count): (String, Option<String>, i64) = conn.query_row(
            "SELECT status, next_attempt_at, attempt_count FROM message_outbox_jobs WHERE id = 'job_wa_742'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap();

        assert_eq!(status, "retry_wait");
        assert!(next_attempt.is_some());
        assert_eq!(attempt_count, 1);
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_auth_error_invalidates_account() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (750, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO whatsapp_sessions 
             (channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at)
             VALUES (750, '84901234567', '84901234567', 'session_750.db', 1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (751, 750, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (752, 751, 'outbound', 'wa_msg_auth_err', 'text', 'Auth fail test', '{}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_752', 750, 752, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        mock_client.set_next_send_error(
            crate::conversation::whatsapp::client::WhatsAppErrorKind::AuthFailure,
            "401 Unauthorized: Session revoked by user",
        );

        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(750, &mock_client)
            .await
            .unwrap();

        // Circuit breaker: auth failure returns false to halt queue drain
        assert!(
            !processed,
            "process_next_whatsapp_outbox_job must return false on auth error to halt queue drain"
        );

        // Verify job failed
        let job_status: String = conn
            .query_row(
                "SELECT status FROM message_outbox_jobs WHERE id = 'job_wa_752'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(job_status, "failed");

        // Verify account marked invalid
        let conn_status: String = conn
            .query_row(
                "SELECT connection_status FROM channel_accounts WHERE id = 750",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(conn_status, "invalid");

        // Verify session marked invalid
        let is_valid: i64 = conn
            .query_row(
                "SELECT is_valid FROM whatsapp_sessions WHERE channel_account_id = 750",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(is_valid, 0);
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_ambiguous_timeout_does_not_retry() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (760, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (761, 760, '84909999999@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (762, 761, 'outbound', 'wa_msg_timeout', 'text', 'Timeout test', '{}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_762', 760, 762, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        mock_client.set_next_send_error(
            crate::conversation::whatsapp::client::WhatsAppErrorKind::AmbiguousTimeout,
            "Ambiguous timeout waiting for server ack",
        );

        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(760, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        // REQ-WA-07 Invariant: Status must be failed / delivery_unknown and NOT retry_wait
        let (job_status, msg_err_code): (String, String) = conn.query_row(
            "SELECT oj.status, m.error_code FROM message_outbox_jobs oj JOIN messages m ON m.id = oj.message_id WHERE oj.id = 'job_wa_762'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();

        assert_eq!(job_status, "failed");
        assert_eq!(msg_err_code, "delivery_unknown");
    }

    #[tokio::test]
    async fn test_process_whatsapp_outbox_job_invalid_recipient_suppresses() {
        let harness = ServiceHarness::new();
        let conn = harness.open_conn();
        crate::conversation::whatsapp::migration::apply_whatsapp_migrations(&conn).unwrap();

        let now = utc_now();
        conn.execute(
            "INSERT INTO channel_accounts 
             (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
             VALUES (770, 'whatsapp', '84901234567', 'WA Sales', 1, 'connected', '{}', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations 
             (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
             VALUES (771, 770, '84909888888@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO messages 
             (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
             VALUES (772, 771, 'outbound', 'wa_msg_invalid_rcpt', 'text', 'Invalid rcpt test', '{}', 'queued', ?1)",
            rusqlite::params![now],
        ).unwrap();

        conn.execute(
            "INSERT INTO message_outbox_jobs 
             (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
             VALUES ('job_wa_772', 770, 772, 'pending', 0, ?1, ?1, ?1)",
            rusqlite::params![now],
        ).unwrap();

        let mock_client = crate::conversation::whatsapp::client::MockWhatsAppClient::new();
        mock_client.set_next_send_error(
            crate::conversation::whatsapp::client::WhatsAppErrorKind::InvalidRecipient,
            "Recipient 84909888888 is not on WhatsApp",
        );

        let processed = harness
            .service
            .process_next_whatsapp_outbox_job(770, &mock_client)
            .await
            .unwrap();

        assert!(processed);

        // Verify job failed
        let job_status: String = conn
            .query_row(
                "SELECT status FROM message_outbox_jobs WHERE id = 'job_wa_772'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(job_status, "failed");

        // Verify phone was added to suppression list
        let suppression_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM contact_suppression_list WHERE channel = 'whatsapp' AND phone_or_email = '84909888888'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(suppression_count, 1);
    }

    #[tokio::test]
    async fn zalo_listener_to_service_to_repository_receipt_flow_and_invalid_quarantine() {
        let harness = ServiceHarness::new();
        let _account_id = harness.seed_zalo_account("imei-a", None);

        let msg_req = NormalizedMessage {
            channel: Channel::Zalo,
            provider_account_key: "imei-a".to_string(),
            account_display_name: Some("Sale Agent".to_string()),
            participant: Some(NormalizedIdentity {
                identity_namespace: "zalo_uid".to_string(),
                external_id: "peer-zalo-100".to_string(),
                display_name: Some("Customer 100".to_string()),
                avatar_url: None,
                lead_id: None,
            }),
            sender_is_participant: false,
            external_thread_id: "peer-zalo-100".to_string(),
            conversation_kind: crate::conversation::types::ConversationKind::Direct,
            conversation_title: Some("Customer 100".to_string()),
            direction: MessageDirection::Outbound,
            status: MessageStatus::Sent,
            provider_message_id: Some("p-msg-100".to_string()),
            provider_client_message_id: Some("cli-msg-100".to_string()),
            local_request_id: None,
            content_type: "text".to_string(),
            text_content: Some("Hello customer".to_string()),
            content: serde_json::json!({}),
            attachments: vec![],
            reply: None,
            mentions: vec![],
            raw_payload: None,
            provider_timestamp: Some("2026-08-30T10:00:00.000Z".to_string()),
            ingested_at: "2026-08-30T10:00:00.000Z".to_string(),
        };

        let applied = harness
            .repository
            .apply_normalized_message(&msg_req)
            .unwrap();
        let msg_id = applied.message_id;

        let conn = harness.open_conn();
        let status_0: String = conn
            .query_row("SELECT status FROM messages WHERE id = ?1", [msg_id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(status_0, "sent");

        // 1. Ingest Delivered Event (seen = 0)
        let del_data = zca_rs::models::event::TDeliveredMessage {
            msg_id: "p-msg-100".to_string(),
            seen: 0,
            delivered_uids: vec!["peer-zalo-100".to_string()],
            seen_uids: vec![],
            real_msg_id: "".to_string(),
            m_s_ts: 1788085000000,
        };
        let del_event = zca_rs::listener::ListenerEvent::DeliveredMessages(vec![
            zca_rs::models::DeliveredMessage::User(zca_rs::models::UserDeliveredMessage::new(
                del_data,
            )),
        ]);
        harness
            .service
            .ingest_zalo_event("imei-a", Some("Sale Agent"), del_event)
            .unwrap();

        let status_1: String = conn
            .query_row("SELECT status FROM messages WHERE id = ?1", [msg_id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(status_1, "delivered");

        // 2. Ingest Seen Event (Read)
        let seen_data = zca_rs::models::event::TUserSeenMessage {
            id_to: "peer-zalo-100".to_string(),
            msg_id: "p-msg-100".to_string(),
            real_msg_id: "".to_string(),
        };
        let seen_event =
            zca_rs::listener::ListenerEvent::SeenMessages(vec![zca_rs::models::SeenMessage::User(
                zca_rs::models::UserSeenMessage::new(seen_data),
            )]);
        harness
            .service
            .ingest_zalo_event("imei-a", Some("Sale Agent"), seen_event)
            .unwrap();

        let status_2: String = conn
            .query_row("SELECT status FROM messages WHERE id = ?1", [msg_id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(status_2, "read");

        // 3. Ingest malformed/unanchored receipts - must be quarantined and cause 0 changes
        let invalid_del_data = zca_rs::models::event::TDeliveredMessage {
            msg_id: "0".to_string(),
            seen: 0,
            delivered_uids: vec!["peer-zalo-100".to_string()],
            seen_uids: vec![],
            real_msg_id: "".to_string(),
            m_s_ts: 0,
        };
        let invalid_del_event = zca_rs::listener::ListenerEvent::DeliveredMessages(vec![
            zca_rs::models::DeliveredMessage::User(zca_rs::models::UserDeliveredMessage::new(
                invalid_del_data,
            )),
        ]);
        harness
            .service
            .ingest_zalo_event("imei-a", Some("Sale Agent"), invalid_del_event)
            .unwrap();

        let status_3: String = conn
            .query_row("SELECT status FROM messages WHERE id = ?1", [msg_id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap();
        assert_eq!(status_3, "read");

        let receipts_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_receipts WHERE message_id = ?1",
                [msg_id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(receipts_count, 2); // 1 delivered + 1 read
    }

    #[test]
    fn test_ingest_zalo_event_publishes_delta_event() {
        let harness = ServiceHarness::new();
        let mut delta_rx = harness.events.subscribe_delta();
        let mut inval_rx = harness.events.subscribe();

        let raw_user_msg: zca_rs::models::TMessage = serde_json::from_value(serde_json::json!({
            "actionId": "action-1",
            "msgId": "delta-msg-999",
            "cliMsgId": "cli-delta-msg-999",
            "msgType": "webchat",
            "uidFrom": "user-1",
            "idTo": "0",
            "dName": "Customer 1",
            "ts": "1722420000000",
            "status": 1,
            "content": "Xin chào shop realtime",
            "notify": "",
            "ttl": 0,
            "userId": "",
            "uin": "",
            "propertyExt": null,
            "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
            "cmd": 501,
            "st": 0,
            "at": 0,
            "realMsgId": "real-delta-msg-999",
            "quote": null
        }))
        .unwrap();

        let event =
            zca_rs::listener::ListenerEvent::Message(Box::new(zca_rs::models::Message::User(
                zca_rs::models::UserMessage::new("self-uid", raw_user_msg),
            )));

        harness
            .service
            .ingest_zalo_event("imei-1", Some("Sale 01"), event)
            .unwrap();

        // 1. Verify invalidation event
        let inval = inval_rx.try_recv().expect("Expected invalidation event");
        assert_eq!(inval.conversation_id, 1);
        assert_eq!(inval.channel, Channel::Zalo);

        // 2. Verify delta event contains full render-ready ConversationMessage
        let delta = delta_rx.try_recv().expect("Expected delta event");
        assert_eq!(delta.conversation_id, 1);
        assert_eq!(delta.channel, Channel::Zalo);
        assert!(delta.sequence > 0);
        assert_eq!(
            delta.message.text_content.as_deref(),
            Some("Xin chào shop realtime")
        );
        assert_eq!(
            delta.message.sender_display_name.as_deref(),
            Some("Customer 1")
        );
        assert!(delta.emitted_at_ms > 0);
    }

    #[test]
    fn test_enqueue_outbound_batch_returns_hydrated_messages_and_publishes_delta() {
        use crate::conversation::types::{
            ConversationSendPartContent, ConversationSendPartRequest, SendConversationBatchRequest,
        };

        let harness = ServiceHarness::new();
        let mut delta_rx = harness.events.subscribe_delta();
        let mut inval_rx = harness.events.subscribe();

        let req = SendConversationBatchRequest {
            conversation_id: 1,
            client_batch_id: "batch-hydrated-01".to_string(),
            reply_to_message_id: None,
            parts: vec![ConversationSendPartRequest {
                client_part_id: "part-01".to_string(),
                content: ConversationSendPartContent::Text {
                    text: "Tin nhắn outbound hydrate".to_string(),
                    mentions: vec![],
                },
            }],
            message_tag: None,
        };

        let res = harness.service.enqueue_batch(&req).unwrap();
        assert_eq!(res.messages.len(), 1);
        let msg = &res.messages[0];
        assert_eq!(
            msg.status,
            crate::conversation::types::MessageStatus::Queued
        );
        assert_eq!(msg.send_batch.as_ref().unwrap().send_batch_id, res.batch_id);
        assert_eq!(res.client_batch_id, "batch-hydrated-01");

        // Verify delta published
        let delta = delta_rx
            .try_recv()
            .expect("Expected delta for enqueued outbound batch");
        assert_eq!(delta.conversation_id, 1);
        assert_eq!(
            delta.message.text_content.as_deref(),
            Some("Tin nhắn outbound hydrate")
        );
        assert_eq!(
            delta.message.status,
            crate::conversation::types::MessageStatus::Queued
        );
        assert!(inval_rx.try_recv().is_ok());
    }

    #[test]
    fn test_ingest_zalo_echo_publishes_delta_event() {
        let harness = ServiceHarness::new();
        let mut delta_rx = harness.events.subscribe_delta();

        // 1. Enqueue outbound message
        let req = crate::conversation::types::SendConversationBatchRequest {
            conversation_id: 1,
            client_batch_id: "batch-echo-01".to_string(),
            reply_to_message_id: None,
            parts: vec![crate::conversation::types::ConversationSendPartRequest {
                client_part_id: "part-echo-01".to_string(),
                content: crate::conversation::types::ConversationSendPartContent::Text {
                    text: "Tin nhắn gửi qua zalo".to_string(),
                    mentions: vec![],
                },
            }],
            message_tag: None,
        };
        let _ = harness.service.enqueue_batch(&req).unwrap();
        let _ = delta_rx.try_recv().unwrap(); // drain enqueue delta

        // 2. Ingest websocket echo for the message (is_self = true)
        let raw_user_msg: zca_rs::models::TMessage = serde_json::from_value(serde_json::json!({
            "actionId": "action-echo-1",
            "msgId": "echo-msg-100",
            "cliMsgId": "cli-delta-msg-echo",
            "msgType": "webchat",
            "uidFrom": "self-uid",
            "idTo": "user-1",
            "dName": "Sale 01",
            "ts": "1722420050000",
            "status": 1,
            "content": "Tin nhắn gửi qua zalo",
            "notify": "",
            "ttl": 0,
            "userId": "",
            "uin": "",
            "propertyExt": null,
            "paramsExt": {"countUnread": 0, "containType": 0, "platformType": 1},
            "cmd": 501,
            "st": 0,
            "at": 0,
            "realMsgId": "real-echo-msg-100",
            "quote": null
        }))
        .unwrap();

        let event =
            zca_rs::listener::ListenerEvent::Message(Box::new(zca_rs::models::Message::User(
                zca_rs::models::UserMessage::new("self-uid", raw_user_msg),
            )));

        harness
            .service
            .ingest_zalo_event("imei-1", Some("Sale 01"), event)
            .unwrap();

        // 3. Verify delta was published on echo receipt
        let delta = delta_rx
            .try_recv()
            .expect("Expected delta event on echo arrival");
        assert_eq!(delta.conversation_id, 1);
        assert_eq!(delta.channel, Channel::Zalo);
        assert_eq!(
            delta.message.text_content.as_deref(),
            Some("Tin nhắn gửi qua zalo")
        );
    }

    #[test]
    fn test_burst_300_events_persists_monotonic_sequence_and_replays_lagged_events() {
        let harness = ServiceHarness::new();
        // 1. Burst ingest 300 messages
        for i in 1..=300 {
            let raw_msg: zca_rs::models::TMessage = serde_json::from_value(serde_json::json!({
                "actionId": format!("act-{}", i),
                "msgId": format!("msg-burst-{}", i),
                "cliMsgId": format!("cli-burst-{}", i),
                "msgType": "webchat",
                "uidFrom": "user-1",
                "idTo": "0",
                "dName": "Customer 1",
                "ts": format!("{}", 1722420000000i64 + i as i64 * 1000),
                "status": 1,
                "content": format!("Burst message {}", i),
                "notify": "",
                "ttl": 0,
                "userId": "",
                "uin": "",
                "propertyExt": null,
                "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
                "cmd": 501,
                "st": 0,
                "at": 0,
                "realMsgId": format!("real-burst-{}", i),
                "quote": null
            }))
            .unwrap();

            let event =
                zca_rs::listener::ListenerEvent::Message(Box::new(zca_rs::models::Message::User(
                    zca_rs::models::UserMessage::new("self-uid", raw_msg),
                )));

            harness
                .service
                .ingest_zalo_event("imei-1", Some("Sale 01"), event)
                .unwrap();
        }

        // 2. Replay all 300 deltas from sequence 0
        let all_deltas = harness.service.resume_deltas(0, Some(500)).unwrap();
        assert_eq!(
            all_deltas.len(),
            300,
            "Must recover all 300 messages from SQLite change log"
        );

        // 3. Verify sequence monotonicity
        for (idx, delta) in all_deltas.iter().enumerate() {
            let expected_seq = (idx + 1) as u64;
            assert_eq!(
                delta.sequence, expected_seq,
                "Sequence must be strictly monotonic and gapless"
            );
            assert_eq!(
                delta.message.text_content.as_deref(),
                Some(format!("Burst message {}", idx + 1).as_str())
            );
        }

        // 4. Replay starting from sequence 250 (resume last 50 missed events)
        let resumed = harness.service.resume_deltas(250, Some(100)).unwrap();
        assert_eq!(resumed.len(), 50);
        assert_eq!(resumed[0].sequence, 251);
        assert_eq!(resumed[49].sequence, 300);
    }
}

mod zalo_webhook_tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::Duration;
    use zca_rs::models::{GroupMessage, TGroupMessage};

    /// Short timings so batching tests finish quickly.
    fn fast_tuning() -> WebhookTuning {
        WebhookTuning {
            quiet_period: Duration::from_millis(250),
            max_wait: Duration::from_millis(900),
            attempt_timeout: Duration::from_secs(2),
            retry_delays: [Duration::from_millis(30), Duration::from_millis(60)],
        }
    }

    fn webhook(url: &str, include_groups: bool) -> ZaloWebhook {
        ZaloWebhook::new(Some(url), include_groups, fast_tuning())
    }

    struct Receiver {
        bodies: Mutex<Vec<serde_json::Value>>,
        /// Statuses to answer with, in order; 200 once exhausted.
        script: Mutex<VecDeque<u16>>,
    }

    /// Starts a local HTTP server that records every JSON POST body.
    async fn start_receiver(script: &[u16]) -> (String, Arc<Receiver>) {
        let state = Arc::new(Receiver {
            bodies: Mutex::new(Vec::new()),
            script: Mutex::new(script.iter().copied().collect()),
        });
        let shared = state.clone();
        let app = axum::Router::new().route(
            "/hook",
            axum::routing::post(move |axum::Json(body): axum::Json<serde_json::Value>| {
                let shared = shared.clone();
                async move {
                    shared.bodies.lock().unwrap().push(body);
                    let next = shared.script.lock().unwrap().pop_front().unwrap_or(200);
                    axum::http::StatusCode::from_u16(next).unwrap()
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{addr}/hook"), state)
    }

    async fn wait_for_posts(receiver: &Receiver, expected: usize) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while receiver.bodies.lock().unwrap().len() < expected
            && std::time::Instant::now() < deadline
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    fn raw_message(
        message_id: &str,
        uid_from: &str,
        id_to: &str,
        content: serde_json::Value,
        msg_type: &str,
    ) -> TMessage {
        serde_json::from_value(serde_json::json!({
            "actionId": "action-hook",
            "msgId": message_id,
            "cliMsgId": format!("client-{message_id}"),
            "msgType": msg_type,
            "uidFrom": uid_from,
            "idTo": id_to,
            "dName": "Chị Bích Việt",
            "ts": "1785492000000",
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
            "realMsgId": format!("real-{message_id}"),
            "quote": null
        }))
        .unwrap()
    }

    fn user_event(message_id: &str, uid_from: &str, text: &str) -> ListenerEvent {
        let raw = raw_message(message_id, uid_from, "self-uid", text.into(), "webchat");
        ListenerEvent::Message(Box::new(Message::User(UserMessage::new("self-uid", raw))))
    }

    fn group_event(message_id: &str, text: &str) -> ListenerEvent {
        let raw = raw_message(message_id, "peer-uid", "group-1", text.into(), "webchat");
        ListenerEvent::Message(Box::new(Message::Group(GroupMessage::new(
            "self-uid",
            TGroupMessage {
                base: raw,
                mentions: None,
            },
        ))))
    }

    fn ingest(harness: &ServiceHarness, event: ListenerEvent) {
        harness
            .service
            .ingest_zalo_event("imei-1", Some("Sale 01"), event)
            .unwrap();
    }

    #[tokio::test]
    async fn three_quick_messages_make_one_post() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));

        ingest(&harness, user_event("b1", "peer-uid", "một"));
        ingest(&harness, user_event("b2", "peer-uid", "hai"));
        ingest(&harness, user_event("b3", "peer-uid", "ba"));
        // Nothing may be sent before the quiet period has passed.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(receiver.bodies.lock().unwrap().is_empty());

        wait_for_posts(&receiver, 1).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let bodies = receiver.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 1, "exactly one POST expected: {bodies:?}");
        assert_eq!(bodies[0]["text"], "một\nhai\nba");
        assert_eq!(bodies[0]["messageCount"], 3);
        assert_eq!(bodies[0]["senderUid"], "peer-uid");
    }

    #[tokio::test]
    async fn two_conversations_make_two_posts() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));

        ingest(&harness, user_event("c1", "peer-a", "từ A"));
        ingest(&harness, user_event("c2", "peer-b", "từ B"));
        wait_for_posts(&receiver, 2).await;
        tokio::time::sleep(Duration::from_millis(400)).await;

        let bodies = receiver.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 2, "{bodies:?}");
        assert_ne!(bodies[0]["conversationId"], bodies[1]["conversationId"]);
        let mut texts: Vec<_> = bodies.iter().map(|b| b["text"].clone()).collect();
        texts.sort_by_key(|t| t.to_string());
        assert_eq!(texts, vec!["từ A", "từ B"]);
    }

    #[tokio::test]
    async fn continuous_chatter_is_flushed_after_max_wait() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));

        // A message every 150 ms never reaches the 250 ms quiet period, so only
        // the 900 ms max wait can flush the first batch.
        for index in 0..8 {
            ingest(&harness, user_event(&format!("m{index}"), "peer-uid", "x"));
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        wait_for_posts(&receiver, 2).await;
        let bodies = receiver.bodies.lock().unwrap();
        assert!(
            bodies.len() >= 2,
            "max wait must split the stream: {bodies:?}"
        );
        let total: u64 = bodies
            .iter()
            .map(|b| b["messageCount"].as_u64().unwrap())
            .sum();
        assert!(total <= 8);
    }

    #[tokio::test]
    async fn group_messages_are_dropped_unless_enabled() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        ingest(&harness, group_event("g1", "tin nhóm"));
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(receiver.bodies.lock().unwrap().is_empty());

        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, true));
        ingest(&harness, group_event("g2", "tin nhóm"));
        wait_for_posts(&receiver, 1).await;
        let bodies = receiver.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0]["isGroup"], true);
    }

    #[tokio::test]
    async fn server_error_is_retried_then_delivered_once() {
        let (url, receiver) = start_receiver(&[500]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        ingest(&harness, user_event("r1", "peer-uid", "thử lại"));

        wait_for_posts(&receiver, 2).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let bodies = receiver.bodies.lock().unwrap();
        // 500 first, then 200: two attempts, and no third one after success.
        assert_eq!(bodies.len(), 2, "{bodies:?}");
        assert_eq!(bodies[0], bodies[1]);
    }

    #[tokio::test]
    async fn too_many_requests_is_retried() {
        let (url, receiver) = start_receiver(&[429]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        ingest(&harness, user_event("t1", "peer-uid", "429"));
        wait_for_posts(&receiver, 2).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert_eq!(receiver.bodies.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn client_error_is_not_retried() {
        let (url, receiver) = start_receiver(&[400, 400, 400]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        ingest(&harness, user_event("e1", "peer-uid", "sai"));

        wait_for_posts(&receiver, 1).await;
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(receiver.bodies.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn persistent_server_error_stops_after_three_attempts() {
        let (url, receiver) = start_receiver(&[503, 503, 503, 503, 503]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        ingest(&harness, user_event("p1", "peer-uid", "hỏng"));

        wait_for_posts(&receiver, 3).await;
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert_eq!(receiver.bodies.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn photo_post_carries_the_stored_attachment_id_but_not_its_url() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        let raw = raw_message(
            "ph1",
            "peer-uid",
            "self-uid",
            serde_json::json!({"url": "https://cdn.example.com/secret-photo.jpg", "caption": "cont này"}),
            "chat.photo",
        );
        ingest(
            &harness,
            ListenerEvent::Message(Box::new(Message::User(UserMessage::new("self-uid", raw)))),
        );

        wait_for_posts(&receiver, 1).await;
        let bodies = receiver.bodies.lock().unwrap();
        let body = &bodies[0];
        let message_id: i64 = harness
            .repository
            .get_conversation_timeline(body["conversationId"].as_i64().unwrap(), None, 10)
            .unwrap()
            .items[0]
            .id;
        let stored = harness
            .repository
            .list_message_attachment_refs(message_id)
            .unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(
            body["attachments"],
            serde_json::json!([{
                "messageId": message_id,
                "attachmentId": stored[0].0,
                "kind": "image"
            }])
        );
        assert!(!body.to_string().contains("secret-photo"));
    }

    #[tokio::test]
    async fn payload_uses_the_agreed_field_names() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        ingest(&harness, user_event("f1", "peer-uid", "xin chào"));

        wait_for_posts(&receiver, 1).await;
        let bodies = receiver.bodies.lock().unwrap();
        let body = bodies[0].as_object().unwrap();
        let mut keys: Vec<&str> = body.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            [
                "attachments",
                "channelAccountId",
                "conversationId",
                "isGroup",
                "messageCount",
                "senderName",
                "senderUid",
                "test",
                "text",
                "ts"
            ]
        );
        assert_eq!(body["text"], "xin chào");
        assert_eq!(body["senderUid"], "peer-uid");
        assert_eq!(body["senderName"], "Chị Bích Việt");
        assert_eq!(body["isGroup"], false);
        assert_eq!(body["test"], false);
        assert_eq!(body["channelAccountId"], 1);
        assert_eq!(body["messageCount"], 1);
        assert_eq!(body["attachments"], serde_json::json!([]));
        assert!(body["conversationId"].is_i64());
        let ts = body["ts"].as_str().unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(ts).is_ok(), "{ts}");
    }

    #[tokio::test]
    async fn replayed_inbound_message_is_not_batched_twice() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        for _ in 0..2 {
            ingest(&harness, user_event("dup", "peer-uid", "lặp"));
        }
        wait_for_posts(&receiver, 1).await;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let bodies = receiver.bodies.lock().unwrap();
        assert_eq!(bodies.len(), 1);
        assert_eq!(bodies[0]["messageCount"], 1);
    }

    #[tokio::test]
    async fn outbound_self_message_does_not_post() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        let raw = raw_message("self1", "self-uid", "user-1", "tôi gửi".into(), "webchat");
        ingest(
            &harness,
            ListenerEvent::Message(Box::new(Message::User(UserMessage::new("self-uid", raw)))),
        );
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(receiver.bodies.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn history_backfill_does_not_post() {
        let (url, receiver) = start_receiver(&[]).await;
        let harness = ServiceHarness::new_with_webhook(webhook(&url, false));
        let ListenerEvent::Message(message) = user_event("old1", "peer-uid", "cũ") else {
            unreachable!("helper always returns a live message");
        };
        ingest(
            &harness,
            ListenerEvent::OldMessages {
                messages: vec![*message],
                thread_type: ThreadType::User,
            },
        );
        tokio::time::sleep(Duration::from_millis(700)).await;
        assert!(receiver.bodies.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_or_missing_url_does_not_post() {
        let (_url, receiver) = start_receiver(&[]).await;
        for hook in [
            ZaloWebhook::new(Some(""), false, fast_tuning()),
            ZaloWebhook::new(Some("   "), false, fast_tuning()),
            ZaloWebhook::new(None, false, fast_tuning()),
            ZaloWebhook::new(Some("ftp://example.invalid/x"), false, fast_tuning()),
        ] {
            assert!(!hook.is_enabled());
            let harness = ServiceHarness::new_with_webhook(hook);
            ingest(&harness, user_event("off", "peer-uid", "x"));
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(receiver.bodies.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn unreachable_webhook_does_not_break_ingestion() {
        // Port 1 on loopback: connection refused.
        let harness = ServiceHarness::new_with_webhook(webhook("http://127.0.0.1:1/hook", false));
        let mut delta_rx = harness.events.subscribe_delta();

        ingest(&harness, user_event("d1", "peer-uid", "một"));
        ingest(&harness, user_event("d2", "peer-uid", "hai"));
        tokio::time::sleep(Duration::from_millis(900)).await;

        assert!(delta_rx.try_recv().is_ok());
        assert!(delta_rx.try_recv().is_ok());
    }

    #[test]
    fn notify_outside_runtime_is_a_silent_noop() {
        let harness = ServiceHarness::new_with_webhook(webhook("http://127.0.0.1:1/hook", false));
        ingest(&harness, user_event("sync", "peer-uid", "x"));
    }
}
