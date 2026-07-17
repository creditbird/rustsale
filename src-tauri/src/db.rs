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
