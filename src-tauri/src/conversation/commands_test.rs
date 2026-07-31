use super::events::{ConversationEventPublisher, ConversationInvalidationReason};
use super::migration::migrate;
use super::repository::{ConversationRepository, NormalizedIdentity, NormalizedMessage};
use super::service::ConversationService;
use super::types::{Channel, ConversationKind, MessageDirection, MessageStatus};
use crate::db::open_db_connection;
use std::sync::Arc;

pub struct ServiceHarness {
    pub path: std::path::PathBuf,
    pub repository: Arc<ConversationRepository>,
    pub events: Arc<ConversationEventPublisher>,
    pub service: ConversationService,
}

impl ServiceHarness {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustsale-conversation-service-{}.db",
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

        let repository = Arc::new(ConversationRepository::new(
            path.to_string_lossy().to_string(),
        ));
        let events = Arc::new(ConversationEventPublisher::new(256));
        let service = ConversationService::new(repository.clone(), events.clone());

        Self {
            path,
            repository,
            events,
            service,
        }
    }

    pub fn insert_lead(&self, id: i64, name: &str) {
        let conn = open_db_connection(self.path.to_str().unwrap()).unwrap();
        conn.execute(
            "INSERT INTO leads (id, name) VALUES (?1, ?2)",
            rusqlite::params![id, name],
        )
        .unwrap();
    }
}

impl Drop for ServiceHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

pub fn direct_message(account: &str, provider_message_id: &str) -> NormalizedMessage {
    direct_message_for(account, "peer-1", provider_message_id)
}

pub fn direct_message_for(
    account: &str,
    peer_id: &str,
    provider_message_id: &str,
) -> NormalizedMessage {
    NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: account.to_string(),
        account_display_name: Some(account.to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: peer_id.to_string(),
            display_name: Some(format!("Peer {peer_id}")),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: peer_id.to_string(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: Some(format!("Peer {peer_id}")),
        direction: MessageDirection::Inbound,
        provider_message_id: Some(provider_message_id.to_string()),
        provider_client_message_id: Some(format!("client-{provider_message_id}")),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some(provider_message_id.to_string()),
        content: serde_json::json!({"text": provider_message_id}),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T00:00:00.000Z".to_string()),
        ingested_at: "2026-07-31T00:00:01.000Z".to_string(),
        status: MessageStatus::Received,
    }
}

pub fn group_message(
    account: &str,
    group_thread_id: &str,
    provider_message_id: &str,
) -> NormalizedMessage {
    NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: account.to_string(),
        account_display_name: Some(account.to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: "peer-g1".to_string(),
            display_name: Some("Group Member 1".to_string()),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: group_thread_id.to_string(),
        conversation_kind: ConversationKind::Group,
        conversation_title: Some(format!("Group {group_thread_id}")),
        direction: MessageDirection::Inbound,
        provider_message_id: Some(provider_message_id.to_string()),
        provider_client_message_id: Some(format!("client-{provider_message_id}")),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("hello group".to_string()),
        content: serde_json::json!({"text": "hello group"}),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T00:00:00.000Z".to_string()),
        ingested_at: "2026-07-31T00:00:01.000Z".to_string(),
        status: MessageStatus::Received,
    }
}

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
