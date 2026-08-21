use rusqlite::{Connection, Result};

pub fn apply_migration_v2(conn: &Connection) -> Result<()> {
    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if current_version >= 2 {
        return Ok(());
    }

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
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
            conn.execute_batch(r#"
                INSERT INTO channel_identities_v2 (
                    id, lead_id, channel, channel_account_id, identity_namespace, external_id,
                    normalized_address, display_name, avatar_url, profile_synced_at,
                    profile_sync_error, profile_sync_attempts, next_profile_retry_at,
                    link_source, link_confidence, verified_at, created_at, updated_at
                )
                SELECT 
                    id, lead_id, channel, NULL, identity_namespace, external_id,
                    normalized_address, display_name, avatar_url, profile_synced_at,
                    profile_sync_error, profile_sync_attempts, next_profile_retry_at,
                    link_source, link_confidence, verified_at, created_at, updated_at
                FROM channel_identities;

                DROP TABLE IF EXISTS channel_identities;
                ALTER TABLE channel_identities_v2 RENAME TO channel_identities;
            "#)?;
        } else {
            conn.execute_batch("ALTER TABLE channel_identities_v2 RENAME TO channel_identities;")?;
        }

        conn.execute_batch(r#"
            -- 2. Rebuild indexes with unique constraints
            CREATE INDEX IF NOT EXISTS idx_channel_identities_lead ON channel_identities(lead_id, channel);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_channel_identities_legacy_unique 
            ON channel_identities(channel, identity_namespace, external_id);
            CREATE INDEX IF NOT EXISTS idx_channel_identities_page_scope 
            ON channel_identities(channel, channel_account_id, identity_namespace, external_id);

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

        // Check FK violations using a scoped statement
        {
            let mut fk_stmt = conn.prepare("PRAGMA foreign_key_check")?;
            let fk_violations: Vec<String> = fk_stmt
                .query_map([], |row| Ok(row.get::<_, String>(0)?))?
                .filter_map(|r| r.ok())
                .collect();

            if !fk_violations.is_empty() {
                return Err(rusqlite::Error::InvalidQuery);
            }
        }

        conn.execute_batch("PRAGMA user_version = 2;")?;
        Ok(())
    })();

    let pragma_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
    match migration_result {
        Ok(_) => pragma_res,
        Err(e) => {
            let _ = pragma_res;
            Err(e)
        }
    }
}

pub fn apply_migration_v3(conn: &Connection) -> Result<()> {
    apply_migration_v2(conn)?;

    let current_version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap_or(0);
    if current_version >= 3 {
        return Ok(());
    }

    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;

    let migration_result = (|| -> Result<()> {
        conn.execute_batch(r#"
            -- 1. Create channel_sync_dead_letters table
            CREATE TABLE IF NOT EXISTS channel_sync_dead_letters (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                scope TEXT NOT NULL,
                event_id INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                event_key TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                error_reason TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE,
                UNIQUE(channel_account_id, scope, event_id)
            );
            CREATE INDEX IF NOT EXISTS idx_dead_letters_account_scope ON channel_sync_dead_letters(channel_account_id, scope);
        "#)?;

        let has_attachments: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='message_attachments'",
            [],
            |r| r.get(0),
        ).unwrap_or(false);

        if has_attachments {
            conn.execute_batch(r#"
                -- 2. Deduplicate any existing duplicate attachments before creating unique index
                DELETE FROM message_attachments 
                WHERE id NOT IN (
                    SELECT MIN(id) 
                    FROM message_attachments 
                    GROUP BY message_id, COALESCE(remote_url, id)
                );
                CREATE UNIQUE INDEX IF NOT EXISTS uq_message_attachments_remote ON message_attachments(message_id, remote_url) WHERE remote_url IS NOT NULL;
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

        conn.execute_batch("PRAGMA user_version = 3;")?;
        Ok(())
    })();

    let pragma_res = conn.execute_batch("PRAGMA foreign_keys = ON;");
    match migration_result {
        Ok(_) => pragma_res,
        Err(e) => {
            let _ = pragma_res;
            Err(e)
        }
    }
}
