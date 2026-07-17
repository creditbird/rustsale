use rusqlite::{Connection, Result};

pub fn init_db(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    conn.execute("PRAGMA foreign_keys = ON;", [])?;

    conn.execute(
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
            about TEXT
        )",
        [],
    )?;

    conn.execute(
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

    conn.execute(
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS zalo_sessions (
            imei TEXT PRIMARY KEY,
            user_agent TEXT NOT NULL,
            cookies_json TEXT NOT NULL,
            language TEXT NOT NULL
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS outreach_templates (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )?;

    Ok(conn)
}

#[allow(dead_code)]
pub fn seed_data(conn: &mut Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))?;
    if count == 0 {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) VALUES 
             ('Sarah Harrington', 'Acme Corp', 'Qualified', 45000.0, '2026-07-15T10:00:00Z', 'Alex Rivera', 'sarah@acme.com', '+12345678', 'CTO of Acme Corp'),
             ('Jordan Smith', 'CloudSync', 'New', 15000.0, '2026-07-16T15:30:00Z', 'Jordan Smith', 'jordan@cloudsync.com', '+87654321', 'Procurement lead')",
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
