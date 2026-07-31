use super::outreach::*;
use crate::db::open_db_connection;
use zca_rs::models::{SendMessageResponse, SendMessageResult};

#[test]
fn provider_receipt_prefers_text_message_id() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: Some(SendMessageResult { msg_id: 42 }),
        attachment: vec![SendMessageResult { msg_id: 99 }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("42"));
    assert_eq!(receipt.attachment_message_ids, vec!["99"]);
}

#[test]
fn provider_receipt_uses_first_attachment_when_text_is_absent() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: None,
        attachment: vec![SendMessageResult { msg_id: 99 }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("99"));
    assert_eq!(receipt.attachment_message_ids, vec!["99"]);
}

#[test]
fn provider_receipt_empty_response_returns_none_primary_id() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: None,
        attachment: vec![],
    });
    assert_eq!(receipt.primary_message_id, None);
    assert!(receipt.attachment_message_ids.is_empty());
}

fn temp_database_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "rustsale-outreach-test-{}.db",
        rand::random::<u64>()
    ))
}

fn initialized_connection(path: &std::path::Path) -> rusqlite::Connection {
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO leads (id, name) VALUES (7, 'Test Lead 7') ON CONFLICT DO NOTHING",
        [],
    )
    .unwrap();
    crate::conversation::migration::migrate(&conn).unwrap();
    conn
}

fn record() -> OutreachConversationRecord {
    OutreachConversationRecord {
        lead_id: 7,
        provider_account_key: "imei-123456789".to_string(),
        account_display_name: Some("Shop Zalo".to_string()),
        external_user_id: "zalo-uid-999".to_string(),
        user_display_name: Some("Nguyen Van A".to_string()),
        user_avatar_url: Some("https://example.com/avatar.jpg".to_string()),
        template_id: 42,
        source: "bulk_outreach".to_string(),
        text: "Xin chào bạn".to_string(),
        local_request_id: "req-abc-123".to_string(),
        provider_receipt: ZaloProviderReceipt {
            primary_message_id: Some("msg-prov-001".to_string()),
            attachment_message_ids: vec!["att-1".to_string(), "att-2".to_string()],
        },
        sent_at: "2026-07-31T12:00:00Z".to_string(),
    }
}

fn count(conn: &rusqlite::Connection, table: &str, where_clause: &str) -> i64 {
    let sql = format!("SELECT COUNT(*) FROM {table} WHERE {where_clause}");
    conn.query_row(&sql, [], |r| r.get(0)).unwrap()
}

struct TestCleanup(std::path::PathBuf);
impl Drop for TestCleanup {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(self.0.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.0.with_extension("db-shm"));
    }
}

#[test]
fn outreach_success_creates_direct_conversation_and_no_group() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);
    let tx = connection.transaction().unwrap();

    let applied = record_outreach_success_in_transaction(&tx, &record()).unwrap();
    assert!(!applied.applied.duplicate);
    assert_eq!(count(&tx, "channel_conversations", "kind = 'direct'"), 1);
    assert_eq!(count(&tx, "channel_conversations", "kind = 'group'"), 0);

    assert_eq!(count(&tx, "channel_identities", "lead_id = 7"), 1);
    assert_eq!(
        count(
            &tx,
            "channel_accounts",
            "provider_account_key = 'imei-123456789'"
        ),
        1
    );
    assert_eq!(
        count(
            &tx,
            "messages",
            "direction = 'outbound' AND status = 'sent'"
        ),
        1
    );

    let content_json_str: String = tx
        .query_row(
            "SELECT content_json FROM messages WHERE id = ?1",
            [applied.applied.message_id],
            |r| r.get(0),
        )
        .unwrap();
    let content_val: serde_json::Value = serde_json::from_str(&content_json_str).unwrap();
    assert_eq!(content_val["text"], "Xin chào bạn");
    assert_eq!(content_val["outreach"]["templateId"], 42);
    assert_eq!(content_val["outreach"]["source"], "bulk_outreach");
    assert_eq!(
        content_val["outreach"]["providerAttachmentMessageIds"],
        serde_json::json!(["att-1", "att-2"])
    );

    let participant_count: i64 = count(&tx, "conversation_participants", "1 = 1");
    assert_eq!(participant_count, 1);
    let identity_id: i64 = tx
        .query_row(
            "SELECT identity_id FROM conversation_participants WHERE conversation_id = ?1",
            [applied.applied.conversation_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(Some(identity_id), applied.applied.identity_id);
}

#[test]
fn same_provider_message_id_is_idempotent() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);
    let tx = connection.transaction().unwrap();

    let first = record_outreach_success_in_transaction(&tx, &record()).unwrap();
    let second = record_outreach_success_in_transaction(&tx, &record()).unwrap();
    assert_eq!(first.applied.message_id, second.applied.message_id);
    assert!(second.applied.duplicate);
}

#[test]
fn same_lead_through_two_imeis_has_two_provider_conversations() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);
    let tx = connection.transaction().unwrap();

    let mut first = record();
    first.provider_account_key = "imei-a".to_string();
    first.local_request_id = "request-a".to_string();
    first.provider_receipt.primary_message_id = Some("message-a".to_string());
    let mut second = first.clone();
    second.provider_account_key = "imei-b".to_string();
    second.local_request_id = "request-b".to_string();
    second.provider_receipt.primary_message_id = Some("message-b".to_string());

    record_outreach_success_in_transaction(&tx, &first).unwrap();
    record_outreach_success_in_transaction(&tx, &second).unwrap();

    assert_eq!(count(&tx, "channel_conversations", "kind = 'direct'"), 2);
    assert_eq!(count(&tx, "channel_identities", "lead_id = 7"), 1);
}

#[test]
fn rolled_back_crm_transaction_leaves_no_conversation_rows() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);
    let tx = connection.transaction().unwrap();
    record_outreach_success_in_transaction(&tx, &record()).unwrap();
    tx.rollback().unwrap();
    drop(connection);

    let reopened = open_db_connection(path.to_str().unwrap()).unwrap();
    assert_eq!(count(&reopened, "channel_conversations", "1 = 1"), 0);
    assert_eq!(count(&reopened, "messages", "1 = 1"), 0);
}
