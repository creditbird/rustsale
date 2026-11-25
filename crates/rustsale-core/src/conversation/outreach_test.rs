use super::outreach::*;
use crate::conversation::repository::{
    ConversationRepository, NormalizedIdentity, NormalizedMessage,
};
use crate::conversation::types::{
    Channel, ConversationKind, InboxItemKind, MessageDirection, MessageStatus,
};
use crate::db::open_db_connection;
use zca_rs::models::{SendMessageResponse, SendMessageResult};

#[test]
fn provider_receipt_prefers_text_message_id() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: Some(SendMessageResult {
            msg_id: 42,
            cli_msg_id: 0,
        }),
        attachment: vec![SendMessageResult {
            msg_id: 99,
            cli_msg_id: 0,
        }],
    });
    assert_eq!(receipt.primary_message_id.as_deref(), Some("42"));
    assert_eq!(receipt.attachment_message_ids, vec!["99"]);
}

#[test]
fn provider_receipt_uses_first_attachment_when_text_is_absent() {
    let receipt = ZaloProviderReceipt::from_response(SendMessageResponse {
        message: None,
        attachment: vec![SendMessageResult {
            msg_id: 99,
            cli_msg_id: 0,
        }],
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

#[test]
fn outreach_then_inbound_reply_merged_in_same_contact_timeline() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);

    let tx = connection.transaction().unwrap();
    let mut outreach_rec = record();
    outreach_rec.sent_at = "2026-07-31T12:00:00.000Z".to_string();
    outreach_rec.provider_receipt.primary_message_id = Some("outbound-msg-001".to_string());
    outreach_rec.text = "Outreach Hello".to_string();
    record_outreach_success_in_transaction(&tx, &outreach_rec).unwrap();
    tx.commit().unwrap();
    drop(connection);

    let repository = ConversationRepository::new(path.to_string_lossy().to_string());
    let inbound = NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: outreach_rec.provider_account_key.clone(),
        account_display_name: outreach_rec.account_display_name.clone(),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: outreach_rec.external_user_id.clone(),
            display_name: outreach_rec.user_display_name.clone(),
            avatar_url: outreach_rec.user_avatar_url.clone(),
            lead_id: Some(outreach_rec.lead_id),
        }),
        sender_is_participant: true,
        external_thread_id: outreach_rec.external_user_id.clone(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: outreach_rec.user_display_name.clone(),
        direction: MessageDirection::Inbound,
        provider_message_id: Some("inbound-msg-002".to_string()),
        provider_client_message_id: None,
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("Inbound Reply".to_string()),
        content: serde_json::json!({"text": "Inbound Reply"}),
        attachments: Vec::new(),
        reply: None,
        mentions: Vec::new(),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T12:05:00.000Z".to_string()),
        ingested_at: "2026-07-31T12:05:01.000Z".to_string(),
        status: MessageStatus::Received,
    };
    let applied_inbound = repository.apply_normalized_message(&inbound).unwrap();
    assert!(!applied_inbound.duplicate);

    let page = repository
        .get_contact_timeline(outreach_rec.lead_id, &[Channel::Zalo], None, 50)
        .unwrap();

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].direction, MessageDirection::Outbound);
    assert_eq!(
        page.items[0].text_content.as_deref(),
        Some("Outreach Hello")
    );
    assert_eq!(page.items[1].direction, MessageDirection::Inbound);
    assert_eq!(page.items[1].text_content.as_deref(), Some("Inbound Reply"));
    assert!(page.items[0].occurred_at <= page.items[1].occurred_at);
}

#[test]
fn duplicate_provider_message_id_suppresses_second_insert() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);

    let mut outreach_rec = record();
    outreach_rec.provider_receipt.primary_message_id = Some("dup-msg-id-100".to_string());

    let tx1 = connection.transaction().unwrap();
    let res1 = record_outreach_success_in_transaction(&tx1, &outreach_rec).unwrap();
    tx1.commit().unwrap();
    assert!(!res1.applied.duplicate);

    let tx2 = connection.transaction().unwrap();
    let res2 = record_outreach_success_in_transaction(&tx2, &outreach_rec).unwrap();
    tx2.commit().unwrap();
    assert!(res2.applied.duplicate);
    assert_eq!(res1.applied.message_id, res2.applied.message_id);

    assert_eq!(count(&connection, "messages", "1 = 1"), 1);

    let repository = ConversationRepository::new(path.to_string_lossy().to_string());
    let realtime_self_msg = NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: outreach_rec.provider_account_key.clone(),
        account_display_name: outreach_rec.account_display_name.clone(),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: outreach_rec.external_user_id.clone(),
            display_name: outreach_rec.user_display_name.clone(),
            avatar_url: outreach_rec.user_avatar_url.clone(),
            lead_id: Some(outreach_rec.lead_id),
        }),
        sender_is_participant: false,
        external_thread_id: outreach_rec.external_user_id.clone(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: outreach_rec.user_display_name.clone(),
        direction: MessageDirection::Outbound,
        provider_message_id: Some("dup-msg-id-100".to_string()),
        provider_client_message_id: None,
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some(outreach_rec.text.clone()),
        content: serde_json::json!({"text": outreach_rec.text}),
        attachments: Vec::new(),
        reply: None,
        mentions: Vec::new(),
        raw_payload: None,
        provider_timestamp: Some(outreach_rec.sent_at.clone()),
        ingested_at: "2026-07-31T12:01:00.000Z".to_string(),
        status: MessageStatus::Sent,
    };
    let res3 = repository
        .apply_normalized_message(&realtime_self_msg)
        .unwrap();
    assert!(res3.duplicate);
    assert_eq!(res3.message_id, res1.applied.message_id);
    assert_eq!(count(&connection, "messages", "1 = 1"), 1);
}

#[test]
fn outreach_message_kind_is_always_direct() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);

    let mut rec1 = record();
    rec1.external_user_id = "user-1".to_string();
    rec1.provider_receipt.primary_message_id = Some("msg-dir-1".to_string());

    let mut rec2 = record();
    rec2.external_user_id = "user-2".to_string();
    rec2.local_request_id = "req-abc-456".to_string();
    rec2.provider_receipt.primary_message_id = Some("msg-dir-2".to_string());

    let tx = connection.transaction().unwrap();
    record_outreach_success_in_transaction(&tx, &rec1).unwrap();
    record_outreach_success_in_transaction(&tx, &rec2).unwrap();
    tx.commit().unwrap();

    assert_eq!(
        count(&connection, "channel_conversations", "kind = 'direct'"),
        2
    );
    assert_eq!(
        count(&connection, "channel_conversations", "kind = 'group'"),
        0
    );
    assert_eq!(
        count(&connection, "channel_conversations", "kind != 'direct'"),
        0
    );
}

#[test]
fn same_lead_contacted_through_two_imeis_has_single_inbox_customer_row_with_two_reply_targets() {
    let path = temp_database_path();
    let _cleanup = TestCleanup(path.clone());
    let mut connection = initialized_connection(&path);

    let mut first = record();
    first.provider_account_key = "imei-a".to_string();
    first.local_request_id = "request-a".to_string();
    first.provider_receipt.primary_message_id = Some("message-a".to_string());
    let mut second = first.clone();
    second.provider_account_key = "imei-b".to_string();
    second.local_request_id = "request-b".to_string();
    second.provider_receipt.primary_message_id = Some("message-b".to_string());

    let tx = connection.transaction().unwrap();
    record_outreach_success_in_transaction(&tx, &first).unwrap();
    record_outreach_success_in_transaction(&tx, &second).unwrap();
    tx.commit().unwrap();
    drop(connection);

    let repository = ConversationRepository::new(path.to_string_lossy().to_string());
    let inbox = repository.list_inbox().unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].key, "lead:7");
    assert_eq!(inbox[0].kind, InboxItemKind::Customer);
    assert_eq!(inbox[0].reply_targets.len(), 2);
    assert_eq!(inbox[0].channels.len(), 1);
    assert_eq!(inbox[0].channels[0].channel, Channel::Zalo);

    let page = repository
        .get_contact_timeline(7, &[Channel::Zalo], None, 50)
        .unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page
        .items
        .iter()
        .all(|m| m.direction == MessageDirection::Outbound));
}
