use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::ThreadType;

use crate::conversation::events::ConversationEventPublisher;
use crate::conversation::migration::migrate;
use crate::conversation::repository::ConversationRepository;
use crate::conversation::service::ConversationService;
use crate::conversation::types::{MessageStatus, SendConversationMessageRequest};
use crate::conversation::zalo::client::ZaloClient;
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;
use crate::db::open_db_connection;

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
        let service = Arc::new(ConversationService::new(repository.clone(), events.clone()));

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
