use rusqlite::{Connection, Result};
use std::collections::HashSet;

pub struct PragmaGuard<'a> {
    conn: &'a Connection,
    initial_fk: i32,
    initial_legacy_alter: i32,
}

impl<'a> PragmaGuard<'a> {
    pub fn new(conn: &'a Connection) -> Result<Self> {
        let initial_fk: i32 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
        let initial_legacy_alter: i32 =
            conn.query_row("PRAGMA legacy_alter_table", [], |r| r.get(0))?;
        Ok(Self {
            conn,
            initial_fk,
            initial_legacy_alter,
        })
    }

    pub fn restore(&self) -> Result<()> {
        self.conn.execute_batch(&format!(
            "PRAGMA legacy_alter_table = {}; PRAGMA foreign_keys = {};",
            if self.initial_legacy_alter == 1 {
                "ON"
            } else {
                "OFF"
            },
            if self.initial_fk == 1 { "ON" } else { "OFF" },
        ))?;
        Ok(())
    }
}

impl<'a> Drop for PragmaGuard<'a> {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

pub fn apply_migration_v2(conn: &Connection) -> Result<()> {
    let current_version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if current_version >= 2 {
        return Ok(());
    }

    let guard = PragmaGuard::new(conn)?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
        conn.execute_batch("SAVEPOINT migration_v2;")?;

        conn.execute_batch(
            r#"
            -- 1. Create v2 table for channel_identities with channel_account_id
            CREATE TABLE IF NOT EXISTS channel_identities_v2 (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER,
                channel TEXT NOT NULL CHECK (channel IN ('zalo', 'whatsapp', 'messenger', 'email')),
                channel_account_id INTEGER,
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
                updated_at TEXT NOT NULL,
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE SET NULL,
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
            );
        "#,
        )?;

        let has_channel_identities: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='channel_identities'",
            [],
            |r| r.get(0),
        ).unwrap_or(false);

        if has_channel_identities {
            let mut existing_cols = HashSet::new();
            {
                let mut stmt = conn.prepare("PRAGMA table_info(channel_identities)")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
                for col in rows.flatten() {
                    existing_cols.insert(col.to_lowercase());
                }
            }

            let lead_id_expr = if existing_cols.contains("lead_id") {
                "lead_id"
            } else {
                "NULL"
            };
            let channel_account_id_expr = if existing_cols.contains("channel_account_id") {
                "channel_account_id"
            } else {
                "NULL"
            };
            let norm_addr_expr = if existing_cols.contains("normalized_address") {
                "normalized_address"
            } else {
                "NULL"
            };
            let disp_name_expr = if existing_cols.contains("display_name") {
                "display_name"
            } else {
                "NULL"
            };
            let avatar_url_expr = if existing_cols.contains("avatar_url") {
                "avatar_url"
            } else {
                "NULL"
            };
            let sync_at_expr = if existing_cols.contains("profile_synced_at") {
                "profile_synced_at"
            } else {
                "NULL"
            };
            let sync_err_expr = if existing_cols.contains("profile_sync_error") {
                "profile_sync_error"
            } else {
                "NULL"
            };
            let sync_att_expr = if existing_cols.contains("profile_sync_attempts") {
                "profile_sync_attempts"
            } else {
                "0"
            };
            let retry_at_expr = if existing_cols.contains("next_profile_retry_at") {
                "next_profile_retry_at"
            } else {
                "NULL"
            };
            let link_src_expr = if existing_cols.contains("link_source") {
                "link_source"
            } else {
                "'unlinked'"
            };
            let link_conf_expr = if existing_cols.contains("link_confidence") {
                "link_confidence"
            } else {
                "NULL"
            };
            let verif_at_expr = if existing_cols.contains("verified_at") {
                "verified_at"
            } else {
                "NULL"
            };

            conn.execute_batch("PRAGMA legacy_alter_table = ON;")?;

            let copy_sql = format!(
                r#"
                INSERT INTO channel_identities_v2 (
                    id, lead_id, channel, channel_account_id, identity_namespace, external_id,
                    normalized_address, display_name, avatar_url, profile_synced_at,
                    profile_sync_error, profile_sync_attempts, next_profile_retry_at,
                    link_source, link_confidence, verified_at, created_at, updated_at
                )
                SELECT 
                    id, {lead_id_expr}, channel, {channel_account_id_expr}, identity_namespace, external_id,
                    {norm_addr_expr}, {disp_name_expr}, {avatar_url_expr}, {sync_at_expr},
                    {sync_err_expr}, {sync_att_expr}, {retry_at_expr},
                    {link_src_expr}, {link_conf_expr}, {verif_at_expr}, created_at, updated_at
                FROM channel_identities;

                DROP TABLE IF EXISTS channel_identities;
                ALTER TABLE channel_identities_v2 RENAME TO channel_identities;
                "#
            );
            conn.execute_batch(&copy_sql)?;
            conn.execute_batch("PRAGMA legacy_alter_table = OFF;")?;
        } else {
            conn.execute_batch("ALTER TABLE channel_identities_v2 RENAME TO channel_identities;")?;
        }

        conn.execute_batch(r#"
            -- 2. Rebuild partial unique constraints and indexes
            CREATE INDEX IF NOT EXISTS idx_channel_identities_lead ON channel_identities(lead_id, channel);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_legacy_unique 
            ON channel_identities(channel, identity_namespace, external_id) 
            WHERE channel_account_id IS NULL;
            CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_page_unique 
            ON channel_identities(channel, channel_account_id, identity_namespace, external_id) 
            WHERE channel_account_id IS NOT NULL;

            -- 3. Create facebook_page_tokens table
            CREATE TABLE IF NOT EXISTS facebook_page_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL UNIQUE,
                page_id TEXT NOT NULL UNIQUE,
                access_token_encrypted TEXT NOT NULL,
                token_status TEXT NOT NULL DEFAULT 'valid' CHECK (token_status IN ('valid', 'invalid', 'revoked')),
                granted_scopes_json TEXT NOT NULL DEFAULT '[]',
                last_validated_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
            );
        "#)?;

        // Conditionally add leadgen columns to leads table if leads table exists
        let has_leads: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='leads'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if has_leads {
            let mut has_leadgen = false;
            let mut has_custom_fields = false;
            {
                let mut stmt = conn.prepare("PRAGMA table_info(leads)")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
                for col in rows.flatten() {
                    if col == "leadgen_id" {
                        has_leadgen = true;
                    }
                    if col == "custom_fields_json" {
                        has_custom_fields = true;
                    }
                }
            }
            if !has_leadgen {
                conn.execute_batch("ALTER TABLE leads ADD COLUMN leadgen_id TEXT;")?;
            }
            conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_leads_leadgen_id ON leads(leadgen_id) WHERE leadgen_id IS NOT NULL;")?;
            if !has_custom_fields {
                conn.execute_batch(
                    "ALTER TABLE leads ADD COLUMN custom_fields_json TEXT NOT NULL DEFAULT '{}';",
                )?;
            }
        }

        // Verify foreign key integrity
        let fk_violations: i64 =
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check()", [], |r| {
                r.get(0)
            })?;
        if fk_violations > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                Some(format!(
                    "Foreign key check failed with {} violations",
                    fk_violations
                )),
            ));
        }

        conn.execute_batch("PRAGMA user_version = 2;")?;
        conn.execute_batch("RELEASE SAVEPOINT migration_v2;")?;
        Ok(())
    })();

    if migration_result.is_err() {
        let _ = conn
            .execute_batch("ROLLBACK TO SAVEPOINT migration_v2; RELEASE SAVEPOINT migration_v2;");
    }

    guard.restore()?;
    migration_result
}

fn merge_duplicate_attachments(conn: &Connection) -> Result<()> {
    let mut msg_stmt = conn.prepare(
        "SELECT DISTINCT message_id FROM message_attachments 
         WHERE remote_url IS NOT NULL OR provider_attachment_id IS NOT NULL",
    )?;
    let msg_rows = msg_stmt.query_map([], |r| r.get::<_, i64>(0))?;
    let message_ids: Vec<i64> = msg_rows.collect::<Result<Vec<_>, _>>()?;

    #[derive(Debug, Clone)]
    struct AttachmentRow {
        id: i64,
        provider_attachment_id: Option<String>,
        attachment_type: Option<String>,
        file_name: Option<String>,
        mime_type: Option<String>,
        size_bytes: Option<i64>,
        remote_url: Option<String>,
        local_path: Option<String>,
        thumbnail_url: Option<String>,
        metadata_json: Option<String>,
    }

    for msg_id in message_ids {
        let mut att_stmt = conn.prepare(
            "SELECT id, provider_attachment_id, attachment_type, file_name, mime_type, size_bytes, remote_url, local_path, thumbnail_url, metadata_json 
             FROM message_attachments 
             WHERE message_id = ?1 
             ORDER BY id ASC"
        )?;
        let rows = att_stmt.query_map(rusqlite::params![msg_id], |r| {
            Ok(AttachmentRow {
                id: r.get(0)?,
                provider_attachment_id: r.get(1)?,
                attachment_type: r.get(2)?,
                file_name: r.get(3)?,
                mime_type: r.get(4)?,
                size_bytes: r.get(5)?,
                remote_url: r.get(6)?,
                local_path: r.get(7)?,
                thumbnail_url: r.get(8)?,
                metadata_json: r.get(9)?,
            })
        })?;
        let items: Vec<AttachmentRow> = rows.collect::<Result<Vec<_>, _>>()?;
        if items.len() <= 1 {
            continue;
        }

        // Build connected components using Disjoint Set Union (DSU)
        let n = items.len();
        let mut parent: Vec<usize> = (0..n).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] == i {
                i
            } else {
                let p = parent[i];
                parent[i] = find(parent, p);
                parent[i]
            }
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let root_i = find(parent, i);
            let root_j = find(parent, j);
            if root_i != root_j {
                parent[root_i] = root_j;
            }
        }

        for i in 0..n {
            for j in (i + 1)..n {
                let shares_remote = match (&items[i].remote_url, &items[j].remote_url) {
                    (Some(r1), Some(r2)) => r1 == r2,
                    _ => false,
                };
                let shares_provider = match (
                    &items[i].provider_attachment_id,
                    &items[j].provider_attachment_id,
                ) {
                    (Some(p1), Some(p2)) => p1 == p2,
                    _ => false,
                };
                if shares_remote || shares_provider {
                    union(&mut parent, i, j);
                }
            }
        }

        let mut groups: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..n {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }

        for (_, member_indices) in groups {
            if member_indices.len() <= 1 {
                continue;
            }

            let survivor_idx = member_indices[0];
            let survivor_id = items[survivor_idx].id;

            let mut merged_prov_id = items[survivor_idx].provider_attachment_id.clone();
            let mut merged_remote = items[survivor_idx].remote_url.clone();
            let mut merged_type = items[survivor_idx].attachment_type.clone();
            let mut merged_file_name = items[survivor_idx].file_name.clone();
            let mut merged_mime = items[survivor_idx].mime_type.clone();
            let mut merged_size = items[survivor_idx].size_bytes;
            let mut merged_local = items[survivor_idx].local_path.clone();
            let mut merged_thumb = items[survivor_idx].thumbnail_url.clone();

            let mut merged_json_map: serde_json::Map<String, serde_json::Value> =
                serde_json::Map::new();
            let mut raw_unparsed_values: Vec<serde_json::Value> = Vec::new();

            let mut all_provider_ids: Vec<String> = Vec::new();
            let mut all_remote_urls: Vec<String> = Vec::new();
            let mut merged_raw_rows: Vec<serde_json::Value> = Vec::new();

            for &idx in &member_indices {
                let item = &items[idx];
                if let Some(pid) = &item.provider_attachment_id {
                    if !all_provider_ids.contains(pid) {
                        all_provider_ids.push(pid.clone());
                    }
                }
                if let Some(rurl) = &item.remote_url {
                    if !all_remote_urls.contains(rurl) {
                        all_remote_urls.push(rurl.clone());
                    }
                }

                merged_raw_rows.push(serde_json::json!({
                    "id": item.id,
                    "provider_attachment_id": item.provider_attachment_id,
                    "attachment_type": item.attachment_type,
                    "file_name": item.file_name,
                    "mime_type": item.mime_type,
                    "size_bytes": item.size_bytes,
                    "remote_url": item.remote_url,
                    "local_path": item.local_path,
                    "thumbnail_url": item.thumbnail_url,
                    "metadata_json": item.metadata_json
                }));

                if merged_prov_id.is_none() && item.provider_attachment_id.is_some() {
                    merged_prov_id = item.provider_attachment_id.clone();
                }
                if merged_remote.is_none() && item.remote_url.is_some() {
                    merged_remote = item.remote_url.clone();
                }
                if merged_type.is_none() && item.attachment_type.is_some() {
                    merged_type = item.attachment_type.clone();
                }
                if merged_file_name.is_none() && item.file_name.is_some() {
                    merged_file_name = item.file_name.clone();
                }
                if merged_mime.is_none() && item.mime_type.is_some() {
                    merged_mime = item.mime_type.clone();
                }
                if merged_size.is_none() && item.size_bytes.is_some() {
                    merged_size = item.size_bytes;
                }
                if merged_local.is_none() && item.local_path.is_some() {
                    merged_local = item.local_path.clone();
                }
                if merged_thumb.is_none() && item.thumbnail_url.is_some() {
                    merged_thumb = item.thumbnail_url.clone();
                }

                if let Some(json_str) = &item.metadata_json {
                    let trimmed = json_str.trim();
                    if !trimmed.is_empty() && trimmed != "{}" {
                        match serde_json::from_str::<serde_json::Value>(trimmed) {
                            Ok(serde_json::Value::Object(map)) => {
                                for (k, v) in map {
                                    if let Some(existing_val) = merged_json_map.get(&k) {
                                        if existing_val != &v {
                                            // Preserve conflicting key from duplicate without overwriting survivor's key
                                            let conflict_key = format!("{}_dup_{}", k, item.id);
                                            merged_json_map.insert(conflict_key, v);
                                        }
                                    } else {
                                        merged_json_map.insert(k, v);
                                    }
                                }
                            }
                            Ok(other) => {
                                raw_unparsed_values.push(other);
                            }
                            Err(_) => {
                                raw_unparsed_values
                                    .push(serde_json::Value::String(json_str.clone()));
                            }
                        }
                    }
                }
            }

            if !all_provider_ids.is_empty() {
                merged_json_map.insert(
                    "provider_attachment_id_aliases".to_string(),
                    serde_json::Value::Array(
                        all_provider_ids
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }

            if !all_remote_urls.is_empty() {
                merged_json_map.insert(
                    "remote_url_aliases".to_string(),
                    serde_json::Value::Array(
                        all_remote_urls
                            .into_iter()
                            .map(serde_json::Value::String)
                            .collect(),
                    ),
                );
            }

            merged_json_map.insert(
                "_merged_attachment_rows".to_string(),
                serde_json::Value::Array(merged_raw_rows),
            );

            if !raw_unparsed_values.is_empty() {
                merged_json_map.insert(
                    "_legacy_unparsed_values".to_string(),
                    serde_json::Value::Array(raw_unparsed_values),
                );
            }

            let merged_metadata_str = serde_json::Value::Object(merged_json_map).to_string();

            conn.execute(
                "UPDATE message_attachments 
                 SET provider_attachment_id = ?1, remote_url = ?2, attachment_type = COALESCE(?3, attachment_type), file_name = ?4, mime_type = ?5, size_bytes = ?6, local_path = ?7, thumbnail_url = ?8, metadata_json = ?9 
                 WHERE id = ?10",
                rusqlite::params![merged_prov_id, merged_remote, merged_type, merged_file_name, merged_mime, merged_size, merged_local, merged_thumb, merged_metadata_str, survivor_id],
            )?;

            for &idx in &member_indices {
                if items[idx].id != survivor_id {
                    conn.execute(
                        "DELETE FROM message_attachments WHERE id = ?1",
                        [items[idx].id],
                    )?;
                }
            }
        }
    }

    Ok(())
}

pub fn apply_migration_v4(conn: &Connection) -> Result<()> {
    apply_migration_v3(conn)?;

    let current_version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if current_version >= 4 {
        return Ok(());
    }

    let guard = PragmaGuard::new(conn)?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
        conn.execute_batch("SAVEPOINT migration_v4;")?;

        let has_attachments: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='message_attachments'",
            [],
            |r| r.get(0),
        ).unwrap_or(false);

        if has_attachments {
            merge_duplicate_attachments(conn)?;

            conn.execute_batch(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_remote 
                ON message_attachments(message_id, remote_url) 
                WHERE remote_url IS NOT NULL;

                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_provider
                ON message_attachments(message_id, provider_attachment_id)
                WHERE provider_attachment_id IS NOT NULL;
            "#,
            )?;
        }

        let fk_violations: i64 =
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check()", [], |r| {
                r.get(0)
            })?;
        if fk_violations > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                Some(format!(
                    "Foreign key check failed with {} violations",
                    fk_violations
                )),
            ));
        }

        conn.execute_batch("PRAGMA user_version = 4;")?;
        conn.execute_batch("RELEASE SAVEPOINT migration_v4;")?;
        Ok(())
    })();

    if migration_result.is_err() {
        let _ = conn
            .execute_batch("ROLLBACK TO SAVEPOINT migration_v4; RELEASE SAVEPOINT migration_v4;");
    }

    guard.restore()?;
    migration_result
}

pub fn apply_migration_v3(conn: &Connection) -> Result<()> {
    apply_migration_v2(conn)?;

    let current_version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if current_version >= 3 {
        return Ok(());
    }

    let guard = PragmaGuard::new(conn)?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
        conn.execute_batch("SAVEPOINT migration_v3;")?;

        conn.execute_batch(r#"
            -- 1. Create channel_sync_dead_letters table with item_index, owner_id, and claim_deadline
            CREATE TABLE IF NOT EXISTS channel_sync_dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                scope TEXT NOT NULL,
                event_id INTEGER NOT NULL,
                item_index INTEGER NOT NULL DEFAULT 0,
                event_type TEXT NOT NULL,
                event_key TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                error_reason TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'retrying', 'resolved', 'ignored', 'failed')),
                attempts INTEGER NOT NULL DEFAULT 0,
                next_retry_at TEXT,
                owner_id TEXT,
                claim_deadline TEXT,
                resolved_at TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
                UNIQUE(channel_account_id, scope, event_id, item_index)
            );
            CREATE INDEX IF NOT EXISTS idx_dead_letters_account_scope ON channel_sync_dead_letters(channel_account_id, scope);
            CREATE INDEX IF NOT EXISTS idx_dead_letters_status ON channel_sync_dead_letters(status, next_retry_at);
        "#)?;

        let has_attachments: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='message_attachments'",
            [],
            |r| r.get(0),
        ).unwrap_or(false);

        if has_attachments {
            let mut existing_att_cols = HashSet::new();
            {
                let mut stmt = conn.prepare("PRAGMA table_info(message_attachments)")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
                for col in rows.flatten() {
                    existing_att_cols.insert(col.to_lowercase());
                }
            }

            if !existing_att_cols.contains("provider_attachment_id") {
                conn.execute_batch(
                    "ALTER TABLE message_attachments ADD COLUMN provider_attachment_id TEXT;",
                )?;
            }
            if !existing_att_cols.contains("attachment_type") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN attachment_type TEXT NOT NULL DEFAULT 'file';")?;
            }
            if !existing_att_cols.contains("file_name") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN file_name TEXT;")?;
            }
            if !existing_att_cols.contains("mime_type") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN mime_type TEXT;")?;
            }
            if !existing_att_cols.contains("size_bytes") {
                conn.execute_batch(
                    "ALTER TABLE message_attachments ADD COLUMN size_bytes INTEGER;",
                )?;
            }
            if !existing_att_cols.contains("local_path") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN local_path TEXT;")?;
            }
            if !existing_att_cols.contains("thumbnail_url") {
                conn.execute_batch(
                    "ALTER TABLE message_attachments ADD COLUMN thumbnail_url TEXT;",
                )?;
            }
            if !existing_att_cols.contains("metadata_json") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';")?;
            }

            // Lossless deduplication and unique index creation for message_attachments
            merge_duplicate_attachments(conn)?;

            conn.execute_batch(
                r#"
                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_remote 
                ON message_attachments(message_id, remote_url) 
                WHERE remote_url IS NOT NULL;

                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_provider
                ON message_attachments(message_id, provider_attachment_id)
                WHERE provider_attachment_id IS NOT NULL;
            "#,
            )?;
        }

        // Conditionally add provider_timestamp to messages table if messages table exists
        let has_messages: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='messages'",
                [],
                |r| r.get(0),
            )
            .unwrap_or(false);

        if has_messages {
            let mut has_provider_timestamp = false;
            {
                let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
                for col in rows.flatten() {
                    if col == "provider_timestamp" {
                        has_provider_timestamp = true;
                    }
                }
            }
            if !has_provider_timestamp {
                conn.execute_batch("ALTER TABLE messages ADD COLUMN provider_timestamp TEXT;")?;
            }
        }

        // Verify foreign key integrity
        let fk_violations: i64 =
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check()", [], |r| {
                r.get(0)
            })?;
        if fk_violations > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                Some(format!(
                    "Foreign key check failed with {} violations",
                    fk_violations
                )),
            ));
        }

        conn.execute_batch("PRAGMA user_version = 3;")?;
        conn.execute_batch("RELEASE SAVEPOINT migration_v3;")?;
        Ok(())
    })();

    if migration_result.is_err() {
        let _ = conn
            .execute_batch("ROLLBACK TO SAVEPOINT migration_v3; RELEASE SAVEPOINT migration_v3;");
    }

    guard.restore()?;
    migration_result
}
