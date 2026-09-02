use std::sync::Arc;
use std::time::Duration;

use super::client::MockWhatsAppClient;
use super::migration::apply_whatsapp_migrations;
use super::supervisor::{ConnectedWhatsAppAccount, WhatsAppSupervisor};
use crate::conversation::events::ConversationEventPublisher;
use crate::conversation::migration::migrate;
use crate::conversation::repository::ConversationRepository;
use crate::conversation::service::ConversationService;
use crate::db::open_db_connection;

fn setup_test_supervisor_db() -> (String, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_wa_supervisor.db")
        .to_str()
        .unwrap()
        .to_string();

    let conn = open_db_connection(&db_path).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS leads (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, phone TEXT, zalo_uid TEXT)",
        [],
    ).unwrap();
    migrate(&conn).unwrap();
    apply_whatsapp_migrations(&conn).unwrap();

    (db_path, temp_dir)
}

#[tokio::test]
async fn test_supervisor_lifecycle_and_client_registration() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = WhatsAppSupervisor::new(db_path, sessions_dir, events);

    let mock_client = Arc::new(MockWhatsAppClient::new());
    let account_id = 101;

    assert!(supervisor.get_client(account_id).await.is_none());

    supervisor
        .register_client(account_id, mock_client.clone())
        .await;
    assert!(supervisor.get_client(account_id).await.is_some());

    supervisor.unregister_client(account_id).await;
    assert!(supervisor.get_client(account_id).await.is_none());

    assert!(!supervisor.cancellation_token().is_cancelled());
    supervisor.stop();
    assert!(supervisor.cancellation_token().is_cancelled());
}

#[test]
fn test_supervisor_get_connected_accounts() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let conn = open_db_connection(&db_path).unwrap();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = WhatsAppSupervisor::new(db_path, sessions_dir, events);

    let now = chrono::Utc::now().to_rfc3339();

    // 1. Connected & enabled account -> SHOULD be returned
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO whatsapp_sessions 
         (channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at)
         VALUES (1, '84901111111', '84901111111', 'session_1.db', 1, ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // 2. Synced & enabled account -> SHOULD be returned
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (2, 'whatsapp', '84902222222', 'Sales Rep 2', 1, 'synced', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // 3. Disconnected account -> MUST NOT be returned
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (3, 'whatsapp', '84903333333', 'Sales Rep 3', 1, 'disconnected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // 4. Disabled account -> MUST NOT be returned
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (4, 'whatsapp', '84904444444', 'Sales Rep 4', 0, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // 5. Non-whatsapp account (zalo) -> MUST NOT be returned
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (5, 'zalo', '84905555555', 'Zalo Rep', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    let connected = supervisor.get_connected_accounts(&conn).unwrap();
    assert_eq!(connected.len(), 2);
    assert_eq!(connected[0].id, 1);
    assert_eq!(connected[0].provider_account_key, "84901111111");
    assert_eq!(connected[0].phone_number.as_deref(), Some("84901111111"));
    assert_eq!(connected[1].id, 2);
    assert_eq!(connected[1].provider_account_key, "84902222222");
}

#[tokio::test]
async fn test_supervisor_sync_account_drains_outbox() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let conn = open_db_connection(&db_path).unwrap();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let repository = Arc::new(ConversationRepository::new(db_path.clone()));
    let spool_root = std::path::Path::new(&db_path)
        .parent()
        .unwrap()
        .join("spool");
    let service = Arc::new(ConversationService::new(
        repository.clone(),
        events.clone(),
        spool_root,
    ));

    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = WhatsAppSupervisor::new(db_path.clone(), sessions_dir, events);
    supervisor.with_service(service);

    let now = chrono::Utc::now().to_rfc3339();

    // Setup channel account, conversation, message, and outbox job
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (10, 'whatsapp', '84909999999', 'WA Rep', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_conversations 
         (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
         VALUES (20, 10, '84901234567@s.whatsapp.net', 'direct', 'Customer', 'active', ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages 
         (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
         VALUES (30, 20, 'outbound', 'wa_cli_1', 'text', 'Hello from supervisor test', '{}', 'queued', ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO message_outbox_jobs 
         (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
         VALUES ('job_wa_1', 10, 30, 'pending', 0, ?1, ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    let mock_client = Arc::new(MockWhatsAppClient::new());
    supervisor.register_client(10, mock_client.clone()).await;

    let account = ConnectedWhatsAppAccount {
        id: 10,
        provider_account_key: "84909999999".to_string(),
        display_name: Some("WA Rep".to_string()),
        phone_number: Some("84909999999".to_string()),
    };

    let processed = supervisor.sync_account_internal(&account).await.unwrap();
    assert_eq!(processed, 1);

    // Verify outbox job is completed
    let job_status: String = conn
        .query_row(
            "SELECT status FROM message_outbox_jobs WHERE id = 'job_wa_1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(job_status, "completed");

    let sent = mock_client.get_sent_messages();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].recipient_jid, "84901234567@s.whatsapp.net");
    assert_eq!(sent[0].text, "Hello from supervisor test");
}

#[tokio::test]
async fn test_supervisor_sync_account_handles_disconnected_client() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = WhatsAppSupervisor::new(db_path, sessions_dir, events);

    let mock_client = Arc::new(MockWhatsAppClient::new());
    mock_client.set_connected(false);

    supervisor.register_client(10, mock_client).await;

    let account = ConnectedWhatsAppAccount {
        id: 10,
        provider_account_key: "84909999999".to_string(),
        display_name: Some("WA Rep".to_string()),
        phone_number: Some("84909999999".to_string()),
    };

    let processed = supervisor.sync_account_internal(&account).await.unwrap();
    assert_eq!(processed, 0);
}

#[tokio::test]
async fn test_supervisor_run_loop_stops_on_cancellation() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = Arc::new(WhatsAppSupervisor::new(db_path, sessions_dir, events));

    let sup_clone = supervisor.clone();
    let handle = tokio::spawn(async move {
        sup_clone.run_loop(Duration::from_millis(20)).await;
    });

    tokio::time::sleep(Duration::from_millis(60)).await;
    supervisor.stop();

    let res = tokio::time::timeout(Duration::from_millis(500), handle).await;
    assert!(res.is_ok(), "Supervisor loop did not stop within timeout");
}

#[tokio::test]
async fn test_supervisor_load_active_sessions_preserves_connected_client() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let conn = open_db_connection(&db_path).unwrap();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = Arc::new(WhatsAppSupervisor::new(
        db_path.clone(),
        sessions_dir,
        events,
    ));

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO whatsapp_sessions 
         (channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at)
         VALUES (1, '84901111111', '84901111111', 'session_1.db', 1, ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    let mock_client = Arc::new(MockWhatsAppClient::new());
    mock_client.set_connected(true);
    supervisor.register_client(1, mock_client.clone()).await;

    let loaded = supervisor.load_active_sessions_on_startup().await.unwrap();
    assert_eq!(loaded, 1);

    // Client must still be the exact registered instance (not disconnected or replaced)
    let retrieved = supervisor.get_client(1).await.unwrap();
    assert!(retrieved.is_connected().await);
}

#[tokio::test]
async fn test_supervisor_notify_outbox_drains_immediately() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let conn = open_db_connection(&db_path).unwrap();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let repository = Arc::new(ConversationRepository::new(db_path.clone()));
    let spool_root = std::path::Path::new(&db_path)
        .parent()
        .unwrap()
        .join("spool");
    let service = Arc::new(ConversationService::new(
        repository.clone(),
        events.clone(),
        spool_root,
    ));

    let sessions_dir = _dir.path().join("wa_sessions");
    let supervisor = Arc::new(WhatsAppSupervisor::new(
        db_path.clone(),
        sessions_dir,
        events,
    ));
    supervisor.with_service(service);

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts 
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO whatsapp_sessions 
         (channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at)
         VALUES (1, '84901111111', '84901111111', 'session_1.db', 1, ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO channel_conversations 
         (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (10, 1, '84902222222@s.whatsapp.net', 'direct', ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages 
         (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
         VALUES (101, 10, 'outbound', 'out_wa_1', 'text', 'Hello Fast WhatsApp!', '{}', 'queued', ?1)",
        rusqlite::params![now],
    ).unwrap();

    conn.execute(
        "INSERT INTO message_outbox_jobs 
         (id, channel_account_id, message_id, status, attempt_count, next_attempt_at, created_at, updated_at)
         VALUES ('job_wa_fast', 1, 101, 'pending', 0, ?1, ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    let mock_client = Arc::new(MockWhatsAppClient::new());
    mock_client.set_connected(true);
    supervisor.register_client(1, mock_client.clone()).await;

    // Start background loop with a long 60-second fallback
    let sup_clone = supervisor.clone();
    let loop_handle = tokio::spawn(async move {
        sup_clone.run_loop(Duration::from_secs(60)).await;
    });

    // Notify outbox immediately
    supervisor.notify_outbox();

    // Verify outbox was processed within 1000ms without waiting for 60s
    let mut drained = false;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        let job_status: Option<String> = conn
            .query_row(
                "SELECT status FROM message_outbox_jobs WHERE id = 'job_wa_fast'",
                [],
                |r| r.get(0),
            )
            .ok();
        if job_status.as_deref() == Some("completed") {
            drained = true;
            break;
        }
    }

    supervisor.stop();
    let _ = loop_handle.await;

    assert!(
        drained,
        "Outbox job was not drained immediately upon notify_outbox"
    );
    let sent_msgs = mock_client.get_sent_messages();
    assert_eq!(sent_msgs.len(), 1);
    assert_eq!(sent_msgs[0].recipient_jid, "84902222222@s.whatsapp.net");
    assert_eq!(sent_msgs[0].text, "Hello Fast WhatsApp!");
}

#[tokio::test]
async fn test_process_and_publish_whatsapp_receipt_success_and_error_handling() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let mut rx = events.subscribe();

    // 1. Set up account, conversation, and message in DB
    let conn = open_db_connection(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (20, 1, 'direct', '84903333333@s.whatsapp.net', 'Test WA', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
         VALUES (201, 20, 'outbound', 'stanza_receipt_wa_1', 'text', 'Receipt msg', '{}', 'sent', ?1)",
        rusqlite::params![now],
    ).unwrap();

    let raw_receipt = super::normalizer::RawWhatsAppReceipt {
        message_ids: vec!["stanza_receipt_wa_1".to_string()],
        chat_jid: "84903333333@s.whatsapp.net".to_string(),
        sender_jid: Some("84903333333@s.whatsapp.net".to_string()),
        receipt_type: "read".to_string(),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
    };

    // 2. Success execution publishes InvalidationEvent
    let result =
        super::supervisor::process_and_publish_whatsapp_receipt(&db_path, 1, &raw_receipt, &events);
    assert!(result.is_ok());

    let event = rx.recv().await.unwrap();
    assert_eq!(event.conversation_id, 20);
    assert_eq!(event.message_id, Some(201));

    // 3. Error propagation on invalid DB path
    let err_result = super::supervisor::process_and_publish_whatsapp_receipt(
        "/non_existent/path/db.sqlite",
        1,
        &raw_receipt,
        &events,
    );
    assert!(err_result.is_err());
    assert!(err_result
        .unwrap_err()
        .to_string()
        .contains("failed to open sqlite database"));
}

#[tokio::test]
async fn test_whatsapp_serverack_nack_without_class_reaches_failed_and_invalidation() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let mut rx = events.subscribe();

    let conn = open_db_connection(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (25, 1, 'direct', '84903333333@s.whatsapp.net', 'Test WA NACK', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
         VALUES (2510, 25, 'outbound', 'nack_realistic_msg_1', 'text', 'Outbound msg', '{}', 'sending', ?1)",
        rusqlite::params![now],
    ).unwrap();

    // Production-realistic NACK shape from wacore/whatsapp-rust 0.7.0:
    // class is None, timestamp is None, from is server JID (s.whatsapp.net), error code is "479"
    let nack = whatsapp_rust::types::events::ServerAck::builder()
        .id("nack_realistic_msg_1".to_string())
        .from("s.whatsapp.net".parse::<whatsapp_rust::Jid>().unwrap())
        .error("479".to_string())
        .build();
    let nack_event = whatsapp_rust::types::events::Event::ServerAck(nack);

    let mapped = super::supervisor::map_whatsapp_receipt_event(&nack_event).unwrap();
    assert_eq!(mapped.receipt_type, "failed");
    assert_eq!(mapped.message_ids, vec!["nack_realistic_msg_1"]);
    assert_eq!(mapped.chat_jid, ""); // Server JID cleared to allow message-ID correlation

    let handle_res =
        super::supervisor::handle_whatsapp_receipt_event(&db_path, 1, &nack_event, &events);
    assert!(handle_res.is_some());
    assert!(handle_res.unwrap().is_ok());

    let published_event = rx.recv().await.unwrap();
    assert_eq!(published_event.conversation_id, 25);
    assert_eq!(published_event.message_id, Some(2510));

    let updated_status: String = conn
        .query_row("SELECT status FROM messages WHERE id = 2510", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(updated_status, "failed");

    let receipt_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM message_receipts WHERE message_id = 2510 AND receipt_type = 'failed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(receipt_count, 1);
}

#[test]
fn test_whatsapp_serverack_negative_coverage() {
    // 1. Error ACK with non-message class (e.g. "call") -> returns None
    let call_ack = whatsapp_rust::types::events::ServerAck::builder()
        .id("call_ack_1".to_string())
        .from("s.whatsapp.net".parse::<whatsapp_rust::Jid>().unwrap())
        .class("call".to_string())
        .error("500".to_string())
        .build();
    let ev1 = whatsapp_rust::types::events::Event::ServerAck(call_ack);
    assert!(super::supervisor::map_whatsapp_receipt_event(&ev1).is_none());

    // 2. Success ACK with non-message class (e.g. "notification") -> returns None
    let notif_ack = whatsapp_rust::types::events::ServerAck::builder()
        .id("notif_ack_1".to_string())
        .from("s.whatsapp.net".parse::<whatsapp_rust::Jid>().unwrap())
        .class("notification".to_string())
        .build();
    let ev2 = whatsapp_rust::types::events::Event::ServerAck(notif_ack);
    assert!(super::supervisor::map_whatsapp_receipt_event(&ev2).is_none());

    // 3. ServerAck with empty ID -> returns None
    let empty_id_ack = whatsapp_rust::types::events::ServerAck::builder()
        .id("".to_string())
        .class("message".to_string())
        .build();
    let ev3 = whatsapp_rust::types::events::Event::ServerAck(empty_id_ack);
    assert!(super::supervisor::map_whatsapp_receipt_event(&ev3).is_none());
}

#[tokio::test]
async fn test_whatsapp_receipt_listener_event_handling_logging_and_liveness() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let mut rx = events.subscribe();

    let conn = open_db_connection(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (25, 1, 'direct', '84903333333@s.whatsapp.net', 'Test WA Liveness', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
         VALUES (2501, 25, 'outbound', 'stanza_ack_live_1', 'text', 'Ack msg', '{}', 'sending', ?1)",
        rusqlite::params![now],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, content_json, status, ingested_at)
         VALUES (2502, 25, 'outbound', 'stanza_ack_live_2', 'text', 'Ack msg 2', '{}', 'sending', ?1)",
        rusqlite::params![now],
    ).unwrap();

    // 1. First event: ServerAck message with error (failed receipt)
    let fail_ack = whatsapp_rust::types::events::ServerAck::builder()
        .id("stanza_ack_live_1".to_string())
        .from(
            "84903333333@s.whatsapp.net"
                .parse::<whatsapp_rust::Jid>()
                .unwrap(),
        )
        .class("message".to_string())
        .error("500".to_string())
        .timestamp(chrono::Utc::now())
        .build();
    let fail_event = whatsapp_rust::types::events::Event::ServerAck(fail_ack);

    // Verify map_whatsapp_receipt_event maps error ack to "failed"
    let mapped_fail = super::supervisor::map_whatsapp_receipt_event(&fail_event).unwrap();
    assert_eq!(mapped_fail.receipt_type, "failed");
    assert_eq!(mapped_fail.message_ids, vec!["stanza_ack_live_1"]);

    // Process first event with an invalid DB path to simulate error logging path
    let fail_handle_res = super::supervisor::handle_whatsapp_receipt_event(
        "/non_existent/path/corrupted.sqlite",
        1,
        &fail_event,
        &events,
    );
    assert!(fail_handle_res.is_some());
    assert!(fail_handle_res.unwrap().is_err());

    // 2. Second event: Sequential execution on the same listener handler
    let success_ack = whatsapp_rust::types::events::ServerAck::builder()
        .id("stanza_ack_live_2".to_string())
        .from(
            "84903333333@s.whatsapp.net"
                .parse::<whatsapp_rust::Jid>()
                .unwrap(),
        )
        .class("message".to_string())
        .timestamp(chrono::Utc::now())
        .build();
    let success_event = whatsapp_rust::types::events::Event::ServerAck(success_ack);

    let mapped_success = super::supervisor::map_whatsapp_receipt_event(&success_event).unwrap();
    assert_eq!(mapped_success.receipt_type, "sent");

    // Process second event on valid DB path using the exact same handler interface
    let success_handle_res =
        super::supervisor::handle_whatsapp_receipt_event(&db_path, 1, &success_event, &events);
    assert!(success_handle_res.is_some());
    assert!(success_handle_res.unwrap().is_ok());

    // Verify invalidation event was published for msg 2502
    let published_event = rx.recv().await.unwrap();
    assert_eq!(published_event.conversation_id, 25);
    assert_eq!(published_event.message_id, Some(2502));

    let updated_status: String = conn
        .query_row("SELECT status FROM messages WHERE id = 2502", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(updated_status, "sent");
}

#[test]
fn test_map_whatsapp_receipt_event_presence_and_server_error() {
    let chat_jid: whatsapp_rust::Jid = "84903333333@s.whatsapp.net".parse().unwrap();
    let sender_jid: whatsapp_rust::Jid = "84903333333@s.whatsapp.net".parse().unwrap();

    // 1. ReceiptType::ServerError -> maps to "failed"
    let server_error_receipt = whatsapp_rust::types::events::Receipt::builder()
        .source(whatsapp_rust::types::message::MessageSource {
            chat: chat_jid.clone(),
            sender: sender_jid.clone(),
            ..Default::default()
        })
        .message_ids(vec!["msg_server_err_1".into()])
        .timestamp(chrono::Utc::now())
        .r#type(whatsapp_rust::types::presence::ReceiptType::ServerError)
        .offline(false)
        .build();
    let ev1 = whatsapp_rust::types::events::Event::Receipt(server_error_receipt);
    let mapped1 = super::supervisor::map_whatsapp_receipt_event(&ev1).unwrap();
    assert_eq!(mapped1.receipt_type, "failed");
    assert_eq!(mapped1.message_ids, vec!["msg_server_err_1"]);

    // 2. ReceiptType::Read -> maps to "read"
    let read_receipt = whatsapp_rust::types::events::Receipt::builder()
        .source(whatsapp_rust::types::message::MessageSource {
            chat: chat_jid.clone(),
            sender: sender_jid.clone(),
            ..Default::default()
        })
        .message_ids(vec!["msg_read_1".into()])
        .timestamp(chrono::Utc::now())
        .r#type(whatsapp_rust::types::presence::ReceiptType::Read)
        .offline(false)
        .build();
    let ev2 = whatsapp_rust::types::events::Event::Receipt(read_receipt);
    let mapped2 = super::supervisor::map_whatsapp_receipt_event(&ev2).unwrap();
    assert_eq!(mapped2.receipt_type, "read");
    assert_eq!(mapped2.message_ids, vec!["msg_read_1"]);

    // 3. ReceiptType::Delivered -> maps to "delivered"
    let del_receipt = whatsapp_rust::types::events::Receipt::builder()
        .source(whatsapp_rust::types::message::MessageSource {
            chat: chat_jid.clone(),
            sender: sender_jid.clone(),
            ..Default::default()
        })
        .message_ids(vec!["msg_del_1".into()])
        .timestamp(chrono::Utc::now())
        .r#type(whatsapp_rust::types::presence::ReceiptType::Delivered)
        .offline(false)
        .build();
    let ev3 = whatsapp_rust::types::events::Event::Receipt(del_receipt);
    let mapped3 = super::supervisor::map_whatsapp_receipt_event(&ev3).unwrap();
    assert_eq!(mapped3.receipt_type, "delivered");
    assert_eq!(mapped3.message_ids, vec!["msg_del_1"]);
}

#[tokio::test]
async fn test_whatsapp_inbound_message_publishes_delta_event() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let sessions_dir = _dir.path().join("wa_sessions");
    let _supervisor = Arc::new(WhatsAppSupervisor::new(db_path.clone(), sessions_dir, events.clone()));

    let mut conn = open_db_connection(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', 1, 'connected', '{}', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    let raw_msg = crate::conversation::whatsapp::normalizer::RawWhatsAppMessage {
        id: "wa_inbound_msg_001".to_string(),
        chat_jid: "84902222222@s.whatsapp.net".to_string(),
        sender_jid: Some("84902222222@s.whatsapp.net".to_string()),
        sender_name: Some("Khach Hang WA".to_string()),
        text: Some("Chao shop qua WhatsApp".to_string()),
        timestamp: Some(now.clone()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
        mentions: vec![],
    };

    let applied = crate::conversation::whatsapp::normalizer::WhatsAppNormalizer::ingest_incoming_message(
        &mut conn,
        1,
        &raw_msg,
    ).unwrap().unwrap();

    let delta = applied.delta_event.expect("Delta must be generated inside the transaction");
    assert_eq!(delta.channel, crate::conversation::types::Channel::WhatsApp);
    assert_eq!(delta.message.text_content.as_deref(), Some("Chao shop qua WhatsApp"));
    assert!(delta.sequence > 0);

    let repo = ConversationRepository::new(db_path);
    let deltas = repo.fetch_deltas_after_sequence(0, 10).unwrap();
    assert_eq!(deltas.len(), 1);
    assert_eq!(deltas[0].sequence, delta.sequence);
    assert_eq!(deltas[0].message.id, applied.message_id);
}

#[tokio::test]
async fn test_set_whatsapp_conversation_reaction_dm_matrix() {
    use crate::conversation::types::{
        ConversationCapabilities, ConversationReactionCode, SetConversationReactionRequest,
    };

    let (db_path, _dir) = setup_test_supervisor_db();
    let conn = open_db_connection(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let cap_json = serde_json::to_string(&ConversationCapabilities::whatsapp()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, self_external_id, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', '84901111111', 1, 'connected', ?1, ?2, ?2)",
        rusqlite::params![cap_json, now],
    ).unwrap();

    // 1. Direct conversation with raw phone thread ID (without @s.whatsapp.net)
    conn.execute(
        "INSERT INTO channel_conversations
         (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
         VALUES (10, 1, '84949743369', 'direct', 'Thao', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // Outbound message (from me, like in user screenshot)
    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, direction, provider_message_id, content_type, text_content, status, ingested_at)
         VALUES (101, 10, 'outbound', 'wamid_outbound_001', 'text', 'tesst nua', 'sent', ?1)",
        rusqlite::params![now],
    ).unwrap();

    // Inbound message (from customer)
    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, direction, provider_message_id, content_type, text_content, status, ingested_at)
         VALUES (102, 10, 'inbound', 'wamid_inbound_002', 'text', 'ok', 'received', ?1)",
        rusqlite::params![now],
    ).unwrap();

    let publisher = Arc::new(ConversationEventPublisher::new(100));
    let repo = Arc::new(ConversationRepository::new(db_path.clone()));
    let service = ConversationService::new(repo, publisher, _dir.path().join("spool"));
    let mock_client = MockWhatsAppClient::new();

    // A. React LIKE to outbound message in DM
    let req_out = SetConversationReactionRequest {
        message_id: 101,
        reaction: ConversationReactionCode::Like,
        active: true,
        count: None,
        client_mutation_id: "mut-wa-out-001".to_string(),
    };
    let res_out = service
        .set_whatsapp_conversation_reaction(&req_out, &mock_client)
        .await
        .unwrap();

    assert_eq!(res_out.status, crate::conversation::types::ReactionMutationStatus::Succeeded);
    let sent = mock_client.get_sent_reactions();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chat_jid, "84949743369@s.whatsapp.net");
    assert_eq!(sent[0].target_message_id, "wamid_outbound_001");
    assert_eq!(sent[0].target_sender_jid, None);
    assert_eq!(sent[0].emoji, "👍");
    assert!(sent[0].is_from_me);

    // B. React HEART to inbound message in DM
    let req_in = SetConversationReactionRequest {
        message_id: 102,
        reaction: ConversationReactionCode::Heart,
        active: true,
        count: None,
        client_mutation_id: "mut-wa-in-002".to_string(),
    };
    let res_in = service
        .set_whatsapp_conversation_reaction(&req_in, &mock_client)
        .await
        .unwrap();

    assert_eq!(res_in.status, crate::conversation::types::ReactionMutationStatus::Succeeded);
    let sent = mock_client.get_sent_reactions();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].chat_jid, "84949743369@s.whatsapp.net");
    assert_eq!(sent[1].target_message_id, "wamid_inbound_002");
    assert_eq!(sent[1].target_sender_jid, None);
    assert_eq!(sent[1].emoji, "❤️");
    assert!(!sent[1].is_from_me);

    // C. Remove reaction (active = false)
    let req_unreact = SetConversationReactionRequest {
        message_id: 101,
        reaction: ConversationReactionCode::Like,
        active: false,
        count: None,
        client_mutation_id: "mut-wa-out-003".to_string(),
    };
    let res_unreact = service
        .set_whatsapp_conversation_reaction(&req_unreact, &mock_client)
        .await
        .unwrap();

    assert_eq!(res_unreact.status, crate::conversation::types::ReactionMutationStatus::Succeeded);
    let sent = mock_client.get_sent_reactions();
    assert_eq!(sent.len(), 3);
    assert_eq!(sent[2].emoji, "");
}

#[tokio::test]
async fn test_set_whatsapp_conversation_reaction_group_matrix() {
    use crate::conversation::types::{
        ConversationCapabilities, ConversationReactionCode, SetConversationReactionRequest,
    };

    let (db_path, _dir) = setup_test_supervisor_db();
    let conn = open_db_connection(&db_path).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    let cap_json = serde_json::to_string(&ConversationCapabilities::whatsapp()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts
         (id, channel, provider_account_key, display_name, self_external_id, enabled, connection_status, capabilities_json, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901111111', 'Sales Rep 1', '84901111111', 1, 'connected', ?1, ?2, ?2)",
        rusqlite::params![cap_json, now],
    ).unwrap();

    // Group conversation
    conn.execute(
        "INSERT INTO channel_conversations
         (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at)
         VALUES (20, 1, '120363123456789@g.us', 'group', 'AI Community', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // Group sender identity with raw phone number (no @s.whatsapp.net in external_id)
    conn.execute(
        "INSERT INTO channel_identities
         (id, channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (301, 'whatsapp', 1, 'phone', '84903333333', 'Member Alex', ?1, ?1)",
        rusqlite::params![now],
    ).unwrap();

    // Inbound group message from Member Alex
    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, sender_identity_id, direction, provider_message_id, content_type, text_content, status, ingested_at)
         VALUES (201, 20, 301, 'inbound', 'wamid_grp_in_001', 'text', 'Hello group', 'received', ?1)",
        rusqlite::params![now],
    ).unwrap();

    // Outbound group message from me
    conn.execute(
        "INSERT INTO messages
         (id, conversation_id, direction, provider_message_id, content_type, text_content, status, ingested_at)
         VALUES (202, 20, 'outbound', 'wamid_grp_out_002', 'text', 'Welcome', 'sent', ?1)",
        rusqlite::params![now],
    ).unwrap();

    let publisher = Arc::new(ConversationEventPublisher::new(100));
    let repo = Arc::new(ConversationRepository::new(db_path.clone()));
    let service = ConversationService::new(repo, publisher, _dir.path().join("spool"));
    let mock_client = MockWhatsAppClient::new();

    // A. React HAHA to inbound group message -> target_sender_jid MUST be normalized to 84903333333@s.whatsapp.net
    let req_in = SetConversationReactionRequest {
        message_id: 201,
        reaction: ConversationReactionCode::Haha,
        active: true,
        count: None,
        client_mutation_id: "mut-wa-grp-001".to_string(),
    };
    let res_in = service
        .set_whatsapp_conversation_reaction(&req_in, &mock_client)
        .await
        .unwrap();

    assert_eq!(res_in.status, crate::conversation::types::ReactionMutationStatus::Succeeded);
    let sent = mock_client.get_sent_reactions();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].chat_jid, "120363123456789@g.us");
    assert_eq!(sent[0].target_message_id, "wamid_grp_in_001");
    assert_eq!(sent[0].target_sender_jid, Some("84903333333@s.whatsapp.net".to_string()));
    assert_eq!(sent[0].emoji, "😂");
    assert!(!sent[0].is_from_me);

    // B. React WOW to outbound group message -> target_sender_jid MUST be None
    let req_out = SetConversationReactionRequest {
        message_id: 202,
        reaction: ConversationReactionCode::Wow,
        active: true,
        count: None,
        client_mutation_id: "mut-wa-grp-002".to_string(),
    };
    let res_out = service
        .set_whatsapp_conversation_reaction(&req_out, &mock_client)
        .await
        .unwrap();

    assert_eq!(res_out.status, crate::conversation::types::ReactionMutationStatus::Succeeded);
    let sent = mock_client.get_sent_reactions();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1].chat_jid, "120363123456789@g.us");
    assert_eq!(sent[1].target_message_id, "wamid_grp_out_002");
    assert_eq!(sent[1].target_sender_jid, None);
    assert_eq!(sent[1].emoji, "😮");
    assert!(sent[1].is_from_me);
}

