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
