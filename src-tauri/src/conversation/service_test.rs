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
use crate::conversation::repository::ConversationRepository;
use crate::conversation::service::ConversationService;
use crate::conversation::types::{MessageStatus, SendConversationMessageRequest};
use crate::conversation::zalo::client::{NormalizedProfile, ZaloClient};
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;
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
                    c_msg_id: "old-client-message".to_string(),
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

pub struct FakeZaloClient {
    pub provider_message_id: Option<String>,
    pub should_fail: bool,
    pub fail_error: Option<String>,
}

impl FakeZaloClient {
    pub fn with_provider_message_id(id: &str) -> Self {
        Self {
            provider_message_id: Some(id.to_string()),
            should_fail: false,
            fail_error: None,
        }
    }

    pub fn failing(err: &str) -> Self {
        Self {
            provider_message_id: None,
            should_fail: true,
            fail_error: Some(err.to_string()),
        }
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

impl ServiceHarness {
    pub fn new() -> Self {
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
        let service = Arc::new(ConversationService::new(
            repository.clone(),
            events.clone(),
            spool_root,
        ));

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
    let message = harness
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

    let client = FakeZaloClient {
        provider_message_id: None,
        should_fail: true,
        fail_error: Some("transient network error".to_string()),
    };

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
