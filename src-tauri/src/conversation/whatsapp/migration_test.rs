use super::migration::apply_whatsapp_migrations;
use rusqlite::Connection;

#[test]
fn test_whatsapp_migrations_idempotency() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();

    // First run
    apply_whatsapp_migrations(&conn).unwrap();
    // Second run (idempotent)
    apply_whatsapp_migrations(&conn).unwrap();

    // Verify all 4 tables exist and are empty
    let tables = [
        "whatsapp_sessions",
        "contact_suppression_list",
        "whatsapp_outreach_campaigns",
        "whatsapp_campaign_recipients",
    ];

    for table in tables {
        let sql = format!("SELECT count(*) FROM {}", table);
        let mut stmt = conn.prepare(&sql).unwrap();
        let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
        assert_eq!(count, 0, "Table {} should be empty", table);
    }
}

#[test]
fn test_whatsapp_sessions_constraints() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    apply_whatsapp_migrations(&conn).unwrap();

    // Create channel_account
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
         VALUES (1, 'whatsapp', 'wa-acc-1', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z')",
        [],
    )
    .unwrap();

    // Insert valid whatsapp_session
    conn.execute(
        "INSERT INTO whatsapp_sessions (
            channel_account_id, account_key, phone_number, lid, push_name, avatar_url,
            session_db_filename, is_valid, last_connected_at, last_error, created_at, updated_at
         ) VALUES (
            1, 'wa-acc-1', '+84901234567', '12345@lid', 'Test Bot', 'https://avatar.url',
            'session_1.db', 1, '2026-08-26T00:00:00Z', NULL, '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    )
    .unwrap();

    // Duplicate account_key should fail UNIQUE constraint
    let dup_key = conn.execute(
        "INSERT INTO whatsapp_sessions (
            channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at
         ) VALUES (
            2, 'wa-acc-1', '+84901234568', 'session_2.db', 1, '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    );
    assert!(
        dup_key.is_err(),
        "Duplicate account_key must violate unique constraint"
    );

    // Invalid is_valid value should fail CHECK constraint
    let invalid_check = conn.execute(
        "INSERT INTO whatsapp_sessions (
            channel_account_id, account_key, phone_number, session_db_filename, is_valid, created_at, updated_at
         ) VALUES (
            2, 'wa-acc-2', '+84901234568', 'session_2.db', 2, '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    );
    assert!(
        invalid_check.is_err(),
        "is_valid not in (0,1) must fail check constraint"
    );

    // Deleting referenced channel_account should be RESTRICTED
    let del_acc = conn.execute("DELETE FROM channel_accounts WHERE id = 1", []);
    assert!(
        del_acc.is_err(),
        "ON DELETE RESTRICT must prevent channel_account deletion while session exists"
    );
}

#[test]
fn test_contact_suppression_list_constraints() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    apply_whatsapp_migrations(&conn).unwrap();

    // Insert suppression entry
    conn.execute(
        "INSERT INTO contact_suppression_list (channel, phone_or_email, reason, source, created_at)
         VALUES ('whatsapp', '+84901234567', 'opt_out', 'user_request', '2026-08-26T00:00:00Z')",
        [],
    )
    .unwrap();

    // Duplicate (channel, phone_or_email) should fail UNIQUE
    let dup = conn.execute(
        "INSERT INTO contact_suppression_list (channel, phone_or_email, reason, source, created_at)
         VALUES ('whatsapp', '+84901234567', 'spam_report', 'system', '2026-08-26T00:00:00Z')",
        [],
    );
    assert!(
        dup.is_err(),
        "Duplicate (channel, phone_or_email) must violate unique constraint"
    );

    // Same phone on different channel is allowed
    let diff_channel = conn.execute(
        "INSERT INTO contact_suppression_list (channel, phone_or_email, reason, source, created_at)
         VALUES ('zalo', '+84901234567', 'opt_out', 'user_request', '2026-08-26T00:00:00Z')",
        [],
    );
    assert!(
        diff_channel.is_ok(),
        "Same phone on different channel should be allowed"
    );

    // Invalid channel value should fail CHECK constraint
    let invalid_channel = conn.execute(
        "INSERT INTO contact_suppression_list (channel, phone_or_email, reason, source, created_at)
         VALUES ('telegram', '+84901234567', 'opt_out', 'user_request', '2026-08-26T00:00:00Z')",
        [],
    );
    assert!(
        invalid_channel.is_err(),
        "Invalid channel must fail check constraint"
    );

    // Verify index exists
    let idx_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_suppression_lookup'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(idx_exists, "idx_suppression_lookup index must exist");
}

#[test]
fn test_whatsapp_outreach_campaigns_and_recipients_constraints() {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    apply_whatsapp_migrations(&conn).unwrap();

    // Insert campaign
    conn.execute(
        "INSERT INTO whatsapp_outreach_campaigns (
            name, status, created_at, updated_at
         ) VALUES (
            'August Promo', 'draft', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    )
    .unwrap();

    // Verify defaults
    let (total, sent, rate_min, rate_max, quiet_enabled): (i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT total_recipients, sent_count, rate_limit_min_delay_secs, rate_limit_max_delay_secs, quiet_hours_enabled
             FROM whatsapp_outreach_campaigns WHERE id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(total, 0);
    assert_eq!(sent, 0);
    assert_eq!(rate_min, 8);
    assert_eq!(rate_max, 20);
    assert_eq!(quiet_enabled, 1);

    // Insert lead
    conn.execute("INSERT INTO leads (id) VALUES (10)", [])
        .unwrap();

    // Insert recipient
    conn.execute(
        "INSERT INTO whatsapp_campaign_recipients (
            campaign_id, lead_id, recipient_phone_raw, recipient_phone_e164, recipient_jid,
            status, created_at, updated_at
         ) VALUES (
            1, 10, '0901234567', '+84901234567', '84901234567@s.whatsapp.net',
            'pending', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    )
    .unwrap();

    // Duplicate recipient_phone_e164 in same campaign should fail UNIQUE
    let dup_recip = conn.execute(
        "INSERT INTO whatsapp_campaign_recipients (
            campaign_id, lead_id, recipient_phone_raw, recipient_phone_e164, recipient_jid,
            status, created_at, updated_at
         ) VALUES (
            1, NULL, '0901234567', '+84901234567', '84901234567@s.whatsapp.net',
            'pending', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    );
    assert!(
        dup_recip.is_err(),
        "Duplicate (campaign_id, recipient_phone_e164) must fail unique constraint"
    );

    // Invalid status should fail CHECK constraint
    let invalid_status = conn.execute(
        "INSERT INTO whatsapp_campaign_recipients (
            campaign_id, recipient_phone_raw, recipient_phone_e164, recipient_jid,
            status, created_at, updated_at
         ) VALUES (
            1, '0909999999', '+84909999999', '84909999999@s.whatsapp.net',
            'unknown_status', '2026-08-26T00:00:00Z', '2026-08-26T00:00:00Z'
         )",
        [],
    );
    assert!(
        invalid_status.is_err(),
        "Invalid status must fail check constraint"
    );

    // CASCADE delete campaign deletes recipient
    conn.execute("DELETE FROM whatsapp_outreach_campaigns WHERE id = 1", [])
        .unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM whatsapp_campaign_recipients WHERE campaign_id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0, "Recipients should cascade delete with campaign");

    // Verify claim index exists
    let idx_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name='idx_wa_campaign_recipients_claim'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        idx_exists,
        "idx_wa_campaign_recipients_claim index must exist"
    );
}

#[test]
fn test_conversation_migrate_applies_whatsapp_migrations() {
    let conn = Connection::open_in_memory().unwrap();
    crate::conversation::migration::migrate(&conn).unwrap();

    // Verify all 4 tables exist after crate::conversation::migration::migrate
    let tables = [
        "whatsapp_sessions",
        "contact_suppression_list",
        "whatsapp_outreach_campaigns",
        "whatsapp_campaign_recipients",
    ];

    for table in tables {
        let sql = format!("SELECT count(*) FROM {}", table);
        let mut stmt = conn.prepare(&sql).unwrap();
        let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
        assert_eq!(count, 0, "Table {} must exist after migrate", table);
    }
}

#[test]
fn test_reconcile_duplicate_whatsapp_merges_lid_and_pn_identities_and_conversations() {
    let root = tempfile::tempdir().unwrap();
    let db_path = root.path().join("test_wa_reconcile.db");
    let conn = crate::db::init_db(db_path.to_str().unwrap()).unwrap();
    let now = "2026-08-27T00:00:00Z";

    // 1. Insert channel account
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901234567', 'WA Business', ?1, ?1)",
        [now],
    )
    .unwrap();

    // 2. Insert Lead "Vject Wang" with domestic China phone
    conn.execute(
        "INSERT INTO leads (id, name, company, status, value, last_activity, owner, email, phone)
         VALUES (10, 'Vject Wang', 'Acme', 'new', 0.0, '2026-08-27', 'admin', 'vject@test.com', '13926842913')",
        [],
    )
    .unwrap();

    // 3. Insert LID identity and conversation (e.g. from initial unlinked chat)
    conn.execute(
        "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, link_source, created_at, updated_at)
         VALUES (100, 'whatsapp', 1, 'phone', '198097598676992@lid', 'Vject Wang', 'unlinked', ?1, ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (200, 1, 'direct', '198097598676992@lid', 'Vject Wang', ?1, ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (200, 100, 'member', ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, status, ingested_at)
         VALUES (300, 200, 100, 'inbound', 'text', 'Hello from LID', 'received', ?1)",
        [now],
    )
    .unwrap();

    // 4. Insert PN identity and conversation (e.g. from inbound reply)
    conn.execute(
        "INSERT INTO channel_identities (id, channel, channel_account_id, identity_namespace, external_id, display_name, link_source, created_at, updated_at)
         VALUES (101, 'whatsapp', 1, 'phone', '8613926842913', 'Unknown (8613926842913)', 'unlinked', ?1, ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, kind, external_thread_id, title, created_at, updated_at)
         VALUES (201, 1, 'direct', '8613926842913@s.whatsapp.net', 'Unknown (8613926842913)', ?1, ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
         VALUES (201, 101, 'member', ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, status, ingested_at)
         VALUES (301, 201, 101, 'inbound', 'text', 'Can you tak picture', 'received', ?1)",
        [now],
    )
    .unwrap();

    // 5. Insert LID-to-PN mapping
    conn.execute(
        "INSERT INTO whatsapp_lid_pn_mappings (lid, phone_e164, created_at, updated_at)
         VALUES ('198097598676992', '8613926842913', ?1, ?1)",
        [now],
    )
    .unwrap();

    // Before reconcile: 2 conversations, 2 identities
    let conv_count_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM channel_conversations", [], |r| {
            r.get(0)
        })
        .unwrap();
    let ident_count_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM channel_identities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(conv_count_before, 2);
    assert_eq!(ident_count_before, 2);

    // Run reconciliation
    crate::conversation::migration::reconcile_duplicate_whatsapp_conversations_and_identities(
        &conn,
    )
    .unwrap();

    // After reconcile: exactly 1 conversation, 1 identity, linked to Lead 10 with name 'Vject Wang'
    let conv_count_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM channel_conversations", [], |r| {
            r.get(0)
        })
        .unwrap();
    let ident_count_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM channel_identities", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        conv_count_after, 1,
        "Duplicate conversation must be merged into 1"
    );
    assert_eq!(
        ident_count_after, 1,
        "Duplicate identity must be merged into 1"
    );

    let (surviving_ident_name, surviving_lead_id): (String, Option<i64>) = conn
        .query_row(
            "SELECT display_name, lead_id FROM channel_identities",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(surviving_ident_name, "Vject Wang");
    assert_eq!(
        surviving_lead_id,
        Some(10),
        "Identity must be linked to Lead 10"
    );

    let (surviving_conv_title, surviving_thread_id): (String, String) = conn
        .query_row(
            "SELECT title, external_thread_id FROM channel_conversations",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(surviving_conv_title, "Vject Wang");
    assert_eq!(surviving_thread_id, "8613926842913@s.whatsapp.net");

    // Both messages are preserved in the surviving conversation
    let msg_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = 201",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        msg_count, 2,
        "All messages must belong to surviving conversation"
    );

    // Verify Repository list_inbox returns only 1 Customer item
    let repo = crate::conversation::repository::ConversationRepository::new(
        db_path.to_str().unwrap().to_string(),
    );
    let inbox = repo.list_inbox().unwrap();
    assert_eq!(inbox.len(), 1, "Inbox must contain exactly 1 customer item");
    assert_eq!(inbox[0].title, "Vject Wang");
    assert_eq!(inbox[0].lead_id, Some(10));
}

#[test]
fn test_start_direct_conversation_for_whatsapp_links_lead_and_prevents_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let db_path = root.path().join("test_wa_start_direct.db");
    let conn = crate::db::init_db(db_path.to_str().unwrap()).unwrap();
    let now = "2026-08-27T00:00:00.000Z";

    // 1. Insert channel account
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, display_name, created_at, updated_at)
         VALUES (1, 'whatsapp', '84901234567', 'WA Business', ?1, ?1)",
        [now],
    )
    .unwrap();

    // 2. Insert Lead
    conn.execute(
        "INSERT INTO leads (id, name, company, status, value, last_activity, owner, email, phone)
         VALUES (20, 'Vject Wang', 'Acme', 'new', 0.0, '2026-08-27', 'admin', 'vject@test.com', '+86 139 2684 2913')",
        [],
    )
    .unwrap();

    let repo = crate::conversation::repository::ConversationRepository::new(
        db_path.to_str().unwrap().to_string(),
    );

    // 3. Start direct conversation from UI/Campaign
    let req = crate::conversation::types::StartDirectConversationRequest {
        channel_account_id: 1,
        phone_or_external_id: "8613926842913".to_string(),
        display_name: Some("Vject Wang".to_string()),
        initial_message: "Hello Vject!".to_string(),
    };

    let msg = repo.start_direct_conversation(&req, now).unwrap();
    assert_eq!(
        msg.direction,
        crate::conversation::types::MessageDirection::Outbound
    );

    // Check identity created
    let (channel, namespace, lead_id): (String, String, Option<i64>) = conn
        .query_row(
            "SELECT ci.channel, ci.identity_namespace, ci.lead_id
         FROM channel_identities ci
         JOIN conversation_participants cp ON cp.identity_id = ci.id
         WHERE cp.conversation_id = ?1",
            [msg.conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_eq!(channel, "whatsapp");
    assert_eq!(namespace, "phone");
    assert_eq!(lead_id, Some(20), "Must be linked to Lead 20");

    // Check conversation thread
    let (conv_title, ext_thread_id): (String, String) = conn
        .query_row(
            "SELECT title, external_thread_id FROM channel_conversations WHERE id = ?1",
            [msg.conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();

    assert_eq!(conv_title, "Vject Wang");
    assert_eq!(ext_thread_id, "8613926842913@s.whatsapp.net");

    // 4. Inbox should list exactly 1 customer row
    let inbox = repo.list_inbox().unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].title, "Vject Wang");
    assert_eq!(inbox[0].lead_id, Some(20));
}

#[test]
fn test_cleanup_corrupted_whatsapp_attachments_cleans_cdn_paths_and_preserves_local_spool() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();

    let now = "2026-08-27T00:00:00.000Z";

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
         VALUES (1, 'whatsapp', 'wa-clean-1', ?1, ?1)",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, status, created_at, updated_at)
         VALUES (10, 1, '84901234567@s.whatsapp.net', 'direct', 'Peer', 'active', ?1, ?1)",
        [now],
    )
    .unwrap();

    // 1. Corrupted inbound message with CDN path in local_path and key in raw_payload_json
    use base64::Engine as _;
    let key_b64 = base64::engine::general_purpose::STANDARD.encode(vec![0x33; 32]);
    let raw_payload = serde_json::json!({
        "id": "MSG_CORRUPT_01",
        "attachments": [{
            "media_type": "image",
            "direct_path": "/v/t62/corrupt.enc",
            "media_key": key_b64,
            "file_sha256": "sha123",
            "file_enc_sha256": "enc123"
        }]
    }).to_string();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, raw_payload_json, ingested_at, status)
         VALUES (100, 10, 'inbound', 'MSG_CORRUPT_01', 'image', ?1, ?2, 'received')",
        rusqlite::params![raw_payload, now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO message_attachments (id, message_id, attachment_type, local_path, remote_url, metadata_json, created_at)
         VALUES (1001, 100, 'image', '/v/t62/corrupt.enc', 'https://mmg.whatsapp.net/v/t62/corrupt.enc', '{}', ?1)",
        [now],
    )
    .unwrap();

    // 2. Valid outbound message with actual local spool path
    let local_spool_path = "/home/arch/.config/rustsale/attachments-spool/stage-valid-123.png";
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, provider_message_id, content_type, ingested_at, status)
         VALUES (200, 10, 'outbound', 'MSG_OUTBOUND_01', 'attachment', ?1, 'sent')",
        [now],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO message_attachments (id, message_id, attachment_type, local_path, remote_url, metadata_json, created_at)
         VALUES (2001, 200, 'image', ?1, NULL, '{}', ?2)",
        rusqlite::params![local_spool_path, now],
    )
    .unwrap();

    // Run migration
    crate::conversation::migration::migrate(&conn).unwrap();

    // Verify corrupted attachment: local_path NULLed, metadata_json backfilled
    let (c_local, c_meta): (Option<String>, String) = conn
        .query_row(
            "SELECT local_path, metadata_json FROM message_attachments WHERE id = 1001",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(c_local, None, "Corrupted CDN local_path must be NULL");
    let c_meta_val: serde_json::Value = serde_json::from_str(&c_meta).unwrap();
    assert_eq!(c_meta_val["media_key"], key_b64);
    assert_eq!(c_meta_val["file_sha256"], "sha123");

    // Verify valid outbound attachment: local_path preserved
    let (v_local, _): (Option<String>, String) = conn
        .query_row(
            "SELECT local_path, metadata_json FROM message_attachments WHERE id = 2001",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        v_local.as_deref(),
        Some(local_spool_path),
        "Valid local spool path must not be wiped"
    );

    // Verify idempotency: run migration second time
    crate::conversation::migration::migrate(&conn).unwrap();
    let (c_local2, c_meta2): (Option<String>, String) = conn
        .query_row(
            "SELECT local_path, metadata_json FROM message_attachments WHERE id = 1001",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(c_local2, None);
    assert_eq!(c_meta2, c_meta);
}
