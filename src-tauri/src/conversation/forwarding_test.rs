use crate::conversation::error::ConversationErrorCode;
use crate::conversation::forwarding::enqueue_forward_operation;
use crate::conversation::migration::migrate;
use crate::conversation::types::{
    utc_now, ConversationCapabilities, ForwardConversationMessageRequest, ForwardOperationStatus,
    ForwardTargetStatus,
};
use crate::db::open_db_connection;
use rusqlite::params;
use std::path::PathBuf;

struct TestHarness {
    db_path: PathBuf,
}

impl TestHarness {
    fn new() -> Self {
        let db_path = std::env::temp_dir().join(format!(
            "rustsale-forwarding-test-{}.db",
            rand::random::<u64>()
        ));
        let conn = open_db_connection(db_path.to_str().unwrap()).unwrap();
        conn.execute(
            "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL, phone TEXT, zalo_uid TEXT)",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        drop(conn);
        Self { db_path }
    }

    fn open_conn(&self) -> rusqlite::Connection {
        open_db_connection(self.db_path.to_str().unwrap()).unwrap()
    }
}

fn seed_zalo_account(
    conn: &rusqlite::Connection,
    provider_account_key: &str,
    caps: Option<ConversationCapabilities>,
) -> i64 {
    let now = utc_now();
    let caps_json =
        serde_json::to_string(&caps.unwrap_or_else(ConversationCapabilities::zalo_phase_one))
            .unwrap();
    conn.query_row(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', ?1, 'connected', ?2, ?3, ?3)
         RETURNING id",
        params![provider_account_key, caps_json, now],
        |row| row.get(0),
    )
    .unwrap()
}

fn seed_conversation(conn: &rusqlite::Connection, account_id: i64, thread_id: &str) -> i64 {
    let now = utc_now();
    conn.query_row(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, title, status, last_synced_at, created_at, updated_at)
         VALUES (?1, ?2, 'direct', ?3, 'active', ?4, ?4, ?4)
         RETURNING id",
        params![account_id, thread_id, thread_id, now],
        |row| row.get(0),
    )
    .unwrap()
}

fn seed_source_message(
    conn: &rusqlite::Connection,
    conversation_id: i64,
    provider_message_id: Option<&str>,
    provider_client_message_id: Option<&str>,
) -> i64 {
    let now = utc_now();
    conn.query_row(
        "INSERT INTO messages
         (conversation_id, sender_identity_id, direction, provider_message_id, provider_client_message_id,
          local_request_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp,
          ingested_at, sent_at, edited_at, removed_at, status, error_code, error_message, reply_to_message_id,
          send_batch_id, batch_index)
         VALUES (?1, NULL, 'inbound', ?2, ?3, NULL, 'text', 'Hello World', '{\"text\":\"Hello World\"}', NULL, ?4, ?4, NULL, NULL, NULL, 'sent', NULL, NULL, NULL, NULL, 0)
         RETURNING id",
        params![conversation_id, provider_message_id, provider_client_message_id, now],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn forward_operation_enqueues_one_batch_job_per_target() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-1", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-target-1");
    let conv_t2 = seed_conversation(&conn, account_id, "thread-target-2");
    let src_msg_id =
        seed_source_message(&conn, conv_src, Some("pmsg-fwd-101"), Some("pcli-fwd-101"));

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-fwd-test-1".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1, conv_t2],
    };

    let tx = conn.transaction().unwrap();
    let res = enqueue_forward_operation(&tx, &req, &now).unwrap();
    tx.commit().unwrap();

    assert_eq!(res.client_operation_id, "op-fwd-test-1");
    assert_eq!(res.source_message_id, src_msg_id);
    assert_eq!(res.status, ForwardOperationStatus::Queued);
    assert_eq!(res.targets.len(), 2);

    assert_eq!(res.targets[0].conversation_id, conv_t1);
    assert_eq!(res.targets[0].status, ForwardTargetStatus::Queued);
    assert!(res.targets[0].forwarded_message_id.is_some());

    assert_eq!(res.targets[1].conversation_id, conv_t2);
    assert_eq!(res.targets[1].status, ForwardTargetStatus::Queued);
    assert!(res.targets[1].forwarded_message_id.is_some());

    let op_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_forward_operations WHERE client_operation_id = 'op-fwd-test-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(op_count, 1);

    let target_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_forward_targets WHERE client_operation_id = 'op-fwd-test-1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(target_count, 2);

    let batch_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_send_batches WHERE client_batch_id LIKE 'op-fwd-test-1:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(batch_count, 2);

    let fwd_msg_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE content_type = 'forward' AND text_content = 'Đã chuyển tiếp một tin nhắn'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fwd_msg_count, 2);

    let job_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_outbox_jobs WHERE status = 'pending'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(job_count, 2);
}

#[test]
fn forward_operation_idempotent_by_client_operation_id() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-2", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-2");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-t1-2");
    let conv_t2 = seed_conversation(&conn, account_id, "thread-t2-2");
    let src_msg_id =
        seed_source_message(&conn, conv_src, Some("pmsg-fwd-102"), Some("pcli-fwd-102"));

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-idempotent-2".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1, conv_t2],
    };

    let tx1 = conn.transaction().unwrap();
    let res1 = enqueue_forward_operation(&tx1, &req, &now).unwrap();
    tx1.commit().unwrap();

    let tx2 = conn.transaction().unwrap();
    let res2 = enqueue_forward_operation(&tx2, &req, &now).unwrap();
    tx2.commit().unwrap();

    assert_eq!(res1, res2);

    let op_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_forward_operations WHERE client_operation_id = 'op-idempotent-2'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(op_count, 1);

    let target_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_forward_targets WHERE client_operation_id = 'op-idempotent-2'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(target_count, 2);

    let batch_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_send_batches WHERE client_batch_id LIKE 'op-idempotent-2:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(batch_count, 2);

    let fwd_msg_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE content_type = 'forward'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fwd_msg_count, 2);
}

#[test]
fn forward_target_limit_exceeded_rejected_at_11_targets() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-3", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-3");
    let src_msg_id =
        seed_source_message(&conn, conv_src, Some("pmsg-fwd-103"), Some("pcli-fwd-103"));

    let mut targets = Vec::new();
    for i in 0..11 {
        targets.push(seed_conversation(
            &conn,
            account_id,
            &format!("thread-t3-{}", i),
        ));
    }

    let now = utc_now();
    let req_11 = ForwardConversationMessageRequest {
        client_operation_id: "op-over-limit".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: targets,
    };

    let tx = conn.transaction().unwrap();
    let err_11 = enqueue_forward_operation(&tx, &req_11, &now).unwrap_err();
    tx.rollback().unwrap();
    assert_eq!(
        err_11.code,
        ConversationErrorCode::ForwardTargetLimitExceeded
    );

    let req_0 = ForwardConversationMessageRequest {
        client_operation_id: "op-zero-limit".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![],
    };
    let tx0 = conn.transaction().unwrap();
    let err_0 = enqueue_forward_operation(&tx0, &req_0, &now).unwrap_err();
    tx0.rollback().unwrap();
    assert_eq!(
        err_0.code,
        ConversationErrorCode::ForwardTargetLimitExceeded
    );
}

#[test]
fn duplicate_target_conversations_rejected() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-4", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-4");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-t4-1");
    let src_msg_id =
        seed_source_message(&conn, conv_src, Some("pmsg-fwd-104"), Some("pcli-fwd-104"));

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-dup-target".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1, conv_t1],
    };

    let tx = conn.transaction().unwrap();
    let err = enqueue_forward_operation(&tx, &req, &now).unwrap_err();
    tx.rollback().unwrap();
    assert_eq!(err.code, ConversationErrorCode::ForwardTargetLimitExceeded);
}

#[test]
fn cross_account_forward_targets_rejected() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_a = seed_zalo_account(&conn, "acc-fwd-5a", None);
    let account_b = seed_zalo_account(&conn, "acc-fwd-5b", None);

    let conv_src = seed_conversation(&conn, account_a, "thread-src-5");
    let conv_ta = seed_conversation(&conn, account_a, "thread-ta-5");
    let conv_tb = seed_conversation(&conn, account_b, "thread-tb-5");

    let src_msg_id =
        seed_source_message(&conn, conv_src, Some("pmsg-fwd-105"), Some("pcli-fwd-105"));

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-cross-account".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_ta, conv_tb],
    };

    let tx = conn.transaction().unwrap();
    let err = enqueue_forward_operation(&tx, &req, &now).unwrap_err();
    tx.rollback().unwrap();
    assert_eq!(
        err.code,
        ConversationErrorCode::ForwardTargetAccountMismatch
    );
}

#[test]
fn source_missing_provider_ids_rejected() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-6", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-6");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-t6-1");

    let src_msg_no_ids = seed_source_message(&conn, conv_src, None, None);

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-missing-pids".to_string(),
        source_message_id: src_msg_no_ids,
        target_conversation_ids: vec![conv_t1],
    };

    let tx = conn.transaction().unwrap();
    let err = enqueue_forward_operation(&tx, &req, &now).unwrap_err();
    tx.rollback().unwrap();
    assert_eq!(
        err.code,
        ConversationErrorCode::ForwardSourceMissingProviderIds
    );
}

#[test]
fn transaction_rollback_on_any_target_error() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-7", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-7");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-t7-1");
    let invalid_conv_id = 999_999_i64;

    let src_msg_id =
        seed_source_message(&conn, conv_src, Some("pmsg-fwd-107"), Some("pcli-fwd-107"));

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-rollback-test".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_t1, invalid_conv_id],
    };

    let tx = conn.transaction().unwrap();
    let res = enqueue_forward_operation(&tx, &req, &now);
    assert!(res.is_err());
    tx.rollback().unwrap();

    let op_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_forward_operations WHERE client_operation_id = 'op-rollback-test'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(op_count, 0);

    let fwd_msg_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE content_type = 'forward'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(fwd_msg_count, 0);

    let batch_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_send_batches WHERE client_batch_id LIKE 'op-rollback-test:%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(batch_count, 0);
}

#[test]
fn forward_operation_conflicting_payload_rejected() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-conflict", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-c");
    let conv_src2 = seed_conversation(&conn, account_id, "thread-src-c2");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-tc-1");
    let conv_t2 = seed_conversation(&conn, account_id, "thread-tc-2");

    let src_msg1 = seed_source_message(&conn, conv_src, Some("pmsg-c1"), Some("pcli-c1"));
    let src_msg2 = seed_source_message(&conn, conv_src2, Some("pmsg-c2"), Some("pcli-c2"));

    let now = utc_now();
    let req1 = ForwardConversationMessageRequest {
        client_operation_id: "op-conflict-1".to_string(),
        source_message_id: src_msg1,
        target_conversation_ids: vec![conv_t1],
    };

    let tx1 = conn.transaction().unwrap();
    let _res1 = enqueue_forward_operation(&tx1, &req1, &now).unwrap();
    tx1.commit().unwrap();

    // Mismatched source_message_id
    let req_diff_src = ForwardConversationMessageRequest {
        client_operation_id: "op-conflict-1".to_string(),
        source_message_id: src_msg2,
        target_conversation_ids: vec![conv_t1],
    };
    let tx2 = conn.transaction().unwrap();
    let err_src = enqueue_forward_operation(&tx2, &req_diff_src, &now).unwrap_err();
    tx2.rollback().unwrap();
    assert_eq!(err_src.code, ConversationErrorCode::InvalidRequest);

    // Mismatched target_conversation_ids
    let req_diff_target = ForwardConversationMessageRequest {
        client_operation_id: "op-conflict-1".to_string(),
        source_message_id: src_msg1,
        target_conversation_ids: vec![conv_t2],
    };
    let tx3 = conn.transaction().unwrap();
    let err_t = enqueue_forward_operation(&tx3, &req_diff_target, &now).unwrap_err();
    tx3.rollback().unwrap();
    assert_eq!(err_t.code, ConversationErrorCode::InvalidRequest);
}

#[test]
fn forward_operation_inactive_target_rejected() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-inactive", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-in");
    let conv_inactive = conn
        .query_row(
            "INSERT INTO channel_conversations
             (channel_account_id, external_thread_id, kind, title, status, last_synced_at, created_at, updated_at)
             VALUES (?1, 'thread-archived', 'direct', 'Archived Chat', 'archived', ?2, ?2, ?2)
             RETURNING id",
            params![account_id, utc_now()],
            |row| row.get(0),
        )
        .unwrap();

    let src_msg_id = seed_source_message(&conn, conv_src, Some("pmsg-in"), Some("pcli-in"));

    let now = utc_now();
    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-inactive-target".to_string(),
        source_message_id: src_msg_id,
        target_conversation_ids: vec![conv_inactive],
    };

    let tx = conn.transaction().unwrap();
    let err = enqueue_forward_operation(&tx, &req, &now).unwrap_err();
    tx.rollback().unwrap();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[test]
fn forward_operation_unsupported_content_type_rejected() {
    let harness = TestHarness::new();
    let mut conn = harness.open_conn();
    let account_id = seed_zalo_account(&conn, "acc-fwd-unsupported", None);
    let conv_src = seed_conversation(&conn, account_id, "thread-src-un");
    let conv_t1 = seed_conversation(&conn, account_id, "thread-t-un");

    let now = utc_now();
    let sys_msg_id: i64 = conn
        .query_row(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id, provider_client_message_id,
              local_request_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp,
              ingested_at, sent_at, edited_at, removed_at, status, error_code, error_message, reply_to_message_id,
              send_batch_id, batch_index)
             VALUES (?1, NULL, 'inbound', 'pmsg-sys', 'pcli-sys', NULL, 'system', 'System event', '{}', NULL, ?2, ?2, NULL, NULL, NULL, 'sent', NULL, NULL, NULL, NULL, 0)
             RETURNING id",
            params![conv_src, now],
            |row| row.get(0),
        )
        .unwrap();

    let req = ForwardConversationMessageRequest {
        client_operation_id: "op-unsupported-type".to_string(),
        source_message_id: sys_msg_id,
        target_conversation_ids: vec![conv_t1],
    };

    let tx = conn.transaction().unwrap();
    let err = enqueue_forward_operation(&tx, &req, &now).unwrap_err();
    tx.rollback().unwrap();
    assert_eq!(err.code, ConversationErrorCode::ForwardNotSupported);
}
