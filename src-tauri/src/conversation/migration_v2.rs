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
        let initial_legacy_alter: i32 = conn.query_row("PRAGMA legacy_alter_table", [], |r| r.get(0))?;
        Ok(Self {
            conn,
            initial_fk,
            initial_legacy_alter,
        })
    }

    pub fn restore(&self) -> Result<()> {
        self.conn.execute_batch(&format!(
            "PRAGMA legacy_alter_table = {}; PRAGMA foreign_keys = {};",
            if self.initial_legacy_alter == 1 { "ON" } else { "OFF" },
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
    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if current_version >= 2 {
        return Ok(());
    }

    let guard = PragmaGuard::new(conn)?;
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
        conn.execute_batch("SAVEPOINT migration_v2;")?;

        conn.execute_batch(r#"
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
        "#)?;

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

            let lead_id_expr = if existing_cols.contains("lead_id") { "lead_id" } else { "NULL" };
            let channel_account_id_expr = if existing_cols.contains("channel_account_id") { "channel_account_id" } else { "NULL" };
            let norm_addr_expr = if existing_cols.contains("normalized_address") { "normalized_address" } else { "NULL" };
            let disp_name_expr = if existing_cols.contains("display_name") { "display_name" } else { "NULL" };
            let avatar_url_expr = if existing_cols.contains("avatar_url") { "avatar_url" } else { "NULL" };
            let sync_at_expr = if existing_cols.contains("profile_synced_at") { "profile_synced_at" } else { "NULL" };
            let sync_err_expr = if existing_cols.contains("profile_sync_error") { "profile_sync_error" } else { "NULL" };
            let sync_att_expr = if existing_cols.contains("profile_sync_attempts") { "profile_sync_attempts" } else { "0" };
            let retry_at_expr = if existing_cols.contains("next_profile_retry_at") { "next_profile_retry_at" } else { "NULL" };
            let link_src_expr = if existing_cols.contains("link_source") { "link_source" } else { "'unlinked'" };
            let link_conf_expr = if existing_cols.contains("link_confidence") { "link_confidence" } else { "NULL" };
            let verif_at_expr = if existing_cols.contains("verified_at") { "verified_at" } else { "NULL" };

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
        let has_leads: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='leads'",
            [],
            |r| r.get(0),
        ).unwrap_or(false);

        if has_leads {
            let mut has_leadgen = false;
            let mut has_custom_fields = false;
            {
                let mut stmt = conn.prepare("PRAGMA table_info(leads)")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
                for col in rows.flatten() {
                    if col == "leadgen_id" { has_leadgen = true; }
                    if col == "custom_fields_json" { has_custom_fields = true; }
                }
            }
            if !has_leadgen {
                conn.execute_batch("ALTER TABLE leads ADD COLUMN leadgen_id TEXT;")?;
            }
            conn.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS idx_leads_leadgen_id ON leads(leadgen_id) WHERE leadgen_id IS NOT NULL;")?;
            if !has_custom_fields {
                conn.execute_batch("ALTER TABLE leads ADD COLUMN custom_fields_json TEXT NOT NULL DEFAULT '{}';")?;
            }
        }

        // Verify foreign key integrity
        let fk_violations: i64 = conn.query_row("SELECT count(*) FROM pragma_foreign_key_check()", [], |r| r.get(0))?;
        if fk_violations > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                Some(format!("Foreign key check failed with {} violations", fk_violations)),
            ));
        }

        conn.execute_batch("PRAGMA user_version = 2;")?;
        conn.execute_batch("RELEASE SAVEPOINT migration_v2;")?;
        Ok(())
    })();

    if migration_result.is_err() {
        let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT migration_v2; RELEASE SAVEPOINT migration_v2;");
    }

    guard.restore()?;
    migration_result
}

pub fn apply_migration_v3(conn: &Connection) -> Result<()> {
    apply_migration_v2(conn)?;

    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
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
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN provider_attachment_id TEXT;")?;
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
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN size_bytes INTEGER;")?;
            }
            if !existing_att_cols.contains("local_path") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN local_path TEXT;")?;
            }
            if !existing_att_cols.contains("thumbnail_url") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN thumbnail_url TEXT;")?;
            }
            if !existing_att_cols.contains("metadata_json") {
                conn.execute_batch("ALTER TABLE message_attachments ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';")?;
            }

            // Lossless metadata merge from duplicates into the surviving MIN(id) record before deletion
            conn.execute_batch(r#"
                UPDATE message_attachments AS target
                SET 
                    provider_attachment_id = COALESCE(target.provider_attachment_id, (
                        SELECT provider_attachment_id FROM message_attachments 
                        WHERE message_id = target.message_id AND remote_url = target.remote_url AND provider_attachment_id IS NOT NULL 
                        LIMIT 1
                    )),
                    file_name = COALESCE(target.file_name, (
                        SELECT file_name FROM message_attachments 
                        WHERE message_id = target.message_id AND remote_url = target.remote_url AND file_name IS NOT NULL 
                        LIMIT 1
                    )),
                    mime_type = COALESCE(target.mime_type, (
                        SELECT mime_type FROM message_attachments 
                        WHERE message_id = target.message_id AND remote_url = target.remote_url AND mime_type IS NOT NULL 
                        LIMIT 1
                    )),
                    size_bytes = COALESCE(target.size_bytes, (
                        SELECT size_bytes FROM message_attachments 
                        WHERE message_id = target.message_id AND remote_url = target.remote_url AND size_bytes IS NOT NULL 
                        LIMIT 1
                    )),
                    local_path = COALESCE(target.local_path, (
                        SELECT local_path FROM message_attachments 
                        WHERE message_id = target.message_id AND remote_url = target.remote_url AND local_path IS NOT NULL 
                        LIMIT 1
                    )),
                    thumbnail_url = COALESCE(target.thumbnail_url, (
                        SELECT thumbnail_url FROM message_attachments 
                        WHERE message_id = target.message_id AND remote_url = target.remote_url AND thumbnail_url IS NOT NULL 
                        LIMIT 1
                    )),
                    metadata_json = CASE 
                        WHEN target.metadata_json IS NOT NULL AND target.metadata_json != '{}' THEN target.metadata_json
                        ELSE COALESCE((
                            SELECT metadata_json FROM message_attachments 
                            WHERE message_id = target.message_id AND remote_url = target.remote_url AND metadata_json IS NOT NULL AND metadata_json != '{}' 
                            LIMIT 1
                        ), '{}')
                    END
                WHERE target.id IN (
                    SELECT MIN(id) 
                    FROM message_attachments 
                    WHERE remote_url IS NOT NULL 
                    GROUP BY message_id, remote_url 
                    HAVING COUNT(*) > 1
                );

                -- Delete redundant duplicate records
                DELETE FROM message_attachments 
                WHERE remote_url IS NOT NULL 
                  AND id NOT IN (
                    SELECT MIN(id) 
                    FROM message_attachments 
                    WHERE remote_url IS NOT NULL
                    GROUP BY message_id, remote_url
                  );

                -- Merge non-null fields into the survivor row (MIN(id)) for provider_attachment_id duplicates
                UPDATE message_attachments
                SET 
                    file_name = COALESCE(file_name, (
                        SELECT d.file_name FROM message_attachments d 
                        WHERE d.message_id = message_attachments.message_id 
                          AND d.provider_attachment_id = message_attachments.provider_attachment_id 
                          AND d.file_name IS NOT NULL LIMIT 1
                    )),
                    mime_type = COALESCE(mime_type, (
                        SELECT d.mime_type FROM message_attachments d 
                        WHERE d.message_id = message_attachments.message_id 
                          AND d.provider_attachment_id = message_attachments.provider_attachment_id 
                          AND d.mime_type IS NOT NULL LIMIT 1
                    )),
                    size_bytes = COALESCE(size_bytes, (
                        SELECT d.size_bytes FROM message_attachments d 
                        WHERE d.message_id = message_attachments.message_id 
                          AND d.provider_attachment_id = message_attachments.provider_attachment_id 
                          AND d.size_bytes IS NOT NULL LIMIT 1
                    )),
                    remote_url = COALESCE(remote_url, (
                        SELECT d.remote_url FROM message_attachments d 
                        WHERE d.message_id = message_attachments.message_id 
                          AND d.provider_attachment_id = message_attachments.provider_attachment_id 
                          AND d.remote_url IS NOT NULL LIMIT 1
                    )),
                    local_path = COALESCE(local_path, (
                        SELECT d.local_path FROM message_attachments d 
                        WHERE d.message_id = message_attachments.message_id 
                          AND d.provider_attachment_id = message_attachments.provider_attachment_id 
                          AND d.local_path IS NOT NULL LIMIT 1
                    )),
                    thumbnail_url = COALESCE(thumbnail_url, (
                        SELECT d.thumbnail_url FROM message_attachments d 
                        WHERE d.message_id = message_attachments.message_id 
                          AND d.provider_attachment_id = message_attachments.provider_attachment_id 
                          AND d.thumbnail_url IS NOT NULL LIMIT 1
                    ))
                WHERE id IN (
                    SELECT MIN(id) 
                    FROM message_attachments 
                    WHERE provider_attachment_id IS NOT NULL 
                    GROUP BY message_id, provider_attachment_id 
                    HAVING COUNT(*) > 1
                );

                -- Delete redundant duplicate records
                DELETE FROM message_attachments 
                WHERE provider_attachment_id IS NOT NULL 
                  AND id NOT IN (
                    SELECT MIN(id) 
                    FROM message_attachments 
                    WHERE provider_attachment_id IS NOT NULL
                    GROUP BY message_id, provider_attachment_id
                  );

                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_remote 
                ON message_attachments(message_id, remote_url) 
                WHERE remote_url IS NOT NULL;

                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_provider
                ON message_attachments(message_id, provider_attachment_id)
                WHERE provider_attachment_id IS NOT NULL;
            "#)?;
        }

        // Conditionally add provider_timestamp to messages table if messages table exists
        let has_messages: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='messages'",
            [],
            |r| r.get(0),
        ).unwrap_or(false);

        if has_messages {
            let mut has_provider_timestamp = false;
            {
                let mut stmt = conn.prepare("PRAGMA table_info(messages)")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(1))?;
                for col in rows.flatten() {
                    if col == "provider_timestamp" { has_provider_timestamp = true; }
                }
            }
            if !has_provider_timestamp {
                conn.execute_batch("ALTER TABLE messages ADD COLUMN provider_timestamp TEXT;")?;
            }
        }

        // Verify foreign key integrity
        let fk_violations: i64 = conn.query_row("SELECT count(*) FROM pragma_foreign_key_check()", [], |r| r.get(0))?;
        if fk_violations > 0 {
            return Err(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                Some(format!("Foreign key check failed with {} violations", fk_violations)),
            ));
        }

        conn.execute_batch("PRAGMA user_version = 3;")?;
        conn.execute_batch("RELEASE SAVEPOINT migration_v3;")?;
        Ok(())
    })();

    if migration_result.is_err() {
        let _ = conn.execute_batch("ROLLBACK TO SAVEPOINT migration_v3; RELEASE SAVEPOINT migration_v3;");
    }

    guard.restore()?;
    migration_result
}
