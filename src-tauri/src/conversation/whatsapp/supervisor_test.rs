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
    migrate(&conn).unwrap();
    apply_whatsapp_migrations(&conn).unwrap();

    (db_path, temp_dir)
}

#[tokio::test]
async fn test_supervisor_lifecycle_and_client_registration() {
    let (db_path, _dir) = setup_test_supervisor_db();
    let events = Arc::new(ConversationEventPublisher::new(100));
    let supervisor = WhatsAppSupervisor::new(db_path, events);

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
    let supervisor = WhatsAppSupervisor::new(db_path, events);

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

    let supervisor = WhatsAppSupervisor::new(db_path.clone(), events);
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
    let supervisor = WhatsAppSupervisor::new(db_path, events);

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
    let supervisor = Arc::new(WhatsAppSupervisor::new(db_path, events));

    let sup_clone = supervisor.clone();
    let handle = tokio::spawn(async move {
        sup_clone.run_loop(Duration::from_millis(20)).await;
    });

    tokio::time::sleep(Duration::from_millis(60)).await;
    supervisor.stop();

    let res = tokio::time::timeout(Duration::from_millis(500), handle).await;
    assert!(res.is_ok(), "Supervisor loop did not stop within timeout");
}
