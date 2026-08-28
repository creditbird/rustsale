use super::normalizer::*;
use super::phone;
use rusqlite::{Connection, OptionalExtension};

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            company TEXT,
            status TEXT NOT NULL DEFAULT 'new',
            value REAL NOT NULL DEFAULT 0.0,
            last_activity TEXT NOT NULL DEFAULT (datetime('now')),
            owner TEXT NOT NULL DEFAULT 'WhatsApp',
            email TEXT,
            phone TEXT,
            about TEXT
        );

        CREATE TABLE channel_accounts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
            provider_account_key TEXT NOT NULL,
            display_name TEXT,
            avatar_url TEXT,
            enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
            connection_status TEXT NOT NULL DEFAULT 'connected',
            last_connected_at TEXT,
            last_error TEXT,
            capabilities_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(channel, provider_account_key)
        );

        CREATE TABLE channel_identities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER REFERENCES leads(id) ON DELETE SET NULL,
            channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
            channel_account_id INTEGER REFERENCES channel_accounts(id) ON DELETE CASCADE,
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
            updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX idx_channel_identities_page_unique
        ON channel_identities(channel, channel_account_id, identity_namespace, external_id)
        WHERE channel_account_id IS NOT NULL;

        CREATE TABLE channel_conversations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel_account_id INTEGER NOT NULL REFERENCES channel_accounts(id) ON DELETE CASCADE,
            external_thread_id TEXT NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('direct', 'group', 'email_thread')),
            title TEXT,
            avatar_url TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            last_message_at TEXT,
            last_synced_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(channel_account_id, external_thread_id, kind)
        );

        CREATE TABLE conversation_participants (
            conversation_id INTEGER NOT NULL REFERENCES channel_conversations(id) ON DELETE CASCADE,
            identity_id INTEGER NOT NULL REFERENCES channel_identities(id) ON DELETE CASCADE,
            participant_role TEXT NOT NULL DEFAULT 'member',
            joined_at TEXT,
            left_at TEXT,
            PRIMARY KEY(conversation_id, identity_id)
        );

        CREATE TABLE messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            conversation_id INTEGER NOT NULL REFERENCES channel_conversations(id) ON DELETE CASCADE,
            sender_identity_id INTEGER REFERENCES channel_identities(id) ON DELETE SET NULL,
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
            reply_to_message_id INTEGER REFERENCES messages(id) ON DELETE SET NULL,
            reply_sender_identity_id INTEGER REFERENCES channel_identities(id) ON DELETE SET NULL
        );
        CREATE UNIQUE INDEX idx_messages_provider_unique
        ON messages(conversation_id, provider_message_id)
        WHERE provider_message_id IS NOT NULL;

        CREATE TABLE message_attachments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
            provider_attachment_id TEXT,
            attachment_type TEXT NOT NULL,
            file_name TEXT,
            mime_type TEXT,
            size_bytes INTEGER,
            remote_url TEXT,
            local_path TEXT,
            thumbnail_url TEXT,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX uq_message_attachments_remote
        ON message_attachments(message_id, remote_url)
        WHERE remote_url IS NOT NULL;

        CREATE TABLE message_reactions (
            message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
            actor_external_id TEXT NOT NULL,
            reaction TEXT NOT NULL,
            count INTEGER NOT NULL DEFAULT 1,
            reacted_at TEXT,
            removed_at TEXT,
            PRIMARY KEY(message_id, actor_external_id, reaction)
        );

        CREATE TABLE message_receipts (
            message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
            recipient_external_id TEXT NOT NULL DEFAULT '',
            receipt_type TEXT NOT NULL CHECK (receipt_type IN ('accepted', 'sent', 'delivered', 'read', 'failed')),
            occurred_at TEXT NOT NULL,
            raw_payload_json TEXT,
            PRIMARY KEY(message_id, recipient_external_id, receipt_type)
        );

        CREATE TABLE pending_message_reactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            channel_account_id INTEGER NOT NULL,
            external_thread_id TEXT NOT NULL,
            conversation_kind TEXT NOT NULL CHECK (conversation_kind IN ('direct', 'group')),
            provider_event_id TEXT NOT NULL DEFAULT '',
            provider_message_id TEXT NOT NULL DEFAULT '',
            provider_client_message_id TEXT NOT NULL DEFAULT '',
            actor_external_id TEXT NOT NULL,
            reaction TEXT NOT NULL,
            removed INTEGER NOT NULL CHECK (removed IN (0, 1)),
            occurred_at TEXT NOT NULL,
            raw_payload_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS whatsapp_lid_pn_mappings (
            lid TEXT PRIMARY KEY,
            phone_e164 TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        INSERT INTO channel_accounts (id, channel, provider_account_key, display_name)
        VALUES (1, 'whatsapp', '84901234567', 'My WhatsApp Biz');
        "#
    ).unwrap();
    conn
}

fn insert_test_lead(conn: &Connection, id: i64, phone: &str) {
    conn.execute(
        "INSERT INTO leads (id, name, phone, about) VALUES (?1, ?2, ?3, 'Test Lead')",
        rusqlite::params![id, format!("Lead {}", id), phone],
    )
    .unwrap();
}

fn get_conversation_lead_id(conn: &Connection, conversation_id: i64) -> Option<i64> {
    conn.query_row(
        "SELECT ci.lead_id FROM conversation_participants cp
         JOIN channel_identities ci ON ci.id = cp.identity_id
         WHERE cp.conversation_id = ?1 AND ci.channel = 'whatsapp'
         LIMIT 1",
        [conversation_id],
        |row| row.get(0),
    )
    .optional()
    .unwrap_or(None)
}

fn mock_incoming_text(chat_jid: &str, text: &str) -> RawWhatsAppMessage {
    RawWhatsAppMessage {
        id: format!("wa_msg_{}", uuid::Uuid::new_v4()),
        chat_jid: chat_jid.to_string(),
        sender_jid: Some(chat_jid.to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some(text.to_string()),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    }
}

#[test]
fn test_normalizer_links_single_matching_lead() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 101, "84901234567");

    let raw = mock_incoming_text("84901234567@s.whatsapp.net", "Hello");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(101));
}

#[test]
fn test_normalizer_links_single_matching_lead_with_domestic_format() {
    let mut conn = setup_test_db();
    // Lead phone stored in Vietnamese domestic format "0901234567"
    insert_test_lead(&conn, 105, "0901234567");

    let raw = mock_incoming_text("84901234567@s.whatsapp.net", "Chào shop");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(105));
}

#[test]
fn test_normalizer_leaves_ambiguous_leads_unlinked() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 101, "84901234567");
    insert_test_lead(&conn, 102, "84901234567"); // Duplicate phone in CRM

    let raw = mock_incoming_text("84901234567@s.whatsapp.net", "Hello");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, None); // Must not guess arbitrarily
}

#[test]
fn test_normalizer_unlinks_when_match_becomes_ambiguous() {
    let mut conn = setup_test_db();
    // 1. Initial state: Exactly 1 matching lead
    insert_test_lead(&conn, 101, "84901234567");

    let raw1 = mock_incoming_text("84901234567@s.whatsapp.net", "First message");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw1)
        .unwrap()
        .unwrap();

    let lead_id1: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id1, Some(101));

    // 2. A second duplicate lead with the same phone is added to CRM
    insert_test_lead(&conn, 102, "84901234567");

    // 3. Next message arrives -> resolution is now ambiguous (>1 matches)
    let raw2 = mock_incoming_text("84901234567@s.whatsapp.net", "Second message");
    WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw2)
        .unwrap()
        .unwrap();

    // 4. Invariant: lead_id MUST be set back to None / unlinked
    let lead_id2: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id2, None);
}

#[test]
fn test_normalizer_leaves_unknown_phone_unlinked() {
    let mut conn = setup_test_db();
    let raw = mock_incoming_text("84999999999@s.whatsapp.net", "Hello from stranger");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, None);
}

#[test]
fn test_normalizer_links_chinese_phone_international_format() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 201, "+86 139 2684 2913");

    let raw = mock_incoming_text("8613926842913@s.whatsapp.net", "Ni hao");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(201));
}

#[test]
fn test_normalizer_links_chinese_phone_national_format() {
    let mut conn = setup_test_db();
    // Lead phone saved in CRM as national 11-digit number without country code
    insert_test_lead(&conn, 202, "13926842913");

    let raw = mock_incoming_text("8613926842913@s.whatsapp.net", "Ni hao");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(202));
}

#[test]
fn test_normalizer_links_us_phone_international_and_national_format() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 203, "+1 (555) 123-4567");

    let raw = mock_incoming_text("15551234567@s.whatsapp.net", "Hello US");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(203));
}

#[test]
fn test_normalizer_resolves_lid_when_pn_mapping_exists() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 204, "+86 139 2684 2913");
    phone::save_lid_pn_mapping(&conn, "198097598676992@lid", "8613926842913").unwrap();

    let raw = RawWhatsAppMessage {
        id: "msg_lid_001".to_string(),
        chat_jid: "198097598676992@lid".to_string(),
        sender_jid: Some("198097598676992@lid".to_string()),
        sender_name: Some("Vject Wang".to_string()),
        text: Some("Can you take picture".to_string()),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    // Verify conversation thread is canonicalized to direct phone JID
    let ext_thread: String = conn
        .query_row(
            "SELECT external_thread_id FROM channel_conversations WHERE id = ?1",
            [conv_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(ext_thread, "8613926842913@s.whatsapp.net");

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(204));
}

#[test]
fn test_normalizer_auto_maps_lid_when_message_has_pn_and_lid() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 205, "+86 139 2684 2913");

    // Incoming message where chat_jid is LID but sender_jid is PN (or vice versa)
    let raw = RawWhatsAppMessage {
        id: "msg_lid_pn_001".to_string(),
        chat_jid: "198097598676992@lid".to_string(),
        sender_jid: Some("8613926842913@s.whatsapp.net".to_string()),
        sender_name: Some("Vject Wang".to_string()),
        text: Some("Auto map test".to_string()),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    // Verify auto mapping was saved in database
    let mapped_phone: String = conn
        .query_row(
            "SELECT phone_e164 FROM whatsapp_lid_pn_mappings WHERE lid = '198097598676992'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mapped_phone, "8613926842913");

    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(205));
}

#[test]
fn test_normalizer_inbound_direct_message_parsing() {
    let mut conn = setup_test_db();
    let raw = RawWhatsAppMessage {
        id: "msg_stanza_001".to_string(),
        chat_jid: "84912345678@s.whatsapp.net".to_string(),
        sender_jid: Some("84912345678@s.whatsapp.net".to_string()),
        sender_name: Some("Nguyen Van A".to_string()),
        text: Some("Tư vấn giúp em gói cước".to_string()),
        timestamp: Some("2026-08-26T10:15:30.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    // Verify conversation
    let (ext_thread, kind, title): (String, String, String) = conn
        .query_row(
            "SELECT external_thread_id, kind, title FROM channel_conversations WHERE id = ?1",
            [conv_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(ext_thread, "84912345678@s.whatsapp.net");
    assert_eq!(kind, "direct");
    assert_eq!(title, "Nguyen Van A");

    // Verify identity
    let (ext_id, disp_name): (String, String) = conn
        .query_row(
            "SELECT external_id, display_name FROM channel_identities WHERE channel = 'whatsapp' AND external_id = '84912345678'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ext_id, "84912345678");
    assert_eq!(disp_name, "Nguyen Van A");

    // Verify message
    let (text, dir, status, prov_mid): (String, String, String, String) = conn
        .query_row(
            "SELECT text_content, direction, status, provider_message_id FROM messages WHERE conversation_id = ?1",
            [conv_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(text, "Tư vấn giúp em gói cước");
    assert_eq!(dir, "inbound");
    assert_eq!(status, "received");
    assert_eq!(prov_mid, "msg_stanza_001");
}

#[test]
fn test_normalizer_inbound_group_message_parsing() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 202, "84988776655");

    let raw = RawWhatsAppMessage {
        id: "msg_group_001".to_string(),
        chat_jid: "120363025342171@g.us".to_string(),
        sender_jid: Some("84988776655@s.whatsapp.net".to_string()),
        sender_name: Some("Tran Thi B".to_string()),
        text: Some("Hello all members".to_string()),
        timestamp: Some("2026-08-26T11:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    // Verify group conversation
    let (ext_thread, kind): (String, String) = conn
        .query_row(
            "SELECT external_thread_id, kind FROM channel_conversations WHERE id = ?1",
            [conv_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ext_thread, "120363025342171@g.us");
    assert_eq!(kind, "group");

    // Verify sender identity linked to lead 202
    let (ext_id, lead_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT external_id, lead_id FROM channel_identities WHERE channel = 'whatsapp' AND external_id = '84988776655'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(ext_id, "84988776655");
    assert_eq!(lead_id, Some(202));
}

#[test]
fn test_normalizer_attachments_parsing() {
    let mut conn = setup_test_db();
    let raw = RawWhatsAppMessage {
        id: "msg_media_001".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Xem ảnh này nha".to_string()),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            file_name: Some("product.jpg".to_string()),
            size_bytes: Some(102400),
            url: Some("https://example.com/product.jpg".to_string()),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            direct_path: None,
            media_key: None,
            file_sha256: None,
            file_enc_sha256: None,
            width: None,
            height: None,
        }],
        quote: None,
        raw_json: None,
    };

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    let (c_type, msg_id): (String, i64) = conn
        .query_row(
            "SELECT content_type, id FROM messages WHERE conversation_id = ?1",
            [conv_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(c_type, "image");

    let (att_type, mime, size, url): (String, Option<String>, Option<i64>, Option<String>) = conn
        .query_row(
            "SELECT attachment_type, mime_type, size_bytes, remote_url FROM message_attachments WHERE message_id = ?1",
            [msg_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(att_type, "image");
    assert_eq!(mime, Some("image/jpeg".to_string()));
    assert_eq!(size, Some(102400i64));
    assert_eq!(url, Some("https://example.com/product.jpg".to_string()));
}

#[test]
fn test_normalizer_quote_reply_parsing() {
    let mut conn = setup_test_db();

    // 1. First message
    let msg1 = RawWhatsAppMessage {
        id: "stanza_orig_100".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Giá sản phẩm là bao nhiêu?".to_string()),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };
    WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &msg1)
        .unwrap()
        .unwrap();

    let orig_msg_id: i64 = conn
        .query_row(
            "SELECT id FROM messages WHERE provider_message_id = 'stanza_orig_100'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    // 2. Second message quoting the first
    let msg2 = RawWhatsAppMessage {
        id: "stanza_reply_101".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Em muốn hỏi thêm chi tiết".to_string()),
        timestamp: Some("2026-08-26T12:01:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: Some(RawWhatsAppQuote {
            stanza_id: "stanza_orig_100".to_string(),
            participant_jid: "84901234567@s.whatsapp.net".to_string(),
            text: Some("Giá sản phẩm là bao nhiêu?".to_string()),
            media_type: None,
            timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        }),
        raw_json: None,
    };
    WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &msg2)
        .unwrap()
        .unwrap();

    let reply_to: Option<i64> = conn
        .query_row(
            "SELECT reply_to_message_id FROM messages WHERE provider_message_id = 'stanza_reply_101'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reply_to, Some(orig_msg_id));
}

#[test]
fn test_normalizer_reaction_and_receipt_ingestion() {
    let mut conn = setup_test_db();

    // Ingest outbound message
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
         VALUES (10, 1, '84901234567@s.whatsapp.net', 'direct', 'Customer', 'active', '2026-08-26T12:00:00.000Z', '2026-08-26T12:00:00.000Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, text_content, status, ingested_at)
         VALUES (500, 10, 'outbound', 'stanza_react_500', 'text', 'Dạ shop đây ạ', 'sent', '2026-08-26T12:00:00.000Z')",
        [],
    ).unwrap();

    // 1. Ingest Reaction
    let reaction = RawWhatsAppReaction {
        message_id: "stanza_react_500".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: "84901234567@s.whatsapp.net".to_string(),
        emoji: "❤️".to_string(),
        timestamp: Some("2026-08-26T12:02:00.000Z".to_string()),
        is_from_me: false,
    };
    WhatsAppNormalizer::ingest_reaction(&mut conn, 1, &reaction).unwrap();

    let (reaction_icon, count): (String, i64) = conn
        .query_row(
            "SELECT reaction, count FROM message_reactions WHERE message_id = 500",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(reaction_icon, "❤️");
    assert_eq!(count, 1);

    // Ingest Second Reaction (replaces the first one)
    let reaction2 = RawWhatsAppReaction {
        message_id: "stanza_react_500".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: "84901234567@s.whatsapp.net".to_string(),
        emoji: "🙏".to_string(),
        timestamp: Some("2026-08-26T12:02:30.000Z".to_string()),
        is_from_me: false,
    };
    WhatsAppNormalizer::ingest_reaction(&mut conn, 1, &reaction2).unwrap();

    let reactions: Vec<(String, i64)> = {
        let mut stmt = conn
            .prepare("SELECT reaction, count FROM message_reactions WHERE message_id = 500")
            .unwrap();
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        rows.collect::<Result<Vec<_>, _>>().unwrap()
    };
    assert_eq!(reactions.len(), 1, "WhatsApp actor must have only 1 active reaction");
    assert_eq!(reactions[0].0, "🙏");

    // 2. Ingest Delivery Receipt
    let delivery_receipt = RawWhatsAppReceipt {
        message_ids: vec!["stanza_react_500".to_string()],
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        receipt_type: "delivered".to_string(),
        timestamp: Some("2026-08-26T12:01:00.000Z".to_string()),
    };
    WhatsAppNormalizer::ingest_receipt(&mut conn, 1, &delivery_receipt).unwrap();

    let status1: String = conn
        .query_row("SELECT status FROM messages WHERE id = 500", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status1, "delivered");

    // 3. Ingest Read Receipt
    let read_receipt = RawWhatsAppReceipt {
        message_ids: vec!["stanza_react_500".to_string()],
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        receipt_type: "read".to_string(),
        timestamp: Some("2026-08-26T12:03:00.000Z".to_string()),
    };
    WhatsAppNormalizer::ingest_receipt(&mut conn, 1, &read_receipt).unwrap();

    let status2: String = conn
        .query_row("SELECT status FROM messages WHERE id = 500", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status2, "read");
}

#[test]
fn test_normalizer_idempotent_ingestion() {
    let mut conn = setup_test_db();
    let raw = mock_incoming_text("84901234567@s.whatsapp.net", "Duplicate test");

    let conv1 = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();
    let conv2 = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    assert_eq!(conv1, conv2);

    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE provider_message_id = ?1",
            [&raw.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        count, 1,
        "Duplicate message stanza should not insert duplicate row"
    );
}

#[test]
fn test_normalizer_outbound_direct_and_group_messages() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 301, "84901234567");

    // 1. Outbound direct message
    let out_direct = RawWhatsAppMessage {
        id: "out_direct_001".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: None,
        sender_name: None,
        text: Some("Chào anh, em gửi báo giá ạ".to_string()),
        timestamp: Some("2026-08-26T14:00:00.000Z".to_string()),
        is_from_me: true,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &out_direct)
        .unwrap()
        .unwrap();

    let (dir, status, sender_id): (String, String, Option<i64>) = conn
        .query_row(
            "SELECT direction, status, sender_identity_id FROM messages WHERE provider_message_id = 'out_direct_001'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(dir, "outbound");
    assert_eq!(status, "sent");
    assert_eq!(
        sender_id, None,
        "Outbound sender_identity_id should be None"
    );

    // Peer participant is still linked to lead 301
    let lead_id = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(301));
}

#[test]
fn test_normalizer_pending_reaction_replayed_when_message_arrives() {
    let mut conn = setup_test_db();

    // 1. Reaction arrives BEFORE message exists
    let reaction = RawWhatsAppReaction {
        message_id: "future_msg_888".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: "84901234567@s.whatsapp.net".to_string(),
        emoji: "👍".to_string(),
        timestamp: Some("2026-08-26T15:00:00.000Z".to_string()),
        is_from_me: false,
    };
    WhatsAppNormalizer::ingest_reaction(&mut conn, 1, &reaction).unwrap();

    // Verify stored in pending_message_reactions
    let pending_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM pending_message_reactions WHERE provider_message_id = 'future_msg_888'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pending_count, 1);

    // 2. Now message arrives
    let raw = RawWhatsAppMessage {
        id: "future_msg_888".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Đã chuyển khoản".to_string()),
        timestamp: Some("2026-08-26T14:59:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![],
        quote: None,
        raw_json: None,
    };
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();

    // Verify pending reaction was applied and cleared
    let pending_after: i64 = conn
        .query_row(
            "SELECT count(*) FROM pending_message_reactions WHERE provider_message_id = 'future_msg_888'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pending_after, 0);

    let msg_id: i64 = conn
        .query_row(
            "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = 'future_msg_888'",
            [conv_id],
            |r| r.get(0),
        )
        .unwrap();

    let (reaction_icon, count): (String, i64) = conn
        .query_row(
            "SELECT reaction, count FROM message_reactions WHERE message_id = ?1",
            [msg_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(reaction_icon, "👍");
    assert_eq!(count, 1);
}

#[test]
fn test_normalizer_normalize_message_model_conversion() {
    let raw = RawWhatsAppMessage {
        id: "stanza_conv_99".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer A".to_string()),
        text: Some("Test normalize_message".to_string()),
        timestamp: Some("2026-08-26T12:00:00.000Z".to_string()),
        is_from_me: false,
        attachments: vec![RawWhatsAppAttachment {
            media_type: "document".to_string(),
            mime_type: Some("application/pdf".to_string()),
            file_name: Some("contract.pdf".to_string()),
            size_bytes: Some(50000),
            url: Some("https://example.com/contract.pdf".to_string()),
            thumbnail_url: None,
            direct_path: Some("/tmp/contract.pdf".to_string()),
            media_key: None,
            file_sha256: None,
            file_enc_sha256: None,
            width: None,
            height: None,
        }],
        quote: None,
        raw_json: None,
    };

    let norm = WhatsAppNormalizer::normalize_message("84901234567", Some("My Biz"), &raw).unwrap();

    assert_eq!(norm.channel, crate::conversation::types::Channel::WhatsApp);
    assert_eq!(norm.provider_account_key, "84901234567");
    assert_eq!(norm.account_display_name, Some("My Biz".to_string()));
    assert_eq!(
        norm.conversation_kind,
        crate::conversation::types::ConversationKind::Direct
    );
    assert_eq!(norm.external_thread_id, "84901234567@s.whatsapp.net");
    assert_eq!(
        norm.direction,
        crate::conversation::types::MessageDirection::Inbound
    );
    assert_eq!(
        norm.status,
        crate::conversation::types::MessageStatus::Received
    );
    assert_eq!(norm.content_type, "file");
    assert_eq!(norm.attachments.len(), 1);
    assert_eq!(
        norm.attachments[0].file_name,
        Some("contract.pdf".to_string())
    );
    assert_eq!(norm.attachments[0].mime_type, "application/pdf");
}

#[test]
fn test_normalizer_formatted_phone_numbers_in_crm() {
    let mut conn = setup_test_db();
    // Leads with varied formatting: international with spaces, dashes, parentheses
    insert_test_lead(&conn, 401, "+84 (90) 123-4567");
    insert_test_lead(&conn, 402, "+1 (555) 019-2834");

    // Message 1 from Vietnamese phone
    let raw1 = mock_incoming_text("84901234567@s.whatsapp.net", "VN Lead Match");
    let conv1 = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw1)
        .unwrap()
        .unwrap();
    assert_eq!(get_conversation_lead_id(&conn, conv1), Some(401));

    // Message 2 from US phone
    let raw2 = mock_incoming_text("15550192834@s.whatsapp.net", "US Lead Match");
    let conv2 = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw2)
        .unwrap()
        .unwrap();
    assert_eq!(get_conversation_lead_id(&conn, conv2), Some(402));
}

#[test]
fn test_ensure_conversation_exists_direct_and_group() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 501, "+84901234567");

    // 1. Direct chat with lead match
    let conv_id = WhatsAppNormalizer::ensure_conversation_exists(
        &mut conn,
        1,
        "84901234567@s.whatsapp.net",
        Some("Customer John"),
    )
    .unwrap();
    assert!(conv_id > 0);
    assert_eq!(get_conversation_lead_id(&conn, conv_id), Some(501));

    // 2. Group chat
    let group_conv_id = WhatsAppNormalizer::ensure_conversation_exists(
        &mut conn,
        1,
        "1234567890@g.us",
        Some("VIP Customers Group"),
    )
    .unwrap();
    assert!(group_conv_id > 0);

    let title: String = conn
        .query_row(
            "SELECT title FROM channel_conversations WHERE id = ?1",
            rusqlite::params![group_conv_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "VIP Customers Group");
}

#[test]
fn test_extract_raw_message_from_proto() {
    let msg = whatsapp_rust::prelude::wa::Message {
        conversation: Some("Hello from WhatsApp test".to_string()),
        ..Default::default()
    };
    let key = whatsapp_rust::prelude::wa::MessageKey {
        id: Some("WA_MSG_12345".to_string()),
        remote_jid: Some("84901234567@s.whatsapp.net".to_string()),
        from_me: Some(false),
        participant: Some("84901234567@s.whatsapp.net".to_string()),
        ..Default::default()
    };

    let raw = WhatsAppNormalizer::extract_raw_message(
        &msg,
        &key,
        Some(1700000000),
        Some("Sender Name"),
        "84901234567@s.whatsapp.net",
        Some("Chat Title"),
    )
    .expect("Should extract raw message");

    assert_eq!(raw.id, "WA_MSG_12345");
    assert_eq!(raw.chat_jid, "84901234567@s.whatsapp.net");
    assert_eq!(raw.text, Some("Hello from WhatsApp test".to_string()));
    assert_eq!(raw.sender_name, Some("Sender Name".to_string()));
    assert!(!raw.is_from_me);
}

#[test]
fn test_extract_raw_message_nested_ephemeral_and_view_once() {
    // 1. Ephemeral Message wrapper
    let inner_text_msg = whatsapp_rust::prelude::wa::Message {
        conversation: Some("Secret ephemeral text".to_string()),
        ..Default::default()
    };
    let ephemeral_wrapper = whatsapp_rust::prelude::wa::Message {
        ephemeral_message: whatsapp_rust::prelude::MessageField::some(
            whatsapp_rust::prelude::wa::message::FutureProofMessage {
                message: whatsapp_rust::prelude::MessageField::some(inner_text_msg),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    let key1 = whatsapp_rust::prelude::wa::MessageKey {
        id: Some("WA_EPH_001".to_string()),
        remote_jid: Some("84901234567@s.whatsapp.net".to_string()),
        from_me: Some(false),
        ..Default::default()
    };

    let raw1 = WhatsAppNormalizer::extract_raw_message(
        &ephemeral_wrapper,
        &key1,
        Some(1700000001),
        Some("Alice"),
        "84901234567@s.whatsapp.net",
        None,
    )
    .expect("Should extract message from ephemeral wrapper");
    assert_eq!(raw1.text, Some("Secret ephemeral text".to_string()));

    // 2. View Once Image wrapper
    let inner_img_msg = whatsapp_rust::prelude::wa::Message {
        image_message: whatsapp_rust::prelude::MessageField::some(
            whatsapp_rust::prelude::wa::message::ImageMessage {
                caption: Some("View once image caption".to_string()),
                mimetype: Some("image/jpeg".to_string()),
                url: Some("https://mmg.whatsapp.net/v/image.jpg".to_string()),
                file_length: Some(102400),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    let view_once_wrapper = whatsapp_rust::prelude::wa::Message {
        view_once_message_v2: whatsapp_rust::prelude::MessageField::some(
            whatsapp_rust::prelude::wa::message::FutureProofMessage {
                message: whatsapp_rust::prelude::MessageField::some(inner_img_msg),
                ..Default::default()
            },
        ),
        ..Default::default()
    };
    let key2 = whatsapp_rust::prelude::wa::MessageKey {
        id: Some("WA_VO_002".to_string()),
        remote_jid: Some("84901234567@s.whatsapp.net".to_string()),
        from_me: Some(false),
        ..Default::default()
    };

    let raw2 = WhatsAppNormalizer::extract_raw_message(
        &view_once_wrapper,
        &key2,
        Some(1700000002),
        Some("Bob"),
        "84901234567@s.whatsapp.net",
        None,
    )
    .expect("Should extract image from view_once_message_v2 wrapper");
    assert_eq!(raw2.text, Some("View once image caption".to_string()));
    assert_eq!(raw2.attachments.len(), 1);
    assert_eq!(raw2.attachments[0].media_type, "image");
}

#[test]
fn test_ensure_conversation_exists_with_lid_mapping() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 601, "+84912345678");

    // Save LID <-> Phone mapping first
    super::phone::save_lid_pn_mapping(&conn, "1029384756@lid", "84912345678").unwrap();

    let conv_id = WhatsAppNormalizer::ensure_conversation_exists(
        &mut conn,
        1,
        "1029384756@lid",
        Some("LID Customer"),
    )
    .unwrap();

    assert!(conv_id > 0);
    assert_eq!(get_conversation_lead_id(&conn, conv_id), Some(601));
}

#[test]
fn test_extract_and_ingest_image_with_encryption_metadata() {
    let mut conn = setup_test_db();

    let media_key = vec![0x12; 32];
    let file_sha = vec![0x34; 32];
    let file_enc_sha = vec![0x56; 32];

    let img_msg = whatsapp_rust::prelude::wa::Message {
        image_message: whatsapp_rust::prelude::MessageField::some(
            whatsapp_rust::prelude::wa::message::ImageMessage {
                caption: Some("Check out this photo".to_string()),
                mimetype: Some("image/jpeg".to_string()),
                url: Some("https://mmg.whatsapp.net/v/t62.7118-24/img.enc".to_string()),
                direct_path: Some("/v/t62.7118-24/img.enc".to_string()),
                file_length: Some(197632),
                media_key: Some(media_key.clone()),
                file_sha256: Some(file_sha.clone()),
                file_enc_sha256: Some(file_enc_sha.clone()),
                width: Some(1080),
                height: Some(1920),
                jpeg_thumbnail: Some(vec![0xFF, 0xD8, 0xFF, 0xE0]),
                ..Default::default()
            },
        ),
        ..Default::default()
    };

    let key = whatsapp_rust::prelude::wa::MessageKey {
        id: Some("WA_MSG_MEDIA_001".to_string()),
        remote_jid: Some("84901234567@s.whatsapp.net".to_string()),
        from_me: Some(false),
        ..Default::default()
    };

    let raw = WhatsAppNormalizer::extract_raw_message(
        &img_msg,
        &key,
        Some(1700000003),
        Some("Sender"),
        "84901234567@s.whatsapp.net",
        None,
    )
    .expect("Should extract raw message");

    assert_eq!(raw.attachments.len(), 1);
    let att = &raw.attachments[0];
    assert_eq!(att.media_type, "image");
    assert_eq!(att.direct_path.as_deref(), Some("/v/t62.7118-24/img.enc"));
    assert_eq!(att.width, Some(1080));
    assert_eq!(att.height, Some(1920));
    assert!(att.media_key.is_some());
    assert!(att.file_sha256.is_some());
    assert!(att.file_enc_sha256.is_some());

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw)
        .unwrap()
        .unwrap();
    assert!(conv_id > 0);

    let (saved_meta, thumb_url): (String, Option<String>) = conn
        .query_row(
            "SELECT ma.metadata_json, ma.thumbnail_url
             FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.conversation_id = ?1",
            [conv_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert!(thumb_url
        .as_ref()
        .unwrap()
        .starts_with("data:image/jpeg;base64,"));
    let meta_val: serde_json::Value = serde_json::from_str(&saved_meta).unwrap();
    assert_eq!(meta_val["direct_path"], "/v/t62.7118-24/img.enc");
    assert_eq!(meta_val["width"], 1080);
    assert_eq!(meta_val["height"], 1920);
    assert!(meta_val["media_key"].as_str().is_some());

    // Invariant: local_path must be NULL for inbound attachments
    let local_path: Option<String> = conn
        .query_row(
            "SELECT ma.local_path FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.conversation_id = ?1",
            [conv_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        local_path, None,
        "Inbound attachment local_path must be NULL"
    );
}

#[test]
fn test_ingest_duplicate_redelivery_enriches_missing_media_key_and_cleans_local_path() {
    let mut conn = setup_test_db();

    // 1. Ingest initial message where media_key is missing (simulating historical record)
    let raw_initial = RawWhatsAppMessage {
        id: "WA_REDELIVER_001".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Historical photo".to_string()),
        timestamp: Some("2026-08-27T03:41:42+00:00".to_string()),
        is_from_me: false,
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            file_name: None,
            size_bytes: Some(197638),
            url: Some("https://mmg.whatsapp.net/v/t62/img.enc".to_string()),
            thumbnail_url: None,
            direct_path: Some("/v/t62/img.enc".to_string()),
            media_key: None,
            file_sha256: None,
            file_enc_sha256: None,
            width: None,
            height: None,
        }],
        quote: None,
        raw_json: None,
    };

    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw_initial)
        .unwrap()
        .unwrap();

    // Simulate polluted local_path from old bug
    conn.execute(
        "UPDATE message_attachments SET local_path = '/v/t62/img.enc' WHERE message_id IN (
            SELECT id FROM messages WHERE provider_message_id = 'WA_REDELIVER_001'
        )",
        [],
    )
    .unwrap();

    // Verify initial state
    let (initial_meta, initial_local_path): (String, Option<String>) = conn
        .query_row(
            "SELECT ma.metadata_json, ma.local_path
             FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.provider_message_id = 'WA_REDELIVER_001'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(initial_local_path.as_deref(), Some("/v/t62/img.enc"));
    let meta_val: serde_json::Value = serde_json::from_str(&initial_meta).unwrap();
    assert!(meta_val["media_key"].is_null());

    // 2. Redeliver same message (e.g. from history sync) with full keys & thumbnail
    use base64::Engine as _;
    let key_b64 = base64::engine::general_purpose::STANDARD.encode(vec![0xAA; 32]);
    let sha_b64 = base64::engine::general_purpose::STANDARD.encode(vec![0xBB; 32]);
    let enc_b64 = base64::engine::general_purpose::STANDARD.encode(vec![0xCC; 32]);

    let raw_enriched = RawWhatsAppMessage {
        id: "WA_REDELIVER_001".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Historical photo".to_string()),
        timestamp: Some("2026-08-27T03:41:42+00:00".to_string()),
        is_from_me: false,
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            file_name: Some("photo.jpg".to_string()),
            size_bytes: Some(197638),
            url: Some("https://mmg.whatsapp.net/v/t62/img.enc".to_string()),
            thumbnail_url: Some("data:image/jpeg;base64,ENRICHED_THUMB".to_string()),
            direct_path: Some("/v/t62/img.enc".to_string()),
            media_key: Some(key_b64.clone()),
            file_sha256: Some(sha_b64.clone()),
            file_enc_sha256: Some(enc_b64.clone()),
            width: Some(1280),
            height: Some(720),
        }],
        quote: None,
        raw_json: None,
    };

    let re_conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw_enriched)
        .unwrap()
        .unwrap();
    assert_eq!(re_conv_id, conv_id);

    // 3. Verify enrichment
    let (enriched_meta, enriched_local_path, enriched_thumb): (
        String,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT ma.metadata_json, ma.local_path, ma.thumbnail_url
             FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.provider_message_id = 'WA_REDELIVER_001'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_eq!(
        enriched_local_path, None,
        "Polluted local_path must be cleaned to NULL"
    );
    assert_eq!(
        enriched_thumb.as_deref(),
        Some("data:image/jpeg;base64,ENRICHED_THUMB")
    );

    let enriched_meta_val: serde_json::Value = serde_json::from_str(&enriched_meta).unwrap();
    assert_eq!(enriched_meta_val["media_key"], key_b64);
    assert_eq!(enriched_meta_val["file_sha256"], sha_b64);
    assert_eq!(enriched_meta_val["file_enc_sha256"], enc_b64);
    assert_eq!(enriched_meta_val["width"], 1280);
    assert_eq!(enriched_meta_val["height"], 720);
}

#[test]
fn test_redelivery_with_refreshed_cdn_url_reuses_canonical_attachment() {
    let mut conn = setup_test_db();

    let initial = RawWhatsAppMessage {
        id: "WA_REFRESHED_URL_001".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Photo".to_string()),
        timestamp: Some("2026-08-28T01:00:00Z".to_string()),
        is_from_me: false,
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            file_name: None,
            size_bytes: Some(1024),
            url: Some("https://mmg.whatsapp.net/old/image.enc".to_string()),
            thumbnail_url: None,
            direct_path: Some("/old/image.enc".to_string()),
            media_key: None,
            file_sha256: None,
            file_enc_sha256: None,
            width: None,
            height: None,
        }],
        quote: None,
        raw_json: None,
    };

    let conversation_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &initial)
        .unwrap()
        .unwrap();

    use base64::Engine as _;
    let media_key = base64::engine::general_purpose::STANDARD.encode(vec![0x21; 32]);
    let redelivered = RawWhatsAppMessage {
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            file_name: Some("customer-photo.jpg".to_string()),
            size_bytes: Some(1024),
            url: Some("https://mmg.whatsapp.net/refreshed/image.enc".to_string()),
            thumbnail_url: Some("data:image/jpeg;base64,REFRESHED".to_string()),
            direct_path: Some("/refreshed/image.enc".to_string()),
            media_key: Some(media_key.clone()),
            file_sha256: None,
            file_enc_sha256: None,
            width: Some(640),
            height: Some(480),
        }],
        ..initial.clone()
    };

    WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &redelivered).unwrap();

    let attachment_count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.conversation_id = ?1 AND m.provider_message_id = 'WA_REFRESHED_URL_001'",
            [conversation_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        attachment_count, 1,
        "a refreshed WhatsApp CDN URL must enrich the existing media slot"
    );

    let (remote_url, metadata_json): (Option<String>, String) = conn
        .query_row(
            "SELECT ma.remote_url, ma.metadata_json
             FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.provider_message_id = 'WA_REFRESHED_URL_001'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        remote_url.as_deref(),
        Some("https://mmg.whatsapp.net/refreshed/image.enc")
    );
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json).unwrap();
    assert_eq!(metadata["media_key"], media_key);
    assert_eq!(metadata["direct_path"], "/refreshed/image.enc");
}

#[test]
fn test_sparse_redelivery_does_not_erase_persisted_media_secrets() {
    let mut conn = setup_test_db();
    use base64::Engine as _;

    let media_key = base64::engine::general_purpose::STANDARD.encode(vec![0x31; 32]);
    let file_sha = base64::engine::general_purpose::STANDARD.encode(vec![0x32; 32]);
    let file_enc_sha = base64::engine::general_purpose::STANDARD.encode(vec![0x33; 32]);
    let rich = RawWhatsAppMessage {
        id: "WA_SPARSE_REDELIVERY_001".to_string(),
        chat_jid: "84901234567@s.whatsapp.net".to_string(),
        sender_jid: Some("84901234567@s.whatsapp.net".to_string()),
        sender_name: Some("Customer".to_string()),
        text: Some("Photo".to_string()),
        timestamp: Some("2026-08-28T01:05:00Z".to_string()),
        is_from_me: false,
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: Some("image/jpeg".to_string()),
            file_name: Some("photo.jpg".to_string()),
            size_bytes: Some(2048),
            url: Some("https://mmg.whatsapp.net/v/rich.enc".to_string()),
            thumbnail_url: Some("data:image/jpeg;base64,RICH".to_string()),
            direct_path: Some("/v/rich.enc".to_string()),
            media_key: Some(media_key.clone()),
            file_sha256: Some(file_sha.clone()),
            file_enc_sha256: Some(file_enc_sha.clone()),
            width: Some(1080),
            height: Some(720),
        }],
        quote: None,
        raw_json: None,
    };
    WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &rich).unwrap();

    let sparse = RawWhatsAppMessage {
        attachments: vec![RawWhatsAppAttachment {
            media_type: "image".to_string(),
            mime_type: None,
            file_name: None,
            size_bytes: None,
            url: Some("https://mmg.whatsapp.net/v/rich.enc".to_string()),
            thumbnail_url: None,
            direct_path: None,
            media_key: None,
            file_sha256: None,
            file_enc_sha256: None,
            width: None,
            height: None,
        }],
        ..rich
    };
    WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &sparse).unwrap();

    let (metadata_json, thumbnail_url): (String, Option<String>) = conn
        .query_row(
            "SELECT ma.metadata_json, ma.thumbnail_url
             FROM message_attachments ma
             JOIN messages m ON m.id = ma.message_id
             WHERE m.provider_message_id = 'WA_SPARSE_REDELIVERY_001'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    let metadata: serde_json::Value = serde_json::from_str(&metadata_json).unwrap();
    assert_eq!(metadata["media_key"], media_key);
    assert_eq!(metadata["file_sha256"], file_sha);
    assert_eq!(metadata["file_enc_sha256"], file_enc_sha);
    assert_eq!(metadata["direct_path"], "/v/rich.enc");
    assert_eq!(metadata["width"], 1080);
    assert_eq!(metadata["height"], 720);
    assert_eq!(
        thumbnail_url.as_deref(),
        Some("data:image/jpeg;base64,RICH")
    );
}
