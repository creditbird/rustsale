use rusqlite::{Connection, Result};

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column_name: &str,
    column_def: &str,
) -> Result<()> {
    let pragma_sql = format!("PRAGMA table_info({table})");
    let mut stmt = conn.prepare(&pragma_sql)?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>>>()?;

    if !cols.iter().any(|c| c.eq_ignore_ascii_case(column_name)) {
        let alter_sql = format!("ALTER TABLE {table} ADD COLUMN {column_name} {column_def}");
        conn.execute(&alter_sql, [])?;
    }
    Ok(())
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(include_str!("schema.sql"))?;

    add_column_if_missing(conn, "channel_identities", "profile_synced_at", "TEXT")?;
    add_column_if_missing(conn, "channel_identities", "profile_sync_error", "TEXT")?;
    add_column_if_missing(
        conn,
        "channel_identities",
        "profile_sync_attempts",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(conn, "channel_identities", "next_profile_retry_at", "TEXT")?;
    add_column_if_missing(conn, "channel_accounts", "self_external_id", "TEXT")?;
    add_column_if_missing(conn, "messages", "send_batch_id", "TEXT")?;
    add_column_if_missing(conn, "messages", "batch_index", "INTEGER")?;

    conn.execute_batch(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_send_batch_part
           ON messages(send_batch_id, batch_index)
           WHERE send_batch_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_provider_client_unique
           ON messages(conversation_id, provider_client_message_id)
           WHERE provider_client_message_id IS NOT NULL;",
    )?;

    Ok(())
}
