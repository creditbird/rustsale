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
