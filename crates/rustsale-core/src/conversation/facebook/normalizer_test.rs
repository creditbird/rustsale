use super::client::*;
use super::normalizer::*;
use rusqlite::Connection;

fn create_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            phone TEXT,
            about TEXT,
            zalo_uid TEXT
        )",
        [],
    )
    .unwrap();
    crate::conversation::migration::migrate(&conn).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, connection_status, created_at, updated_at)
         VALUES (1, 'messenger', 'page_123', 'My Fanpage', 'connected', datetime('now'), datetime('now'))",
        [],
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
    assert_eq!(summary.delta_events.len(), 2);
    assert_eq!(summary.delta_events[0].sequence, 1);
    assert_eq!(
        summary.delta_events[0].message.text_content,
        Some("Chào bạn!".to_string())
    );
    assert_eq!(summary.delta_events[1].sequence, 2);
    assert_eq!(
        summary.delta_events[1].message.text_content,
        Some("Chào chị, shop có thể hỗ trợ gì ạ?".to_string())
    );
    assert_eq!(summary.delta_events[1].message.attachments.len(), 1);
    assert_eq!(
        summary.delta_events[1].message.attachments[0]
            .file_name
            .as_deref(),
        Some("catalog.png")
    );

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
    let cid = conv_id.unwrap().conversation_id;

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

#[test]
fn test_facebook_normalizer_delta_fault_injection_rolls_back_atomically() {
    let mut conn = create_test_db();

    // 1. Install trigger to force delta failure
    conn.execute_batch(
        "CREATE TRIGGER fail_delta_trigger_fb BEFORE INSERT ON conversation_deltas
         BEGIN
             SELECT RAISE(ABORT, 'Fault injection: delta insert failure');
         END;",
    )
    .unwrap();

    let payload = serde_json::json!({
        "sender": { "id": "psid_customer_fault" },
        "recipient": { "id": "page_123" },
        "timestamp": 1724508965000_i64,
        "message": {
            "mid": "mid.fault.fb.001",
            "text": "Fault message should be rolled back"
        }
    });

    let res = FacebookMessengerNormalizer::ingest_webhook_event(&mut conn, 1, "page_123", &payload);
    assert!(res.is_err(), "Must fail-closed when delta fails");

    // Verify messages table has no entry
    let msg_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE provider_message_id = 'mid.fault.fb.001'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(msg_count, 0);

    let delta_count: i64 = conn
        .query_row("SELECT count(*) FROM conversation_deltas", [], |r| r.get(0))
        .unwrap();
    assert_eq!(delta_count, 0);
}

// ---- Regression: repeated "UNIQUE constraint failed: message_attachments.message_id,
// message_attachments.provider_attachment_id" (v2 production log, 10-21/11/2026).
// Facebook's CDN URLs are signed and change between polls, so re-ingesting an
// already stored message used to miss the `(message_id, remote_url)` conflict target
// and trip the `(message_id, provider_attachment_id)` unique index instead. The
// error aborted the whole batch transaction, so the poll failed identically forever.

fn attachment_node(id: &str, url: &str) -> FacebookAttachmentNode {
    FacebookAttachmentNode {
        id: Some(id.to_string()),
        mime_type: Some("image/jpeg".to_string()),
        name: Some("photo.jpg".to_string()),
        size: Some(1024),
        file_url: Some(url.to_string()),
        image_data: Some(FacebookImageData {
            url: Some(url.to_string()),
            preview_url: Some(format!("{url}&thumb=1")),
            max_width: Some(800),
            max_height: Some(600),
        }),
        video_data: None,
    }
}

fn thread_with_attachments(atts: Vec<FacebookAttachmentNode>) -> Vec<FacebookConversationThread> {
    vec![FacebookConversationThread {
        id: "t_att".to_string(),
        updated_time: "2026-11-20T02:00:00+0000".to_string(),
        participants: Some(FacebookParticipantsContainer {
            data: vec![
                FacebookParticipantNode {
                    id: "page_123".to_string(),
                    name: Some("My Fanpage".to_string()),
                    email: None,
                },
                FacebookParticipantNode {
                    id: "psid_att".to_string(),
                    name: Some("Khach".to_string()),
                    email: None,
                },
            ],
        }),
        messages: Some(FacebookMessagesContainer {
            data: vec![FacebookMessageNode {
                id: "m_att".to_string(),
                created_time: "2026-11-20T01:50:00+0000".to_string(),
                from: Some(FacebookParticipantNode {
                    id: "psid_att".to_string(),
                    name: Some("Khach".to_string()),
                    email: None,
                }),
                to: None,
                message: None,
                attachments: Some(FacebookAttachmentsContainer { data: atts }),
            }],
            paging: None,
        }),
    }]
}

fn attachment_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM message_attachments", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn test_resync_with_rotated_attachment_url_is_idempotent() {
    let mut conn = create_test_db();

    let first = thread_with_attachments(vec![attachment_node(
        "att_1",
        "https://cdn.example/a.jpg?oh=aaa&oe=1",
    )]);
    FacebookMessengerNormalizer::ingest_conversations_batch(&mut conn, 1, "page_123", &first)
        .unwrap();
    assert_eq!(attachment_count(&conn), 1);

    // Same message and attachment id, new signed URL on the next poll.
    let second = thread_with_attachments(vec![attachment_node(
        "att_1",
        "https://cdn.example/a.jpg?oh=bbb&oe=2",
    )]);
    let summary =
        FacebookMessengerNormalizer::ingest_conversations_batch(&mut conn, 1, "page_123", &second)
            .expect("re-polling a stored message must not fail");

    assert_eq!(summary.new_messages_count, 0);
    assert_eq!(attachment_count(&conn), 1);
}

#[test]
fn test_same_attachment_id_repeated_in_one_message_is_stored_once() {
    let mut conn = create_test_db();

    let threads = thread_with_attachments(vec![
        attachment_node("att_dup", "https://cdn.example/x.jpg?oh=1"),
        attachment_node("att_dup", "https://cdn.example/x.jpg?oh=2"),
    ]);
    FacebookMessengerNormalizer::ingest_conversations_batch(&mut conn, 1, "page_123", &threads)
        .expect("duplicate attachment ids inside one message must not abort the batch");

    assert_eq!(attachment_count(&conn), 1);
}

#[test]
fn test_distinct_attachments_in_one_message_are_all_stored() {
    let mut conn = create_test_db();

    let threads = thread_with_attachments(vec![
        attachment_node("att_a", "https://cdn.example/a.jpg"),
        attachment_node("att_b", "https://cdn.example/b.jpg"),
    ]);
    FacebookMessengerNormalizer::ingest_conversations_batch(&mut conn, 1, "page_123", &threads)
        .unwrap();

    assert_eq!(attachment_count(&conn), 2);
}
