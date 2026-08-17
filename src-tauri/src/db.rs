use rusqlite::{Connection, Result};
use std::time::Duration;

fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    col_name: &str,
    col_def: &str,
) -> Result<()> {
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

pub fn open_db_connection(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

pub fn init_db(db_path: &str) -> Result<Connection> {
    let mut conn = open_db_connection(db_path)?;

    {
        let tx = conn.transaction()?;

        // ARCHITECTURAL INVARIANT:
        // All numeric monetary amounts in the database (`leads.value`, `deals.value`) are canonical
        // Vietnamese Đồng (VND). Raw values are persisted directly as `f64` / `REAL` without
        // ad-hoc FX conversion or multi-currency mutations.
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
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS zalo_sessions (
                imei TEXT PRIMARY KEY,
                user_agent TEXT NOT NULL,
                cookies_json TEXT NOT NULL,
                language TEXT NOT NULL,
                display_name TEXT,
                avatar TEXT
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
                status TEXT NOT NULL,
                error_message TEXT,
                sent_at TEXT NOT NULL,
                session_imei TEXT NOT NULL,
                FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE,
                FOREIGN KEY(template_id) REFERENCES outreach_templates(id) ON DELETE CASCADE
            )",
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
                    CHECK (coverage_truncated IN (0, 1))
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

        tx.execute(
            "CREATE TABLE IF NOT EXISTS app_metadata (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        // Monetary metadata versioning:
        // Canonical monetary storage is VND (VND_v1).
        // Ensure currency_semantics metadata is recorded as VND_v1 without mutating existing rows.
        tx.execute(
            "INSERT OR IGNORE INTO app_metadata (key, value) VALUES ('currency_semantics', 'VND_v1')",
            [],
        )?;

        tx.commit()?;
    }

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
        "SELECT imei, user_agent, cookies_json, language, display_name, avatar, zalo_uid, channel_account_id FROM zalo_sessions",
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

pub fn update_outreach_template(
    conn: &Connection,
    id: i32,
    name: &str,
    content: &str,
    image_url: Option<&str>,
) -> Result<()> {
    conn.execute(
        "UPDATE outreach_templates SET name = ?1, content = ?2, image_url = ?3 WHERE id = ?4",
        rusqlite::params![name, content, image_url, id],
    )?;
    Ok(())
}

pub fn delete_outreach_template(conn: &Connection, id: i32) -> Result<()> {
    conn.execute("DELETE FROM outreach_templates WHERE id = ?1", [id])?;
    Ok(())
}

pub fn create_category(conn: &Connection, name: &str) -> Result<i32> {
    conn.execute("INSERT INTO categories (name) VALUES (?1)", [name])?;
    Ok(conn.last_insert_rowid() as i32)
}

pub fn delete_category(conn: &Connection, id: i32) -> Result<()> {
    conn.execute("DELETE FROM categories WHERE id = ?1", [id])?;
    Ok(())
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
        "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt
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
            conn.execute(
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
    conn.execute("DELETE FROM smtp_servers WHERE id = ?1", [id])?;
    Ok(())
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
    conn.execute(
        "UPDATE email_templates SET name = ?1, subject = ?2, content = ?3 WHERE id = ?4",
        rusqlite::params![name, subject, content, id],
    )?;
    Ok(())
}

pub fn delete_email_template(conn: &Connection, id: i32) -> Result<()> {
    conn.execute("DELETE FROM email_templates WHERE id = ?1", [id])?;
    Ok(())
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
