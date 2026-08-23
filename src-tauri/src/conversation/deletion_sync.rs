use anyhow::{anyhow, Result};
use reqwest::Client;
use rusqlite::{params, Connection};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DeletionWorkerConfig<'a> {
    pub supabase_url: &'a str,
    pub anon_key: &'a str,
    pub user_jwt: &'a str,
    pub device_id: &'a str,
    pub device_secret: &'a str,
}

pub async fn process_pending_device_tasks(
    conn: &mut Connection,
    client: &Client,
    config: &DeletionWorkerConfig<'_>,
    spool_root: &std::path::Path,
) -> Result<usize> {
    // 0. Ensure device is enrolled and active via register_or_heartbeat_device RPC
    let heartbeat_url = format!(
        "{}/rest/v1/rpc/register_or_heartbeat_device",
        config.supabase_url
    );
    let heartbeat_resp = client
        .post(&heartbeat_url)
        .header("apikey", config.anon_key)
        .header("Authorization", format!("Bearer {}", config.user_jwt))
        .json(&serde_json::json!({
            "p_device_id": config.device_id,
            "p_device_secret": config.device_secret,
            "p_device_name": "RustSale Desktop"
        }))
        .send()
        .await?;

    if !heartbeat_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to register/heartbeat device before claiming tasks: {}",
            heartbeat_resp.text().await?
        ));
    }

    // 1. Claim pending deletion tasks via secure RPC (bound to authenticated device_secret and returning unforgeable single-use lease tokens)
    let claim_url = format!(
        "{}/rest/v1/rpc/claim_device_deletion_tasks",
        config.supabase_url
    );
    let claim_resp = client
        .post(&claim_url)
        .header("apikey", config.anon_key)
        .header("Authorization", format!("Bearer {}", config.user_jwt))
        .json(&serde_json::json!({
            "p_device_id": config.device_id,
            "p_device_secret": config.device_secret,
            "p_lease_seconds": 300
        }))
        .send()
        .await?;

    if !claim_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to claim device deletion tasks: {}",
            claim_resp.status()
        ));
    }

    let tasks: Vec<Value> = claim_resp.json().await?;
    let mut completed_count = 0;

    for task in tasks {
        let task_id = task.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let page_id = task.get("page_id").and_then(|v| v.as_str()).unwrap_or("");
        let user_id = task.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
        let lease_token = task
            .get("lease_token")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if task_id.is_empty() || page_id.is_empty() || user_id.is_empty() || lease_token.is_empty()
        {
            continue;
        }

        // 2. Perform transactional local anonymization
        if let Err(err) = execute_local_anonymization(conn, task_id, page_id, user_id) {
            tracing::error!(
                "Failed local anonymization for task {} (page {}, user {}): {}",
                task_id,
                page_id,
                user_id,
                err
            );
            let release_url = format!(
                "{}/rest/v1/rpc/release_device_deletion_task",
                config.supabase_url
            );
            let _ = client
                .post(&release_url)
                .header("apikey", config.anon_key)
                .header("Authorization", format!("Bearer {}", config.user_jwt))
                .json(&serde_json::json!({
                    "p_task_id": task_id,
                    "p_device_id": config.device_id,
                    "p_device_secret": config.device_secret,
                    "p_lease_token": lease_token
                }))
                .send()
                .await;
            continue;
        }

        // 3. Task-scoped spool sweep from disk with canonical path, try_exists error preservation, and fail-closed active reference validation
        let sweep_result = sweep_task_spool_files(conn, task_id, spool_root);
        let remaining_spools: i64 = conn.query_row(
            "SELECT COUNT(*) FROM meta_deletion_pending_cleanups WHERE task_id = ?1",
            [task_id],
            |r| r.get(0),
        )?;

        // 4. Update task status on Supabase via single-use lease token & device-secret RPCs
        if sweep_result.is_ok() && remaining_spools == 0 {
            let complete_url = format!(
                "{}/rest/v1/rpc/complete_device_deletion_task",
                config.supabase_url
            );
            let patch_resp = client
                .post(&complete_url)
                .header("apikey", config.anon_key)
                .header("Authorization", format!("Bearer {}", config.user_jwt))
                .json(&serde_json::json!({
                    "p_task_id": task_id,
                    "p_device_id": config.device_id,
                    "p_device_secret": config.device_secret,
                    "p_lease_token": lease_token
                }))
                .send()
                .await?;

            if patch_resp.status().is_success() {
                let is_completed: bool = patch_resp.json().await.unwrap_or(false);
                if is_completed {
                    completed_count += 1;
                }
            }
        } else {
            let release_url = format!(
                "{}/rest/v1/rpc/release_device_deletion_task",
                config.supabase_url
            );
            let _ = client
                .post(&release_url)
                .header("apikey", config.anon_key)
                .header("Authorization", format!("Bearer {}", config.user_jwt))
                .json(&serde_json::json!({
                    "p_task_id": task_id,
                    "p_device_id": config.device_id,
                    "p_device_secret": config.device_secret,
                    "p_lease_token": lease_token
                }))
                .send()
                .await;
        }
    }

    Ok(completed_count)
}

pub fn execute_local_anonymization(
    conn: &mut Connection,
    task_id: &str,
    page_id: &str,
    user_id: &str,
) -> Result<()> {
    let tx = conn.transaction()?;

    // Explicit Page -> Messenger channel account relationship resolution via facebook_page_tokens foreign key
    let matched_accounts: Vec<i64> = {
        let mut stmt = tx.prepare(
            "SELECT ca.id
             FROM channel_accounts ca
             JOIN facebook_page_tokens fpt ON ca.id = fpt.channel_account_id
             WHERE ca.channel = 'messenger' AND fpt.page_id = ?1",
        )?;
        let rows = stmt.query_map([page_id], |r| r.get(0))?;
        let mut acc = Vec::new();
        for r in rows {
            acc.push(r?);
        }
        acc
    };

    let account_id = match matched_accounts.as_slice() {
        [single_id] => *single_id,
        [] => {
            return Err(anyhow!(
                "Fail-closed: No authoritative Messenger channel account found for page_id {}",
                page_id
            ))
        }
        _ => {
            return Err(anyhow!(
                "Fail-closed: Ambiguous mapping: multiple Messenger channel accounts found for page_id {}",
                page_id
            ))
        }
    };

    // Find all attachments to delete belonging to Messenger messages sent by target user in this Page account
    let paths: Vec<String> = {
        let mut att_stmt = tx.prepare(
            "SELECT ma.local_path
             FROM message_attachments ma
             JOIN messages m ON ma.message_id = m.id
             JOIN channel_conversations cc ON m.conversation_id = cc.id
             WHERE cc.channel_account_id = ?1
               AND m.sender_identity_id IN (
                   SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2
               )
               AND ma.local_path IS NOT NULL AND TRIM(ma.local_path) != ''",
        )?;
        let rows = att_stmt.query_map(params![account_id, user_id], |r| r.get(0))?;
        let mut acc = Vec::new();
        for r in rows {
            acc.push(r?);
        }
        acc
    };

    for path in &paths {
        tx.execute(
            "INSERT OR IGNORE INTO meta_deletion_pending_cleanups (task_id, local_path, created_at) VALUES (?1, ?2, datetime('now'))",
            params![task_id, path],
        )?;
    }

    // Wipe all attachment identifiers, remote URLs, and metadata
    tx.execute(
        "UPDATE message_attachments
         SET provider_attachment_id = NULL,
             remote_url = NULL,
             thumbnail_url = NULL,
             file_name = '[Deleted]',
             local_path = NULL,
             metadata_json = '{}'
         WHERE message_id IN (
             SELECT m.id FROM messages m
             JOIN channel_conversations cc ON m.conversation_id = cc.id
             WHERE cc.channel_account_id = ?1
               AND m.sender_identity_id IN (
                   SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2
               )
         )",
        params![account_id, user_id],
    )?;

    // Scrub residual user identifiers across Messenger reactions, receipts, pending reactions, and mentions for this account
    tx.execute(
        "DELETE FROM message_reactions
         WHERE actor_external_id = ?2
           AND message_id IN (
               SELECT m.id FROM messages m
               JOIN channel_conversations cc ON m.conversation_id = cc.id
               WHERE cc.channel_account_id = ?1
           )",
        params![account_id, user_id],
    )?;
    tx.execute(
        "DELETE FROM message_receipts
         WHERE recipient_external_id = ?2
           AND message_id IN (
               SELECT m.id FROM messages m
               JOIN channel_conversations cc ON m.conversation_id = cc.id
               WHERE cc.channel_account_id = ?1
           )",
        params![account_id, user_id],
    )?;
    tx.execute(
        "DELETE FROM pending_message_reactions
         WHERE channel_account_id = ?1 AND actor_external_id = ?2",
        params![account_id, user_id],
    )?;
    tx.execute(
        "DELETE FROM message_mentions
         WHERE (external_id = ?2 OR identity_id IN (SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2))
           AND message_id IN (
               SELECT m.id FROM messages m
               JOIN channel_conversations cc ON m.conversation_id = cc.id
               WHERE cc.channel_account_id = ?1
           )",
        params![account_id, user_id],
    )?;

    // Scrub channel sync dead letters for this Messenger user and account across all structured payload shapes
    tx.execute(
        "DELETE FROM channel_sync_dead_letters
         WHERE channel_account_id = ?1 AND (
             event_key = ?2
             OR json_extract(payload_json, '$.sender.id') = ?2
             OR json_extract(payload_json, '$.recipient.id') = ?2
             OR json_extract(payload_json, '$.from.id') = ?2
             OR json_extract(payload_json, '$.sender_id') = ?2
             OR json_extract(payload_json, '$.user_id') = ?2
             OR json_extract(payload_json, '$.value.from.id') = ?2
             OR json_extract(payload_json, '$.value.sender_id') = ?2
             OR json_extract(payload_json, '$.value.user_id') = ?2
             OR json_extract(payload_json, '$.value.sender.id') = ?2
             OR json_extract(payload_json, '$.entry[0].messaging[0].sender.id') = ?2
             OR json_extract(payload_json, '$.entry[0].messaging[0].recipient.id') = ?2
             OR json_extract(payload_json, '$.entry[0].changes[0].value.from.id') = ?2
             OR json_extract(payload_json, '$.entry[0].changes[0].value.sender_id') = ?2
             OR json_extract(payload_json, '$.entry[0].changes[0].value.user_id') = ?2
         )",
        params![account_id, user_id],
    )?;

    // Redact messages
    tx.execute(
        "UPDATE messages
         SET text_content = '[Deleted for Compliance]',
             content_json = '{\"type\":\"text\",\"text\":\"[Deleted for Compliance]\"}',
             raw_payload_json = NULL
         WHERE conversation_id IN (
             SELECT cc.id FROM channel_conversations cc
             WHERE cc.channel_account_id = ?1
         ) AND sender_identity_id IN (
             SELECT id FROM channel_identities WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2
         )",
        params![account_id, user_id],
    )?;

    // Anonymize identity including external_id tombstone scoped to this channel account
    tx.execute(
        "UPDATE channel_identities
         SET display_name = 'Deleted User',
             avatar_url = NULL,
             normalized_address = NULL,
             external_id = 'deleted_' || id || '_' || hex(randomblob(4)),
             profile_synced_at = datetime('now'),
             profile_sync_error = NULL,
             updated_at = datetime('now')
         WHERE channel_account_id = ?1 AND channel = 'messenger' AND external_id = ?2",
        params![account_id, user_id],
    )?;

    // Scrub Facebook comment logs matching page_id and sender_psid
    tx.execute(
        "DELETE FROM facebook_comment_logs WHERE page_id = ?1 AND sender_psid = ?2",
        params![page_id, user_id],
    )?;

    tx.commit()?;
    Ok(())
}

pub fn sweep_task_spool_files(
    conn: &Connection,
    task_id: &str,
    spool_root: &std::path::Path,
) -> Result<()> {
    let pending_cleanups: Vec<String> = {
        let mut stmt = conn
            .prepare("SELECT local_path FROM meta_deletion_pending_cleanups WHERE task_id = ?1")?;
        let rows = stmt.query_map([task_id], |r| r.get(0))?;
        let mut acc = Vec::new();
        for r in rows {
            acc.push(r?);
        }
        acc
    };

    let mut all_cleaned = true;
    for path in &pending_cleanups {
        let p = std::path::Path::new(path);
        let is_canonical_spool = p
            .canonicalize()
            .ok()
            .map(|canon| canon.starts_with(spool_root))
            .unwrap_or(false);

        let active_refs: rusqlite::Result<i64> = conn.query_row(
            "SELECT COUNT(*) FROM message_attachments WHERE local_path = ?1",
            [path],
            |r| r.get(0),
        );

        match active_refs {
            Ok(0) => {
                match p.try_exists() {
                    Ok(true) => {
                        if is_canonical_spool && std::fs::remove_file(p).is_ok() {
                            conn.execute(
                                "DELETE FROM meta_deletion_pending_cleanups WHERE task_id = ?1 AND local_path = ?2",
                                params![task_id, path],
                            )?;
                        } else {
                            all_cleaned = false;
                        }
                    }
                    Ok(false) => {
                        // File strictly absent from disk, safely purge queue entry
                        conn.execute(
                            "DELETE FROM meta_deletion_pending_cleanups WHERE task_id = ?1 AND local_path = ?2",
                            params![task_id, path],
                        )?;
                    }
                    Err(err) => {
                        // Permission denied or I/O error accessing metadata: FAIL CLOSED
                        tracing::error!("Filesystem try_exists error for path {}: {}", path, err);
                        all_cleaned = false;
                    }
                }
            }
            Ok(_) => {
                // Retain shared file on disk for other active records, clean up task queue entry safely
                conn.execute(
                    "DELETE FROM meta_deletion_pending_cleanups WHERE task_id = ?1 AND local_path = ?2",
                    params![task_id, path],
                )?;
            }
            Err(err) => {
                tracing::error!(
                    "Failed to check active references for path {}: {}",
                    path,
                    err
                );
                all_cleaned = false;
            }
        }
    }

    if all_cleaned {
        Ok(())
    } else {
        Err(anyhow!(
            "Fail-closed: One or more spool files could not be cleanly swept for task {}",
            task_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE channel_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL,
                account_name TEXT NOT NULL,
                provider_account_key TEXT NOT NULL,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE facebook_page_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL REFERENCES channel_accounts(id) ON DELETE CASCADE,
                page_id TEXT NOT NULL UNIQUE,
                access_token_encrypted TEXT NOT NULL,
                token_status TEXT NOT NULL DEFAULT 'valid',
                granted_scopes_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE channel_identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL REFERENCES channel_accounts(id) ON DELETE CASCADE,
                channel TEXT NOT NULL,
                external_id TEXT NOT NULL,
                display_name TEXT,
                avatar_url TEXT,
                normalized_address TEXT,
                profile_synced_at TEXT,
                profile_sync_error TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(channel_account_id, channel, external_id)
            );
            CREATE TABLE channel_conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL REFERENCES channel_accounts(id) ON DELETE CASCADE,
                channel TEXT NOT NULL,
                external_thread_id TEXT NOT NULL,
                title TEXT,
                is_group INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(channel_account_id, channel, external_thread_id)
            );
            CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL REFERENCES channel_conversations(id) ON DELETE CASCADE,
                sender_identity_id INTEGER REFERENCES channel_identities(id) ON DELETE SET NULL,
                direction TEXT NOT NULL,
                text_content TEXT,
                content_json TEXT NOT NULL,
                raw_payload_json TEXT,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE message_attachments (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                provider_attachment_id TEXT,
                remote_url TEXT,
                thumbnail_url TEXT,
                file_name TEXT,
                local_path TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE message_reactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                actor_external_id TEXT NOT NULL,
                reaction TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE message_receipts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                recipient_external_id TEXT NOT NULL,
                receipt_type TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE pending_message_reactions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                actor_external_id TEXT NOT NULL,
                reaction TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE message_mentions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                message_id INTEGER NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                identity_id INTEGER,
                external_id TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE channel_sync_dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                scope TEXT NOT NULL,
                event_key TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE facebook_comment_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                comment_id TEXT NOT NULL UNIQUE,
                page_id TEXT NOT NULL,
                sender_psid TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE TABLE meta_deletion_pending_cleanups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                task_id TEXT NOT NULL,
                local_path TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_execute_local_anonymization_scrubs_all_pii_tables() {
        let mut conn = setup_test_db();

        // 1. Seed channel account, page token, identity, conversation, messages, attachments, dead letters, comment logs
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, account_name, provider_account_key)
             VALUES (10, 'messenger', 'Test Page', 'page_custom_key_456')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO facebook_page_tokens (channel_account_id, page_id, access_token_encrypted, token_status, created_at, updated_at)
             VALUES (10, 'page_123', 'enc_token', 'valid', datetime('now'), datetime('now'))",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_identities (id, channel_account_id, channel, external_id, display_name)
             VALUES (20, 10, 'messenger', 'psid_user_456', 'Alice Example')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_conversations (id, channel_account_id, channel, external_thread_id)
             VALUES (30, 10, 'messenger', 'thread_30')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, text_content, content_json, raw_payload_json, status)
             VALUES (40, 30, 20, 'inbound', 'Sensitive private user message', '{\"type\":\"text\",\"text\":\"Sensitive private user message\"}', '{\"raw\":\"secret\"}', 'delivered')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO message_attachments (id, message_id, provider_attachment_id, remote_url, local_path, file_name, metadata_json)
             VALUES (50, 40, 'att_remote_1', 'https://cdn.fb.com/pic.jpg', '/tmp/spool/pic.jpg', 'pic.jpg', '{\"meta\":\"info\"}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO message_reactions (message_id, actor_external_id, reaction)
             VALUES (40, 'psid_user_456', 'like')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type)
             VALUES (40, 'psid_user_456', 'read')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pending_message_reactions (channel_account_id, actor_external_id, reaction)
             VALUES (10, 'psid_user_456', 'love')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO channel_sync_dead_letters (channel_account_id, scope, event_key, payload_json)
             VALUES (10, 'meta_webhook_messenger_cursor', 'dlq_1', '{\"sender\":{\"id\":\"psid_user_456\"}}')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO facebook_comment_logs (comment_id, page_id, sender_psid)
             VALUES ('comm_789', 'page_123', 'psid_user_456')",
            [],
        )
        .unwrap();

        // 2. Execute local anonymization
        let res =
            execute_local_anonymization(&mut conn, "task_uuid_1", "page_123", "psid_user_456");
        assert!(res.is_ok(), "Local anonymization must succeed");

        // 3. Verify all tables scrubbed
        let text_content: String = conn
            .query_row("SELECT text_content FROM messages WHERE id = 40", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(text_content, "[Deleted for Compliance]");

        let (file_name, local_path): (String, Option<String>) = conn
            .query_row(
                "SELECT file_name, local_path FROM message_attachments WHERE id = 50",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(file_name, "[Deleted]");
        assert!(local_path.is_none());

        let pending_cleanups: i64 = conn.query_row(
            "SELECT COUNT(*) FROM meta_deletion_pending_cleanups WHERE task_id = 'task_uuid_1' AND local_path = '/tmp/spool/pic.jpg'",
            [], |r| r.get(0)
        ).unwrap();
        assert_eq!(pending_cleanups, 1);

        let reactions: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_reactions WHERE actor_external_id = 'psid_user_456'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reactions, 0);

        let receipts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM message_receipts WHERE recipient_external_id = 'psid_user_456'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(receipts, 0);

        let dead_letters: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM channel_sync_dead_letters WHERE channel_account_id = 10",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(dead_letters, 0);

        let comment_logs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM facebook_comment_logs WHERE sender_psid = 'psid_user_456'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(comment_logs, 0);

        let (display_name, external_id): (String, String) = conn
            .query_row(
                "SELECT display_name, external_id FROM channel_identities WHERE id = 20",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(display_name, "Deleted User");
        assert!(external_id.starts_with("deleted_20_"));
    }

    #[test]
    fn test_execute_local_anonymization_fails_closed_when_page_account_missing() {
        let mut conn = setup_test_db();
        let res = execute_local_anonymization(
            &mut conn,
            "task_uuid_missing",
            "unregistered_page",
            "user_1",
        );
        assert!(
            res.is_err(),
            "Must fail closed if page_id has no registered channel account"
        );
    }
}
