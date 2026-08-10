use super::migration::migrate;
use crate::db::open_db_connection;

fn table_names(conn: &rusqlite::Connection) -> Vec<String> {
    let mut statement = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn conversation_schema_is_idempotent() {
    let path = std::env::temp_dir().join(format!(
        "rustsale-conversation-migration-{}.db",
        rand::random::<u64>()
    ));
    let conn = open_db_connection(path.to_str().unwrap()).unwrap();

    migrate(&conn).unwrap();
    migrate(&conn).unwrap();

    let tables = table_names(&conn);
    for expected in [
        "channel_accounts",
        "channel_identities",
        "channel_conversations",
        "conversation_participants",
        "messages",
        "message_attachments",
        "message_reactions",
        "message_receipts",
        "channel_sync_checkpoints",
        "message_outbox_jobs",
        "message_send_batches",
        "conversation_staged_attachments",
    ] {
        assert!(tables.contains(&expected.to_string()), "missing {expected}");
    }

    drop(conn);
    let _ = std::fs::remove_file(path);
}

fn column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
    let pragma_sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&pragma_sql).unwrap();
    stmt.query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn rich_chat_migration_adds_expected_columns() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    assert!(column_names(&conn, "channel_accounts").contains(&"self_external_id".into()));
    assert!(column_names(&conn, "messages").contains(&"send_batch_id".into()));
    assert!(column_names(&conn, "messages").contains(&"batch_index".into()));
    assert!(column_names(&conn, "message_reactions").contains(&"count".into()));
    assert!(column_names(&conn, "reaction_mutations").contains(&"count".into()));
}

#[test]
fn rich_chat_migration_preserves_existing_messages() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-a', 'disconnected', '{}', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'peer-1', 'direct', 'active', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages
         (conversation_id, direction, provider_message_id, content_type, content_json, ingested_at, status)
         VALUES (1, 'inbound', 'message-existing', 'text', '{}', '2026-07-31T00:00:00.000Z', 'received')",
        [],
    )
    .unwrap();

    // Re-run migration twice (idempotency check during upgrade)
    migrate(&conn).unwrap();
    migrate(&conn).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE provider_message_id = 'message-existing'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn reaction_count_columns_are_added_before_duplicate_account_merge() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();

    conn.execute_batch(
        "DROP TABLE reaction_mutations;
         DROP TABLE message_reactions;
         CREATE TABLE message_reactions (
             message_id INTEGER NOT NULL,
             actor_external_id TEXT NOT NULL,
             reaction TEXT NOT NULL,
             reacted_at TEXT,
             removed_at TEXT,
             PRIMARY KEY(message_id, actor_external_id, reaction),
             FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
         );
         CREATE TABLE reaction_mutations (
             client_mutation_id TEXT PRIMARY KEY,
             message_id INTEGER NOT NULL,
             reaction_code TEXT NOT NULL,
             active INTEGER NOT NULL CHECK (active IN (0, 1)),
             status TEXT NOT NULL DEFAULT 'running',
             error_code TEXT,
             error_message TEXT,
             created_at TEXT NOT NULL,
             updated_at TEXT NOT NULL,
             FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
         );
         DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id;
         INSERT INTO channel_accounts
             (id, channel, provider_account_key, self_external_id, display_name, created_at, updated_at)
         VALUES
             (1, 'zalo', 'old-account', 'same-self', 'Old', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z'),
             (2, 'zalo', 'new-account', 'same-self', 'New', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z');
         INSERT INTO channel_conversations
             (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES
             (10, 1, 'shared-thread', 'direct', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z'),
             (20, 2, 'shared-thread', 'direct', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z');
         INSERT INTO messages
             (id, conversation_id, direction, provider_message_id, provider_client_message_id,
              content_type, content_json, ingested_at, status)
         VALUES
             (100, 10, 'outbound', 'provider-1', 'client-1', 'text', '{}', '2026-08-01T00:00:00Z', 'sent'),
             (200, 20, 'outbound', 'provider-1', 'client-1', 'text', '{}', '2026-08-02T00:00:00Z', 'sent');
         INSERT INTO message_reactions
             (message_id, actor_external_id, reaction, reacted_at, removed_at)
         VALUES (200, 'peer-1', 'heart', '2026-08-02T00:01:00Z', NULL);",
    )
    .unwrap();

    migrate(&conn).unwrap();

    assert!(column_names(&conn, "message_reactions").contains(&"count".into()));
    assert!(column_names(&conn, "reaction_mutations").contains(&"count".into()));
    assert_eq!(
        conn.query_row(
            "SELECT count FROM message_reactions WHERE actor_external_id = 'peer-1' AND reaction = 'heart'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
}

#[test]
fn duplicate_zalo_merge_preserves_conflicting_state_and_all_dependents() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id",
        [],
    )
    .unwrap();

    conn.execute_batch(
        "INSERT INTO channel_accounts
            (id, channel, provider_account_key, self_external_id, display_name, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            (1, 'zalo', 'uid-merge', 'uid-merge', 'Canonical', 'disconnected', '{}', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z'),
            (2, 'zalo', 'imei-relogin', 'uid-merge', 'Relogin', 'connected', '{\"canSendText\":true}', '2026-08-02T00:00:00Z', '2026-08-02T00:00:00Z');
         INSERT INTO zalo_sessions
            (imei, user_agent, cookies_json, language, display_name)
         VALUES ('imei-relogin', 'UA/1.0', '[]', 'vi', 'Relogin');
         INSERT INTO channel_conversations
            (id, channel_account_id, external_thread_id, kind, title, status, last_message_at, last_synced_at, created_at, updated_at)
         VALUES
            (10, 1, 'shared-thread', 'direct', NULL, 'active', '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z', '2026-08-01T00:00:00Z', '2026-08-01T10:00:00Z'),
            (20, 2, 'shared-thread', 'direct', 'Peer Name', 'active', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z', '2026-08-02T00:00:00Z', '2026-08-02T10:00:00Z');
         INSERT INTO channel_identities
            (id, channel, identity_namespace, external_id, display_name, created_at, updated_at)
         VALUES (50, 'zalo', 'zalo_uid', 'peer-1', 'Peer One', '2026-08-01T00:00:00Z', '2026-08-01T00:00:00Z');
         INSERT INTO conversation_participants
            (conversation_id, identity_id, participant_role, joined_at, left_at)
         VALUES
            (10, 50, 'member', '2026-08-02T00:00:00Z', '2026-08-03T00:00:00Z'),
            (20, 50, 'admin', '2026-08-01T00:00:00Z', NULL);
         INSERT INTO messages
            (id, conversation_id, direction, provider_message_id, provider_client_message_id, content_type, text_content, content_json, ingested_at, sent_at, status)
         VALUES
            (100, 10, 'outbound', 'provider-1', 'client-1', 'text', NULL, '{}', '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z', 'sent'),
            (200, 20, 'outbound', 'provider-1', 'client-1', 'text', 'richer payload', '{\"text\":\"richer payload\"}', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z', 'read'),
            (201, 20, 'inbound', 'provider-2', 'client-2', 'text', 'replying message', '{}', '2026-08-02T11:00:00Z', NULL, 'received');
         UPDATE messages SET reply_to_message_id = 200 WHERE id = 201;
         INSERT INTO message_attachments
            (id, message_id, attachment_type, file_name, created_at)
         VALUES (300, 200, 'image', 'proof.png', '2026-08-02T10:00:00Z');
          INSERT INTO message_reactions
             (message_id, actor_external_id, reaction, count, reacted_at)
          VALUES (200, 'peer-1', 'heart', 5, '2026-08-02T10:01:00Z');
         INSERT INTO message_receipts
            (message_id, recipient_external_id, receipt_type, occurred_at)
         VALUES (200, 'peer-1', 'read', '2026-08-02T10:02:00Z');
         INSERT INTO message_mentions
            (id, message_id, identity_id, external_id, display_text, start_utf16, length_utf16, created_at)
         VALUES (400, 200, 50, 'peer-1', '@Peer', 0, 5, '2026-08-02T10:00:00Z');
         INSERT INTO reaction_mutations
            (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
         VALUES ('mutation-1', 200, 'heart', 1, 'succeeded', '2026-08-02T10:00:00Z', '2026-08-02T10:01:00Z');
         INSERT INTO message_forward_operations
            (client_operation_id, source_message_id, status, created_at, updated_at)
         VALUES ('forward-1', 200, 'sent', '2026-08-02T10:00:00Z', '2026-08-02T10:01:00Z');
         INSERT INTO message_outbox_jobs
            (id, message_id, channel_account_id, status, attempt_count, created_at, updated_at, completed_at)
         VALUES
            ('outbox-canonical', 100, 1, 'pending', 1, '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z', NULL),
            ('outbox-duplicate', 200, 2, 'completed', 3, '2026-08-02T10:00:00Z', '2026-08-02T10:03:00Z', '2026-08-02T10:03:00Z');
         INSERT INTO message_send_batches
            (id, conversation_id, channel_account_id, client_batch_id, reply_to_message_id, part_count, status, created_at, updated_at, completed_at)
         VALUES
            ('batch-canonical', 10, 1, 'client-batch-1', 100, 1, 'queued', '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z', NULL),
            ('batch-duplicate', 20, 2, 'client-batch-1', 200, 1, 'sent', '2026-08-02T10:00:00Z', '2026-08-02T10:03:00Z', '2026-08-02T10:03:00Z');
         INSERT INTO conversation_staged_attachments
            (id, conversation_id, staged_attachment_id, media_type, mime_type, file_name, file_size_bytes, local_path, status, expires_at, created_at)
         VALUES ('stage-1', 20, 'staged-1', 'image', 'image/png', 'upload.png', 42, '/tmp/upload.png', 'staged', '2026-08-10T00:00:00Z', '2026-08-02T10:00:00Z');
         INSERT INTO message_forward_targets
            (id, client_operation_id, target_conversation_id, send_batch_id, status, created_at, updated_at)
         VALUES
            (1, 'forward-1', 10, 'batch-canonical', 'queued', '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z'),
            (2, 'forward-1', 20, 'batch-duplicate', 'sent', '2026-08-02T10:00:00Z', '2026-08-02T10:03:00Z');
         INSERT INTO conversation_pinned_messages
            (id, conversation_id, message_id, pinned_by, pinned_at)
         VALUES
            (1, 10, 100, 'canonical', '2026-08-01T10:00:00Z'),
            (2, 20, 200, 'duplicate', '2026-08-02T10:00:00Z');
         INSERT INTO channel_sync_checkpoints
            (channel_account_id, scope, cursor, last_event_at, last_success_at, reconnect_count, last_error, updated_at)
         VALUES
            (1, 'direct_messages', 'cursor-old', '2026-08-01T10:00:00Z', '2026-08-01T10:00:00Z', 1, 'old error', '2026-08-01T10:00:00Z'),
            (2, 'direct_messages', 'cursor-new', '2026-08-02T10:00:00Z', '2026-08-02T10:00:00Z', 3, NULL, '2026-08-02T10:00:00Z');",
    )
    .unwrap();

    super::migration::reconcile_duplicate_zalo_channel_accounts(&conn).unwrap();

    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM channel_accounts WHERE channel = 'zalo' AND self_external_id = 'uid-merge'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT provider_account_key FROM channel_accounts WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        "uid-merge"
    );
    assert_eq!(
        conn.query_row(
            "SELECT connection_status FROM channel_accounts WHERE id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        "connected"
    );
    assert_eq!(
        conn.query_row(
            "SELECT zalo_uid, channel_account_id FROM zalo_sessions WHERE imei = 'imei-relogin'",
            [],
            |row| Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<i64>>(1)?,
            )),
        )
        .unwrap(),
        (Some("uid-merge".to_string()), Some(1))
    );
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE external_thread_id = 'shared-thread'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT title FROM channel_conversations WHERE id = 10",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        "Peer Name"
    );
    assert_eq!(
        conn.query_row(
            "SELECT status, text_content FROM messages WHERE id = 100",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .unwrap(),
        ("read".to_string(), Some("richer payload".to_string()))
    );
    assert_eq!(
        conn.query_row(
            "SELECT conversation_id, reply_to_message_id FROM messages WHERE id = 201",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .unwrap(),
        (10, Some(100))
    );

    for (table, foreign_key) in [
        ("message_attachments", "message_id"),
        ("message_reactions", "message_id"),
        ("message_receipts", "message_id"),
        ("message_mentions", "message_id"),
        ("reaction_mutations", "message_id"),
        ("message_forward_operations", "source_message_id"),
    ] {
        let sql = format!("SELECT COUNT(*) FROM {table} WHERE {foreign_key} = 100");
        assert_eq!(
            conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1,
            "{table} was not preserved"
        );
    }

    assert_eq!(
        conn.query_row(
            "SELECT count FROM message_reactions WHERE message_id = 100 AND actor_external_id = 'peer-1' AND reaction = 'heart'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        5
    );

    assert_eq!(
        conn.query_row(
            "SELECT participant_role, joined_at, left_at FROM conversation_participants WHERE conversation_id = 10 AND identity_id = 50",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, Option<String>>(2)?)),
        )
        .unwrap(),
        ("admin".to_string(), Some("2026-08-01T00:00:00Z".to_string()), None)
    );
    assert_eq!(
        conn.query_row(
            "SELECT message_id, channel_account_id, status, attempt_count FROM message_outbox_jobs",
            [],
            |row| Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            )),
        )
        .unwrap(),
        (100, 1, "completed".to_string(), 3)
    );
    assert_eq!(
        conn.query_row(
            "SELECT conversation_id, channel_account_id, reply_to_message_id, status FROM message_send_batches",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?, row.get::<_, Option<i64>>(2)?, row.get::<_, String>(3)?)),
        )
        .unwrap(),
        (10, 1, Some(100), "sent".to_string())
    );
    assert_eq!(
        conn.query_row(
            "SELECT conversation_id FROM conversation_staged_attachments WHERE id = 'stage-1'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        10
    );
    assert_eq!(
        conn.query_row(
            "SELECT target_conversation_id, send_batch_id, status FROM message_forward_targets",
            [],
            |row| Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            )),
        )
        .unwrap(),
        (10, Some("batch-canonical".to_string()), "sent".to_string())
    );
    assert_eq!(
        conn.query_row(
            "SELECT conversation_id, message_id FROM conversation_pinned_messages",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap(),
        (10, 100)
    );
    assert_eq!(
        conn.query_row(
            "SELECT cursor, reconnect_count FROM channel_sync_checkpoints WHERE channel_account_id = 1 AND scope = 'direct_messages'",
            [],
            |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, i64>(1)?)),
        )
        .unwrap(),
        (Some("cursor-new".to_string()), 3)
    );

    let foreign_key_violations: i64 = conn
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |_| Ok(()))
        .unwrap()
        .count() as i64;
    assert_eq!(foreign_key_violations, 0);
}

#[test]
fn provider_message_id_is_unique_within_conversation() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-a', 'disconnected', '{}', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'peer-1', 'direct', 'active', '2026-07-31T00:00:00.000Z', '2026-07-31T00:00:00.000Z')",
        [],
    )
    .unwrap();
    let insert = || {
        conn.execute(
            "INSERT INTO messages
             (conversation_id, direction, provider_message_id, content_type, content_json, ingested_at, status)
             VALUES (1, 'inbound', 'message-1', 'text', '{}', '2026-07-31T00:00:00.000Z', 'received')",
            [],
        )
    };

    insert().unwrap();
    assert!(insert().is_err());
}

#[test]
fn phase2a_migration_creates_template_table() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    let cols = column_names(&conn, "conversation_quick_templates");
    for expected in [
        "id",
        "shortcut",
        "title",
        "body_text",
        "category",
        "is_active",
        "usage_count",
        "last_used_at",
        "created_at",
        "updated_at",
    ] {
        assert!(
            cols.contains(&expected.to_string()),
            "missing column {expected} in conversation_quick_templates"
        );
    }

    conn.execute(
        "INSERT INTO conversation_quick_templates (shortcut, title, body_text, created_at, updated_at)
         VALUES ('/hello', 'Greeting', 'Hello there!', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    // Verify shortcut UNIQUE constraint
    let res = conn.execute(
        "INSERT INTO conversation_quick_templates (shortcut, title, body_text, created_at, updated_at)
         VALUES ('/hello', 'Greeting Duplicate', 'Hello again!', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    );
    assert!(res.is_err());
}

#[test]
fn phase2a_migration_creates_mentions_table() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&conn).unwrap();

    let cols = column_names(&conn, "message_mentions");
    for expected in [
        "id",
        "message_id",
        "identity_id",
        "external_id",
        "display_text",
        "start_utf16",
        "length_utf16",
        "created_at",
    ] {
        assert!(
            cols.contains(&expected.to_string()),
            "missing column {expected} in message_mentions"
        );
    }

    // Setup foreign key dependencies
    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-1', 'disconnected', '{}', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'thread-1', 'direct', 'active', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages
         (conversation_id, direction, content_type, text_content, ingested_at, status)
         VALUES (1, 'inbound', 'text', 'Hello @User', '2026-08-02T00:00:00.000Z', 'received')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO message_mentions
         (message_id, external_id, display_text, start_utf16, length_utf16, created_at)
         VALUES (1, 'user-123', '@User', 6, 5, '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    // Verify length_utf16 > 0 CHECK constraint
    let zero_len = conn.execute(
        "INSERT INTO message_mentions
         (message_id, external_id, display_text, start_utf16, length_utf16, created_at)
         VALUES (1, 'user-123', '@User', 6, 0, '2026-08-02T00:00:00.000Z')",
        [],
    );
    assert!(zero_len.is_err());

    // Verify UNIQUE(message_id, start_utf16, length_utf16) constraint
    let dup = conn.execute(
        "INSERT INTO message_mentions
         (message_id, external_id, display_text, start_utf16, length_utf16, created_at)
         VALUES (1, 'user-456', '@User2', 6, 5, '2026-08-02T00:00:00.000Z')",
        [],
    );
    assert!(dup.is_err());
}

#[test]
fn migration_adds_reply_sender_identity_reference() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&conn).unwrap();

    let cols = column_names(&conn, "messages");
    assert!(cols.contains(&"reply_sender_identity_id".to_string()));
}

#[test]
fn legacy_schema_adds_reply_sender_identity_before_creating_index() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();

    let legacy_schema = include_str!("schema.sql")
        .replace("    reply_sender_identity_id INTEGER,\n", "")
        .replace(
            ",\n    FOREIGN KEY(reply_sender_identity_id) REFERENCES channel_identities(id) ON DELETE SET NULL",
            "",
        )
        .replace(
            "\nCREATE INDEX IF NOT EXISTS idx_messages_reply_sender_identity\nON messages(reply_sender_identity_id)\nWHERE reply_sender_identity_id IS NOT NULL;\n",
            "\n",
        );
    conn.execute_batch(&legacy_schema).unwrap();

    migrate(&conn).unwrap();

    let cols = column_names(&conn, "messages");
    assert!(cols.contains(&"reply_sender_identity_id".to_string()));
    let index_exists = conn
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type = 'index' AND name = 'idx_messages_reply_sender_identity'
            )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap();
    assert!(index_exists);
}

#[test]
fn phase2a_migration_creates_reaction_mutations() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&conn).unwrap();

    let cols = column_names(&conn, "reaction_mutations");
    for expected in [
        "client_mutation_id",
        "message_id",
        "reaction_code",
        "active",
        "status",
        "error_code",
        "error_message",
        "created_at",
        "updated_at",
    ] {
        assert!(
            cols.contains(&expected.to_string()),
            "missing column {expected} in reaction_mutations"
        );
    }

    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-1', 'disconnected', '{}', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'thread-1', 'direct', 'active', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages
         (conversation_id, direction, content_type, text_content, ingested_at, status)
         VALUES (1, 'inbound', 'text', 'Test msg', '2026-08-02T00:00:00.000Z', 'received')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO reaction_mutations
         (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
         VALUES ('mut-1', 1, 'LIKE', 1, 'running', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    // Invalid active flag check
    let invalid_active = conn.execute(
        "INSERT INTO reaction_mutations
         (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
         VALUES ('mut-2', 1, 'LIKE', 2, 'running', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    );
    assert!(invalid_active.is_err());

    // Invalid status check
    let invalid_status = conn.execute(
        "INSERT INTO reaction_mutations
         (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at)
         VALUES ('mut-3', 1, 'LIKE', 1, 'invalid_status', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    );
    assert!(invalid_status.is_err());
}

#[test]
fn phase2a_migration_creates_forward_tables() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    migrate(&conn).unwrap();

    let ops_cols = column_names(&conn, "message_forward_operations");
    for expected in [
        "client_operation_id",
        "source_message_id",
        "status",
        "created_at",
        "updated_at",
    ] {
        assert!(
            ops_cols.contains(&expected.to_string()),
            "missing column {expected} in message_forward_operations"
        );
    }

    let target_cols = column_names(&conn, "message_forward_targets");
    for expected in [
        "id",
        "client_operation_id",
        "target_conversation_id",
        "send_batch_id",
        "status",
        "error_code",
        "error_message",
        "created_at",
        "updated_at",
    ] {
        assert!(
            target_cols.contains(&expected.to_string()),
            "missing column {expected} in message_forward_targets"
        );
    }

    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-1', 'disconnected', '{}', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'thread-1', 'direct', 'active', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'thread-2', 'direct', 'active', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages
         (conversation_id, direction, content_type, text_content, ingested_at, status)
         VALUES (1, 'inbound', 'text', 'Forward me', '2026-08-02T00:00:00.000Z', 'received')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO message_forward_operations
         (client_operation_id, source_message_id, status, created_at, updated_at)
         VALUES ('fwd-op-1', 1, 'queued', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO message_forward_targets
         (client_operation_id, target_conversation_id, status, created_at, updated_at)
         VALUES ('fwd-op-1', 2, 'queued', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();

    // Verify UNIQUE(client_operation_id, target_conversation_id) constraint
    let dup_target = conn.execute(
        "INSERT INTO message_forward_targets
         (client_operation_id, target_conversation_id, status, created_at, updated_at)
         VALUES ('fwd-op-1', 2, 'queued', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    );
    assert!(dup_target.is_err());

    // Verify CASCADE delete from operation to target
    conn.execute(
        "DELETE FROM message_forward_operations WHERE client_operation_id = 'fwd-op-1'",
        [],
    )
    .unwrap();

    let target_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_forward_targets WHERE client_operation_id = 'fwd-op-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(target_count, 0);
}

#[test]
fn phase2a_migration_is_idempotent() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    migrate(&conn).unwrap();
    migrate(&conn).unwrap();

    let tables = table_names(&conn);
    for expected in [
        "conversation_quick_templates",
        "message_mentions",
        "reaction_mutations",
        "message_forward_operations",
        "message_forward_targets",
    ] {
        assert!(
            tables.contains(&expected.to_string()),
            "missing Phase 2A table {expected}"
        );
    }
}

#[test]
fn phase2a_migration_preserves_phase1_rows() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', 'imei-phase1', 'disconnected', '{}', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, status, created_at, updated_at)
         VALUES (1, 'thread-phase1', 'direct', 'active', '2026-08-02T00:00:00.000Z', '2026-08-02T00:00:00.000Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages
         (conversation_id, direction, content_type, text_content, ingested_at, status)
         VALUES (1, 'inbound', 'text', 'Phase 1 content', '2026-08-02T00:00:00.000Z', 'received')",
        [],
    )
    .unwrap();

    // Re-run migration
    migrate(&conn).unwrap();

    let msg_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE text_content = 'Phase 1 content'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(msg_count, 1);
}

#[test]
fn reaction_reliability_migration_creates_pending_storage_and_repair_markers() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    let tables = table_names(&conn);
    for expected in [
        "pending_message_reactions",
        "channel_account_migration_markers",
    ] {
        assert!(tables.contains(&expected.to_string()), "missing {expected}");
    }

    let pending_columns = column_names(&conn, "pending_message_reactions");
    for expected in [
        "channel_account_id",
        "external_thread_id",
        "conversation_kind",
        "provider_event_id",
        "provider_message_id",
        "provider_client_message_id",
        "actor_external_id",
        "reaction",
        "removed",
        "occurred_at",
        "raw_payload_json",
    ] {
        assert!(
            pending_columns.contains(&expected.to_string()),
            "missing pending reaction column {expected}"
        );
    }
}

#[test]
fn reaction_history_recovery_resets_only_zalo_history_scopes() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();

    conn.execute_batch(
        "INSERT INTO channel_accounts
            (id, channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            (1, 'zalo', 'zalo-existing', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z'),
            (2, 'email', 'email-existing', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z');
         INSERT INTO channel_sync_checkpoints
            (channel_account_id, scope, cursor, updated_at)
         VALUES
            (1, 'direct_reaction_history', 'direct-history-cursor', '2026-08-09T00:00:00Z'),
            (1, 'group_reaction_history', 'group-history-cursor', '2026-08-09T00:00:00Z'),
            (1, 'direct_reactions', 'direct-realtime-watermark', '2026-08-09T00:00:00Z'),
            (1, 'group_reactions', 'group-realtime-watermark', '2026-08-09T00:00:00Z'),
            (1, 'direct_message_history', 'message-history-cursor', '2026-08-09T00:00:00Z'),
            (2, 'direct_reaction_history', 'email-history-cursor', '2026-08-09T00:00:00Z');",
    )
    .unwrap();

    migrate(&conn).unwrap();

    let mut statement = conn
        .prepare(
            "SELECT channel_account_id, scope, cursor
             FROM channel_sync_checkpoints
             ORDER BY channel_account_id, scope",
        )
        .unwrap();
    let remaining = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();

    assert_eq!(
        remaining,
        vec![
            (
                1,
                "direct_message_history".to_string(),
                "message-history-cursor".to_string(),
            ),
            (
                1,
                "direct_reactions".to_string(),
                "direct-realtime-watermark".to_string(),
            ),
            (
                1,
                "group_reactions".to_string(),
                "group-realtime-watermark".to_string(),
            ),
            (
                2,
                "direct_reaction_history".to_string(),
                "email-history-cursor".to_string(),
            ),
        ]
    );

    let marker_count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM channel_account_migration_markers
             WHERE channel_account_id = 1
               AND migration_key = 'zalo_reaction_history_recovery_v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(marker_count, 1);
}

#[test]
fn reaction_history_recovery_does_not_reset_new_cursor_after_marker() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    migrate(&conn).unwrap();
    conn.execute_batch(
        "INSERT INTO channel_accounts
            (id, channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            (1, 'zalo', 'zalo-existing', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z');
         INSERT INTO channel_sync_checkpoints
            (channel_account_id, scope, cursor, updated_at)
         VALUES
            (1, 'direct_reaction_history', 'stale-cursor', '2026-08-09T00:00:00Z');",
    )
    .unwrap();

    migrate(&conn).unwrap();
    let stale_count: i64 = conn
        .query_row(
            "SELECT COUNT(*)
             FROM channel_sync_checkpoints
             WHERE channel_account_id = 1 AND scope = 'direct_reaction_history'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stale_count, 0);

    conn.execute(
        "INSERT INTO channel_sync_checkpoints
            (channel_account_id, scope, cursor, updated_at)
         VALUES
            (1, 'direct_reaction_history', 'fresh-cursor', '2026-08-09T01:00:00Z')",
        [],
    )
    .unwrap();

    migrate(&conn).unwrap();

    let cursor: String = conn
        .query_row(
            "SELECT cursor
             FROM channel_sync_checkpoints
             WHERE channel_account_id = 1 AND scope = 'direct_reaction_history'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(cursor, "fresh-cursor");
}

#[test]
fn duplicate_zalo_merge_preserves_pending_reactions_and_repair_markers() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    conn.execute("CREATE TABLE leads (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    migrate(&conn).unwrap();
    conn.execute(
        "DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id",
        [],
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO channel_accounts
            (id, channel, provider_account_key, self_external_id, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            (1, 'zalo', 'uid-merge-reactions', 'uid-merge-reactions', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z'),
            (2, 'zalo', 'imei-merge-reactions', 'uid-merge-reactions', 'connected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z');
         INSERT INTO pending_message_reactions
            (channel_account_id, external_thread_id, conversation_kind, provider_event_id,
             provider_message_id, provider_client_message_id, actor_external_id, reaction,
             removed, occurred_at, raw_payload_json, created_at, updated_at)
         VALUES
            (2, 'peer-merge', 'direct', 'event-merge', 'message-merge', 'client-merge',
             'actor-merge', 'heart', 0, '2026-08-09T00:01:00Z', '{}',
             '2026-08-09T00:01:00Z', '2026-08-09T00:01:00Z');
         INSERT INTO channel_account_migration_markers
            (channel_account_id, migration_key, applied_at)
         VALUES
            (2, 'zalo_reaction_history_recovery_v1', '2026-08-09T00:00:00Z');",
    )
    .unwrap();

    super::migration::reconcile_duplicate_zalo_channel_accounts(&conn).unwrap();

    let pending_account_id: i64 = conn
        .query_row(
            "SELECT channel_account_id FROM pending_message_reactions",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(pending_account_id, 1);

    let marker_account_id: i64 = conn
        .query_row(
            "SELECT channel_account_id
             FROM channel_account_migration_markers
             WHERE migration_key = 'zalo_reaction_history_recovery_v1'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(marker_account_id, 1);
}
