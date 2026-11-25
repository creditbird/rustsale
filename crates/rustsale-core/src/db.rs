use rusqlite::functions::FunctionFlags;
use rusqlite::{Connection, Result};
use std::time::Duration;

fn is_safe_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Idempotent migration helper that checks SQLite PRAGMA table_info before altering the table.
/// Operates on the active connection or transaction (&conn / &tx). Validates table and column
/// identifiers to prevent SQL injection. If the column already exists, it safely no-ops.
/// If column addition fails, the returned Result propagates via `?` to allow clean rollback
/// of the enclosing transaction.
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    col_name: &str,
    col_def: &str,
) -> Result<()> {
    if !is_safe_identifier(table) || !is_safe_identifier(col_name) {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "Unsafe SQL identifier in migration: table={}, column={}",
            table, col_name
        )));
    }

    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    let mut exists = false;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == col_name {
            exists = true;
            break;
        }
    }
    if !exists {
        conn.execute(&format!("ALTER TABLE {} ADD COLUMN {}", table, col_def), [])?;
    }
    Ok(())
}

/// Registers Unicode-aware SQL helper functions on `conn`.
///
/// Bundled SQLite `lower()` only folds ASCII, so queries that match or sort
/// Vietnamese text case-insensitively call `unicode_lower(...)` instead.
///
/// Only connections from `open_db_connection`/`init_db` have these.
/// Do NOT use `unicode_lower` in indexes, views, triggers or CHECK constraints,
/// or any connection that skips this will fail with `no such function`.
pub fn register_sql_functions(conn: &Connection) -> Result<()> {
    conn.create_scalar_function(
        "unicode_lower",
        1,
        FunctionFlags::SQLITE_UTF8
            | FunctionFlags::SQLITE_DETERMINISTIC
            | FunctionFlags::SQLITE_INNOCUOUS,
        |ctx| {
            let value: Option<String> = ctx.get(0)?;
            Ok(value.map(|text| text.to_lowercase()))
        },
    )
}

pub fn open_db_connection(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    register_sql_functions(&conn)?;
    Ok(conn)
}

pub fn init_db(db_path: &str) -> Result<Connection> {
    let mut conn = open_db_connection(db_path)?;
    conn.pragma_update(None, "foreign_keys", "OFF")?;

    let init_result = (|| -> Result<()> {
        let tx = conn.transaction()?;

        // ARCHITECTURAL INVARIANT (MULTI_CURRENCY_v1):
        // Numeric monetary amounts in the database (`leads.value`, `deals.value`) represent the raw
        // numeric quantity in their respective row `currency` ('VND' | 'USD'). Database integrity
        // triggers and backend validation ensure valid currency codes, with legacy rows defaulting to 'VND'.
        tx.execute(
            "CREATE TABLE IF NOT EXISTS leads (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                company TEXT NOT NULL,
                status TEXT NOT NULL,
                value REAL NOT NULL DEFAULT 0.0,
                last_activity TEXT NOT NULL,
                owner TEXT NOT NULL,
                email TEXT NOT NULL,
                phone TEXT NOT NULL,
                about TEXT,
                address TEXT,
                has_zalo INTEGER DEFAULT NULL,
                zalo_reached INTEGER NOT NULL DEFAULT 0,
                zalo_uid TEXT,
                zalo_status TEXT DEFAULT 'UNTESTED',
                relationship_status TEXT DEFAULT 'UNKNOWN',
                last_lookup_attempt TEXT
            )",
            [],
        )?;

        add_column_if_missing(&tx, "leads", "address", "address TEXT")?;
        add_column_if_missing(&tx, "leads", "has_zalo", "has_zalo INTEGER DEFAULT NULL")?;
        add_column_if_missing(
            &tx,
            "leads",
            "zalo_reached",
            "zalo_reached INTEGER NOT NULL DEFAULT 0",
        )?;
        add_column_if_missing(&tx, "leads", "zalo_uid", "zalo_uid TEXT")?;
        add_column_if_missing(
            &tx,
            "leads",
            "zalo_status",
            "zalo_status TEXT DEFAULT 'UNTESTED'",
        )?;
        add_column_if_missing(
            &tx,
            "leads",
            "relationship_status",
            "relationship_status TEXT DEFAULT 'UNKNOWN'",
        )?;
        add_column_if_missing(
            &tx,
            "leads",
            "last_lookup_attempt",
            "last_lookup_attempt TEXT",
        )?;
        add_column_if_missing(
            &tx,
            "leads",
            "currency",
            "currency TEXT NOT NULL DEFAULT 'VND'",
        )?;

        // SQLite triggers to enforce currency integrity on insert and update for leads
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS validate_lead_currency_insert
             BEFORE INSERT ON leads
             FOR EACH ROW
             WHEN NEW.currency NOT IN ('VND', 'USD')
             BEGIN
                 SELECT RAISE(ABORT, 'Invalid currency: must be VND or USD');
             END;",
            [],
        )?;
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS validate_lead_currency_update
             BEFORE UPDATE OF currency ON leads
             FOR EACH ROW
             WHEN NEW.currency NOT IN ('VND', 'USD')
             BEGIN
                 SELECT RAISE(ABORT, 'Invalid currency: must be VND or USD');
             END;",
            [],
        )?;

        crate::conversation::migration::migrate(&tx)?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS activities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER NOT NULL,
                activity_type TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                performed_by TEXT NOT NULL,
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS deals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                value REAL NOT NULL DEFAULT 0.0,
                stage TEXT NOT NULL,
                created_at TEXT NOT NULL,
                currency TEXT NOT NULL DEFAULT 'VND',
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
            )",
            [],
        )?;
        add_column_if_missing(
            &tx,
            "deals",
            "currency",
            "currency TEXT NOT NULL DEFAULT 'VND'",
        )?;

        // SQLite triggers to enforce currency integrity on insert and update for deals
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS validate_deal_currency_insert
             BEFORE INSERT ON deals
             FOR EACH ROW
             WHEN NEW.currency NOT IN ('VND', 'USD')
             BEGIN
                 SELECT RAISE(ABORT, 'Invalid currency: must be VND or USD');
             END;",
            [],
        )?;
        tx.execute(
            "CREATE TRIGGER IF NOT EXISTS validate_deal_currency_update
             BEFORE UPDATE OF currency ON deals
             FOR EACH ROW
             WHEN NEW.currency NOT IN ('VND', 'USD')
             BEGIN
                 SELECT RAISE(ABORT, 'Invalid currency: must be VND or USD');
             END;",
            [],
        )?;

        tx.execute(
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
            )",
            [],
        )?;
        let _ = tx.execute("ALTER TABLE zalo_sessions ADD COLUMN display_name TEXT", []);
        let _ = tx.execute("ALTER TABLE zalo_sessions ADD COLUMN avatar TEXT", []);
        let _ = tx.execute(
            "ALTER TABLE zalo_sessions ADD COLUMN auth_status TEXT NOT NULL DEFAULT 'unknown'",
            [],
        );
        let _ = tx.execute(
            "ALTER TABLE zalo_sessions ADD COLUMN auth_checked_at TEXT",
            [],
        );
        let _ = tx.execute("ALTER TABLE zalo_sessions ADD COLUMN auth_error TEXT", []);
        let _ = tx.execute("ALTER TABLE zalo_sessions ADD COLUMN zalo_uid TEXT", []);
        let _ = tx.execute(
            "ALTER TABLE zalo_sessions ADD COLUMN channel_account_id INTEGER REFERENCES channel_accounts(id) ON DELETE SET NULL",
            [],
        );
        let _ = tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_zalo_sessions_zalo_uid ON zalo_sessions(zalo_uid) WHERE zalo_uid IS NOT NULL AND zalo_uid != ''",
            [],
        );
        let _ = tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_zalo_sessions_account_id ON zalo_sessions(channel_account_id) WHERE channel_account_id IS NOT NULL",
            [],
        );

        tx.execute(
            "CREATE TABLE IF NOT EXISTS outreach_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                image_url TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;
        let _ = tx.execute(
            "ALTER TABLE outreach_templates ADD COLUMN image_url TEXT",
            [],
        );

        tx.execute(
            "CREATE TABLE IF NOT EXISTS categories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS lead_categories (
                lead_id INTEGER NOT NULL,
                category_id INTEGER NOT NULL,
                PRIMARY KEY (lead_id, category_id),
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE,
                FOREIGN KEY(category_id) REFERENCES categories(id) ON DELETE CASCADE
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS outreach_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER NOT NULL,
                template_id INTEGER NOT NULL,
                channel TEXT NOT NULL DEFAULT 'OMNICHANNEL',
                status TEXT NOT NULL,
                error_message TEXT,
                sent_at TEXT NOT NULL,
                session_imei TEXT NOT NULL,
                idempotency_key TEXT UNIQUE,
                provider_message_id TEXT,
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE,
                FOREIGN KEY(template_id) REFERENCES outreach_templates(id) ON DELETE CASCADE
            )",
            [],
        )?;

        add_column_if_missing(
            &tx,
            "outreach_history",
            "channel",
            "channel TEXT NOT NULL DEFAULT 'OMNICHANNEL'",
        )?;
        add_column_if_missing(
            &tx,
            "outreach_history",
            "idempotency_key",
            "idempotency_key TEXT",
        )?;
        add_column_if_missing(
            &tx,
            "outreach_history",
            "provider_message_id",
            "provider_message_id TEXT",
        )?;
        tx.execute(
            "UPDATE outreach_history SET idempotency_key = 'legacy:history:' || id WHERE idempotency_key IS NULL",
            [],
        )?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_outreach_history_idempotency ON outreach_history(idempotency_key)",
            [],
        )?;

        // Scoped composite unique index on (channel, provider_message_id) preserves
        // externally issued identifiers intact without mutating historical values.
        tx.execute("DROP INDEX IF EXISTS idx_outreach_provider_message_id", [])?;
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_outreach_channel_provider_msg_id
             ON outreach_history(channel, provider_message_id)
             WHERE provider_message_id IS NOT NULL",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_lead_categories_category ON lead_categories(category_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_outreach_history_lead ON outreach_history(lead_id)",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS smtp_servers (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                password TEXT NOT NULL,
                from_email TEXT NOT NULL,
                from_name TEXT NOT NULL,
                security TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS email_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                subject TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS email_outreach_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER NOT NULL,
                template_id INTEGER NOT NULL,
                smtp_id INTEGER NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                sent_at TEXT NOT NULL,
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE,
                FOREIGN KEY(template_id) REFERENCES email_templates(id) ON DELETE CASCADE,
                FOREIGN KEY(smtp_id) REFERENCES smtp_servers(id) ON DELETE CASCADE
            )",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_email_outreach_history_lead ON email_outreach_history(lead_id)",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS google_maps_scrape_jobs (
                id TEXT PRIMARY KEY,
                mode TEXT NOT NULL,
                keyword TEXT NOT NULL,
                location TEXT,
                bbox_json TEXT,
                step_km REAL,
                headless INTEGER NOT NULL CHECK (headless IN (0, 1)),
                profile_dir TEXT NOT NULL DEFAULT '',
                blocked_url TEXT,
                status TEXT NOT NULL,
                stage TEXT NOT NULL,
                total_units INTEGER NOT NULL DEFAULT 0 CHECK (total_units >= 0),
                processed_units INTEGER NOT NULL DEFAULT 0 CHECK (processed_units >= 0),
                failed_units INTEGER NOT NULL DEFAULT 0 CHECK (failed_units >= 0),
                businesses_discovered INTEGER NOT NULL DEFAULT 0 CHECK (businesses_discovered >= 0),
                businesses_qualified INTEGER NOT NULL DEFAULT 0 CHECK (businesses_qualified >= 0),
                leads_created INTEGER NOT NULL DEFAULT 0 CHECK (leads_created >= 0),
                last_error_kind TEXT,
                last_error_message TEXT,
                action_required TEXT,
                event_sequence INTEGER NOT NULL DEFAULT 0 CHECK (event_sequence >= 0),
                version INTEGER NOT NULL DEFAULT 0 CHECK (version >= 0),
                run_generation INTEGER NOT NULL DEFAULT 0 CHECK (run_generation >= 0),
                created_at TEXT NOT NULL,
                started_at TEXT,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                coverage_truncated INTEGER NOT NULL DEFAULT 0
                    CHECK (coverage_truncated IN (0, 1)),
                owner_id TEXT,
                heartbeat_at TEXT
            )",
            [],
        )?;

        add_column_if_missing(
            &tx,
            "google_maps_scrape_jobs",
            "profile_dir",
            "profile_dir TEXT NOT NULL DEFAULT ''",
        )?;
        add_column_if_missing(
            &tx,
            "google_maps_scrape_jobs",
            "blocked_url",
            "blocked_url TEXT",
        )?;
        add_column_if_missing(
            &tx,
            "google_maps_scrape_jobs",
            "coverage_truncated",
            "coverage_truncated INTEGER NOT NULL DEFAULT 0 CHECK (coverage_truncated IN (0, 1))",
        )?;
        // Lease of the instance running the job (A-U2). Both stay NULL for rows
        // written before leases existed, which recovery treats as dead.
        add_column_if_missing(&tx, "google_maps_scrape_jobs", "owner_id", "owner_id TEXT")?;
        add_column_if_missing(
            &tx,
            "google_maps_scrape_jobs",
            "heartbeat_at",
            "heartbeat_at TEXT",
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS google_maps_scrape_units (
                id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL,
                unit_key TEXT NOT NULL,
                unit_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                status TEXT NOT NULL,
                depth INTEGER NOT NULL DEFAULT 0 CHECK (depth >= 0),
                attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
                next_retry_at TEXT,
                last_error_kind TEXT,
                last_error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                completed_at TEXT,
                UNIQUE (job_id, unit_key),
                UNIQUE (job_id, id),
                FOREIGN KEY (job_id) REFERENCES google_maps_scrape_jobs(id) ON DELETE CASCADE
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS google_maps_scrape_results (
                id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL,
                source_unit_id TEXT NOT NULL,
                place_id TEXT,
                canonical_url TEXT,
                dedup_key TEXT NOT NULL,
                name TEXT NOT NULL,
                phone TEXT,
                normalized_phone TEXT,
                email TEXT,
                normalized_email TEXT,
                address TEXT,
                website TEXT,
                facebook TEXT,
                zalo TEXT,
                latitude REAL,
                longitude REAL,
                inside_requested_area INTEGER CHECK (inside_requested_area IN (0, 1)),
                detail_status TEXT NOT NULL,
                enrichment_status TEXT NOT NULL,
                import_status TEXT NOT NULL,
                last_error_kind TEXT,
                last_error_message TEXT,
                first_seen_at TEXT NOT NULL,
                last_seen_at TEXT NOT NULL,
                UNIQUE (job_id, dedup_key),
                FOREIGN KEY (job_id) REFERENCES google_maps_scrape_jobs(id) ON DELETE CASCADE,
                FOREIGN KEY (job_id, source_unit_id)
                    REFERENCES google_maps_scrape_units(job_id, id) ON DELETE CASCADE
            )",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_google_maps_jobs_status
             ON google_maps_scrape_jobs(status, updated_at)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_google_maps_units_job_status
             ON google_maps_scrape_units(job_id, status, next_retry_at)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_google_maps_units_job_created
             ON google_maps_scrape_units(job_id, status, created_at, id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_google_maps_results_job_import
             ON google_maps_scrape_results(job_id, import_status, enrichment_status)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_leads_phone ON leads(phone)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_leads_email_normalized
             ON leads(lower(trim(email)))",
            [],
        )?;

        // ====================================================================
        // Unified Template Management Center (P1-A)
        // ====================================================================
        tx.execute(
            "CREATE TABLE IF NOT EXISTS template_folders (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                color TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                folder_id INTEGER REFERENCES template_folders(id) ON DELETE SET NULL,
                name TEXT NOT NULL,
                channel TEXT NOT NULL,
                stage TEXT,
                subject TEXT,
                content TEXT NOT NULL,
                media_url TEXT,
                ai_prompt TEXT,
                tags_json TEXT,
                usage_count INTEGER NOT NULL DEFAULT 0,
                last_used_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_templates_channel ON templates(channel)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_templates_folder ON templates(folder_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_templates_stage ON templates(stage)",
            [],
        )?;

        // Idempotent migration from legacy outreach_templates (zalo) and email_templates (email)
        tx.execute(
            "INSERT INTO templates (
                name, channel, stage, subject, content, media_url, ai_prompt, tags_json, usage_count, last_used_at, created_at, updated_at
            )
            SELECT 
                ot.name,
                'zalo',
                NULL,
                NULL,
                ot.content,
                ot.image_url,
                NULL,
                '[\"zalo\"]',
                0,
                NULL,
                ot.created_at,
                ot.created_at
            FROM outreach_templates ot
            WHERE NOT EXISTS (
                SELECT 1 FROM templates t 
                WHERE t.channel = 'zalo' AND t.name = ot.name AND t.content = ot.content
            )",
            [],
        )?;

        tx.execute(
            "INSERT INTO templates (
                name, channel, stage, subject, content, media_url, ai_prompt, tags_json, usage_count, last_used_at, created_at, updated_at
            )
            SELECT 
                et.name,
                'email',
                NULL,
                et.subject,
                et.content,
                NULL,
                NULL,
                '[\"email\"]',
                0,
                NULL,
                et.created_at,
                et.created_at
            FROM email_templates et
            WHERE NOT EXISTS (
                SELECT 1 FROM templates t 
                WHERE t.channel = 'email' AND t.name = et.name AND t.content = et.content
            )",
            [],
        )?;

        // ====================================================================
        // Unified Omnichannel Campaigns & Sequences (P1-A)
        // ====================================================================
        tx.execute(
            "CREATE TABLE IF NOT EXISTS campaigns (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                description TEXT,
                channel_type TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'DRAFT',
                audience_filter_json TEXT,
                schedule_config_json TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS campaign_steps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
                step_order INTEGER NOT NULL,
                channel TEXT NOT NULL,
                delay_minutes INTEGER NOT NULL DEFAULT 0,
                condition_type TEXT NOT NULL DEFAULT 'ALWAYS',
                template_content TEXT NOT NULL,
                ai_prompt_override TEXT,
                created_at TEXT NOT NULL,
                template_id INTEGER REFERENCES templates(id) ON DELETE SET NULL
            )",
            [],
        )?;

        let _ = tx.execute(
            "ALTER TABLE campaign_steps ADD COLUMN template_id INTEGER REFERENCES templates(id) ON DELETE SET NULL",
            [],
        );

        // A step an edit removed but whose sends are history is kept with `retired_at` set, so the
        // executions that point at it are not cascade-deleted. Readers of live steps skip it.
        let _ = tx.execute("ALTER TABLE campaign_steps ADD COLUMN retired_at TEXT", []);

        tx.execute(
            "CREATE TABLE IF NOT EXISTS campaign_executions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
                step_id INTEGER NOT NULL REFERENCES campaign_steps(id) ON DELETE CASCADE,
                lead_id INTEGER NOT NULL REFERENCES leads(id) ON DELETE CASCADE,
                target_phone TEXT,
                target_account_id TEXT,
                status TEXT NOT NULL DEFAULT 'PENDING',
                error_message TEXT,
                scheduled_at TEXT NOT NULL,
                executed_at TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS lead_journey_states (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER NOT NULL UNIQUE REFERENCES leads(id) ON DELETE CASCADE,
                current_stage TEXT NOT NULL DEFAULT 'DISCOVERY',
                active_campaign_id INTEGER REFERENCES campaigns(id) ON DELETE SET NULL,
                last_inbound_at TEXT,
                last_outbound_at TEXT,
                ai_intent_tag TEXT,
                ai_qualification_score REAL,
                notes TEXT,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS ai_evidence_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER REFERENCES leads(id) ON DELETE CASCADE,
                conversation_id INTEGER,
                action_type TEXT NOT NULL,
                evidence_source TEXT NOT NULL,
                evidence_data_json TEXT NOT NULL,
                confidence_score REAL NOT NULL,
                decision TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS ai_action_proposals (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                lead_id INTEGER REFERENCES leads(id) ON DELETE CASCADE,
                conversation_id INTEGER,
                platform TEXT NOT NULL,
                proposed_content TEXT NOT NULL,
                reasoning TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'PENDING',
                reviewed_by TEXT,
                reviewed_at TEXT,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_campaign_steps_order
             ON campaign_steps(campaign_id, step_order)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_campaign_executions_status
             ON campaign_executions(status, scheduled_at)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_campaign_executions_campaign_lead
             ON campaign_executions(campaign_id, lead_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_lead_journey_lead
             ON lead_journey_states(lead_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_ai_evidence_logs_lead
             ON ai_evidence_logs(lead_id)",
            [],
        )?;
        tx.execute(
            "CREATE INDEX IF NOT EXISTS idx_ai_action_proposals_status
             ON ai_action_proposals(status, created_at)",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS app_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        // Monetary metadata versioning:
        // Multi-currency storage (MULTI_CURRENCY_v1):
        // leads.value and deals.value store the monetary amount in their designated leads.currency / deals.currency column ('VND' | 'USD').
        // Defaults to 'VND' for existing rows and fallback.
        tx.execute(
            "INSERT INTO app_metadata (key, value) VALUES ('currency_semantics', 'MULTI_CURRENCY_v1')
             ON CONFLICT(key) DO UPDATE SET value = 'MULTI_CURRENCY_v1'",
            [],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO app_metadata (key, value) VALUES ('base_currency', 'VND')",
            [],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO app_metadata (key, value) VALUES ('usd_vnd_exchange_rate', '25400')",
            [],
        )?;

        // Verify foreign key integrity inside transaction before commit
        {
            let mut stmt = tx
                .prepare("SELECT \"table\", rowid, parent, fkid FROM pragma_foreign_key_check()")?;
            let violations: Vec<String> = stmt
                .query_map([], |row| {
                    let table: String = row.get(0)?;
                    let rowid: i64 = row.get(1)?;
                    let parent: String = row.get(2)?;
                    let fkid: i64 = row.get(3)?;
                    Ok(format!(
                        "table='{}', rowid={}, parent='{}', fkid={}",
                        table, rowid, parent, fkid
                    ))
                })?
                .filter_map(|r| r.ok())
                .collect();

            if !violations.is_empty() {
                eprintln!(
                    "Database initialization failed foreign key check: {:?}",
                    violations
                );
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY),
                    Some(format!(
                        "Foreign key check failed before commit with {} violations: [{}]",
                        violations.len(),
                        violations.join("; ")
                    )),
                ));
            }
        }

        tx.commit()?;
        Ok(())
    })();

    // Always re-enable foreign keys even on error exit
    let pragma_restore = conn.pragma_update(None, "foreign_keys", "ON");
    init_result?;
    pragma_restore?;
    Ok(conn)
}

#[allow(dead_code)]
pub fn seed_data(conn: &mut Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))?;
    if count == 0 {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached) VALUES 
             ('Sarah Harrington', 'Acme Corp', 'Qualified', 45000000.0, '2026-07-15T10:00:00Z', 'Alex Rivera', 'sarah@acme.com', '+12345678', 'CTO of Acme Corp', '123 Main St, New York', 1, 0),
             ('Jordan Smith', 'CloudSync', 'New', 15000000.0, '2026-07-16T15:30:00Z', 'Jordan Smith', 'jordan@cloudsync.com', '+87654321', 'Procurement lead', '456 Oak Ave, Boston', NULL, 0),
             ('lele', 'Zalo Corp', 'Qualified', 20000000.0, '2026-07-17T12:00:00Z', 'Alex Rivera', 'lele@example.com', '+84987654321', 'Zalo developer', '789 Pine Rd, Seattle', 1, 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO deals (lead_id, title, value, stage, created_at) VALUES 
             (1, 'Acme Platform Expansion', 45000000.0, 'Proposal', '2026-07-15T10:00:00Z'),
             (2, 'CloudSync Core Migration', 15000000.0, 'Prospecting', '2026-07-16T15:30:00Z')",
            [],
        )?;
        tx.execute(
            "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES 
             (1, 'Email', 'Initial discovery email sent.', '2026-07-15T10:00:00Z', 'Alex Rivera')",
            [],
        )?;
        tx.commit()?;
    }
    Ok(())
}

pub fn save_zalo_session(
    conn: &Connection,
    imei: &str,
    user_agent: &str,
    cookies_json: &str,
    language: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(imei) DO UPDATE SET
             user_agent = excluded.user_agent,
             cookies_json = excluded.cookies_json,
             language = excluded.language",
        rusqlite::params![imei, user_agent, cookies_json, language],
    )?;
    Ok(())
}

pub fn update_zalo_session_profile(
    conn: &Connection,
    imei: &str,
    display_name: Option<&str>,
    avatar: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE zalo_sessions
         SET display_name = ?1, avatar = ?2
         WHERE imei = ?3 OR zalo_uid = ?3
            OR channel_account_id IN (
                SELECT id FROM channel_accounts WHERE channel = 'zalo' AND (provider_account_key = ?3 OR self_external_id = ?3)
            )",
        rusqlite::params![display_name, avatar, imei],
    )?;
    Ok(())
}

pub fn update_zalo_session_auth_status(
    conn: &Connection,
    imei: &str,
    auth_status: &str,
    auth_error: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE zalo_sessions
         SET auth_status = ?1,
             auth_checked_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
             auth_error = ?2
         WHERE imei = ?3 OR zalo_uid = ?3
            OR channel_account_id IN (
                SELECT id FROM channel_accounts WHERE channel = 'zalo' AND (provider_account_key = ?3 OR self_external_id = ?3)
            )",
        rusqlite::params![auth_status, auth_error, imei],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_zalo_session(
    conn: &Connection,
    imei: &str,
) -> Result<Option<crate::models::ZaloSession>> {
    let mut stmt = conn.prepare(
        "SELECT zs.imei, zs.user_agent, zs.cookies_json, zs.language, zs.display_name, zs.avatar, zs.zalo_uid, zs.channel_account_id
         FROM zalo_sessions zs
         LEFT JOIN channel_accounts ca ON ca.id = zs.channel_account_id AND ca.channel = 'zalo'
         WHERE zs.imei = ?1 OR zs.zalo_uid = ?1
            OR (ca.provider_account_key IS NOT NULL AND ca.provider_account_key = ?1)
            OR (ca.self_external_id IS NOT NULL AND ca.self_external_id = ?1)
         ORDER BY zs.rowid DESC LIMIT 1",
    )?;
    let mut rows = stmt.query([imei])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::ZaloSession {
            imei: row.get(0)?,
            user_agent: row.get(1)?,
            cookies_json: row.get(2)?,
            language: row.get(3)?,
            display_name: row.get(4)?,
            avatar: row.get(5)?,
            zalo_uid: row.get(6)?,
            channel_account_id: row.get(7)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn get_all_zalo_sessions(conn: &Connection) -> Result<Vec<crate::models::ZaloSession>> {
    let mut stmt = conn.prepare(
        "SELECT zs.imei, zs.user_agent, zs.cookies_json, zs.language, zs.display_name, zs.avatar, zs.zalo_uid, zs.channel_account_id
         FROM zalo_sessions zs
         INNER JOIN (
             SELECT COALESCE(NULLIF(TRIM(zalo_uid), ''), imei) as account_key, MAX(rowid) as max_rowid
             FROM zalo_sessions
             GROUP BY COALESCE(NULLIF(TRIM(zalo_uid), ''), imei)
         ) latest ON zs.rowid = latest.max_rowid
         ORDER BY zs.rowid DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::ZaloSession {
            imei: row.get(0)?,
            user_agent: row.get(1)?,
            cookies_json: row.get(2)?,
            language: row.get(3)?,
            display_name: row.get(4)?,
            avatar: row.get(5)?,
            zalo_uid: row.get(6)?,
            channel_account_id: row.get(7)?,
        })
    })?;
    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row?);
    }
    Ok(sessions)
}

pub fn delete_zalo_session(conn: &Connection, imei: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM channel_accounts
         WHERE channel = 'zalo' AND (
             provider_account_key = ?1 OR self_external_id = ?1
             OR id IN (
                 SELECT channel_account_id FROM zalo_sessions
                 WHERE imei = ?1 OR zalo_uid = ?1
             )
         )",
        [imei],
    )?;
    conn.execute(
        "DELETE FROM zalo_sessions
         WHERE imei = ?1 OR zalo_uid = ?1
            OR channel_account_id IN (
                SELECT id FROM channel_accounts WHERE channel = 'zalo' AND (provider_account_key = ?1 OR self_external_id = ?1)
            )",
        [imei],
    )?;
    Ok(())
}

pub fn get_outreach_templates(conn: &Connection) -> Result<Vec<crate::models::OutreachTemplate>> {
    let mut stmt =
        conn.prepare("SELECT id, name, content, image_url, created_at FROM outreach_templates")?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::OutreachTemplate {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            content: row.get(2)?,
            image_url: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut templates = Vec::new();
    for row in rows {
        templates.push(row?);
    }
    Ok(templates)
}

pub fn create_outreach_template(
    conn: &Connection,
    name: &str,
    content: &str,
    image_url: Option<&str>,
) -> Result<i32> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO outreach_templates (name, content, image_url, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, content, image_url, now],
    )?;
    let last_id = conn.last_insert_rowid() as i32;
    Ok(last_id)
}

/// An UPDATE that matched no row means the target does not exist (any more).
fn ensure_row_changed(changed: usize) -> Result<()> {
    if changed == 0 {
        Err(rusqlite::Error::QueryReturnedNoRows)
    } else {
        Ok(())
    }
}

pub fn update_outreach_template(
    conn: &Connection,
    id: i32,
    name: &str,
    content: &str,
    image_url: Option<&str>,
) -> Result<()> {
    let changed = conn.execute(
        "UPDATE outreach_templates SET name = ?1, content = ?2, image_url = ?3 WHERE id = ?4",
        rusqlite::params![name, content, image_url, id],
    )?;
    ensure_row_changed(changed)
}

pub fn delete_outreach_template(conn: &Connection, id: i32) -> Result<()> {
    let changed = conn.execute("DELETE FROM outreach_templates WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

pub fn create_category(conn: &Connection, name: &str) -> Result<i32> {
    conn.execute("INSERT INTO categories (name) VALUES (?1)", [name])?;
    Ok(conn.last_insert_rowid() as i32)
}

pub fn delete_category(conn: &Connection, id: i32) -> Result<()> {
    let changed = conn.execute("DELETE FROM categories WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

pub fn add_lead_to_category(conn: &Connection, lead_id: i32, category_id: i32) -> Result<()> {
    conn.execute(
        "INSERT INTO lead_categories (lead_id, category_id) VALUES (?1, ?2)
         ON CONFLICT(lead_id, category_id) DO NOTHING",
        rusqlite::params![lead_id, category_id],
    )?;
    Ok(())
}

pub fn remove_lead_from_category(conn: &Connection, lead_id: i32, category_id: i32) -> Result<()> {
    conn.execute(
        "DELETE FROM lead_categories WHERE lead_id = ?1 AND category_id = ?2",
        rusqlite::params![lead_id, category_id],
    )?;
    Ok(())
}

pub fn log_outreach_attempt(
    conn: &Connection,
    history: &crate::models::OutreachHistory,
) -> Result<()> {
    conn.execute(
        "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            history.lead_id,
            history.template_id,
            history.status,
            history.error_message,
            history.sent_at,
            history.session_imei,
        ],
    )?;
    Ok(())
}

pub fn get_categories(conn: &Connection) -> Result<Vec<crate::models::Category>> {
    let mut stmt = conn.prepare("SELECT id, name FROM categories")?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::Category {
            id: Some(row.get(0)?),
            name: row.get(1)?,
        })
    })?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row?);
    }
    Ok(categories)
}

pub fn get_categories_for_lead(
    conn: &Connection,
    lead_id: i32,
) -> Result<Vec<crate::models::Category>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name
         FROM categories c
         JOIN lead_categories lc ON c.id = lc.category_id
         WHERE lc.lead_id = ?1
         ORDER BY lower(c.name)",
    )?;
    let rows = stmt.query_map([lead_id], |row| {
        Ok(crate::models::Category {
            id: Some(row.get(0)?),
            name: row.get(1)?,
        })
    })?;
    let mut categories = Vec::new();
    for row in rows {
        categories.push(row?);
    }
    Ok(categories)
}

pub fn get_leads_by_category(
    conn: &Connection,
    category_id: i32,
) -> Result<Vec<crate::models::Lead>> {
    let mut stmt = conn.prepare(
        "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency
         FROM leads l
         JOIN lead_categories lc ON l.id = lc.lead_id
         WHERE lc.category_id = ?1"
    )?;
    let rows = stmt.query_map([category_id], |row| {
        Ok(crate::models::Lead {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            company: row.get(2)?,
            status: row.get(3)?,
            value: row.get(4)?,
            last_activity: row.get(5)?,
            owner: row.get(6)?,
            email: row.get(7)?,
            phone: row.get(8)?,
            about: row.get(9)?,
            address: row.get(10)?,
            has_zalo: row.get(11)?,
            zalo_reached: row.get(12)?,
            zalo_uid: row.get(13)?,
            zalo_status: row.get(14)?,
            relationship_status: row.get(15)?,
            last_lookup_attempt: row.get(16)?,
            currency: row
                .get::<_, Option<String>>(17)
                .ok()
                .flatten()
                .or(Some("VND".to_string())),
        })
    })?;
    let mut leads = Vec::new();
    for row in rows {
        leads.push(row?);
    }
    Ok(leads)
}

pub fn save_smtp_server(conn: &Connection, server: &crate::models::SmtpServer) -> Result<i32> {
    let now = if server.created_at.is_empty() {
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?
    } else {
        server.created_at.clone()
    };

    if let Some(id) = server.id {
        if id > 0 {
            let changed = conn.execute(
                "UPDATE smtp_servers SET name = ?1, host = ?2, port = ?3, username = ?4,
                    password = CASE WHEN trim(?5) = '' THEN password ELSE ?5 END,
                    from_email = ?6, from_name = ?7, security = ?8 WHERE id = ?9",
                rusqlite::params![
                    server.name,
                    server.host,
                    server.port,
                    server.username,
                    server.password,
                    server.from_email,
                    server.from_name,
                    server.security,
                    id,
                ],
            )?;
            ensure_row_changed(changed)?;
            return Ok(id);
        }
    }

    conn.execute(
        "INSERT INTO smtp_servers (name, host, port, username, password, from_email, from_name, security, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            server.name,
            server.host,
            server.port,
            server.username,
            server.password,
            server.from_email,
            server.from_name,
            server.security,
            now,
        ],
    )?;
    Ok(conn.last_insert_rowid() as i32)
}

pub fn get_smtp_servers(conn: &Connection) -> Result<Vec<crate::models::SmtpServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, from_email, from_name, security, created_at FROM smtp_servers ORDER BY id ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::SmtpServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            from_email: row.get(6)?,
            from_name: row.get(7)?,
            security: row.get(8)?,
            created_at: row.get(9)?,
        })
    })?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn get_smtp_server_by_id(
    conn: &Connection,
    id: i32,
) -> Result<Option<crate::models::SmtpServer>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, from_email, from_name, security, created_at FROM smtp_servers WHERE id = ?1"
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(crate::models::SmtpServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            from_email: row.get(6)?,
            from_name: row.get(7)?,
            security: row.get(8)?,
            created_at: row.get(9)?,
        })
    })?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

pub fn delete_smtp_server(conn: &Connection, id: i32) -> Result<()> {
    let changed = conn.execute("DELETE FROM smtp_servers WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

pub fn create_email_template(
    conn: &Connection,
    name: &str,
    subject: &str,
    content: &str,
) -> Result<i32> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO email_templates (name, subject, content, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, subject, content, now],
    )?;
    Ok(conn.last_insert_rowid() as i32)
}

pub fn update_email_template(
    conn: &Connection,
    id: i32,
    name: &str,
    subject: &str,
    content: &str,
) -> Result<()> {
    let changed = conn.execute(
        "UPDATE email_templates SET name = ?1, subject = ?2, content = ?3 WHERE id = ?4",
        rusqlite::params![name, subject, content, id],
    )?;
    ensure_row_changed(changed)
}

pub fn delete_email_template(conn: &Connection, id: i32) -> Result<()> {
    let changed = conn.execute("DELETE FROM email_templates WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

pub fn get_email_templates(conn: &Connection) -> Result<Vec<crate::models::EmailTemplate>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, subject, content, created_at FROM email_templates ORDER BY id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::EmailTemplate {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            subject: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    let mut templates = Vec::new();
    for r in rows {
        templates.push(r?);
    }
    Ok(templates)
}

pub fn get_email_template_by_id(
    conn: &Connection,
    id: i32,
) -> Result<Option<crate::models::EmailTemplate>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, subject, content, created_at FROM email_templates WHERE id = ?1",
    )?;
    let mut rows = stmt.query_map([id], |row| {
        Ok(crate::models::EmailTemplate {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            subject: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
        })
    })?;
    if let Some(row) = rows.next() {
        Ok(Some(row?))
    } else {
        Ok(None)
    }
}

pub fn log_email_outreach_attempt(
    conn: &Connection,
    history: &crate::models::EmailOutreachHistory,
) -> Result<()> {
    conn.execute(
        "INSERT INTO email_outreach_history (lead_id, template_id, smtp_id, status, error_message, sent_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            history.lead_id,
            history.template_id,
            history.smtp_id,
            history.status,
            history.error_message,
            history.sent_at,
        ],
    )?;
    Ok(())
}

// ============================================================================
// Unified Campaigns & Sequence Management
// ============================================================================

#[allow(dead_code)]
pub fn create_campaign(
    conn: &Connection,
    name: &str,
    description: Option<&str>,
    channel_type: &str,
    status: &str,
    audience_filter_json: Option<&str>,
    schedule_config_json: Option<&str>,
) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO campaigns (name, description, channel_type, status, audience_filter_json, schedule_config_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        rusqlite::params![
            name,
            description,
            channel_type,
            status,
            audience_filter_json,
            schedule_config_json,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn get_campaign(conn: &Connection, id: i64) -> Result<Option<crate::models::Campaign>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, channel_type, status, audience_filter_json, schedule_config_json, created_at, updated_at
         FROM campaigns WHERE id = ?1",
    )?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::Campaign {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            description: row.get(2)?,
            channel_type: row.get(3)?,
            status: row.get(4)?,
            audience_filter_json: row.get(5)?,
            schedule_config_json: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn get_all_campaigns(conn: &Connection) -> Result<Vec<crate::models::Campaign>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, channel_type, status, audience_filter_json, schedule_config_json, created_at, updated_at
         FROM campaigns ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::Campaign {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            description: row.get(2)?,
            channel_type: row.get(3)?,
            status: row.get(4)?,
            audience_filter_json: row.get(5)?,
            schedule_config_json: row.get(6)?,
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        })
    })?;
    let mut campaigns = Vec::new();
    for r in rows {
        campaigns.push(r?);
    }
    Ok(campaigns)
}

#[allow(dead_code)]
pub fn update_campaign(
    conn: &Connection,
    id: i64,
    name: &str,
    description: Option<&str>,
    channel_type: &str,
    status: &str,
    audience_filter_json: Option<&str>,
    schedule_config_json: Option<&str>,
) -> Result<()> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "UPDATE campaigns
         SET name = ?1, description = ?2, channel_type = ?3, status = ?4,
             audience_filter_json = ?5, schedule_config_json = ?6, updated_at = ?7
         WHERE id = ?8",
        rusqlite::params![
            name,
            description,
            channel_type,
            status,
            audience_filter_json,
            schedule_config_json,
            now,
            id
        ],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn delete_campaign(conn: &Connection, id: i64) -> Result<()> {
    let changed = conn.execute("DELETE FROM campaigns WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

// ============================================================================
// Campaign Steps
// ============================================================================

#[allow(dead_code)]
pub fn create_campaign_step(
    conn: &Connection,
    campaign_id: i64,
    step_order: i32,
    channel: &str,
    delay_minutes: i32,
    condition_type: &str,
    template_content: &str,
    ai_prompt_override: Option<&str>,
    template_id: Option<i64>,
) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO campaign_steps (campaign_id, step_order, channel, delay_minutes, condition_type, template_content, ai_prompt_override, created_at, template_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            campaign_id,
            step_order,
            channel,
            delay_minutes,
            condition_type,
            template_content,
            ai_prompt_override,
            now,
            template_id,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn get_campaign_steps(
    conn: &Connection,
    campaign_id: i64,
) -> Result<Vec<crate::models::CampaignStep>> {
    let mut stmt = conn.prepare(
        "SELECT id, campaign_id, step_order, channel, delay_minutes, condition_type, template_content, ai_prompt_override, created_at, template_id
         FROM campaign_steps WHERE campaign_id = ?1 AND retired_at IS NULL
         ORDER BY step_order ASC, id ASC",
    )?;
    let rows = stmt.query_map([campaign_id], |row| {
        Ok(crate::models::CampaignStep {
            id: Some(row.get(0)?),
            campaign_id: row.get(1)?,
            step_order: row.get(2)?,
            channel: row.get(3)?,
            delay_minutes: row.get(4)?,
            condition_type: row.get(5)?,
            template_content: row.get(6)?,
            ai_prompt_override: row.get(7)?,
            created_at: row.get(8)?,
            template_id: row.get(9)?,
        })
    })?;
    let mut steps = Vec::new();
    for r in rows {
        steps.push(r?);
    }
    Ok(steps)
}

#[allow(dead_code)]
pub fn get_campaign_step(
    conn: &Connection,
    id: i64,
) -> Result<Option<crate::models::CampaignStep>> {
    let mut stmt = conn.prepare(
        "SELECT id, campaign_id, step_order, channel, delay_minutes, condition_type, template_content, ai_prompt_override, created_at, template_id
         FROM campaign_steps WHERE id = ?1 AND retired_at IS NULL",
    )?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::CampaignStep {
            id: Some(row.get(0)?),
            campaign_id: row.get(1)?,
            step_order: row.get(2)?,
            channel: row.get(3)?,
            delay_minutes: row.get(4)?,
            condition_type: row.get(5)?,
            template_content: row.get(6)?,
            ai_prompt_override: row.get(7)?,
            created_at: row.get(8)?,
            template_id: row.get(9)?,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn update_campaign_step(
    conn: &Connection,
    id: i64,
    step_order: i32,
    channel: &str,
    delay_minutes: i32,
    condition_type: &str,
    template_content: &str,
    ai_prompt_override: Option<&str>,
    template_id: Option<i64>,
) -> Result<()> {
    conn.execute(
        "UPDATE campaign_steps
         SET step_order = ?1, channel = ?2, delay_minutes = ?3, condition_type = ?4,
             template_content = ?5, ai_prompt_override = ?6, template_id = ?7
         WHERE id = ?8",
        rusqlite::params![
            step_order,
            channel,
            delay_minutes,
            condition_type,
            template_content,
            ai_prompt_override,
            template_id,
            id
        ],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn delete_campaign_step(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM campaign_steps WHERE id = ?1", [id])?;
    Ok(())
}

// ============================================================================
// Campaign Executions
// ============================================================================

#[allow(dead_code)]
pub fn create_campaign_execution(
    conn: &Connection,
    campaign_id: i64,
    step_id: i64,
    lead_id: i32,
    target_phone: Option<&str>,
    target_account_id: Option<&str>,
    scheduled_at: &str,
) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO campaign_executions (campaign_id, step_id, lead_id, target_phone, target_account_id, status, error_message, scheduled_at, executed_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'PENDING', NULL, ?6, NULL, ?7)",
        rusqlite::params![
            campaign_id,
            step_id,
            lead_id,
            target_phone,
            target_account_id,
            scheduled_at,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn get_campaign_executions(
    conn: &Connection,
    campaign_id: i64,
) -> Result<Vec<crate::models::CampaignExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, campaign_id, step_id, lead_id, target_phone, target_account_id, status, error_message, scheduled_at, executed_at, created_at
         FROM campaign_executions WHERE campaign_id = ?1 ORDER BY scheduled_at ASC",
    )?;
    let rows = stmt.query_map([campaign_id], |row| {
        Ok(crate::models::CampaignExecution {
            id: Some(row.get(0)?),
            campaign_id: row.get(1)?,
            step_id: row.get(2)?,
            lead_id: row.get(3)?,
            target_phone: row.get(4)?,
            target_account_id: row.get(5)?,
            status: row.get(6)?,
            error_message: row.get(7)?,
            scheduled_at: row.get(8)?,
            executed_at: row.get(9)?,
            created_at: row.get(10)?,
        })
    })?;
    let mut executions = Vec::new();
    for r in rows {
        executions.push(r?);
    }
    Ok(executions)
}

#[allow(dead_code)]
pub fn get_due_campaign_executions(
    conn: &Connection,
    now_iso: &str,
    limit: usize,
) -> Result<Vec<crate::models::CampaignExecution>> {
    let mut stmt = conn.prepare(
        "SELECT id, campaign_id, step_id, lead_id, target_phone, target_account_id, status, error_message, scheduled_at, executed_at, created_at
         FROM campaign_executions
         WHERE status = 'PENDING' AND scheduled_at <= ?1
         ORDER BY scheduled_at ASC
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(rusqlite::params![now_iso, limit as i64], |row| {
        Ok(crate::models::CampaignExecution {
            id: Some(row.get(0)?),
            campaign_id: row.get(1)?,
            step_id: row.get(2)?,
            lead_id: row.get(3)?,
            target_phone: row.get(4)?,
            target_account_id: row.get(5)?,
            status: row.get(6)?,
            error_message: row.get(7)?,
            scheduled_at: row.get(8)?,
            executed_at: row.get(9)?,
            created_at: row.get(10)?,
        })
    })?;
    let mut executions = Vec::new();
    for r in rows {
        executions.push(r?);
    }
    Ok(executions)
}

#[allow(dead_code)]
pub fn claim_campaign_execution(conn: &Connection, id: i64, now_iso: &str) -> Result<bool> {
    let rows_affected = conn.execute(
        "UPDATE campaign_executions
         SET status = 'RUNNING', executed_at = ?1
         WHERE id = ?2 AND status = 'PENDING'",
        rusqlite::params![now_iso, id],
    )?;
    Ok(rows_affected > 0)
}

#[allow(dead_code)]
pub fn update_campaign_execution_status(
    conn: &Connection,
    id: i64,
    status: &str,
    error_message: Option<&str>,
    executed_at: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE campaign_executions
         SET status = ?1, error_message = ?2, executed_at = ?3
         WHERE id = ?4",
        rusqlite::params![status, error_message, executed_at, id],
    )?;
    Ok(())
}

// ============================================================================
// Lead Journey States
// ============================================================================

#[allow(dead_code)]
pub fn get_lead_journey_state(
    conn: &Connection,
    lead_id: i32,
) -> Result<Option<crate::models::LeadJourneyState>> {
    let mut stmt = conn.prepare(
        "SELECT id, lead_id, current_stage, active_campaign_id, last_inbound_at, last_outbound_at, ai_intent_tag, ai_qualification_score, notes, updated_at
         FROM lead_journey_states WHERE lead_id = ?1",
    )?;
    let mut rows = stmt.query([lead_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::LeadJourneyState {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            current_stage: row.get(2)?,
            active_campaign_id: row.get(3)?,
            last_inbound_at: row.get(4)?,
            last_outbound_at: row.get(5)?,
            ai_intent_tag: row.get(6)?,
            ai_qualification_score: row.get(7)?,
            notes: row.get(8)?,
            updated_at: row.get(9)?,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn get_or_create_lead_journey_state(
    conn: &Connection,
    lead_id: i32,
) -> Result<crate::models::LeadJourneyState> {
    if let Some(state) = get_lead_journey_state(conn, lead_id)? {
        return Ok(state);
    }
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO lead_journey_states (lead_id, current_stage, active_campaign_id, last_inbound_at, last_outbound_at, ai_intent_tag, ai_qualification_score, notes, updated_at)
         VALUES (?1, 'DISCOVERY', NULL, NULL, NULL, NULL, NULL, NULL, ?2)",
        rusqlite::params![lead_id, now],
    )?;
    let id = conn.last_insert_rowid();
    Ok(crate::models::LeadJourneyState {
        id: Some(id),
        lead_id,
        current_stage: "DISCOVERY".to_string(),
        active_campaign_id: None,
        last_inbound_at: None,
        last_outbound_at: None,
        ai_intent_tag: None,
        ai_qualification_score: None,
        notes: None,
        updated_at: now,
    })
}

#[allow(dead_code)]
pub fn update_lead_journey_state(
    conn: &Connection,
    lead_id: i32,
    current_stage: &str,
    active_campaign_id: Option<i64>,
    last_inbound_at: Option<&str>,
    last_outbound_at: Option<&str>,
    ai_intent_tag: Option<&str>,
    ai_qualification_score: Option<f64>,
    notes: Option<&str>,
) -> Result<()> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO lead_journey_states (lead_id, current_stage, active_campaign_id, last_inbound_at, last_outbound_at, ai_intent_tag, ai_qualification_score, notes, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(lead_id) DO UPDATE SET
             current_stage = excluded.current_stage,
             active_campaign_id = excluded.active_campaign_id,
             last_inbound_at = COALESCE(excluded.last_inbound_at, lead_journey_states.last_inbound_at),
             last_outbound_at = COALESCE(excluded.last_outbound_at, lead_journey_states.last_outbound_at),
             ai_intent_tag = COALESCE(excluded.ai_intent_tag, lead_journey_states.ai_intent_tag),
             ai_qualification_score = COALESCE(excluded.ai_qualification_score, lead_journey_states.ai_qualification_score),
             notes = COALESCE(excluded.notes, lead_journey_states.notes),
             updated_at = excluded.updated_at",
        rusqlite::params![
            lead_id,
            current_stage,
            active_campaign_id,
            last_inbound_at,
            last_outbound_at,
            ai_intent_tag,
            ai_qualification_score,
            notes,
            now
        ],
    )?;
    Ok(())
}

// ============================================================================
// AI Evidence Logs
// ============================================================================

#[allow(dead_code)]
pub fn insert_ai_evidence_log(
    conn: &Connection,
    lead_id: Option<i32>,
    conversation_id: Option<i64>,
    action_type: &str,
    evidence_source: &str,
    evidence_data_json: &str,
    confidence_score: f64,
    decision: &str,
) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO ai_evidence_logs (lead_id, conversation_id, action_type, evidence_source, evidence_data_json, confidence_score, decision, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            lead_id,
            conversation_id,
            action_type,
            evidence_source,
            evidence_data_json,
            confidence_score,
            decision,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn get_ai_evidence_logs_by_lead(
    conn: &Connection,
    lead_id: i32,
) -> Result<Vec<crate::models::AiEvidenceLog>> {
    let mut stmt = conn.prepare(
        "SELECT id, lead_id, conversation_id, action_type, evidence_source, evidence_data_json, confidence_score, decision, created_at
         FROM ai_evidence_logs WHERE lead_id = ?1 ORDER BY id DESC",
    )?;
    let rows = stmt.query_map([lead_id], |row| {
        Ok(crate::models::AiEvidenceLog {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            conversation_id: row.get(2)?,
            action_type: row.get(3)?,
            evidence_source: row.get(4)?,
            evidence_data_json: row.get(5)?,
            confidence_score: row.get(6)?,
            decision: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    let mut logs = Vec::new();
    for r in rows {
        logs.push(r?);
    }
    Ok(logs)
}

// ============================================================================
// AI Action Proposals
// ============================================================================

#[allow(dead_code)]
pub fn create_ai_action_proposal(
    conn: &Connection,
    lead_id: Option<i32>,
    conversation_id: Option<i64>,
    platform: &str,
    proposed_content: &str,
    reasoning: &str,
) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO ai_action_proposals (lead_id, conversation_id, platform, proposed_content, reasoning, status, reviewed_by, reviewed_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'PENDING', NULL, NULL, ?6)",
        rusqlite::params![
            lead_id,
            conversation_id,
            platform,
            proposed_content,
            reasoning,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn get_ai_action_proposals(
    conn: &Connection,
    status_filter: Option<&str>,
) -> Result<Vec<crate::models::AiActionProposal>> {
    let mut proposals = Vec::new();
    if let Some(status) = status_filter {
        let mut stmt = conn.prepare(
            "SELECT id, lead_id, conversation_id, platform, proposed_content, reasoning, status, reviewed_by, reviewed_at, created_at
             FROM ai_action_proposals WHERE status = ?1 ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([status], |row| {
            Ok(crate::models::AiActionProposal {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                conversation_id: row.get(2)?,
                platform: row.get(3)?,
                proposed_content: row.get(4)?,
                reasoning: row.get(5)?,
                status: row.get(6)?,
                reviewed_by: row.get(7)?,
                reviewed_at: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;
        for r in rows {
            proposals.push(r?);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, lead_id, conversation_id, platform, proposed_content, reasoning, status, reviewed_by, reviewed_at, created_at
             FROM ai_action_proposals ORDER BY id DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(crate::models::AiActionProposal {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                conversation_id: row.get(2)?,
                platform: row.get(3)?,
                proposed_content: row.get(4)?,
                reasoning: row.get(5)?,
                status: row.get(6)?,
                reviewed_by: row.get(7)?,
                reviewed_at: row.get(8)?,
                created_at: row.get(9)?,
            })
        })?;
        for r in rows {
            proposals.push(r?);
        }
    }
    Ok(proposals)
}

#[allow(dead_code)]
pub fn get_ai_action_proposal(
    conn: &Connection,
    id: i64,
) -> Result<Option<crate::models::AiActionProposal>> {
    let mut stmt = conn.prepare(
        "SELECT id, lead_id, conversation_id, platform, proposed_content, reasoning, status, reviewed_by, reviewed_at, created_at
         FROM ai_action_proposals WHERE id = ?1",
    )?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::AiActionProposal {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            conversation_id: row.get(2)?,
            platform: row.get(3)?,
            proposed_content: row.get(4)?,
            reasoning: row.get(5)?,
            status: row.get(6)?,
            reviewed_by: row.get(7)?,
            reviewed_at: row.get(8)?,
            created_at: row.get(9)?,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn update_ai_action_proposal_status(
    conn: &Connection,
    id: i64,
    status: &str,
    reviewed_by: Option<&str>,
) -> Result<()> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    let changed = conn.execute(
        "UPDATE ai_action_proposals
         SET status = ?1, reviewed_by = ?2, reviewed_at = ?3
         WHERE id = ?4",
        rusqlite::params![status, reviewed_by, now, id],
    )?;
    ensure_row_changed(changed)
}

/// Replaces the text of a proposal that is still `PENDING`; `false` when no such row matched.
pub fn update_ai_action_proposal_content(
    conn: &Connection,
    id: i64,
    content: &str,
) -> Result<bool> {
    let changed = conn.execute(
        "UPDATE ai_action_proposals SET proposed_content = ?1 WHERE id = ?2 AND status = 'PENDING'",
        rusqlite::params![content, id],
    )?;
    Ok(changed > 0)
}

// ============================================================================
// Unified Template Management Center (P1-C)
// ============================================================================

#[allow(dead_code)]
pub fn get_template_folders(conn: &Connection) -> Result<Vec<crate::models::TemplateFolderRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at, updated_at
         FROM template_folders
         ORDER BY name ASC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::TemplateFolderRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            color: row.get(2)?,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })?;
    let mut folders = Vec::new();
    for r in rows {
        folders.push(r?);
    }
    Ok(folders)
}

#[allow(dead_code)]
pub fn create_template_folder(conn: &Connection, name: &str, color: Option<&str>) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO template_folders (name, color, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![name, color, now, now],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn delete_template_folder(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE templates SET folder_id = NULL WHERE folder_id = ?1",
        [id],
    )?;
    let changed = conn.execute("DELETE FROM template_folders WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

#[allow(dead_code)]
pub fn get_templates(
    conn: &Connection,
    filter: Option<&crate::models::TemplateFilter>,
) -> Result<Vec<crate::models::TemplateRecord>> {
    let mut sql = String::from(
        "SELECT id, folder_id, name, channel, stage, subject, content, media_url, ai_prompt, tags_json, usage_count, last_used_at, created_at, updated_at
         FROM templates
         WHERE 1=1",
    );
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(f) = filter {
        if let Some(ch) = &f.channel {
            let ch_trimmed = ch.trim();
            if !ch_trimmed.is_empty() && ch_trimmed.to_lowercase() != "all" {
                sql.push_str(" AND LOWER(channel) = LOWER(?)");
                params.push(Box::new(ch_trimmed.to_string()));
            }
        }
        if let Some(folder_id) = f.folder_id {
            if folder_id > 0 {
                sql.push_str(" AND folder_id = ?");
                params.push(Box::new(folder_id));
            } else if folder_id == -1 {
                sql.push_str(" AND folder_id IS NULL");
            }
        }
        if let Some(unfiled) = f.unfiled {
            sql.push_str(if unfiled {
                " AND folder_id IS NULL"
            } else {
                " AND folder_id IS NOT NULL"
            });
        }
        if let Some(stage) = &f.stage {
            let st = stage.trim();
            if !st.is_empty() && st.to_lowercase() != "all" {
                sql.push_str(" AND stage = ?");
                params.push(Box::new(st.to_string()));
            }
        }
        if let Some(query) = &f.search_query {
            let q = query.trim();
            if !q.is_empty() {
                sql.push_str(" AND (name LIKE ? OR content LIKE ? OR subject LIKE ?)");
                let pattern = format!("%{}%", q);
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern.clone()));
                params.push(Box::new(pattern));
            }
        }
        if let Some(tag) = &f.tag {
            let t = tag.trim();
            if !t.is_empty() && t.to_lowercase() != "all" {
                sql.push_str(" AND tags_json LIKE ?");
                let pattern = format!("%{}%", t);
                params.push(Box::new(pattern));
            }
        }
    }

    sql.push_str(" ORDER BY updated_at DESC, id DESC");

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(param_refs.as_slice(), |row| {
        Ok(crate::models::TemplateRecord {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            name: row.get(2)?,
            channel: row.get(3)?,
            stage: row.get(4)?,
            subject: row.get(5)?,
            content: row.get(6)?,
            media_url: row.get(7)?,
            ai_prompt: row.get(8)?,
            tags_json: row.get(9)?,
            usage_count: row.get(10)?,
            last_used_at: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        })
    })?;

    let mut templates = Vec::new();
    for r in rows {
        templates.push(r?);
    }
    Ok(templates)
}

#[allow(dead_code)]
pub fn get_template_by_id(
    conn: &Connection,
    id: i64,
) -> Result<Option<crate::models::TemplateRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, folder_id, name, channel, stage, subject, content, media_url, ai_prompt, tags_json, usage_count, last_used_at, created_at, updated_at
         FROM templates
         WHERE id = ?1",
    )?;
    let mut rows = stmt.query([id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::TemplateRecord {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            name: row.get(2)?,
            channel: row.get(3)?,
            stage: row.get(4)?,
            subject: row.get(5)?,
            content: row.get(6)?,
            media_url: row.get(7)?,
            ai_prompt: row.get(8)?,
            tags_json: row.get(9)?,
            usage_count: row.get(10)?,
            last_used_at: row.get(11)?,
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        }))
    } else {
        Ok(None)
    }
}

#[allow(dead_code)]
pub fn create_template(
    conn: &Connection,
    input: &crate::models::CreateTemplateInput,
) -> Result<i64> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO templates (
            folder_id, name, channel, stage, subject, content, media_url, ai_prompt, tags_json, usage_count, last_used_at, created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, NULL, ?10, ?11)",
        rusqlite::params![
            input.folder_id,
            input.name,
            input.channel,
            input.stage,
            input.subject,
            input.content,
            input.media_url,
            input.ai_prompt,
            input.tags_json,
            now,
            now
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

#[allow(dead_code)]
pub fn update_template(
    conn: &Connection,
    input: &crate::models::UpdateTemplateInput,
) -> Result<()> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    let changed = conn.execute(
        "UPDATE templates
         SET folder_id = ?1, name = ?2, channel = ?3, stage = ?4, subject = ?5,
             content = ?6, media_url = ?7, ai_prompt = ?8, tags_json = ?9, updated_at = ?10
         WHERE id = ?11",
        rusqlite::params![
            input.folder_id,
            input.name,
            input.channel,
            input.stage,
            input.subject,
            input.content,
            input.media_url,
            input.ai_prompt,
            input.tags_json,
            now,
            input.id
        ],
    )?;
    ensure_row_changed(changed)
}

#[allow(dead_code)]
pub fn delete_template(conn: &Connection, id: i64) -> Result<()> {
    let changed = conn.execute("DELETE FROM templates WHERE id = ?1", [id])?;
    ensure_row_changed(changed)
}

#[allow(dead_code)]
pub fn increment_template_usage(conn: &Connection, id: i64) -> Result<()> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "UPDATE templates
         SET usage_count = usage_count + 1, last_used_at = ?1, updated_at = ?2
         WHERE id = ?3",
        rusqlite::params![now, now, id],
    )?;
    Ok(())
}
