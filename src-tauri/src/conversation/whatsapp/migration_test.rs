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
