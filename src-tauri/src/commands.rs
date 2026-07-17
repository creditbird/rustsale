use std::sync::Mutex;
use rusqlite::Connection;
use crate::models::{Lead, Deal, Activity};

pub struct DbState {
    pub conn: Mutex<Connection>,
}

#[tauri::command]
pub fn get_leads(state: tauri::State<DbState>) -> Result<Vec<Lead>, String> {
    get_leads_internal(&state.conn)
}

pub fn get_leads_internal(lock: &Mutex<Connection>) -> Result<Vec<Lead>, String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, company, status, value, last_activity, owner, email, phone, about FROM leads")
        .map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map([], |row| {
        Ok(Lead {
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
        })
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_lead_details(id: i32, state: tauri::State<DbState>) -> Result<Lead, String> {
    get_lead_details_internal(&state.conn, id)
}

pub fn get_lead_details_internal(lock: &Mutex<Connection>, id: i32) -> Result<Lead, String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, name, company, status, value, last_activity, owner, email, phone, about FROM leads WHERE id = ?1",
        [id],
        |row| {
            Ok(Lead {
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
            })
        }
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_lead(
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    add_lead_internal(&state.conn, name, company, status, value, owner, email, phone, about)
}

pub fn add_lead_internal(
    lock: &Mutex<Connection>,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
) -> Result<i32, String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    let now: String = conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![name, company, status, value, now, owner, email, phone, about],
    ).map_err(|e| e.to_string())?;
    let last_id = conn.last_insert_rowid() as i32;
    Ok(last_id)
}

#[tauri::command]
pub fn update_lead_status(id: i32, status: String, state: tauri::State<DbState>) -> Result<(), String> {
    update_lead_status_internal(&state.conn, id, status)
}

pub fn update_lead_status_internal(lock: &Mutex<Connection>, id: i32, status: String) -> Result<(), String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE leads SET status = ?1 WHERE id = ?2",
        rusqlite::params![status, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_deals(state: tauri::State<DbState>) -> Result<Vec<Deal>, String> {
    get_deals_internal(&state.conn)
}

pub fn get_deals_internal(lock: &Mutex<Connection>) -> Result<Vec<Deal>, String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, lead_id, title, value, stage, created_at FROM deals")
        .map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map([], |row| {
        Ok(Deal {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            title: row.get(2)?,
            value: row.get(3)?,
            stage: row.get(4)?,
            created_at: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn update_deal_stage(id: i32, stage: String, state: tauri::State<DbState>) -> Result<(), String> {
    update_deal_stage_internal(&state.conn, id, stage)
}

pub fn update_deal_stage_internal(lock: &Mutex<Connection>, id: i32, stage: String) -> Result<(), String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE deals SET stage = ?1 WHERE id = ?2",
        rusqlite::params![stage, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_activities(lead_id: i32, state: tauri::State<DbState>) -> Result<Vec<Activity>, String> {
    get_activities_internal(&state.conn, lead_id)
}

pub fn get_activities_internal(lock: &Mutex<Connection>, lead_id: i32) -> Result<Vec<Activity>, String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, lead_id, activity_type, content, created_at, performed_by FROM activities WHERE lead_id = ?1")
        .map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map([lead_id], |row| {
        Ok(Activity {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            activity_type: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            performed_by: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn add_activity(
    lead_id: i32,
    activity_type: String,
    content: String,
    performed_by: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    add_activity_internal(&state.conn, lead_id, activity_type, content, performed_by)
}

pub fn add_activity_internal(
    lock: &Mutex<Connection>,
    lead_id: i32,
    activity_type: String,
    content: String,
    performed_by: String,
) -> Result<(), String> {
    let mut conn = lock.lock().map_err(|e| e.to_string())?;
    let now: String = conn.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![lead_id, activity_type, content, &now, performed_by],
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
        rusqlite::params![&now, lead_id],
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
