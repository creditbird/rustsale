use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, Result};

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

    if !cols
        .iter()
        .any(|column| column.eq_ignore_ascii_case(column_name))
    {
        let alter_sql = format!("ALTER TABLE {table} ADD COLUMN {column_name} {column_def}");
        conn.execute(&alter_sql, [])?;
    }
    Ok(())
}

fn ensure_zalo_session_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS zalo_sessions (
            imei TEXT PRIMARY KEY,
            user_agent TEXT NOT NULL,
            cookies_json TEXT NOT NULL,
            language TEXT NOT NULL,
            display_name TEXT,
            avatar TEXT,
            auth_status TEXT NOT NULL DEFAULT 'unknown',
            auth_checked_at TEXT,
            auth_error TEXT,
            zalo_uid TEXT,
            channel_account_id INTEGER REFERENCES channel_accounts(id) ON DELETE SET NULL
         );",
    )?;
    add_column_if_missing(conn, "zalo_sessions", "display_name", "TEXT")?;
    add_column_if_missing(conn, "zalo_sessions", "avatar", "TEXT")?;
    add_column_if_missing(
        conn,
        "zalo_sessions",
        "auth_status",
        "TEXT NOT NULL DEFAULT 'unknown'",
    )?;
    add_column_if_missing(conn, "zalo_sessions", "auth_checked_at", "TEXT")?;
    add_column_if_missing(conn, "zalo_sessions", "auth_error", "TEXT")?;
    add_column_if_missing(conn, "zalo_sessions", "zalo_uid", "TEXT")?;
    add_column_if_missing(
        conn,
        "zalo_sessions",
        "channel_account_id",
        "INTEGER REFERENCES channel_accounts(id) ON DELETE SET NULL",
    )?;
    Ok(())
}

fn ensure_facebook_upload_sessions_schema_v2(conn: &Connection) -> Result<()> {
    let table_sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='facebook_upload_sessions'",
            [],
            |row| row.get(0),
        )
        .optional()?;

    if let Some(sql) = table_sql {
        // Inspect the table definition: if 'init_pending' is not in the CHECK constraint, rebuild table
        if !sql.contains("'init_pending'") {
            conn.execute_batch(
                "CREATE TABLE facebook_upload_sessions_v2 (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    post_id INTEGER NOT NULL REFERENCES facebook_scheduled_posts(id) ON DELETE CASCADE,
                    page_id TEXT NOT NULL,
                    upload_type TEXT NOT NULL DEFAULT 'video' CHECK (upload_type IN ('video', 'reel')),
                    upload_session_id TEXT NOT NULL UNIQUE,
                    video_id TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_hash TEXT NOT NULL,
                    file_size_bytes INTEGER NOT NULL,
                    start_offset INTEGER NOT NULL DEFAULT 0,
                    status TEXT NOT NULL DEFAULT 'init_pending' CHECK (status IN ('init_pending', 'in_progress', 'finishing', 'completed', 'failed')),
                    owner_id TEXT,
                    lease_deadline TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                INSERT INTO facebook_upload_sessions_v2 (
                    id, post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, owner_id, lease_deadline, created_at, updated_at
                )
                SELECT id, post_id, page_id, upload_type, upload_session_id, video_id, file_path, file_hash, file_size_bytes, start_offset, status, owner_id, lease_deadline, created_at, updated_at
                FROM facebook_upload_sessions;

                DROP TABLE facebook_upload_sessions;

                ALTER TABLE facebook_upload_sessions_v2 RENAME TO facebook_upload_sessions;",
            )?;
        }
    }

    conn.execute_batch(
        "DROP INDEX IF EXISTS idx_fb_upload_active_unique;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_fb_upload_active_unique
         ON facebook_upload_sessions(post_id, page_id, upload_type, file_hash, file_size_bytes)
         WHERE status IN ('init_pending', 'in_progress', 'finishing');

         DROP INDEX IF EXISTS idx_fb_scheduled_posts_status;
         CREATE INDEX IF NOT EXISTS idx_fb_scheduled_posts_status
         ON facebook_scheduled_posts(status, scheduled_publish_time, next_retry_at);",
    )?;

    Ok(())
}

pub fn migrate(conn: &Connection) -> Result<()> {
    super::migration_v2::apply_migration_v4(conn)?;

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
    add_column_if_missing(
        conn,
        "messages",
        "reply_sender_identity_id",
        "INTEGER REFERENCES channel_identities(id) ON DELETE SET NULL",
    )?;
    ensure_zalo_session_schema(conn)?;
    add_column_if_missing(
        conn,
        "message_reactions",
        "count",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(
        conn,
        "reaction_mutations",
        "count",
        "INTEGER NOT NULL DEFAULT 1",
    )?;
    add_column_if_missing(conn, "facebook_scheduled_posts", "next_retry_at", "TEXT")?;
    ensure_facebook_upload_sessions_schema_v2(conn)?;

    reconcile_duplicate_zalo_channel_accounts(conn)?;

    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_zalo_sessions_zalo_uid
           ON zalo_sessions(zalo_uid)
           WHERE zalo_uid IS NOT NULL AND zalo_uid != '';
         CREATE INDEX IF NOT EXISTS idx_zalo_sessions_account_id
           ON zalo_sessions(channel_account_id)
           WHERE channel_account_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_send_batch_part
           ON messages(send_batch_id, batch_index)
           WHERE send_batch_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_messages_provider_client_unique
           ON messages(conversation_id, provider_client_message_id)
           WHERE provider_client_message_id IS NOT NULL;
         CREATE INDEX IF NOT EXISTS idx_messages_reply_sender_identity
           ON messages(reply_sender_identity_id)
           WHERE reply_sender_identity_id IS NOT NULL;
         CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_accounts_zalo_self_ext_id
           ON channel_accounts(self_external_id)
           WHERE channel = 'zalo' AND self_external_id IS NOT NULL AND self_external_id != '';",
    )?;

    reset_zalo_reaction_history_once(conn)?;

    Ok(())
}

fn reset_zalo_reaction_history_once(conn: &Connection) -> Result<()> {
    const MIGRATION_KEY: &str = "zalo_reaction_history_recovery_v1";

    conn.execute(
        "DELETE FROM channel_sync_checkpoints
         WHERE scope IN ('direct_reaction_history', 'group_reaction_history')
           AND channel_account_id IN (
               SELECT ca.id
               FROM channel_accounts ca
               WHERE ca.channel = 'zalo'
                 AND NOT EXISTS (
                     SELECT 1
                     FROM channel_account_migration_markers marker
                     WHERE marker.channel_account_id = ca.id
                       AND marker.migration_key = ?1
                 )
           )",
        [MIGRATION_KEY],
    )?;

    conn.execute(
        "INSERT INTO channel_account_migration_markers
            (channel_account_id, migration_key, applied_at)
         SELECT id, ?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         FROM channel_accounts
         WHERE channel = 'zalo'
         ON CONFLICT(channel_account_id, migration_key) DO NOTHING",
        [MIGRATION_KEY],
    )?;

    Ok(())
}

#[derive(Clone)]
struct ZaloAccountRow {
    id: i64,
    provider_account_key: String,
}

#[derive(Clone)]
struct AccountMetadata {
    display_name: Option<String>,
    avatar_url: Option<String>,
    enabled: i64,
    connection_status: String,
    last_connected_at: Option<String>,
    last_error: Option<String>,
    capabilities_json: String,
    created_at: String,
    updated_at: String,
}

fn earlier(left: &str, right: &str) -> String {
    if left <= right {
        left.to_string()
    } else {
        right.to_string()
    }
}

fn later(left: &str, right: &str) -> String {
    if left >= right {
        left.to_string()
    } else {
        right.to_string()
    }
}

fn later_optional(left: Option<String>, right: Option<String>) -> Option<String> {
    match (left, right) {
        (Some(left), Some(right)) => Some(later(&left, &right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn account_status_rank(status: &str) -> i32 {
    match status {
        "connected" => 4,
        "connecting" => 3,
        "degraded" => 2,
        "disconnected" => 1,
        _ => 0,
    }
}

fn message_status_rank(status: &str) -> i32 {
    match status {
        "removed" => 70,
        "read" => 60,
        "delivered" => 50,
        "sent" => 40,
        "received" | "accepted" => 35,
        "sending" => 20,
        "queued" => 10,
        "failed" => 5,
        _ => 0,
    }
}

fn outbox_status_rank(status: &str) -> i32 {
    match status {
        "completed" => 60,
        "running" => 50,
        "retry_wait" => 40,
        "pending" => 30,
        "failed" => 20,
        "cancelled" => 10,
        _ => 0,
    }
}

fn batch_status_rank(status: &str) -> i32 {
    match status {
        "sent" => 50,
        "partial" => 40,
        "sending" => 30,
        "queued" => 20,
        "failed" => 10,
        _ => 0,
    }
}

fn forward_target_status_rank(status: &str) -> i32 {
    match status {
        "sent" => 50,
        "delivery_unknown" => 40,
        "sending" => 30,
        "queued" => 20,
        "failed" => 10,
        _ => 0,
    }
}

fn load_account_metadata(conn: &Connection, account_id: i64) -> Result<AccountMetadata> {
    conn.query_row(
        "SELECT display_name, avatar_url, enabled, connection_status, last_connected_at,
                last_error, capabilities_json, created_at, updated_at
         FROM channel_accounts WHERE id = ?1",
        [account_id],
        |row| {
            Ok(AccountMetadata {
                display_name: row.get(0)?,
                avatar_url: row.get(1)?,
                enabled: row.get(2)?,
                connection_status: row.get(3)?,
                last_connected_at: row.get(4)?,
                last_error: row.get(5)?,
                capabilities_json: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
}

fn merge_account_metadata(
    conn: &Connection,
    canonical_id: i64,
    duplicate_id: i64,
    zalo_uid: &str,
) -> Result<()> {
    let canonical = load_account_metadata(conn, canonical_id)?;
    let duplicate = load_account_metadata(conn, duplicate_id)?;
    let duplicate_is_newer = duplicate.updated_at > canonical.updated_at;
    let duplicate_status_wins = account_status_rank(&duplicate.connection_status)
        > account_status_rank(&canonical.connection_status)
        || (account_status_rank(&duplicate.connection_status)
            == account_status_rank(&canonical.connection_status)
            && duplicate_is_newer);
    let connection_status = if duplicate_status_wins {
        duplicate.connection_status.clone()
    } else {
        canonical.connection_status.clone()
    };
    let last_error = if duplicate_status_wins {
        duplicate.last_error.clone()
    } else {
        canonical.last_error.clone()
    };
    let capabilities_json = if canonical.capabilities_json.trim().is_empty()
        || canonical.capabilities_json.trim() == "{}"
    {
        duplicate.capabilities_json.clone()
    } else {
        canonical.capabilities_json.clone()
    };

    conn.execute(
        "UPDATE channel_accounts
         SET self_external_id = ?2,
             display_name = ?3,
             avatar_url = ?4,
             enabled = ?5,
             connection_status = ?6,
             last_connected_at = ?7,
             last_error = ?8,
             capabilities_json = ?9,
             created_at = ?10,
             updated_at = ?11
         WHERE id = ?1",
        params![
            canonical_id,
            zalo_uid,
            canonical.display_name.or(duplicate.display_name),
            canonical.avatar_url.or(duplicate.avatar_url),
            canonical.enabled.max(duplicate.enabled),
            connection_status,
            later_optional(canonical.last_connected_at, duplicate.last_connected_at),
            last_error,
            capabilities_json,
            earlier(&canonical.created_at, &duplicate.created_at),
            later(&canonical.updated_at, &duplicate.updated_at),
        ],
    )?;
    Ok(())
}

fn merge_conversation_metadata(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE channel_conversations
         SET title = COALESCE(title, (SELECT title FROM channel_conversations WHERE id = ?2)),
             avatar_url = COALESCE(avatar_url, (SELECT avatar_url FROM channel_conversations WHERE id = ?2)),
             status = CASE
                 WHEN status = 'active' OR (SELECT status FROM channel_conversations WHERE id = ?2) != 'active'
                 THEN status
                 ELSE (SELECT status FROM channel_conversations WHERE id = ?2)
             END,
             last_message_at = CASE
                 WHEN last_message_at IS NULL THEN (SELECT last_message_at FROM channel_conversations WHERE id = ?2)
                 WHEN (SELECT last_message_at FROM channel_conversations WHERE id = ?2) IS NULL THEN last_message_at
                 WHEN (SELECT last_message_at FROM channel_conversations WHERE id = ?2) > last_message_at
                 THEN (SELECT last_message_at FROM channel_conversations WHERE id = ?2)
                 ELSE last_message_at
             END,
             last_synced_at = CASE
                 WHEN last_synced_at IS NULL THEN (SELECT last_synced_at FROM channel_conversations WHERE id = ?2)
                 WHEN (SELECT last_synced_at FROM channel_conversations WHERE id = ?2) IS NULL THEN last_synced_at
                 WHEN (SELECT last_synced_at FROM channel_conversations WHERE id = ?2) > last_synced_at
                 THEN (SELECT last_synced_at FROM channel_conversations WHERE id = ?2)
                 ELSE last_synced_at
             END,
             created_at = MIN(created_at, (SELECT created_at FROM channel_conversations WHERE id = ?2)),
             updated_at = MAX(updated_at, (SELECT updated_at FROM channel_conversations WHERE id = ?2))
         WHERE id = ?1",
        params![canonical_conversation_id, duplicate_conversation_id],
    )?;
    Ok(())
}

#[derive(Clone)]
struct MessageKey {
    id: i64,
    provider_message_id: Option<String>,
    provider_client_message_id: Option<String>,
    local_request_id: Option<String>,
    send_batch_id: Option<String>,
    batch_index: Option<i64>,
}

fn meaningful(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn find_matching_message_ids(
    conn: &Connection,
    canonical_conversation_id: i64,
    message: &MessageKey,
) -> Result<BTreeSet<i64>> {
    let mut ids = BTreeSet::new();
    if let Some(provider_message_id) = meaningful(message.provider_message_id.clone()) {
        if let Some(id) = conn
            .query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![canonical_conversation_id, provider_message_id],
                |row| row.get(0),
            )
            .optional()?
        {
            ids.insert(id);
        }
    }
    if let Some(provider_client_message_id) = meaningful(message.provider_client_message_id.clone())
    {
        if let Some(id) = conn
            .query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_client_message_id = ?2",
                params![canonical_conversation_id, provider_client_message_id],
                |row| row.get(0),
            )
            .optional()?
        {
            ids.insert(id);
        }
    }
    if let Some(local_request_id) = meaningful(message.local_request_id.clone()) {
        if let Some(id) = conn
            .query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND local_request_id = ?2",
                params![canonical_conversation_id, local_request_id],
                |row| row.get(0),
            )
            .optional()?
        {
            ids.insert(id);
        }
    }
    if let (Some(send_batch_id), Some(batch_index)) = (
        meaningful(message.send_batch_id.clone()),
        message.batch_index,
    ) {
        if let Some(id) = conn
            .query_row(
                "SELECT canonical_message.id
                 FROM messages canonical_message
                 JOIN message_send_batches canonical_batch
                   ON canonical_batch.id = canonical_message.send_batch_id
                 JOIN message_send_batches duplicate_batch
                   ON duplicate_batch.id = ?2
                 WHERE canonical_message.conversation_id = ?1
                   AND canonical_batch.client_batch_id = duplicate_batch.client_batch_id
                   AND canonical_message.batch_index = ?3
                 LIMIT 1",
                params![canonical_conversation_id, send_batch_id, batch_index],
                |row| row.get(0),
            )
            .optional()?
        {
            ids.insert(id);
        }
    }
    Ok(ids)
}

#[derive(Clone)]
struct OutboxJob {
    id: String,
    status: String,
    attempt_count: i64,
    next_attempt_at: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

fn load_outbox_job(conn: &Connection, message_id: i64) -> Result<Option<OutboxJob>> {
    conn.query_row(
        "SELECT id, status, attempt_count, next_attempt_at, last_error, created_at, updated_at, completed_at
         FROM message_outbox_jobs WHERE message_id = ?1",
        [message_id],
        |row| {
            Ok(OutboxJob {
                id: row.get(0)?,
                status: row.get(1)?,
                attempt_count: row.get(2)?,
                next_attempt_at: row.get(3)?,
                last_error: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                completed_at: row.get(7)?,
            })
        },
    )
    .optional()
}

fn merge_outbox_jobs(
    conn: &Connection,
    canonical_message_id: i64,
    duplicate_message_id: i64,
) -> Result<()> {
    let canonical = load_outbox_job(conn, canonical_message_id)?;
    let duplicate = load_outbox_job(conn, duplicate_message_id)?;
    match (canonical, duplicate) {
        (Some(canonical), Some(duplicate)) => {
            let duplicate_wins = outbox_status_rank(&duplicate.status)
                > outbox_status_rank(&canonical.status)
                || (outbox_status_rank(&duplicate.status) == outbox_status_rank(&canonical.status)
                    && duplicate.updated_at > canonical.updated_at);
            conn.execute(
                "UPDATE message_outbox_jobs
                 SET status = ?2,
                     attempt_count = ?3,
                     next_attempt_at = ?4,
                     last_error = ?5,
                     created_at = ?6,
                     updated_at = ?7,
                     completed_at = ?8
                 WHERE id = ?1",
                params![
                    canonical.id,
                    if duplicate_wins {
                        duplicate.status.clone()
                    } else {
                        canonical.status.clone()
                    },
                    canonical.attempt_count.max(duplicate.attempt_count),
                    later_optional(canonical.next_attempt_at, duplicate.next_attempt_at),
                    if duplicate_wins {
                        duplicate.last_error.clone()
                    } else {
                        canonical.last_error.clone()
                    },
                    earlier(&canonical.created_at, &duplicate.created_at),
                    later(&canonical.updated_at, &duplicate.updated_at),
                    later_optional(canonical.completed_at, duplicate.completed_at),
                ],
            )?;
            conn.execute(
                "DELETE FROM message_outbox_jobs WHERE id = ?1",
                [duplicate.id],
            )?;
        }
        (None, Some(duplicate)) => {
            conn.execute(
                "UPDATE message_outbox_jobs SET message_id = ?1 WHERE id = ?2",
                params![canonical_message_id, duplicate.id],
            )?;
        }
        _ => {}
    }
    Ok(())
}

fn merge_message_children(
    conn: &Connection,
    canonical_message_id: i64,
    duplicate_message_id: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE message_attachments SET message_id = ?1 WHERE message_id = ?2",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "INSERT INTO message_reactions
            (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
         SELECT ?1, actor_external_id, reaction, count, reacted_at, removed_at
         FROM message_reactions WHERE message_id = ?2 AND true
         ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
            count = MAX(message_reactions.count, excluded.count),
            reacted_at = CASE
                WHEN excluded.reacted_at IS NULL THEN message_reactions.reacted_at
                WHEN message_reactions.reacted_at IS NULL OR excluded.reacted_at > message_reactions.reacted_at
                THEN excluded.reacted_at ELSE message_reactions.reacted_at END,
            removed_at = CASE
                WHEN excluded.removed_at IS NULL THEN message_reactions.removed_at
                WHEN message_reactions.removed_at IS NULL OR excluded.removed_at > message_reactions.removed_at
                THEN excluded.removed_at ELSE message_reactions.removed_at END",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "DELETE FROM message_reactions WHERE message_id = ?1",
        [duplicate_message_id],
    )?;
    conn.execute(
        "INSERT INTO message_receipts
            (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
         SELECT ?1, recipient_external_id, receipt_type, occurred_at, raw_payload_json
         FROM message_receipts WHERE message_id = ?2 AND true
         ON CONFLICT(message_id, recipient_external_id, receipt_type) DO UPDATE SET
            occurred_at = MAX(message_receipts.occurred_at, excluded.occurred_at),
            raw_payload_json = CASE
                WHEN excluded.occurred_at >= message_receipts.occurred_at
                THEN COALESCE(excluded.raw_payload_json, message_receipts.raw_payload_json)
                ELSE message_receipts.raw_payload_json END",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "DELETE FROM message_receipts WHERE message_id = ?1",
        [duplicate_message_id],
    )?;
    conn.execute(
        "INSERT INTO message_mentions
            (message_id, identity_id, external_id, display_text, start_utf16, length_utf16, created_at)
         SELECT ?1, identity_id, external_id, display_text, start_utf16, length_utf16, created_at
         FROM message_mentions WHERE message_id = ?2 AND true
         ON CONFLICT(message_id, start_utf16, length_utf16) DO UPDATE SET
            identity_id = COALESCE(message_mentions.identity_id, excluded.identity_id),
            external_id = COALESCE(message_mentions.external_id, excluded.external_id),
            display_text = CASE
                WHEN LENGTH(excluded.display_text) > LENGTH(message_mentions.display_text)
                THEN excluded.display_text ELSE message_mentions.display_text END,
            created_at = MIN(message_mentions.created_at, excluded.created_at)",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "DELETE FROM message_mentions WHERE message_id = ?1",
        [duplicate_message_id],
    )?;
    conn.execute(
        "UPDATE reaction_mutations SET message_id = ?1 WHERE message_id = ?2",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "UPDATE message_forward_operations SET source_message_id = ?1 WHERE source_message_id = ?2",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "INSERT INTO conversation_pinned_messages
            (conversation_id, message_id, pinned_by, pinned_at)
         SELECT conversation_id, ?1, pinned_by, pinned_at
         FROM conversation_pinned_messages WHERE message_id = ?2 AND true
         ON CONFLICT(conversation_id, message_id) DO UPDATE SET
            pinned_by = COALESCE(excluded.pinned_by, conversation_pinned_messages.pinned_by),
            pinned_at = MAX(conversation_pinned_messages.pinned_at, excluded.pinned_at)",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "DELETE FROM conversation_pinned_messages WHERE message_id = ?1",
        [duplicate_message_id],
    )?;
    merge_outbox_jobs(conn, canonical_message_id, duplicate_message_id)?;
    conn.execute(
        "UPDATE messages SET reply_to_message_id = ?1 WHERE reply_to_message_id = ?2",
        params![canonical_message_id, duplicate_message_id],
    )?;
    conn.execute(
        "UPDATE message_send_batches SET reply_to_message_id = ?1 WHERE reply_to_message_id = ?2",
        params![canonical_message_id, duplicate_message_id],
    )?;
    Ok(())
}

fn merge_message(
    conn: &Connection,
    canonical_message_id: i64,
    duplicate_message_id: i64,
) -> Result<()> {
    if canonical_message_id == duplicate_message_id {
        return Ok(());
    }
    merge_message_children(conn, canonical_message_id, duplicate_message_id)?;

    let canonical_status: String = conn.query_row(
        "SELECT status FROM messages WHERE id = ?1",
        [canonical_message_id],
        |row| row.get(0),
    )?;
    let duplicate_status: String = conn.query_row(
        "SELECT status FROM messages WHERE id = ?1",
        [duplicate_message_id],
        |row| row.get(0),
    )?;
    let merged_status =
        if message_status_rank(&duplicate_status) > message_status_rank(&canonical_status) {
            duplicate_status
        } else {
            canonical_status
        };

    conn.execute(
        "UPDATE messages
         SET sender_identity_id = COALESCE(sender_identity_id, (SELECT sender_identity_id FROM messages WHERE id = ?2)),
             provider_message_id = COALESCE(NULLIF(provider_message_id, ''), NULLIF((SELECT provider_message_id FROM messages WHERE id = ?2), '')),
             provider_client_message_id = COALESCE(NULLIF(provider_client_message_id, ''), NULLIF((SELECT provider_client_message_id FROM messages WHERE id = ?2), '')),
             local_request_id = COALESCE(local_request_id, (SELECT local_request_id FROM messages WHERE id = ?2)),
             text_content = CASE
                 WHEN (text_content IS NULL OR text_content = '')
                      AND COALESCE((SELECT text_content FROM messages WHERE id = ?2), '') != ''
                 THEN (SELECT text_content FROM messages WHERE id = ?2)
                 ELSE text_content END,
             content_json = CASE
                 WHEN LENGTH(COALESCE((SELECT content_json FROM messages WHERE id = ?2), '')) > LENGTH(COALESCE(content_json, ''))
                 THEN (SELECT content_json FROM messages WHERE id = ?2)
                 ELSE content_json END,
             raw_payload_json = CASE
                 WHEN LENGTH(COALESCE((SELECT raw_payload_json FROM messages WHERE id = ?2), '')) > LENGTH(COALESCE(raw_payload_json, ''))
                 THEN (SELECT raw_payload_json FROM messages WHERE id = ?2)
                 ELSE raw_payload_json END,
             provider_timestamp = CASE
                 WHEN provider_timestamp IS NULL THEN (SELECT provider_timestamp FROM messages WHERE id = ?2)
                 WHEN (SELECT provider_timestamp FROM messages WHERE id = ?2) IS NULL THEN provider_timestamp
                 ELSE MIN(provider_timestamp, (SELECT provider_timestamp FROM messages WHERE id = ?2)) END,
             ingested_at = MIN(ingested_at, (SELECT ingested_at FROM messages WHERE id = ?2)),
             sent_at = CASE
                 WHEN sent_at IS NULL THEN (SELECT sent_at FROM messages WHERE id = ?2)
                 WHEN (SELECT sent_at FROM messages WHERE id = ?2) IS NULL THEN sent_at
                 ELSE MIN(sent_at, (SELECT sent_at FROM messages WHERE id = ?2)) END,
             edited_at = CASE
                 WHEN edited_at IS NULL THEN (SELECT edited_at FROM messages WHERE id = ?2)
                 WHEN (SELECT edited_at FROM messages WHERE id = ?2) IS NULL THEN edited_at
                 ELSE MAX(edited_at, (SELECT edited_at FROM messages WHERE id = ?2)) END,
             removed_at = CASE
                 WHEN removed_at IS NULL THEN (SELECT removed_at FROM messages WHERE id = ?2)
                 WHEN (SELECT removed_at FROM messages WHERE id = ?2) IS NULL THEN removed_at
                 ELSE MAX(removed_at, (SELECT removed_at FROM messages WHERE id = ?2)) END,
             status = ?3,
             error_code = COALESCE(error_code, (SELECT error_code FROM messages WHERE id = ?2)),
             error_message = COALESCE(error_message, (SELECT error_message FROM messages WHERE id = ?2)),
             reply_to_message_id = COALESCE(reply_to_message_id, (SELECT reply_to_message_id FROM messages WHERE id = ?2)),
             send_batch_id = COALESCE(send_batch_id, (SELECT send_batch_id FROM messages WHERE id = ?2)),
             batch_index = COALESCE(batch_index, (SELECT batch_index FROM messages WHERE id = ?2))
         WHERE id = ?1",
        params![canonical_message_id, duplicate_message_id, merged_status],
    )?;
    conn.execute("DELETE FROM messages WHERE id = ?1", [duplicate_message_id])?;
    Ok(())
}

fn merge_messages(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, provider_message_id, provider_client_message_id, local_request_id, send_batch_id, batch_index
         FROM messages WHERE conversation_id = ?1 ORDER BY id",
    )?;
    let duplicate_messages = stmt
        .query_map([duplicate_conversation_id], |row| {
            Ok(MessageKey {
                id: row.get(0)?,
                provider_message_id: row.get(1)?,
                provider_client_message_id: row.get(2)?,
                local_request_id: row.get(3)?,
                send_batch_id: row.get(4)?,
                batch_index: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>>>()?;

    for message in duplicate_messages {
        let candidates = find_matching_message_ids(conn, canonical_conversation_id, &message)?;
        if let Some(canonical_message_id) = candidates.iter().next().copied() {
            for extra_candidate in candidates.into_iter().skip(1) {
                merge_message(conn, canonical_message_id, extra_candidate)?;
            }
            merge_message(conn, canonical_message_id, message.id)?;
        } else {
            conn.execute(
                "UPDATE messages SET conversation_id = ?1 WHERE id = ?2",
                params![canonical_conversation_id, message.id],
            )?;
        }
    }
    Ok(())
}

fn merge_participants(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO conversation_participants
            (conversation_id, identity_id, participant_role, joined_at, left_at)
         SELECT ?1, identity_id, participant_role, joined_at, left_at
         FROM conversation_participants WHERE conversation_id = ?2 AND true
         ON CONFLICT(conversation_id, identity_id) DO UPDATE SET
            participant_role = CASE
                WHEN CASE excluded.participant_role
                    WHEN 'owner' THEN 4 WHEN 'admin' THEN 3 WHEN 'moderator' THEN 2 ELSE 1 END
                   > CASE conversation_participants.participant_role
                    WHEN 'owner' THEN 4 WHEN 'admin' THEN 3 WHEN 'moderator' THEN 2 ELSE 1 END
                THEN excluded.participant_role ELSE conversation_participants.participant_role END,
            joined_at = CASE
                WHEN conversation_participants.joined_at IS NULL THEN excluded.joined_at
                WHEN excluded.joined_at IS NULL THEN conversation_participants.joined_at
                ELSE MIN(conversation_participants.joined_at, excluded.joined_at) END,
            left_at = CASE
                WHEN conversation_participants.left_at IS NULL OR excluded.left_at IS NULL THEN NULL
                ELSE MAX(conversation_participants.left_at, excluded.left_at) END",
        params![canonical_conversation_id, duplicate_conversation_id],
    )?;
    conn.execute(
        "DELETE FROM conversation_participants WHERE conversation_id = ?1",
        [duplicate_conversation_id],
    )?;
    Ok(())
}

#[derive(Clone)]
struct SendBatch {
    id: String,
    reply_to_message_id: Option<i64>,
    part_count: i64,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

fn load_send_batch(conn: &Connection, batch_id: &str) -> Result<SendBatch> {
    conn.query_row(
        "SELECT id, reply_to_message_id, part_count, status, error_code, error_message,
                created_at, updated_at, completed_at
         FROM message_send_batches WHERE id = ?1",
        [batch_id],
        |row| {
            Ok(SendBatch {
                id: row.get(0)?,
                reply_to_message_id: row.get(1)?,
                part_count: row.get(2)?,
                status: row.get(3)?,
                error_code: row.get(4)?,
                error_message: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                completed_at: row.get(8)?,
            })
        },
    )
}

fn merge_send_batch(
    conn: &Connection,
    canonical_batch_id: &str,
    duplicate_batch_id: &str,
) -> Result<()> {
    if canonical_batch_id == duplicate_batch_id {
        return Ok(());
    }
    let canonical = load_send_batch(conn, canonical_batch_id)?;
    let duplicate = load_send_batch(conn, duplicate_batch_id)?;
    conn.execute(
        "UPDATE messages SET send_batch_id = ?1 WHERE send_batch_id = ?2",
        params![canonical_batch_id, duplicate_batch_id],
    )?;
    conn.execute(
        "UPDATE message_forward_targets SET send_batch_id = ?1 WHERE send_batch_id = ?2",
        params![canonical_batch_id, duplicate_batch_id],
    )?;
    let duplicate_wins = batch_status_rank(&duplicate.status)
        > batch_status_rank(&canonical.status)
        || (batch_status_rank(&duplicate.status) == batch_status_rank(&canonical.status)
            && duplicate.updated_at > canonical.updated_at);
    conn.execute(
        "UPDATE message_send_batches
         SET reply_to_message_id = ?2,
             part_count = ?3,
             status = ?4,
             error_code = ?5,
             error_message = ?6,
             created_at = ?7,
             updated_at = ?8,
             completed_at = ?9
         WHERE id = ?1",
        params![
            canonical.id,
            canonical
                .reply_to_message_id
                .or(duplicate.reply_to_message_id),
            canonical.part_count.max(duplicate.part_count),
            if duplicate_wins {
                duplicate.status.clone()
            } else {
                canonical.status.clone()
            },
            if duplicate_wins {
                duplicate.error_code.clone()
            } else {
                canonical.error_code.clone()
            },
            if duplicate_wins {
                duplicate.error_message.clone()
            } else {
                canonical.error_message.clone()
            },
            earlier(&canonical.created_at, &duplicate.created_at),
            later(&canonical.updated_at, &duplicate.updated_at),
            later_optional(canonical.completed_at, duplicate.completed_at),
        ],
    )?;
    conn.execute(
        "DELETE FROM message_send_batches WHERE id = ?1",
        [duplicate_batch_id],
    )?;
    Ok(())
}

fn merge_send_batches_for_conversation(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
    canonical_account_id: i64,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, client_batch_id FROM message_send_batches WHERE conversation_id = ?1",
    )?;
    let batches = stmt
        .query_map([duplicate_conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>>>()?;
    for (batch_id, client_batch_id) in batches {
        let canonical_batch_id: Option<String> = conn
            .query_row(
                "SELECT id FROM message_send_batches
                 WHERE channel_account_id = ?1 AND client_batch_id = ?2
                 ORDER BY id LIMIT 1",
                params![canonical_account_id, client_batch_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(canonical_batch_id) = canonical_batch_id {
            merge_send_batch(conn, &canonical_batch_id, &batch_id)?;
        } else {
            conn.execute(
                "UPDATE message_send_batches
                 SET conversation_id = ?1, channel_account_id = ?2
                 WHERE id = ?3",
                params![canonical_conversation_id, canonical_account_id, batch_id],
            )?;
        }
    }
    Ok(())
}

fn merge_send_batches_for_account(
    conn: &Connection,
    canonical_account_id: i64,
    duplicate_account_id: i64,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id, client_batch_id FROM message_send_batches WHERE channel_account_id = ?1",
    )?;
    let batches = stmt
        .query_map([duplicate_account_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>>>()?;
    for (batch_id, client_batch_id) in batches {
        let canonical_batch_id: Option<String> = conn
            .query_row(
                "SELECT id FROM message_send_batches
                 WHERE channel_account_id = ?1 AND client_batch_id = ?2
                 ORDER BY id LIMIT 1",
                params![canonical_account_id, client_batch_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(canonical_batch_id) = canonical_batch_id {
            merge_send_batch(conn, &canonical_batch_id, &batch_id)?;
        } else {
            conn.execute(
                "UPDATE message_send_batches SET channel_account_id = ?1 WHERE id = ?2",
                params![canonical_account_id, batch_id],
            )?;
        }
    }
    Ok(())
}

#[derive(Clone)]
struct ForwardTarget {
    id: i64,
    client_operation_id: String,
    send_batch_id: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    created_at: String,
    updated_at: String,
}

fn load_forward_target(conn: &Connection, target_id: i64) -> Result<ForwardTarget> {
    conn.query_row(
        "SELECT id, client_operation_id, send_batch_id, status, error_code, error_message, created_at, updated_at
         FROM message_forward_targets WHERE id = ?1",
        [target_id],
        |row| {
            Ok(ForwardTarget {
                id: row.get(0)?,
                client_operation_id: row.get(1)?,
                send_batch_id: row.get(2)?,
                status: row.get(3)?,
                error_code: row.get(4)?,
                error_message: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        },
    )
}

fn merge_forward_targets(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT id FROM message_forward_targets WHERE target_conversation_id = ?1 ORDER BY id",
    )?;
    let target_ids = stmt
        .query_map([duplicate_conversation_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>>>()?;
    for target_id in target_ids {
        let duplicate = load_forward_target(conn, target_id)?;
        let canonical_target_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM message_forward_targets
                 WHERE client_operation_id = ?1 AND target_conversation_id = ?2",
                params![duplicate.client_operation_id, canonical_conversation_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(canonical_target_id) = canonical_target_id {
            let canonical = load_forward_target(conn, canonical_target_id)?;
            let duplicate_wins = forward_target_status_rank(&duplicate.status)
                > forward_target_status_rank(&canonical.status)
                || (forward_target_status_rank(&duplicate.status)
                    == forward_target_status_rank(&canonical.status)
                    && duplicate.updated_at > canonical.updated_at);
            conn.execute(
                "UPDATE message_forward_targets
                 SET send_batch_id = ?2,
                     status = ?3,
                     error_code = ?4,
                     error_message = ?5,
                     created_at = ?6,
                     updated_at = ?7
                 WHERE id = ?1",
                params![
                    canonical.id,
                    canonical.send_batch_id.or(duplicate.send_batch_id),
                    if duplicate_wins {
                        duplicate.status.clone()
                    } else {
                        canonical.status.clone()
                    },
                    if duplicate_wins {
                        duplicate.error_code.clone()
                    } else {
                        canonical.error_code.clone()
                    },
                    if duplicate_wins {
                        duplicate.error_message.clone()
                    } else {
                        canonical.error_message.clone()
                    },
                    earlier(&canonical.created_at, &duplicate.created_at),
                    later(&canonical.updated_at, &duplicate.updated_at),
                ],
            )?;
            conn.execute(
                "DELETE FROM message_forward_targets WHERE id = ?1",
                [duplicate.id],
            )?;
        } else {
            conn.execute(
                "UPDATE message_forward_targets SET target_conversation_id = ?1 WHERE id = ?2",
                params![canonical_conversation_id, duplicate.id],
            )?;
        }
    }
    Ok(())
}

fn merge_pinned_messages(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO conversation_pinned_messages
            (conversation_id, message_id, pinned_by, pinned_at)
         SELECT ?1, message_id, pinned_by, pinned_at
         FROM conversation_pinned_messages WHERE conversation_id = ?2 AND true
         ON CONFLICT(conversation_id, message_id) DO UPDATE SET
            pinned_by = COALESCE(excluded.pinned_by, conversation_pinned_messages.pinned_by),
            pinned_at = MAX(conversation_pinned_messages.pinned_at, excluded.pinned_at)",
        params![canonical_conversation_id, duplicate_conversation_id],
    )?;
    conn.execute(
        "DELETE FROM conversation_pinned_messages WHERE conversation_id = ?1",
        [duplicate_conversation_id],
    )?;
    Ok(())
}

fn merge_conversation(
    conn: &Connection,
    canonical_conversation_id: i64,
    duplicate_conversation_id: i64,
    canonical_account_id: i64,
) -> Result<()> {
    merge_conversation_metadata(conn, canonical_conversation_id, duplicate_conversation_id)?;
    merge_messages(conn, canonical_conversation_id, duplicate_conversation_id)?;
    merge_participants(conn, canonical_conversation_id, duplicate_conversation_id)?;
    merge_send_batches_for_conversation(
        conn,
        canonical_conversation_id,
        duplicate_conversation_id,
        canonical_account_id,
    )?;
    conn.execute(
        "UPDATE conversation_staged_attachments SET conversation_id = ?1 WHERE conversation_id = ?2",
        params![canonical_conversation_id, duplicate_conversation_id],
    )?;
    merge_forward_targets(conn, canonical_conversation_id, duplicate_conversation_id)?;
    merge_pinned_messages(conn, canonical_conversation_id, duplicate_conversation_id)?;
    conn.execute(
        "DELETE FROM channel_conversations WHERE id = ?1",
        [duplicate_conversation_id],
    )?;
    Ok(())
}

fn merge_checkpoints(
    conn: &Connection,
    canonical_account_id: i64,
    duplicate_account_id: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO channel_sync_checkpoints
            (channel_account_id, scope, cursor, last_event_at, last_success_at, reconnect_count, last_error, updated_at)
         SELECT ?1, scope, cursor, last_event_at, last_success_at, reconnect_count, last_error, updated_at
         FROM channel_sync_checkpoints WHERE channel_account_id = ?2 AND true
         ON CONFLICT(channel_account_id, scope) DO UPDATE SET
            cursor = CASE
                WHEN excluded.updated_at >= channel_sync_checkpoints.updated_at
                THEN COALESCE(excluded.cursor, channel_sync_checkpoints.cursor)
                ELSE channel_sync_checkpoints.cursor END,
            last_event_at = CASE
                WHEN channel_sync_checkpoints.last_event_at IS NULL THEN excluded.last_event_at
                WHEN excluded.last_event_at IS NULL THEN channel_sync_checkpoints.last_event_at
                ELSE MAX(channel_sync_checkpoints.last_event_at, excluded.last_event_at) END,
            last_success_at = CASE
                WHEN channel_sync_checkpoints.last_success_at IS NULL THEN excluded.last_success_at
                WHEN excluded.last_success_at IS NULL THEN channel_sync_checkpoints.last_success_at
                ELSE MAX(channel_sync_checkpoints.last_success_at, excluded.last_success_at) END,
            reconnect_count = MAX(channel_sync_checkpoints.reconnect_count, excluded.reconnect_count),
            last_error = CASE
                WHEN excluded.updated_at >= channel_sync_checkpoints.updated_at
                THEN excluded.last_error ELSE channel_sync_checkpoints.last_error END,
            updated_at = MAX(channel_sync_checkpoints.updated_at, excluded.updated_at)",
        params![canonical_account_id, duplicate_account_id],
    )?;
    conn.execute(
        "DELETE FROM channel_sync_checkpoints WHERE channel_account_id = ?1",
        [duplicate_account_id],
    )?;
    Ok(())
}

fn merge_pending_message_reactions(
    conn: &Connection,
    canonical_account_id: i64,
    duplicate_account_id: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO pending_message_reactions
            (channel_account_id, external_thread_id, conversation_kind, provider_event_id,
             provider_message_id, provider_client_message_id, actor_external_id, reaction,
             removed, occurred_at, raw_payload_json, created_at, updated_at)
         SELECT ?1, external_thread_id, conversation_kind, provider_event_id,
                provider_message_id, provider_client_message_id, actor_external_id, reaction,
                removed, occurred_at, raw_payload_json, created_at, updated_at
         FROM pending_message_reactions
         WHERE channel_account_id = ?2
         ON CONFLICT(
             channel_account_id, external_thread_id, conversation_kind, provider_event_id,
             provider_message_id, provider_client_message_id, actor_external_id, reaction, removed
         ) DO UPDATE SET
             occurred_at = MAX(pending_message_reactions.occurred_at, excluded.occurred_at),
             raw_payload_json = CASE
                 WHEN excluded.occurred_at >= pending_message_reactions.occurred_at
                 THEN excluded.raw_payload_json
                 ELSE pending_message_reactions.raw_payload_json
             END,
             created_at = MIN(pending_message_reactions.created_at, excluded.created_at),
             updated_at = MAX(pending_message_reactions.updated_at, excluded.updated_at)",
        params![canonical_account_id, duplicate_account_id],
    )?;
    conn.execute(
        "DELETE FROM pending_message_reactions WHERE channel_account_id = ?1",
        [duplicate_account_id],
    )?;
    Ok(())
}

fn merge_channel_account_migration_markers(
    conn: &Connection,
    canonical_account_id: i64,
    duplicate_account_id: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO channel_account_migration_markers
            (channel_account_id, migration_key, applied_at)
         SELECT ?1, migration_key, applied_at
         FROM channel_account_migration_markers
         WHERE channel_account_id = ?2
         ON CONFLICT(channel_account_id, migration_key) DO UPDATE SET
             applied_at = MIN(channel_account_migration_markers.applied_at, excluded.applied_at)",
        params![canonical_account_id, duplicate_account_id],
    )?;
    conn.execute(
        "DELETE FROM channel_account_migration_markers WHERE channel_account_id = ?1",
        [duplicate_account_id],
    )?;
    Ok(())
}

fn merge_deleted_conversations(
    conn: &Connection,
    canonical_account_id: i64,
    duplicate_account_id: i64,
) -> Result<()> {
    conn.execute(
        "INSERT INTO deleted_conversations
            (channel_account_id, external_thread_id, kind, deleted_at)
         SELECT ?1, external_thread_id, kind, deleted_at
         FROM deleted_conversations
         WHERE channel_account_id = ?2
         ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
             deleted_at = MAX(deleted_conversations.deleted_at, excluded.deleted_at)",
        params![canonical_account_id, duplicate_account_id],
    )?;
    conn.execute(
        "DELETE FROM deleted_conversations WHERE channel_account_id = ?1",
        [duplicate_account_id],
    )?;
    Ok(())
}

fn merge_account_into(
    conn: &Connection,
    canonical_id: i64,
    duplicate: &ZaloAccountRow,
    zalo_uid: &str,
) -> Result<()> {
    merge_account_metadata(conn, canonical_id, duplicate.id, zalo_uid)?;
    conn.execute(
        "UPDATE zalo_sessions
         SET zalo_uid = ?1, channel_account_id = ?2
         WHERE channel_account_id = ?3 OR imei = ?4 OR zalo_uid = ?1",
        params![
            zalo_uid,
            canonical_id,
            duplicate.id,
            duplicate.provider_account_key
        ],
    )?;

    let mut stmt = conn.prepare(
        "SELECT id, external_thread_id, kind
         FROM channel_conversations WHERE channel_account_id = ?1 ORDER BY id",
    )?;
    let conversations = stmt
        .query_map([duplicate.id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>>>()?;

    for (duplicate_conversation_id, external_thread_id, kind) in conversations {
        let canonical_conversation_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM channel_conversations
                 WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
                params![canonical_id, external_thread_id, kind],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(canonical_conversation_id) = canonical_conversation_id {
            merge_conversation(
                conn,
                canonical_conversation_id,
                duplicate_conversation_id,
                canonical_id,
            )?;
        } else {
            conn.execute(
                "UPDATE channel_conversations SET channel_account_id = ?1 WHERE id = ?2",
                params![canonical_id, duplicate_conversation_id],
            )?;
        }
    }

    merge_send_batches_for_account(conn, canonical_id, duplicate.id)?;
    conn.execute(
        "UPDATE message_outbox_jobs SET channel_account_id = ?1 WHERE channel_account_id = ?2",
        params![canonical_id, duplicate.id],
    )?;
    merge_checkpoints(conn, canonical_id, duplicate.id)?;
    merge_pending_message_reactions(conn, canonical_id, duplicate.id)?;
    merge_channel_account_migration_markers(conn, canonical_id, duplicate.id)?;
    merge_deleted_conversations(conn, canonical_id, duplicate.id)?;

    let remaining_children: i64 = conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM channel_conversations WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM channel_sync_checkpoints WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM pending_message_reactions WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM channel_account_migration_markers WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM message_outbox_jobs WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM message_send_batches WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM deleted_conversations WHERE channel_account_id = ?1) +
            (SELECT COUNT(*) FROM zalo_sessions WHERE channel_account_id = ?1)",
        [duplicate.id],
        |row| row.get(0),
    )?;
    if remaining_children != 0 {
        return Err(rusqlite::Error::InvalidQuery);
    }

    conn.execute("DELETE FROM channel_accounts WHERE id = ?1", [duplicate.id])?;
    Ok(())
}

pub fn reconcile_duplicate_zalo_channel_accounts(conn: &Connection) -> Result<()> {
    ensure_zalo_session_schema(conn)?;
    let mut stmt = conn.prepare(
        "SELECT self_external_id FROM channel_accounts
         WHERE channel = 'zalo' AND self_external_id IS NOT NULL AND self_external_id != ''
         GROUP BY self_external_id HAVING COUNT(*) > 1",
    )?;
    let duplicate_uids = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;

    for uid in duplicate_uids {
        let mut account_stmt = conn.prepare(
            "SELECT ca.id, ca.provider_account_key
             FROM channel_accounts ca
             LEFT JOIN channel_conversations cc ON cc.channel_account_id = ca.id
             WHERE ca.channel = 'zalo' AND ca.self_external_id = ?1
             GROUP BY ca.id, ca.provider_account_key
             ORDER BY CASE WHEN ca.provider_account_key = ?1 THEN 0 ELSE 1 END,
                      COUNT(cc.id) DESC,
                      ca.id ASC",
        )?;
        let accounts = account_stmt
            .query_map([&uid], |row| {
                Ok(ZaloAccountRow {
                    id: row.get(0)?,
                    provider_account_key: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        let Some(canonical) = accounts.first().cloned() else {
            continue;
        };

        for account in &accounts {
            conn.execute(
                "UPDATE zalo_sessions
                 SET zalo_uid = ?1, channel_account_id = ?2
                 WHERE channel_account_id = ?3 OR imei = ?4 OR zalo_uid = ?1",
                params![uid, canonical.id, account.id, account.provider_account_key],
            )?;
        }
        for duplicate in accounts.iter().skip(1) {
            merge_account_into(conn, canonical.id, duplicate, &uid)?;
        }
        conn.execute(
            "UPDATE channel_accounts
             SET provider_account_key = ?2, self_external_id = ?2
             WHERE id = ?1",
            params![canonical.id, uid],
        )?;
        conn.execute(
            "UPDATE zalo_sessions SET zalo_uid = ?1, channel_account_id = ?2
             WHERE zalo_uid = ?1 OR channel_account_id = ?2",
            params![uid, canonical.id],
        )?;
    }

    Ok(())
}
