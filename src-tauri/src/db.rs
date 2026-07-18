use rusqlite::{Connection, Result};

fn add_column_if_missing(conn: &Connection, table: &str, col_name: &str, col_def: &str) -> Result<()> {
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

pub fn init_db(db_path: &str) -> Result<Connection> {
    let mut conn = Connection::open(db_path)?;
    conn.pragma_update(None, "journal_mode", &"WAL")?;
    conn.pragma_update(None, "synchronous", &"NORMAL")?;

    {
        let tx = conn.transaction()?;
        tx.execute("PRAGMA foreign_keys = ON;", [])?;

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
        add_column_if_missing(&tx, "leads", "zalo_reached", "zalo_reached INTEGER NOT NULL DEFAULT 0")?;
        add_column_if_missing(&tx, "leads", "zalo_uid", "zalo_uid TEXT")?;
        add_column_if_missing(&tx, "leads", "zalo_status", "zalo_status TEXT DEFAULT 'UNTESTED'")?;
        add_column_if_missing(&tx, "leads", "relationship_status", "relationship_status TEXT DEFAULT 'UNKNOWN'")?;
        add_column_if_missing(&tx, "leads", "last_lookup_attempt", "last_lookup_attempt TEXT")?;

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
                language TEXT NOT NULL
            )",
            [],
        )?;

        tx.execute(
            "CREATE TABLE IF NOT EXISTS outreach_templates (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL
            )",
            [],
        )?;

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
             ('Sarah Harrington', 'Acme Corp', 'Qualified', 45000.0, '2026-07-15T10:00:00Z', 'Alex Rivera', 'sarah@acme.com', '+12345678', 'CTO of Acme Corp', '123 Main St, New York', 1, 0),
             ('Jordan Smith', 'CloudSync', 'New', 15000.0, '2026-07-16T15:30:00Z', 'Jordan Smith', 'jordan@cloudsync.com', '+87654321', 'Procurement lead', '456 Oak Ave, Boston', NULL, 0),
             ('lele', 'Zalo Corp', 'Qualified', 20000.0, '2026-07-17T12:00:00Z', 'Alex Rivera', 'lele@example.com', '+84987654321', 'Zalo developer', '789 Pine Rd, Seattle', 1, 1)",
            [],
        )?;
        tx.execute(
            "INSERT INTO deals (lead_id, title, value, stage, created_at) VALUES 
             (1, 'Acme Platform Expansion', 45000.0, 'Proposal', '2026-07-15T10:00:00Z'),
             (2, 'CloudSync Core Migration', 15000.0, 'Prospecting', '2026-07-16T15:30:00Z')",
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
        "INSERT OR REPLACE INTO zalo_sessions (imei, user_agent, cookies_json, language) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![imei, user_agent, cookies_json, language],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn get_zalo_session(
    conn: &Connection,
    imei: &str,
) -> Result<Option<crate::models::ZaloSession>> {
    let mut stmt = conn.prepare(
        "SELECT imei, user_agent, cookies_json, language FROM zalo_sessions WHERE imei = ?1",
    )?;
    let mut rows = stmt.query([imei])?;
    if let Some(row) = rows.next()? {
        Ok(Some(crate::models::ZaloSession {
            imei: row.get(0)?,
            user_agent: row.get(1)?,
            cookies_json: row.get(2)?,
            language: row.get(3)?,
        }))
    } else {
        Ok(None)
    }
}

pub fn get_all_zalo_sessions(conn: &Connection) -> Result<Vec<crate::models::ZaloSession>> {
    let mut stmt =
        conn.prepare("SELECT imei, user_agent, cookies_json, language FROM zalo_sessions")?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::ZaloSession {
            imei: row.get(0)?,
            user_agent: row.get(1)?,
            cookies_json: row.get(2)?,
            language: row.get(3)?,
        })
    })?;
    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row?);
    }
    Ok(sessions)
}

pub fn delete_zalo_session(conn: &Connection, imei: &str) -> Result<()> {
    conn.execute("DELETE FROM zalo_sessions WHERE imei = ?1", [imei])?;
    Ok(())
}

pub fn get_outreach_templates(conn: &Connection) -> Result<Vec<crate::models::OutreachTemplate>> {
    let mut stmt = conn.prepare("SELECT id, name, content, created_at FROM outreach_templates")?;
    let rows = stmt.query_map([], |row| {
        Ok(crate::models::OutreachTemplate {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            content: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;
    let mut templates = Vec::new();
    for row in rows {
        templates.push(row?);
    }
    Ok(templates)
}

pub fn create_outreach_template(conn: &Connection, name: &str, content: &str) -> Result<i32> {
    let now: String =
        conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })?;
    conn.execute(
        "INSERT INTO outreach_templates (name, content, created_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![name, content, now],
    )?;
    let last_id = conn.last_insert_rowid() as i32;
    Ok(last_id)
}

pub fn update_outreach_template(
    conn: &Connection,
    id: i32,
    name: &str,
    content: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE outreach_templates SET name = ?1, content = ?2 WHERE id = ?3",
        rusqlite::params![name, content, id],
    )?;
    Ok(())
}

pub fn delete_outreach_template(conn: &Connection, id: i32) -> Result<()> {
    conn.execute("DELETE FROM outreach_templates WHERE id = ?1", [id])?;
    Ok(())
}

pub fn create_category(conn: &Connection, name: &str) -> Result<i32> {
    conn.execute(
        "INSERT INTO categories (name) VALUES (?1)",
        [name],
    )?;
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

pub fn log_outreach_attempt(conn: &Connection, history: &crate::models::OutreachHistory) -> Result<()> {
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

pub fn get_leads_by_category(conn: &Connection, category_id: i32) -> Result<Vec<crate::models::Lead>> {
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
