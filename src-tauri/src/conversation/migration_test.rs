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
