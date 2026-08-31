use async_trait::async_trait;
use rusqlite::Connection;
use std::sync::Arc;

use super::client::*;
use super::supervisor::*;
use crate::conversation::events::ConversationEventPublisher;
use crate::conversation::types::Channel;

struct MockFacebookGraphClient {
    conversations: Vec<FacebookConversationThread>,
}

#[async_trait]
impl FacebookGraphClient for MockFacebookGraphClient {
    async fn fetch_page_conversations(
        &self,
        _page_id: &str,
        _page_token: &str,
        _limit: usize,
        after: Option<&str>,
    ) -> anyhow::Result<FacebookConversationsPage> {
        if after.is_some() {
            // Return empty for second page
            Ok(FacebookConversationsPage {
                data: vec![],
                paging: None,
            })
        } else {
            Ok(FacebookConversationsPage {
                data: self.conversations.clone(),
                paging: Some(FacebookPaging {
                    cursors: Some(FacebookCursors {
                        before: None,
                        after: Some("next_cursor_page_2".to_string()),
                    }),
                    next: None,
                    previous: None,
                }),
            })
        }
    }

    async fn fetch_thread_messages(
        &self,
        _thread_id: &str,
        _page_token: &str,
        _limit: usize,
        _after: Option<&str>,
    ) -> anyhow::Result<FacebookMessagesPage> {
        Ok(FacebookMessagesPage {
            data: vec![],
            paging: None,
        })
    }

    async fn send_messenger_message(
        &self,
        _page_id: &str,
        _page_token: &str,
        recipient_psid: &str,
        _text: &str,
        _message_tag: Option<&str>,
    ) -> anyhow::Result<SendFacebookMessageResponse> {
        Ok(SendFacebookMessageResponse {
            recipient_id: recipient_psid.to_string(),
            message_id: "m_mid_mock_123".to_string(),
        })
    }
}

fn create_supervisor_test_db(db_path: &str) {
    let conn = Connection::open(db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            phone TEXT,
            about TEXT
        );
        CREATE TABLE IF NOT EXISTS channel_accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
            provider_account_key TEXT NOT NULL,
            display_name TEXT,
            avatar_url TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            connection_status TEXT NOT NULL DEFAULT 'disconnected',
            last_connected_at TEXT,
            last_error TEXT,
            capabilities_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(channel, provider_account_key)
        );
        CREATE TABLE IF NOT EXISTS channel_identities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER,
            channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
            channel_account_id INTEGER,
            identity_namespace TEXT NOT NULL,
            external_id TEXT NOT NULL,
            normalized_address TEXT,
            display_name TEXT,
            avatar_url TEXT,
            profile_synced_at TEXT,
            profile_sync_error TEXT,
            profile_sync_attempts INTEGER NOT NULL DEFAULT 0,
            next_profile_retry_at TEXT,
            link_source TEXT NOT NULL DEFAULT 'unlinked',
            link_confidence REAL,
            verified_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE SET NULL,
            FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_page_unique
        ON channel_identities(channel, channel_account_id, identity_namespace, external_id)
        WHERE channel_account_id IS NOT NULL;

        CREATE TABLE IF NOT EXISTS channel_conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel_account_id INTEGER NOT NULL,
            external_thread_id TEXT NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('direct', 'group', 'email_thread')),
            title TEXT,
            avatar_url TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            last_message_at TEXT,
            last_synced_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(channel_account_id, external_thread_id, kind),
            FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS conversation_participants (
            conversation_id INTEGER NOT NULL,
            identity_id INTEGER NOT NULL,
            participant_role TEXT NOT NULL DEFAULT 'member',
            joined_at TEXT,
            left_at TEXT,
            PRIMARY KEY(conversation_id, identity_id),
            FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
            FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL,
            sender_identity_id INTEGER,
            direction TEXT NOT NULL CHECK (direction IN ('inbound', 'outbound', 'system')),
            provider_message_id TEXT,
            provider_client_message_id TEXT,
            local_request_id TEXT,
            content_type TEXT NOT NULL,
            text_content TEXT,
            content_json TEXT NOT NULL DEFAULT '{}',
            raw_payload_json TEXT,
            provider_timestamp TEXT,
            ingested_at TEXT NOT NULL,
            sent_at TEXT,
            edited_at TEXT,
            removed_at TEXT,
            status TEXT NOT NULL DEFAULT 'received',
            error_code TEXT,
            error_message TEXT,
            reply_to_message_id INTEGER,
            reply_sender_identity_id INTEGER,
            FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
            FOREIGN KEY(sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL,
            FOREIGN KEY(reply_to_message_id) REFERENCES messages(id) ON DELETE SET NULL,
            FOREIGN KEY(reply_sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_provider_unique
        ON messages(conversation_id, provider_message_id)
        WHERE provider_message_id IS NOT NULL;

        CREATE TABLE IF NOT EXISTS message_attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            provider_attachment_id TEXT,
            attachment_type TEXT NOT NULL,
            file_name TEXT,
            mime_type TEXT,
            size_bytes INTEGER,
            remote_url TEXT,
            local_path TEXT,
            thumbnail_url TEXT,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
        );
        CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_remote
        ON message_attachments(message_id, remote_url)
        WHERE remote_url IS NOT NULL;

        CREATE TABLE IF NOT EXISTS channel_sync_checkpoints (
            channel_account_id INTEGER NOT NULL,
            scope TEXT NOT NULL,
            cursor TEXT,
            last_event_at TEXT,
            last_success_at TEXT,
            reconnect_count INTEGER NOT NULL DEFAULT 0,
            last_error TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY(channel_account_id, scope),
            FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS facebook_page_tokens (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel_account_id INTEGER NOT NULL,
            page_id TEXT NOT NULL UNIQUE,
            access_token_encrypted TEXT NOT NULL,
            token_status TEXT NOT NULL DEFAULT 'valid' CHECK (token_status IN ('valid', 'invalid', 'expired', 'revoked')),
            granted_scopes_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );"
    ).unwrap();
    crate::conversation::migration::migrate(&conn).unwrap();
}

#[tokio::test]
async fn test_supervisor_sync_all_with_connected_page() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_fb_sup.db")
        .to_str()
        .unwrap()
        .to_string();
    create_supervisor_test_db(&db_path);

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'page_sup_123', 'Supervisor Fanpage', 'connected', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    // Store encrypted token into vault
    crate::facebook::vault::FacebookTokenVault::encrypt_and_upsert_page_token(
        &conn,
        1,
        "page_sup_123",
        "mock_valid_token_value",
        &["pages_messaging".to_string()],
    )
    .unwrap();

    let events = Arc::new(ConversationEventPublisher::new(64));
    let mut rx = events.subscribe();

    let mock_conversations = vec![FacebookConversationThread {
        id: "t_sup_100".to_string(),
        updated_time: "2026-08-24T02:00:00+0000".to_string(),
        participants: Some(FacebookParticipantsContainer {
            data: vec![
                FacebookParticipantNode {
                    id: "page_sup_123".to_string(),
                    name: Some("Supervisor Fanpage".to_string()),
                    email: None,
                },
                FacebookParticipantNode {
                    id: "psid_customer_789".to_string(),
                    name: Some("Customer Seven".to_string()),
                    email: None,
                },
            ],
        }),
        messages: Some(FacebookMessagesContainer {
            data: vec![FacebookMessageNode {
                id: "m_sup_001".to_string(),
                created_time: "2026-08-24T02:00:00+0000".to_string(),
                from: Some(FacebookParticipantNode {
                    id: "psid_customer_789".to_string(),
                    name: Some("Customer Seven".to_string()),
                    email: None,
                }),
                to: None,
                message: Some("Tin nhắn test từ khách".to_string()),
                attachments: None,
            }],
            paging: None,
        }),
    }];

    let client = Arc::new(MockFacebookGraphClient {
        conversations: mock_conversations,
    });

    let supervisor = FacebookMessengerSupervisor::new(db_path.clone(), events.clone(), client);
    let new_count = supervisor.sync_all().await.unwrap();

    assert_eq!(new_count, 1);

    // Verify invalidation event received
    let event = rx.recv().await.unwrap();
    assert_eq!(event.channel, Channel::Messenger);

    // Verify message inserted in DB
    let conn_verify = Connection::open(&db_path).unwrap();
    let count: i64 = conn_verify
        .query_row(
            "SELECT count(*) FROM messages WHERE provider_message_id = 'm_sup_001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);

    let title: String = conn_verify
        .query_row(
            "SELECT title FROM channel_conversations WHERE external_thread_id = 't_sup_100'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "Customer Seven");
}

#[tokio::test]
async fn test_supervisor_background_loop_cancellation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_fb_sup_cancel.db")
        .to_str()
        .unwrap()
        .to_string();
    create_supervisor_test_db(&db_path);

    let events = Arc::new(ConversationEventPublisher::new(64));
    let client = Arc::new(MockFacebookGraphClient {
        conversations: vec![],
    });

    let supervisor = Arc::new(FacebookMessengerSupervisor::new(db_path, events, client));
    let handle = supervisor
        .clone()
        .start_background_loop(std::time::Duration::from_millis(50));

    // Cancel loop
    supervisor.stop();

    // The handle should resolve promptly
    let res = tokio::time::timeout(std::time::Duration::from_secs(1), handle).await;
    assert!(res.is_ok());
}

struct FailingMockFacebookGraphClient {
    error_message: String,
}

#[async_trait]
impl FacebookGraphClient for FailingMockFacebookGraphClient {
    async fn fetch_page_conversations(
        &self,
        _page_id: &str,
        _page_token: &str,
        _limit: usize,
        _after: Option<&str>,
    ) -> anyhow::Result<FacebookConversationsPage> {
        Err(anyhow::anyhow!("{}", self.error_message))
    }

    async fn fetch_thread_messages(
        &self,
        _thread_id: &str,
        _page_token: &str,
        _limit: usize,
        _after: Option<&str>,
    ) -> anyhow::Result<FacebookMessagesPage> {
        Err(anyhow::anyhow!("{}", self.error_message))
    }

    async fn send_messenger_message(
        &self,
        _page_id: &str,
        _page_token: &str,
        _recipient_psid: &str,
        _text: &str,
        _message_tag: Option<&str>,
    ) -> anyhow::Result<SendFacebookMessageResponse> {
        Err(anyhow::anyhow!("{}", self.error_message))
    }
}

#[tokio::test]
async fn test_supervisor_sync_records_token_expired_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_fb_sup_err.db")
        .to_str()
        .unwrap()
        .to_string();
    create_supervisor_test_db(&db_path);

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'page_sup_err_123', 'Error Fanpage', 'connected', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    crate::facebook::vault::FacebookTokenVault::encrypt_and_upsert_page_token(
        &conn,
        1,
        "page_sup_err_123",
        "mock_valid_token_value",
        &["pages_messaging".to_string()],
    )
    .unwrap();

    let events = Arc::new(ConversationEventPublisher::new(64));
    let client = Arc::new(FailingMockFacebookGraphClient {
        error_message: "Meta Graph API error (code 190): OAuthException Error validating access token: Session has expired".to_string(),
    });

    let supervisor = FacebookMessengerSupervisor::new(db_path.clone(), events, client);
    let sync_res = supervisor.sync_all().await;
    assert!(sync_res.is_ok()); // sync_all logs errors per account and returns total new

    let conn_verify = Connection::open(&db_path).unwrap();
    let last_error: Option<String> = conn_verify
        .query_row(
            "SELECT last_error FROM channel_accounts WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(last_error.is_some());
    assert!(last_error.unwrap().contains("OAuthException"));

    let connection_status: String = conn_verify
        .query_row(
            "SELECT connection_status FROM channel_accounts WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(connection_status, "invalid");

    let token_status: String = conn_verify
        .query_row(
            "SELECT token_status FROM facebook_page_tokens WHERE channel_account_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(token_status, "expired");
}

#[tokio::test]
async fn test_supervisor_sync_transient_error_does_not_expire_token() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_fb_sup_transient.db")
        .to_str()
        .unwrap()
        .to_string();
    create_supervisor_test_db(&db_path);

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'page_sup_trans_123', 'Transient Fanpage', 'connected', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    crate::facebook::vault::FacebookTokenVault::encrypt_and_upsert_page_token(
        &conn,
        1,
        "page_sup_trans_123",
        "mock_valid_token_value",
        &["pages_messaging".to_string()],
    )
    .unwrap();

    let events = Arc::new(ConversationEventPublisher::new(64));
    // Meta returns 500 with OAuthException, code: 2, is_transient: true
    let client = Arc::new(FailingMockFacebookGraphClient {
        error_message: "Meta Graph API error (code 2): OAuthException An unexpected error has occurred. Please retry your request later. is_transient: true".to_string(),
    });

    let supervisor = FacebookMessengerSupervisor::new(db_path.clone(), events, client);
    let sync_res = supervisor.sync_all().await;
    assert!(sync_res.is_ok());

    let conn_verify = Connection::open(&db_path).unwrap();
    let last_error: Option<String> = conn_verify
        .query_row(
            "SELECT last_error FROM channel_accounts WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(last_error.is_some());

    // Token status MUST remain 'valid', NOT 'expired'
    let token_status: String = conn_verify
        .query_row(
            "SELECT token_status FROM facebook_page_tokens WHERE channel_account_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(token_status, "valid");
}

#[tokio::test]
async fn test_supervisor_ingest_webhook_events_emits_invalidation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_fb_sup_ingest.db")
        .to_str()
        .unwrap()
        .to_string();
    create_supervisor_test_db(&db_path);

    let conn = Connection::open(&db_path).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'page_sup_ingest_123', 'Ingest Fanpage', 'connected', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let events = Arc::new(ConversationEventPublisher::new(64));
    let mut rx = events.subscribe();
    let mut delta_rx = events.subscribe_delta();
    let client = Arc::new(MockFacebookGraphClient {
        conversations: vec![],
    });

    let supervisor = FacebookMessengerSupervisor::new(db_path.clone(), events, client);

    let webhook_payload = serde_json::json!({
        "sender": { "id": "psid_live_123" },
        "recipient": { "id": "page_sup_ingest_123" },
        "timestamp": 1724508965000_i64,
        "message": {
            "mid": "mid.live.001",
            "text": "Quận 8 e ơi"
        }
    });

    let count = supervisor
        .ingest_webhook_events("page_sup_ingest_123", &[webhook_payload])
        .unwrap();
    assert_eq!(count, 1);

    // Invalidation event must be emitted
    let event = rx.try_recv().expect("Expected invalidation event");
    assert_eq!(
        event.channel,
        crate::conversation::types::Channel::Messenger
    );

    // Delta event must also be emitted with durable sequence
    let delta = delta_rx.try_recv().expect("Expected delta event");
    assert_eq!(delta.channel, crate::conversation::types::Channel::Messenger);
    assert_eq!(delta.message.text_content.as_deref(), Some("Quận 8 e ơi"));
    assert!(delta.sequence > 0);
}

#[tokio::test]
async fn test_supervisor_ingest_webhook_events_rejects_unregistered_or_disabled_page() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir
        .path()
        .join("test_fb_sup_disabled.db")
        .to_str()
        .unwrap()
        .to_string();
    create_supervisor_test_db(&db_path);

    let conn = Connection::open(&db_path).unwrap();
    // Insert disabled channel account
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, enabled, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'page_disabled_123', 'Disabled Fanpage', 0, 'disconnected', datetime('now'), datetime('now'))",
        [],
    ).unwrap();

    let events = Arc::new(ConversationEventPublisher::new(64));
    let client = Arc::new(MockFacebookGraphClient {
        conversations: vec![],
    });

    let supervisor = FacebookMessengerSupervisor::new(db_path.clone(), events, client);

    let webhook_payload = serde_json::json!({
        "sender": { "id": "psid_live_123" },
        "recipient": { "id": "page_disabled_123" },
        "timestamp": 1724508965000_i64,
        "message": {
            "mid": "mid.disabled.001",
            "text": "Hello"
        }
    });

    // Ingest for disabled account returns 0 and writes nothing
    let count = supervisor
        .ingest_webhook_events("page_disabled_123", &[webhook_payload.clone()])
        .unwrap();
    assert_eq!(count, 0);

    // Ingest for completely unregistered page returns 0
    let count_unregistered = supervisor
        .ingest_webhook_events("page_unregistered_999", &[webhook_payload])
        .unwrap();
    assert_eq!(count_unregistered, 0);
}
