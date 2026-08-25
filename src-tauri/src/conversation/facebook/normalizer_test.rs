use super::client::*;
use super::normalizer::*;
use rusqlite::Connection;

fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            phone TEXT,
            about TEXT
        );
        CREATE TABLE channel_accounts (
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
        CREATE TABLE channel_identities (
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
        CREATE UNIQUE INDEX idx_channel_identities_page_unique
        ON channel_identities(channel, channel_account_id, identity_namespace, external_id)
        WHERE channel_account_id IS NOT NULL;

        CREATE TABLE channel_conversations (
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

        CREATE TABLE conversation_participants (
            conversation_id INTEGER NOT NULL,
            identity_id INTEGER NOT NULL,
            participant_role TEXT NOT NULL DEFAULT 'member',
            joined_at TEXT,
            left_at TEXT,
            PRIMARY KEY(conversation_id, identity_id),
            FOREIGN KEY(conversation_id) REFERENCES channel_conversations(id) ON DELETE CASCADE,
            FOREIGN KEY(identity_id) REFERENCES channel_identities(id) ON DELETE CASCADE
        );

        CREATE TABLE messages (
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
        CREATE UNIQUE INDEX idx_messages_provider_unique
        ON messages(conversation_id, provider_message_id)
        WHERE provider_message_id IS NOT NULL;

        CREATE TABLE message_attachments (
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
        CREATE UNIQUE INDEX uq_message_attachments_remote
        ON message_attachments(message_id, remote_url)
        WHERE remote_url IS NOT NULL;

        CREATE TABLE channel_sync_checkpoints (
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
        INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
        VALUES (1, 'messenger', 'page_123', 'My Fanpage', 'connected', datetime('now'), datetime('now'));"
    ).unwrap();
    conn
}

#[test]
fn test_ingest_conversations_batch_success() {
    let mut conn = create_test_db();

    let threads = vec![FacebookConversationThread {
        id: "t_100".to_string(),
        updated_time: "2026-08-24T02:00:00+0000".to_string(),
        participants: Some(FacebookParticipantsContainer {
            data: vec![
                FacebookParticipantNode {
                    id: "page_123".to_string(),
                    name: Some("My Fanpage".to_string()),
                    email: None,
                },
                FacebookParticipantNode {
                    id: "psid_456".to_string(),
                    name: Some("Tran Thi B".to_string()),
                    email: None,
                },
            ],
        }),
        messages: Some(FacebookMessagesContainer {
            data: vec![
                FacebookMessageNode {
                    id: "m_1".to_string(),
                    created_time: "2026-08-24T01:50:00+0000".to_string(),
                    from: Some(FacebookParticipantNode {
                        id: "psid_456".to_string(),
                        name: Some("Tran Thi B".to_string()),
                        email: None,
                    }),
                    to: None,
                    message: Some("Chào bạn!".to_string()),
                    attachments: None,
                },
                FacebookMessageNode {
                    id: "m_2".to_string(),
                    created_time: "2026-08-24T01:55:00+0000".to_string(),
                    from: Some(FacebookParticipantNode {
                        id: "page_123".to_string(),
                        name: Some("My Fanpage".to_string()),
                        email: None,
                    }),
                    to: None,
                    message: Some("Chào chị, shop có thể hỗ trợ gì ạ?".to_string()),
                    attachments: Some(FacebookAttachmentsContainer {
                        data: vec![FacebookAttachmentNode {
                            id: Some("att_99".to_string()),
                            mime_type: Some("image/png".to_string()),
                            name: Some("catalog.png".to_string()),
                            size: Some(2048),
                            file_url: Some("https://example.com/catalog.png".to_string()),
                            image_data: Some(FacebookImageData {
                                url: Some("https://example.com/catalog.png".to_string()),
                                preview_url: Some("https://example.com/thumb.png".to_string()),
                                max_width: Some(800),
                                max_height: Some(600),
                            }),
                            video_data: None,
                        }],
                    }),
                },
            ],
            paging: None,
        }),
    }];

    let summary =
        FacebookMessengerNormalizer::ingest_conversations_batch(&mut conn, 1, "page_123", &threads)
            .unwrap();

    assert_eq!(summary.conversations_synced, 1);
    assert_eq!(summary.messages_inserted, 2);
    assert_eq!(summary.new_messages_count, 2);

    // Verify channel_identities
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM channel_identities WHERE external_id = 'psid_456'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);

    // Verify channel_conversations
    let title: String = conn
        .query_row(
            "SELECT title FROM channel_conversations WHERE external_thread_id = 't_100'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "Tran Thi B");

    // Verify messages
    let msg_count: i64 = conn
        .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(msg_count, 2);

    let m1_dir: String = conn
        .query_row(
            "SELECT direction FROM messages WHERE provider_message_id = 'm_1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(m1_dir, "inbound");

    let m2_dir: String = conn
        .query_row(
            "SELECT direction FROM messages WHERE provider_message_id = 'm_2'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(m2_dir, "outbound");

    // Verify attachment
    let att_count: i64 = conn
        .query_row("SELECT count(*) FROM message_attachments", [], |r| r.get(0))
        .unwrap();
    assert_eq!(att_count, 1);

    // Verify checkpoint
    let cp_count: i64 = conn.query_row("SELECT count(*) FROM channel_sync_checkpoints WHERE channel_account_id = 1 AND scope = 'meta_messenger_sync'", [], |r| r.get(0)).unwrap();
    assert_eq!(cp_count, 1);

    // Idempotency: Ingesting again should not add duplicate rows
    let summary2 =
        FacebookMessengerNormalizer::ingest_conversations_batch(&mut conn, 1, "page_123", &threads)
            .unwrap();
    assert_eq!(summary2.new_messages_count, 0);

    let msg_count_after: i64 = conn
        .query_row("SELECT count(*) FROM messages", [], |r| r.get(0))
        .unwrap();
    assert_eq!(msg_count_after, 2);
}

#[test]
fn test_unresolved_participant_skipped_without_corrupting_identity() {
    let mut conn = create_test_db();

    // Thread has only the page itself as participant (e.g. self-chat or malformed thread)
    let thread_no_customer = FacebookConversationThread {
        id: "t_corrupt".to_string(),
        updated_time: "2026-08-24T02:00:00+0000".to_string(),
        participants: Some(FacebookParticipantsContainer {
            data: vec![FacebookParticipantNode {
                id: "page_123".to_string(),
                name: Some("My Fanpage".to_string()),
                email: None,
            }],
        }),
        messages: Some(FacebookMessagesContainer {
            data: vec![FacebookMessageNode {
                id: "m_corrupt_1".to_string(),
                created_time: "2026-08-24T02:00:00+0000".to_string(),
                from: Some(FacebookParticipantNode {
                    id: "page_123".to_string(),
                    name: Some("My Fanpage".to_string()),
                    email: None,
                }),
                to: None,
                message: Some("Only page in thread".to_string()),
                attachments: None,
            }],
            paging: None,
        }),
    };

    let summary = FacebookMessengerNormalizer::ingest_conversations_batch(
        &mut conn,
        1,
        "page_123",
        &[thread_no_customer],
    )
    .unwrap();

    // Should skip cleanly
    assert_eq!(summary.conversations_synced, 0);
    assert_eq!(summary.messages_inserted, 0);
    assert_eq!(summary.new_messages_count, 0);

    // Ensure NO synthetic identity or conversation was created
    let id_count: i64 = conn
        .query_row("SELECT count(*) FROM channel_identities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(id_count, 0);
    let conv_count: i64 = conn
        .query_row("SELECT count(*) FROM channel_conversations", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(conv_count, 0);
}

#[test]
fn test_fallback_customer_resolved_from_messages_when_participants_empty() {
    let mut conn = create_test_db();

    // Thread has NO participants, but messages contain customer PSID & name
    let thread = FacebookConversationThread {
        id: "t_fallback".to_string(),
        updated_time: "2026-08-24T02:00:00+0000".to_string(),
        participants: Some(FacebookParticipantsContainer {
            data: vec![], // Empty participants list from Graph API
        }),
        messages: Some(FacebookMessagesContainer {
            data: vec![FacebookMessageNode {
                id: "m_fb_1".to_string(),
                created_time: "2026-08-24T02:00:00+0000".to_string(),
                from: Some(FacebookParticipantNode {
                    id: "psid_customer_789".to_string(),
                    name: Some("Khach Hang Cu".to_string()),
                    email: None,
                }),
                to: None,
                message: Some("Gia san pham nay bao nhieu?".to_string()),
                attachments: None,
            }],
            paging: None,
        }),
    };

    let summary = FacebookMessengerNormalizer::ingest_conversations_batch(
        &mut conn,
        1,
        "page_123",
        &[thread],
    )
    .unwrap();

    assert_eq!(summary.conversations_synced, 1);
    assert_eq!(summary.messages_inserted, 1);
    assert_eq!(summary.new_messages_count, 1);

    // Verify identity was resolved from messages
    let (ext_id, display_name): (String, String) = conn
        .query_row(
            "SELECT external_id, display_name FROM channel_identities WHERE external_id = 'psid_customer_789'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ext_id, "psid_customer_789");
    assert_eq!(display_name, "Khach Hang Cu");

    // Verify conversation was created
    let title: String = conn
        .query_row(
            "SELECT title FROM channel_conversations WHERE external_thread_id = 't_fallback'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "Khach Hang Cu");
}

#[test]
fn test_ingest_webhook_event_inbound_message_delivery_and_read() {
    let mut conn = create_test_db();

    // 1. Ingest inbound message webhook event
    let msg_payload = serde_json::json!({
        "sender": { "id": "psid_999" },
        "recipient": { "id": "page_123" },
        "timestamp": 1724508965000_i64,
        "message": {
            "mid": "mid.webhook.001",
            "text": "Quận 8 e ơi"
        }
    });

    let conv_id =
        FacebookMessengerNormalizer::ingest_webhook_event(&mut conn, 1, "page_123", &msg_payload)
            .unwrap();

    assert!(conv_id.is_some());
    let cid = conv_id.unwrap();

    // Verify identity created
    let ext_id: String = conn
        .query_row(
            "SELECT external_id FROM channel_identities WHERE external_id = 'psid_999'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ext_id, "psid_999");

    // Verify message inserted
    let (text, dir, status): (String, String, String) = conn
        .query_row(
            "SELECT text_content, direction, status FROM messages WHERE provider_message_id = 'mid.webhook.001'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(text, "Quận 8 e ơi");
    assert_eq!(dir, "inbound");
    assert_eq!(status, "received");

    // 2. Insert an outbound message and apply delivery & read webhook
    conn.execute(
        "INSERT INTO messages (conversation_id, direction, provider_message_id, provider_timestamp, content_type, text_content, status, ingested_at)
         VALUES (?1, 'outbound', 'mid.out.001', '2026-08-24T00:00:00.000Z', 'text', 'Dạ shop đây ạ', 'sent', datetime('now'))",
        rusqlite::params![cid],
    ).unwrap();

    let delivery_payload = serde_json::json!({
        "sender": { "id": "psid_999" },
        "recipient": { "id": "page_123" },
        "delivery": {
            "mids": ["mid.out.001"],
            "watermark": 1724508966000_i64
        }
    });

    FacebookMessengerNormalizer::ingest_webhook_event(&mut conn, 1, "page_123", &delivery_payload)
        .unwrap();

    let out_status: String = conn
        .query_row(
            "SELECT status FROM messages WHERE provider_message_id = 'mid.out.001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(out_status, "delivered");

    let msg_time = chrono::DateTime::parse_from_rfc3339("2026-08-24T00:00:00.000Z").unwrap();
    let watermark_ms = (msg_time + chrono::Duration::seconds(2)).timestamp_millis();

    let read_payload = serde_json::json!({
        "sender": { "id": "psid_999" },
        "recipient": { "id": "page_123" },
        "read": {
            "watermark": watermark_ms
        }
    });

    FacebookMessengerNormalizer::ingest_webhook_event(&mut conn, 1, "page_123", &read_payload)
        .unwrap();

    let out_status_read: String = conn
        .query_row(
            "SELECT status FROM messages WHERE provider_message_id = 'mid.out.001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(out_status_read, "read");
}

#[test]
fn test_ingest_webhook_event_rejects_unauthorized_mismatched_page_id() {
    let mut conn = create_test_db();

    // Webhook event addressed between third-party identities (neither is page_123)
    let unauthorized_payload = serde_json::json!({
        "sender": { "id": "psid_stranger" },
        "recipient": { "id": "other_page_456" },
        "timestamp": 1724508965000_i64,
        "message": {
            "mid": "mid.hacker.001",
            "text": "Unauthorized message"
        }
    });

    let res = FacebookMessengerNormalizer::ingest_webhook_event(
        &mut conn,
        1,
        "page_123",
        &unauthorized_payload,
    )
    .unwrap();

    // Must reject cleanly (None)
    assert!(res.is_none());

    // Verify nothing written to DB
    let msg_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE provider_message_id = 'mid.hacker.001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(msg_count, 0);

    let id_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM channel_identities WHERE external_id = 'psid_stranger'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(id_count, 0);
}
