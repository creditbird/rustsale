use crate::conversation::commands::ConversationState;
use crate::models::{
    Activity, Category, Deal, EmailTemplate, Lead, ManagedSmtpAccount, ManagedZaloAccount,
    SmtpServer, ZaloSessionStatus,
};
use rusqlite::{params_from_iter, types::Value, Connection, OptionalExtension, Row};
use std::collections::HashSet;
use std::sync::Mutex;

pub struct DbState {
    pub conn: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct OutreachLeadQuery {
    pub category_id: Option<i32>,
    pub segmentation: String,
    pub filter_template_id: Option<i32>,
    pub channel: String,
    pub search: Option<String>,
    pub filter_verified_zalo: bool,
    pub filter_unreached: bool,
    pub filter_mobile_only: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct OutreachLeadsPage {
    pub items: Vec<Lead>,
    pub total: i64,
    pub next_cursor: Option<i32>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CurrencySettings {
    pub base_currency: String,
    pub exchange_rate: f64,
}

pub fn validate_currency(currency: Option<&str>) -> Result<String, String> {
    match currency {
        None => Ok("VND".to_string()),
        Some(c) => {
            let trimmed = c.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("VND") {
                Ok("VND".to_string())
            } else if trimmed.eq_ignore_ascii_case("USD") {
                Ok("USD".to_string())
            } else {
                Err(format!(
                    "Invalid currency '{}': RustSale CRM currently only supports 'VND' and 'USD'",
                    trimmed
                ))
            }
        }
    }
}

fn outreach_lead_from_row(row: &Row<'_>) -> rusqlite::Result<Lead> {
    Ok(Lead {
        id: row.get(0)?,
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
}

fn build_outreach_lead_filter(query: &OutreachLeadQuery) -> Result<(String, Vec<Value>), String> {
    let mut sql = String::from(" FROM leads l WHERE 1 = 1");
    let mut params = Vec::new();

    if let Some(category_id) = query.category_id {
        sql.push_str(
            " AND EXISTS (SELECT 1 FROM lead_categories lc WHERE lc.lead_id = l.id AND lc.category_id = ?)",
        );
        params.push(Value::Integer(i64::from(category_id)));
    }

    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|search| !search.is_empty())
    {
        let pattern = format!("%{}%", search.to_lowercase());
        sql.push_str(
            " AND (LOWER(l.name) LIKE ? OR LOWER(l.company) LIKE ? OR LOWER(l.email) LIKE ? OR LOWER(l.phone) LIKE ?)",
        );
        for _ in 0..4 {
            params.push(Value::Text(pattern.clone()));
        }
    }

    if query.filter_verified_zalo {
        sql.push_str(" AND l.has_zalo = 1");
    } else if query.channel != "email" {
        sql.push_str(" AND COALESCE(l.has_zalo, 1) != 0");
    }
    if query.filter_unreached {
        sql.push_str(" AND COALESCE(l.zalo_reached, 0) = 0");
    }
    if query.filter_mobile_only {
        sql.push_str(
            " AND (l.phone LIKE '09%' OR l.phone LIKE '03%' OR l.phone LIKE '07%' OR l.phone LIKE '08%' OR l.phone LIKE '05%')",
        );
    }

    if query.category_id.is_some() {
        let history_table = if query.channel == "email" {
            "email_outreach_history"
        } else {
            "outreach_history"
        };

        match query.segmentation.as_str() {
            "never_sent" => {
                if query.channel != "email" {
                    sql.push_str(" AND COALESCE(l.zalo_reached, 0) = 0");
                    sql.push_str(" AND COALESCE(l.zalo_status, '') NOT IN ('BLOCKED_STRANGER', 'VERIFIED_NOT_EXIST', 'NOT_FOUND', 'INVALID_PHONE')");
                }
                sql.push_str(&format!(
                    " AND NOT EXISTS (SELECT 1 FROM {history_table} history WHERE history.lead_id = l.id AND history.status = 'SUCCESS')"
                ));
            }
            "last_failed" => {
                sql.push_str(&format!(
                    " AND (SELECT history.status FROM {history_table} history WHERE history.lead_id = l.id ORDER BY history.sent_at DESC, history.id DESC LIMIT 1) = 'FAILED'"
                ));
            }
            "template_success" => {
                let template_id = query
                    .filter_template_id
                    .ok_or_else(|| "Template ID is required for success filtering".to_string())?;
                sql.push_str(&format!(
                    " AND EXISTS (SELECT 1 FROM {history_table} history WHERE history.lead_id = l.id AND history.template_id = ? AND history.status = 'SUCCESS')"
                ));
                params.push(Value::Integer(i64::from(template_id)));
            }
            _ => {}
        }
    }

    Ok((sql, params))
}

pub fn get_outreach_leads_page_internal(
    conn: &Connection,
    query: &OutreachLeadQuery,
    cursor: Option<i32>,
    limit: u32,
) -> Result<OutreachLeadsPage, String> {
    let (filter_sql, filter_params) = build_outreach_lead_filter(query)?;
    let count_sql = format!("SELECT COUNT(*){filter_sql}");
    let total = conn
        .query_row(&count_sql, params_from_iter(filter_params.iter()), |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| error.to_string())?;

    let bounded_limit = limit.clamp(1, 200);
    let mut page_sql = format!(
        "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency{filter_sql}"
    );
    let mut page_params = filter_params;
    if let Some(cursor) = cursor {
        page_sql.push_str(" AND l.id < ?");
        page_params.push(Value::Integer(i64::from(cursor)));
    }
    page_sql.push_str(" ORDER BY l.id DESC LIMIT ?");
    page_params.push(Value::Integer(i64::from(bounded_limit + 1)));

    let mut stmt = conn.prepare(&page_sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(page_params.iter()), outreach_lead_from_row)
        .map_err(|error| error.to_string())?;
    let mut items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;

    let has_more = items.len() > bounded_limit as usize;
    if has_more {
        items.pop();
    }
    let next_cursor = if has_more {
        items.last().and_then(|lead| lead.id)
    } else {
        None
    };

    Ok(OutreachLeadsPage {
        items,
        total,
        next_cursor,
    })
}

pub fn resolve_outreach_lead_ids_internal(
    conn: &Connection,
    query: &OutreachLeadQuery,
    excluded_lead_ids: &[i32],
) -> Result<Vec<i32>, String> {
    let (filter_sql, filter_params) = build_outreach_lead_filter(query)?;
    let sql = format!("SELECT l.id{filter_sql} ORDER BY l.id DESC");
    let excluded = excluded_lead_ids.iter().copied().collect::<HashSet<_>>();
    let mut stmt = conn.prepare(&sql).map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params_from_iter(filter_params.iter()), |row| {
            row.get::<_, i32>(0)
        })
        .map_err(|error| error.to_string())?;

    rows.filter_map(|row| match row {
        Ok(lead_id) if !excluded.contains(&lead_id) => Some(Ok(lead_id)),
        Ok(_) => None,
        Err(error) => Some(Err(error.to_string())),
    })
    .collect()
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn get_outreach_leads_page(
    state: tauri::State<DbState>,
    category_id: Option<i32>,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: String,
    search: Option<String>,
    filter_verified_zalo: bool,
    filter_unreached: bool,
    filter_mobile_only: bool,
    cursor: Option<i32>,
    limit: u32,
) -> Result<OutreachLeadsPage, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|error| format!("Database lock error: {error}"))?;
    let query = OutreachLeadQuery {
        category_id,
        segmentation,
        filter_template_id,
        channel,
        search,
        filter_verified_zalo,
        filter_unreached,
        filter_mobile_only,
    };
    get_outreach_leads_page_internal(&conn, &query, cursor, limit)
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn resolve_outreach_lead_ids(
    state: tauri::State<DbState>,
    category_id: Option<i32>,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: String,
    search: Option<String>,
    filter_verified_zalo: bool,
    filter_unreached: bool,
    filter_mobile_only: bool,
    excluded_lead_ids: Vec<i32>,
) -> Result<Vec<i32>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|error| format!("Database lock error: {error}"))?;
    let query = OutreachLeadQuery {
        category_id,
        segmentation,
        filter_template_id,
        channel,
        search,
        filter_verified_zalo,
        filter_unreached,
        filter_mobile_only,
    };
    resolve_outreach_lead_ids_internal(&conn, &query, &excluded_lead_ids)
}

#[tauri::command]
pub fn get_leads(state: tauri::State<DbState>) -> Result<Vec<Lead>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_leads_internal(&conn)
}

pub fn get_leads_internal(conn: &Connection) -> Result<Vec<Lead>, String> {
    let mut stmt = conn
        .prepare("SELECT id, name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached, zalo_uid, zalo_status, relationship_status, last_lookup_attempt, currency FROM leads")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
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
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_lead_details(id: i32, state: tauri::State<DbState>) -> Result<Lead, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_lead_details_internal(&conn, id)
}

pub fn get_lead_details_internal(conn: &Connection, id: i32) -> Result<Lead, String> {
    conn.query_row(
        "SELECT id, name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached, zalo_uid, zalo_status, relationship_status, last_lookup_attempt, currency FROM leads WHERE id = ?1",
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
                address: row.get(10)?,
                has_zalo: row.get(11)?,
                zalo_reached: row.get(12)?,
                zalo_uid: row.get(13)?,
                zalo_status: row.get(14)?,
                relationship_status: row.get(15)?,
                last_lookup_attempt: row.get(16)?,
                currency: row.get::<_, Option<String>>(17).ok().flatten().or(Some("VND".to_string())),
            })
        }
    ).map_err(|e| format!("Database query error: {}", e))
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "preserve the existing Tauri add_lead IPC argument contract with optional currency"
)]
pub fn add_lead(
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
    currency: Option<String>,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    add_lead_with_currency_internal(
        &mut conn, name, company, status, value, owner, email, phone, about, address, currency,
    )
}

#[allow(dead_code)]
#[expect(
    clippy::too_many_arguments,
    reason = "preserve the established CRM helper contract and its audited callers"
)]
pub fn add_lead_internal(
    conn: &mut Connection,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
) -> Result<i32, String> {
    add_lead_with_currency_internal(
        conn, name, company, status, value, owner, email, phone, about, address, None,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "preserve the established CRM helper contract and its audited callers"
)]
pub fn add_lead_with_currency_internal(
    conn: &mut Connection,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
    currency: Option<String>,
) -> Result<i32, String> {
    let valid_curr = validate_currency(currency.as_deref())?;
    let now: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Database query error: {}", e))?;
    conn.execute(
        "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about, address, has_zalo, zalo_reached, currency) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, 0, ?11)",
        rusqlite::params![name, company, status, value, now, owner, email, phone, about, address, valid_curr],
    ).map_err(|e| format!("Database query error: {}", e))?;
    let last_id = conn.last_insert_rowid() as i32;
    Ok(last_id)
}

#[tauri::command]
pub fn update_lead_status(
    id: i32,
    status: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    update_lead_status_internal(&conn, id, status)
}

pub fn update_lead_status_internal(
    conn: &Connection,
    id: i32,
    status: String,
) -> Result<(), String> {
    conn.execute(
        "UPDATE leads SET status = ?1 WHERE id = ?2",
        rusqlite::params![status, id],
    )
    .map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_deals(state: tauri::State<DbState>) -> Result<Vec<Deal>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_deals_internal(&conn)
}

pub fn get_deals_internal(conn: &Connection) -> Result<Vec<Deal>, String> {
    let mut stmt = conn
        .prepare("SELECT id, lead_id, title, value, stage, created_at, currency FROM deals")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Deal {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                title: row.get(2)?,
                value: row.get(3)?,
                stage: row.get(4)?,
                created_at: row.get(5)?,
                currency: row
                    .get::<_, Option<String>>(6)
                    .ok()
                    .flatten()
                    .or(Some("VND".to_string())),
            })
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
    }
    Ok(result)
}

#[tauri::command]
pub fn update_deal_stage(
    id: i32,
    stage: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    update_deal_stage_internal(&conn, id, stage)
}

pub fn update_deal_stage_internal(conn: &Connection, id: i32, stage: String) -> Result<(), String> {
    conn.execute(
        "UPDATE deals SET stage = ?1 WHERE id = ?2",
        rusqlite::params![stage, id],
    )
    .map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn create_deal(
    lead_id: i32,
    title: String,
    value: f64,
    stage: String,
    currency: Option<String>,
    state: tauri::State<DbState>,
) -> Result<Deal, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    create_deal_internal(&conn, lead_id, title, value, stage, currency)
}

pub fn create_deal_internal(
    conn: &Connection,
    lead_id: i32,
    title: String,
    value: f64,
    stage: String,
    currency: Option<String>,
) -> Result<Deal, String> {
    let valid_curr = validate_currency(currency.as_deref())?;
    let created_at = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO deals (lead_id, title, value, stage, created_at, currency) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![lead_id, title, value, stage, created_at, valid_curr],
    )
    .map_err(|e| format!("Database query error: {}", e))?;

    let id = conn.last_insert_rowid() as i32;
    Ok(Deal {
        id: Some(id),
        lead_id,
        title,
        value,
        stage,
        created_at,
        currency: Some(valid_curr),
    })
}

#[tauri::command]
pub fn get_currency_settings(state: tauri::State<DbState>) -> Result<CurrencySettings, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_currency_settings_internal(&conn)
}

pub fn get_currency_settings_internal(conn: &Connection) -> Result<CurrencySettings, String> {
    let base_currency: String = conn
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'base_currency'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "VND".to_string());

    let rate_str: String = conn
        .query_row(
            "SELECT value FROM app_metadata WHERE key = 'usd_vnd_exchange_rate'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| "25400".to_string());

    let exchange_rate: f64 = rate_str.parse().unwrap_or(25400.0);

    Ok(CurrencySettings {
        base_currency,
        exchange_rate,
    })
}

#[tauri::command]
pub fn update_currency_settings(
    base_currency: String,
    exchange_rate: f64,
    state: tauri::State<DbState>,
) -> Result<CurrencySettings, String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    update_currency_settings_internal(&mut conn, base_currency, exchange_rate)
}

pub fn update_currency_settings_internal(
    conn: &mut Connection,
    base_currency: String,
    exchange_rate: f64,
) -> Result<CurrencySettings, String> {
    let valid_curr = validate_currency(Some(&base_currency))?;
    if exchange_rate <= 0.0 || exchange_rate.is_nan() || exchange_rate.is_infinite() {
        return Err("Exchange rate must be a positive number".to_string());
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("Database transaction error: {}", e))?;

    tx.execute(
        "INSERT INTO app_metadata (key, value) VALUES ('base_currency', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        rusqlite::params![valid_curr],
    )
    .map_err(|e| format!("Database query error: {}", e))?;

    tx.execute(
        "INSERT INTO app_metadata (key, value) VALUES ('usd_vnd_exchange_rate', ?1)
         ON CONFLICT(key) DO UPDATE SET value = ?1",
        rusqlite::params![exchange_rate.to_string()],
    )
    .map_err(|e| format!("Database query error: {}", e))?;

    tx.commit()
        .map_err(|e| format!("Database commit error: {}", e))?;

    Ok(CurrencySettings {
        base_currency: valid_curr,
        exchange_rate,
    })
}

#[tauri::command]
pub fn delete_deal(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    delete_deal_internal(&conn, id)
}

pub fn delete_deal_internal(conn: &Connection, id: i32) -> Result<(), String> {
    conn.execute("DELETE FROM deals WHERE id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn get_activities(lead_id: i32, state: tauri::State<DbState>) -> Result<Vec<Activity>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_activities_internal(&conn, lead_id)
}

pub fn get_activities_internal(conn: &Connection, lead_id: i32) -> Result<Vec<Activity>, String> {
    let mut stmt = conn
        .prepare("SELECT id, lead_id, activity_type, content, created_at, performed_by FROM activities WHERE lead_id = ?1")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([lead_id], |row| {
            Ok(Activity {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                activity_type: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                performed_by: row.get(5)?,
            })
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_recent_activities(
    limit: Option<i32>,
    state: tauri::State<DbState>,
) -> Result<Vec<Activity>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    let limit_val = limit.unwrap_or(10);
    let mut stmt = conn
        .prepare("SELECT id, lead_id, activity_type, content, created_at, performed_by FROM activities ORDER BY id DESC LIMIT ?1")
        .map_err(|e| format!("Database query error: {}", e))?;

    let rows = stmt
        .query_map([limit_val], |row| {
            Ok(Activity {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                activity_type: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                performed_by: row.get(5)?,
            })
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| format!("Database query error: {}", e))?);
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
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    add_activity_internal(&mut conn, lead_id, activity_type, content, performed_by)
}

pub fn add_activity_internal(
    conn: &mut Connection,
    lead_id: i32,
    activity_type: String,
    content: String,
    performed_by: String,
) -> Result<(), String> {
    let now: String = conn
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Database transaction error: {}", e))?;
    tx.execute(
        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![lead_id, activity_type, content, &now, performed_by],
    ).map_err(|e| format!("Database query error: {}", e))?;

    tx.execute(
        "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
        rusqlite::params![&now, lead_id],
    )
    .map_err(|e| format!("Database query error: {}", e))?;

    tx.commit()
        .map_err(|e| format!("Database transaction commit error: {}", e))?;
    Ok(())
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "preserve the existing Tauri update_lead IPC argument contract with optional currency"
)]
pub fn update_lead(
    id: i32,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
    currency: Option<String>,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    update_lead_with_currency_internal(
        &conn, id, name, company, status, value, owner, email, phone, about, address, currency,
    )
}

#[allow(dead_code)]
#[expect(
    clippy::too_many_arguments,
    reason = "preserve the established CRM helper contract and its audited callers"
)]
pub fn update_lead_internal(
    conn: &Connection,
    id: i32,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
) -> Result<(), String> {
    let existing_currency: Option<String> = conn
        .query_row("SELECT currency FROM leads WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .ok();
    update_lead_with_currency_internal(
        conn,
        id,
        name,
        company,
        status,
        value,
        owner,
        email,
        phone,
        about,
        address,
        existing_currency,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "preserve the established CRM helper contract and its audited callers"
)]
pub fn update_lead_with_currency_internal(
    conn: &Connection,
    id: i32,
    name: String,
    company: String,
    status: String,
    value: f64,
    owner: String,
    email: String,
    phone: String,
    about: Option<String>,
    address: Option<String>,
    currency: Option<String>,
) -> Result<(), String> {
    let valid_curr = if let Some(ref c) = currency {
        validate_currency(Some(c.as_str()))?
    } else {
        conn.query_row("SELECT currency FROM leads WHERE id = ?1", [id], |row| {
            row.get::<_, Option<String>>(0)
        })
        .ok()
        .flatten()
        .unwrap_or_else(|| "VND".to_string())
    };
    conn.execute(
        "UPDATE leads SET name = ?1, company = ?2, status = ?3, value = ?4, owner = ?5, email = ?6, phone = ?7, about = ?8, address = ?9, currency = ?10 WHERE id = ?11",
        rusqlite::params![name, company, status, value, owner, email, phone, about, address, valid_curr, id],
    ).map_err(|e| format!("Database query error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub fn delete_lead(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    let tx = conn
        .transaction()
        .map_err(|e| format!("Database transaction error: {}", e))?;

    tx.execute("DELETE FROM activities WHERE lead_id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;

    tx.execute("DELETE FROM deals WHERE lead_id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;

    tx.execute("DELETE FROM leads WHERE id = ?1", [id])
        .map_err(|e| format!("Database query error: {}", e))?;

    tx.commit()
        .map_err(|e| format!("Database transaction commit error: {}", e))?;
    Ok(())
}

use crate::google_maps_jobs::commands::{GoogleMapsJobService, GoogleMapsJobState};
use crate::google_maps_jobs::types::{
    CreateScrapeJobRequest, ScrapeJob, ScrapeJobStatus, ScrapeMode,
};
use tauri::{Emitter, Manager};

#[derive(Debug, Clone)]
pub struct LegacyGoogleMapsScrapeRequest {
    pub keyword: String,
    pub mode: String,
    pub location: Option<String>,
    pub bbox: Option<[f64; 4]>,
    pub step_km: Option<f64>,
    pub districts: Option<Vec<String>>,
    pub headless: Option<bool>,
}

impl LegacyGoogleMapsScrapeRequest {
    fn into_durable_request(self) -> Result<CreateScrapeJobRequest, String> {
        let mode = match self.mode.trim().to_ascii_lowercase().as_str() {
            "text" | "keyword" => ScrapeMode::Keyword,
            "grid" => ScrapeMode::Grid,
            other => return Err(format!("unsupported Google Maps scrape mode: {other}")),
        };
        Ok(CreateScrapeJobRequest {
            mode,
            keyword: self.keyword,
            location: self.location,
            bbox: self.bbox,
            step_km: self.step_km,
            districts: self.districts.unwrap_or_default(),
            headless: self.headless.unwrap_or(true),
        })
    }
}

pub async fn legacy_start_google_maps_scrape(
    service: &GoogleMapsJobService,
    request: LegacyGoogleMapsScrapeRequest,
) -> Result<ScrapeJob, String> {
    let created = service
        .create_job(request.into_durable_request()?)
        .await
        .map_err(|error| error.to_string())?;
    service
        .resume_job(&created.id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn legacy_stop_google_maps_scrape(
    service: &GoogleMapsJobService,
) -> Result<Option<ScrapeJob>, String> {
    let active = service
        .list_jobs()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|job| {
            matches!(
                job.status,
                ScrapeJobStatus::Queued
                    | ScrapeJobStatus::Running
                    | ScrapeJobStatus::Paused
                    | ScrapeJobStatus::PausedRequiresAction
                    | ScrapeJobStatus::PausedLimitReached
                    | ScrapeJobStatus::Interrupted
            )
        });
    match active {
        Some(job) => service
            .cancel_job(&job.id)
            .await
            .map(Some)
            .map_err(|error| error.to_string()),
        None => Ok(None),
    }
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "preserve the legacy Google Maps scraper IPC compatibility contract"
)]
pub async fn start_google_maps_scrape(
    state: tauri::State<'_, GoogleMapsJobState>,
    keyword: String,
    mode: String,
    location: Option<String>,
    bbox: Option<[f64; 4]>,
    step_km: Option<f64>,
    districts: Option<Vec<String>>,
    headless: Option<bool>,
) -> Result<(), String> {
    legacy_start_google_maps_scrape(
        state.service.as_ref(),
        LegacyGoogleMapsScrapeRequest {
            keyword,
            mode,
            location,
            bbox,
            step_km,
            districts,
            headless,
        },
    )
    .await
    .map(|_| ())
}

#[tauri::command]
pub async fn stop_google_maps_scrape(
    state: tauri::State<'_, GoogleMapsJobState>,
) -> Result<(), String> {
    legacy_stop_google_maps_scrape(state.service.as_ref())
        .await
        .map(|_| ())
}

#[tauri::command]
pub fn get_preset_provinces() -> Result<serde_json::Value, String> {
    let json_data = serde_json::json!({
        "Thành phố Cần Thơ": { "minLat": 9.9, "maxLat": 10.3, "minLng": 105.3, "maxLng": 105.9 },
        "Thành phố Đà Nẵng": { "minLat": 15.9, "maxLat": 16.3, "minLng": 107.9, "maxLng": 108.3 },
        "Thành phố Hà Nội": { "minLat": 20.58, "maxLat": 21.36, "minLng": 105.28, "maxLng": 106.03 },
        "Thành phố Hải Phòng": { "minLat": 20.68, "maxLat": 21.03, "minLng": 106.52, "maxLng": 106.9 },
        "Thành phố Hồ Chí Minh": { "minLat": 10.37, "maxLat": 11.16, "minLng": 106.36, "maxLng": 107.03 },
        "Thành phố Huế": { "minLat": 15.9949, "maxLat": 17.1109, "minLng": 107.0343, "maxLng": 108.4806 },
        "Tỉnh An Giang": { "minLat": 8.9946, "maxLat": 10.9623, "minLng": 103.0522, "maxLng": 105.5756 },
        "Tỉnh Bà Rịa - Vũng Tàu": { "minLat": 10.3, "maxLat": 10.8, "minLng": 107, "maxLng": 107.6 },
        "Tỉnh Bạc Liêu": { "minLat": 8.2912, "maxLat": 9.6375, "minLng": 105.2324, "maxLng": 106.0553 },
        "Tỉnh Bắc Giang": { "minLat": 21.122, "maxLat": 21.6263, "minLng": 105.8806, "maxLng": 107.0336 },
        "Tỉnh Bắc Kạn": { "minLat": 21.8043, "maxLat": 22.7414, "minLng": 105.4311, "maxLng": 106.247 },
        "Tỉnh Bắc Ninh": { "minLat": 20.9693, "maxLat": 21.6263, "minLng": 105.8806, "maxLng": 107.0336 },
        "Tỉnh Bến Tre": { "minLat": 9.4273, "maxLat": 10.3403, "minLng": 106.0275, "maxLng": 106.9934 },
        "Tỉnh Bình Dương": { "minLat": 10.85, "maxLat": 11.52, "minLng": 106.38, "maxLng": 106.91 },
        "Tỉnh Bình Định": { "minLat": 13.5152, "maxLat": 14.7032, "minLng": 108.6011, "maxLng": 109.5849 },
        "Tỉnh Bình Phước": { "minLat": 11.3016, "maxLat": 12.2985, "minLng": 106.4116, "maxLng": 107.4271 },
        "Tỉnh Bình Thuận": { "minLat": 9.378, "maxLat": 11.5561, "minLng": 107.3942, "maxLng": 109.4019 },
        "Tỉnh Cà Mau": { "minLat": 8.1791, "maxLat": 9.6375, "minLng": 103.4821, "maxLng": 106.0553 },
        "Tỉnh Cao Bằng": { "minLat": 22.3569, "maxLat": 23.1188, "minLng": 105.2667, "maxLng": 106.8376 },
        "Tỉnh Đắk Lắk": { "minLat": 12.1605, "maxLat": 13.6953, "minLng": 107.4842, "maxLng": 109.6665 },
        "Tỉnh Đắk Nông": { "minLat": 11.7489, "maxLat": 12.8125, "minLng": 107.2061, "maxLng": 108.1164 },
        "Tỉnh Điện Biên": { "minLat": 20.8927, "maxLat": 22.5479, "minLng": 102.1439, "maxLng": 103.5988 },
        "Tỉnh Đồng Nai": { "minLat": 10.73, "maxLat": 11.45, "minLng": 106.75, "maxLng": 107.58 },
        "Tỉnh Đồng Tháp": { "minLat": 10.1348, "maxLat": 10.9735, "minLng": 105.1856, "maxLng": 106.9934 },
        "Tỉnh Gia Lai": { "minLat": 12.9962, "maxLat": 14.7032, "minLng": 107.4508, "maxLng": 109.5849 },
        "Tỉnh Hà Giang": { "minLat": 22.1656, "maxLat": 23.3927, "minLng": 104.3343, "maxLng": 105.5741 },
        "Tỉnh Hà Nam": { "minLat": 20.3629, "maxLat": 20.7044, "minLng": 105.769, "maxLng": 106.1832 },
        "Tỉnh Hà Tĩnh": { "minLat": 17.9096, "maxLat": 18.767, "minLng": 105.1035, "maxLng": 106.7537 },
        "Tỉnh Hải Dương": { "minLat": 20.6836, "maxLat": 21.237, "minLng": 106.1242, "maxLng": 106.611 },
        "Tỉnh Hậu Giang": { "minLat": 9.5822, "maxLat": 9.9942, "minLng": 105.3285, "maxLng": 105.8968 },
        "Tỉnh Hoà Bình": { "minLat": 20.3054, "maxLat": 21.1128, "minLng": 104.8365, "maxLng": 105.8571 },
        "Tỉnh Hưng Yên": { "minLat": 20.0772, "maxLat": 21.007, "minLng": 105.8953, "maxLng": 107.1193 },
        "Tỉnh Khánh Hòa": { "minLat": 11.72, "maxLat": 12.87, "minLng": 108.68, "maxLng": 109.35 },
        "Tỉnh Kiên Giang": { "minLat": 9.3, "maxLat": 10.5, "minLng": 103.4, "maxLng": 105.4 },
        "Tỉnh Kon Tum": { "minLat": 13.9217, "maxLat": 15.4176, "minLng": 107.3354, "maxLng": 108.5472 },
        "Tỉnh Lai Châu": { "minLat": 21.6861, "maxLat": 22.8139, "minLng": 102.3205, "maxLng": 103.9858 },
        "Tỉnh Lạng Sơn": { "minLat": 21.3252, "maxLat": 22.4615, "minLng": 106.0954, "maxLng": 107.3641 },
        "Tỉnh Lào Cai": { "minLat": 21.3259, "maxLat": 22.8449, "minLng": 103.5294, "maxLng": 105.1004 },
        "Tỉnh Lâm Đồng": { "minLat": 11.18, "maxLat": 12.3, "minLng": 107.27, "maxLng": 108.78 },
        "Tỉnh Long An": { "minLat": 10.2, "maxLat": 11.03, "minLng": 105.5, "maxLng": 106.68 },
        "Tỉnh Nam Định": { "minLat": 19.49, "maxLat": 20.4992, "minLng": 105.924, "maxLng": 106.9524 },
        "Tỉnh Nghệ An": { "minLat": 18.5521, "maxLat": 20.0024, "minLng": 103.8746, "maxLng": 106.2645 },
        "Tỉnh Ninh Bình": { "minLat": 19.4111, "maxLat": 20.7044, "minLng": 105.5417, "maxLng": 106.9524 },
        "Tỉnh Ninh Thuận": { "minLat": 10.7631, "maxLat": 12.1648, "minLng": 108.5515, "maxLng": 109.5374 },
        "Tỉnh Phú Thọ": { "minLat": 20.3054, "maxLat": 21.7196, "minLng": 104.8143, "maxLng": 105.8571 },
        "Tỉnh Phú Yên": { "minLat": 12.7048, "maxLat": 13.6953, "minLng": 108.6725, "maxLng": 109.6665 },
        "Tỉnh Quảng Bình": { "minLat": 16.921, "maxLat": 18.1316, "minLng": 105.6077, "maxLng": 107.2961 },
        "Tỉnh Quảng Nam": { "minLat": 14.9514, "maxLat": 16.1663, "minLng": 107.2109, "maxLng": 109.0236 },
        "Tỉnh Quảng Ngãi": { "minLat": 13.9217, "maxLat": 15.7973, "minLng": 107.3354, "maxLng": 109.4632 },
        "Tỉnh Quảng Ninh": { "minLat": 20.4646, "maxLat": 21.6635, "minLng": 106.4392, "maxLng": 108.2086 },
        "Tỉnh Quảng Trị": { "minLat": 16.3022, "maxLat": 18.1316, "minLng": 105.6077, "maxLng": 107.692 },
        "Tỉnh Sóc Trăng": { "minLat": 8.7025, "maxLat": 9.9381, "minLng": 105.544, "maxLng": 106.4538 },
        "Tỉnh Sơn La": { "minLat": 20.573, "maxLat": 22.0307, "minLng": 103.2123, "maxLng": 105.0253 },
        "Tỉnh Tây Ninh": { "minLat": 10.3948, "maxLat": 11.7829, "minLng": 105.5004, "maxLng": 106.7472 },
        "Tỉnh Thái Bình": { "minLat": 20.0772, "maxLat": 20.729, "minLng": 106.1097, "maxLng": 107.1193 },
        "Tỉnh Thái Nguyên": { "minLat": 21.3265, "maxLat": 22.7414, "minLng": 105.4311, "maxLng": 106.247 },
        "Tỉnh Thanh Hóa": { "minLat": 19.1875, "maxLat": 20.67, "minLng": 104.376, "maxLng": 106.2691 },
        "Tỉnh Tiền Giang": { "minLat": 10.1399, "maxLat": 10.587, "minLng": 105.8158, "maxLng": 106.9934 },
        "Tỉnh Trà Vinh": { "minLat": 9.0801, "maxLat": 10.0822, "minLng": 105.9531, "maxLng": 106.8723 },
        "Tỉnh Tuyên Quang": { "minLat": 21.4976, "maxLat": 23.3927, "minLng": 104.3343, "maxLng": 105.5983 },
        "Tỉnh Vĩnh Long": { "minLat": 9.0801, "maxLat": 10.3403, "minLng": 105.6819, "maxLng": 106.9934 },
        "Tỉnh Vĩnh Phúc": { "minLat": 21.1556, "maxLat": 21.5738, "minLng": 105.3225, "maxLng": 105.7866 },
        "Tỉnh Yên Bái": { "minLat": 21.3259, "maxLat": 22.2919, "minLng": 103.8862, "maxLng": 105.1004 }
    });
    Ok(json_data)
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRCodePayload {
    pub image_base64: String,
    pub token: String,
    pub code: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRScannedPayload {
    pub avatar: String,
    pub display_name: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRSuccessPayload {
    pub imei: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ZaloQRErrorPayload {
    pub message: String,
}

fn emit_zalo_qr_error(app_handle: &tauri::AppHandle, message: &str) {
    if let Err(error) = app_handle.emit(
        "zalo-qr-error",
        ZaloQRErrorPayload {
            message: message.to_string(),
        },
    ) {
        eprintln!("Failed to emit Zalo QR error event: {}", error);
    }
}

pub(crate) fn zalo_api_options() -> zca_rs::Options {
    zca_rs::Options {
        auto_start_listener: false,
        ..Default::default()
    }
}

pub fn persist_authenticated_zalo_session(
    conn: &Connection,
    imei: &str,
    user_agent: &str,
    cookies: &[zca_rs::context::ZaloCookie],
) -> Result<ZaloQRSuccessPayload, String> {
    let imei = imei.trim();
    if imei.is_empty() {
        return Err("Zalo returned a session without a device IMEI".to_string());
    }

    let cookies_json = serde_json::to_string(cookies).map_err(|error| error.to_string())?;
    crate::db::save_zalo_session(conn, imei, user_agent, &cookies_json, "vi")
        .map_err(|error| error.to_string())?;

    Ok(ZaloQRSuccessPayload {
        imei: imei.to_string(),
    })
}

pub(crate) fn sanitize_valid_zalo_uid(uid: Option<&str>) -> Option<String> {
    uid.map(str::trim)
        .filter(|value| {
            let lowercase = value.to_ascii_lowercase();
            !value.is_empty() && !matches!(lowercase.as_str(), "0" | "-1" | "null" | "undefined")
        })
        .map(str::to_string)
}

pub(crate) fn require_zalo_uid(zalo_uid: Option<String>) -> Result<String, String> {
    sanitize_valid_zalo_uid(zalo_uid.as_deref()).ok_or_else(|| {
        "Zalo login succeeded, but the account UID could not be determined. No session was saved. Please try again."
            .to_string()
    })
}

#[derive(Clone)]
pub(crate) struct ZaloSessionSnapshot {
    session: crate::models::ZaloSession,
    auth_status: String,
    auth_checked_at: Option<String>,
    auth_error: Option<String>,
}

pub(crate) fn capture_zalo_session_snapshot(
    conn: &Connection,
    imei: &str,
) -> Result<Option<ZaloSessionSnapshot>, String> {
    conn.query_row(
        "SELECT imei, user_agent, cookies_json, language, display_name, avatar,
                zalo_uid, channel_account_id, COALESCE(auth_status, 'unknown'),
                auth_checked_at, auth_error
         FROM zalo_sessions WHERE imei = ?1",
        [imei],
        |row| {
            Ok(ZaloSessionSnapshot {
                session: crate::models::ZaloSession {
                    imei: row.get(0)?,
                    user_agent: row.get(1)?,
                    cookies_json: row.get(2)?,
                    language: row.get(3)?,
                    display_name: row.get(4)?,
                    avatar: row.get(5)?,
                    zalo_uid: row.get(6)?,
                    channel_account_id: row.get(7)?,
                },
                auth_status: row.get(8)?,
                auth_checked_at: row.get(9)?,
                auth_error: row.get(10)?,
            })
        },
    )
    .optional()
    .map_err(|error| error.to_string())
}

pub(crate) fn rollback_zalo_session_persistence(
    conn: &Connection,
    imei: &str,
    snapshot: Option<ZaloSessionSnapshot>,
) -> Result<(), String> {
    if let Some(snapshot) = snapshot {
        conn.execute(
            "INSERT INTO zalo_sessions
                (imei, user_agent, cookies_json, language, display_name, avatar,
                 auth_status, auth_checked_at, auth_error, zalo_uid, channel_account_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(imei) DO UPDATE SET
                user_agent = excluded.user_agent,
                cookies_json = excluded.cookies_json,
                language = excluded.language,
                display_name = excluded.display_name,
                avatar = excluded.avatar,
                auth_status = excluded.auth_status,
                auth_checked_at = excluded.auth_checked_at,
                auth_error = excluded.auth_error,
                zalo_uid = excluded.zalo_uid,
                channel_account_id = excluded.channel_account_id",
            rusqlite::params![
                snapshot.session.imei,
                snapshot.session.user_agent,
                snapshot.session.cookies_json,
                snapshot.session.language,
                snapshot.session.display_name,
                snapshot.session.avatar,
                snapshot.auth_status,
                snapshot.auth_checked_at,
                snapshot.auth_error,
                snapshot.session.zalo_uid,
                snapshot.session.channel_account_id,
            ],
        )
        .map_err(|error| error.to_string())?;
    } else {
        crate::db::delete_zalo_session(conn, imei).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn existing_zalo_account_for_login(
    conn: &Connection,
    imei: &str,
    zalo_uid: Option<&str>,
) -> Result<Option<i64>, String> {
    let uid = zalo_uid.unwrap_or("");
    conn.query_row(
        "SELECT ca.id
         FROM channel_accounts ca
         WHERE ca.channel = 'zalo'
           AND (
                (?2 != '' AND (ca.provider_account_key = ?2 OR ca.self_external_id = ?2))
                OR ca.provider_account_key = ?1
                OR ca.id IN (
                    SELECT channel_account_id FROM zalo_sessions
                    WHERE imei = ?1 AND channel_account_id IS NOT NULL
                )
           )
         ORDER BY CASE WHEN ?2 != '' AND ca.provider_account_key = ?2 THEN 0 ELSE 1 END,
                  ca.id
         LIMIT 1",
        rusqlite::params![imei, uid],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn rollback_failed_zalo_login(
    conn: &Connection,
    imei: &str,
    snapshot: Option<ZaloSessionSnapshot>,
    newly_created_account_id: Option<i64>,
) -> Result<(), String> {
    rollback_zalo_session_persistence(conn, imei, snapshot)?;
    if let Some(account_id) = newly_created_account_id {
        conn.execute(
            "DELETE FROM channel_accounts
             WHERE id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM channel_conversations WHERE channel_account_id = ?1
               )
               AND NOT EXISTS (
                   SELECT 1 FROM zalo_sessions WHERE channel_account_id = ?1
               )",
            [account_id],
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn start_zalo_qr_login(
    app_handle: tauri::AppHandle,
    _state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    let callback_app_handle = app_handle.clone();
    let callback =
        move |event: zca_rs::apis::auth::LoginQREvent| -> zca_rs::apis::auth::QRCallbackResult {
            let is_declined = matches!(&event, zca_rs::apis::auth::LoginQREvent::QRCodeDeclined);
            if matches!(
                &event,
                zca_rs::apis::auth::LoginQREvent::GotLoginInfo { .. }
            ) {
                return zca_rs::apis::auth::QRCallbackResult::Continue;
            }

            match handle_zalo_qr_event(event, None) {
                Ok((event_name, payload)) => {
                    let _ = callback_app_handle.emit(&event_name, payload);
                }
                Err(error) => {
                    eprintln!("Error handling Zalo QR event: {error}");
                    emit_zalo_qr_error(
                        &callback_app_handle,
                        "Zalo confirmed the scan but did not create a usable session. Please try again.",
                    );
                }
            }

            if is_declined {
                zca_rs::apis::auth::QRCallbackResult::Cancel
            } else {
                zca_rs::apis::auth::QRCallbackResult::Continue
            }
        };

    let zalo = match zca_rs::Zalo::login_qr(zalo_api_options(), callback).await {
        Ok(zalo) => zalo,
        Err(zca_rs::ZaloError::LoginQRAborted | zca_rs::ZaloError::LoginQRDeclined) => {
            return Ok(());
        }
        Err(error) => {
            eprintln!("Zalo QR login failed: {error:?}");
            let message =
                "Zalo could not complete the login session. No session was saved. Please try again.";
            emit_zalo_qr_error(&app_handle, message);
            return Err(message.to_string());
        }
    };

    let cookies = zalo.ctx.cookie_jar.to_vec();
    let mut display_name = None;
    let mut avatar = None;
    let mut zalo_uid = zalo.get_own_id().filter(|value| !value.trim().is_empty());
    if let Ok(info) = zalo.fetch_account_info().await {
        let (name, phone, profile_avatar) = extract_zalo_profile_info(&info);
        display_name = name.or(phone);
        avatar = profile_avatar;
        zalo_uid = extract_zalo_uid(&info, &cookies).or(zalo_uid);
    }
    if zalo_uid.is_none() {
        zalo_uid = extract_zalo_uid(&serde_json::Value::Null, &cookies);
    }
    let zalo_uid = require_zalo_uid(zalo_uid).inspect_err(|message| {
        emit_zalo_qr_error(&app_handle, message);
    })?;

    let imei = zalo.ctx.imei.clone();
    let user_agent = zalo.ctx.user_agent.clone();
    drop(zalo);

    let (conversation_service, supervisor) = app_handle
        .try_state::<crate::conversation::commands::ConversationState>()
        .map(|state| (state.service.clone(), state.zalo_supervisor.clone()))
        .ok_or_else(|| {
            let message = "Conversation runtime is unavailable. Zalo login was not saved.";
            emit_zalo_qr_error(&app_handle, message);
            message.to_string()
        })?;

    let (session_snapshot, existing_account_id, payload) = {
        let db_state = app_handle.state::<DbState>();
        let conn = db_state.conn.lock().map_err(|error| {
            eprintln!("Failed to lock database for Zalo session: {error}");
            let message =
                "Zalo login succeeded, but the session could not be saved. Please try again.";
            emit_zalo_qr_error(&app_handle, message);
            message.to_string()
        })?;
        let session_snapshot = capture_zalo_session_snapshot(&conn, &imei)?;
        let existing_account_id = existing_zalo_account_for_login(&conn, &imei, Some(&zalo_uid))?;
        let payload = match persist_authenticated_zalo_session(&conn, &imei, &user_agent, &cookies)
        {
            Ok(payload) => payload,
            Err(error) => {
                eprintln!("Failed to persist Zalo session: {error}");
                let message =
                    "Zalo login succeeded, but the session could not be saved. Please try again.";
                emit_zalo_qr_error(&app_handle, message);
                return Err(message.to_string());
            }
        };
        if let Err(error) = crate::db::update_zalo_session_profile(
            &conn,
            &imei,
            display_name.as_deref(),
            avatar.as_deref(),
        )
        .and_then(|_| crate::db::update_zalo_session_auth_status(&conn, &imei, "valid", None))
        {
            let _ = rollback_zalo_session_persistence(&conn, &imei, session_snapshot.clone());
            eprintln!("Failed to update Zalo session metadata: {error}");
            let message =
                "Zalo login succeeded, but the session metadata could not be saved. Please try again.";
            emit_zalo_qr_error(&app_handle, message);
            return Err(message.to_string());
        }
        (session_snapshot, existing_account_id, payload)
    };

    let account_id = match conversation_service
        .repository
        .reconcile_zalo_channel_account(
            &imei,
            Some(&zalo_uid),
            display_name.as_deref(),
            avatar.as_deref(),
        ) {
        Ok(account_id) => account_id,
        Err(error) => {
            let db_state = app_handle.state::<DbState>();
            if let Ok(conn) = db_state.conn.lock() {
                let _ = rollback_failed_zalo_login(&conn, &imei, session_snapshot, None);
            }
            eprintln!("Failed to reconcile Zalo channel account: {error}");
            let message = "Failed to reconcile Zalo account. Please try logging in again.";
            emit_zalo_qr_error(&app_handle, message);
            return Err(message.to_string());
        }
    };

    let target_key = zalo_uid;
    if let Err(error) = supervisor.restart_account(&target_key).await {
        let _ = conversation_service.repository.set_account_status(
            &target_key,
            crate::conversation::types::AccountConnectionStatus::Disconnected,
            Some(&error.to_string()),
        );
        let db_state = app_handle.state::<DbState>();
        let rollback_error = db_state
            .conn
            .lock()
            .map_err(|lock_error| lock_error.to_string())
            .and_then(|conn| {
                rollback_failed_zalo_login(
                    &conn,
                    &imei,
                    session_snapshot,
                    existing_account_id.is_none().then_some(account_id),
                )
            })
            .err();
        if let Some(rollback_error) = rollback_error {
            eprintln!("Failed to rollback Zalo login state: {rollback_error}");
        }
        eprintln!("Failed to start Zalo supervisor session: {error}");
        let message =
            "Zalo login was saved but the realtime connection could not start. Please try again.";
        emit_zalo_qr_error(&app_handle, message);
        return Err(message.to_string());
    }

    let payload = serde_json::to_value(payload).map_err(|error| {
        eprintln!("Failed to serialize Zalo QR success event: {error}");
        let message = "Zalo login succeeded, but the success response could not be created.";
        emit_zalo_qr_error(&app_handle, message);
        message.to_string()
    })?;
    app_handle
        .emit("zalo-qr-success", payload)
        .map_err(|error| {
            eprintln!("Failed to emit Zalo QR success event: {error}");
            "Zalo login succeeded, but the UI could not be notified.".to_string()
        })?;

    Ok(())
}

pub fn handle_zalo_qr_event(
    event: zca_rs::apis::auth::LoginQREvent,
    conn: Option<&Connection>,
) -> Result<(String, serde_json::Value), String> {
    match event {
        zca_rs::apis::auth::LoginQREvent::QRCodeGenerated {
            code,
            image_base64,
            token,
        } => {
            let payload = serde_json::to_value(ZaloQRCodePayload {
                image_base64,
                token,
                code,
            })
            .map_err(|e| e.to_string())?;
            Ok(("zalo-qr-code".to_string(), payload))
        }
        zca_rs::apis::auth::LoginQREvent::QRCodeScanned {
            avatar,
            display_name,
        } => {
            let payload = serde_json::to_value(ZaloQRScannedPayload {
                avatar,
                display_name,
            })
            .map_err(|e| e.to_string())?;
            Ok(("zalo-qr-scanned".to_string(), payload))
        }
        zca_rs::apis::auth::LoginQREvent::QRCodeExpired => {
            Ok(("zalo-qr-expired".to_string(), serde_json::Value::Null))
        }
        zca_rs::apis::auth::LoginQREvent::QRCodeDeclined => {
            Ok(("zalo-qr-declined".to_string(), serde_json::Value::Null))
        }
        zca_rs::apis::auth::LoginQREvent::GotLoginInfo {
            cookie,
            imei,
            user_agent,
        } => {
            if let Some(c) = conn {
                let payload = persist_authenticated_zalo_session(c, &imei, &user_agent, &cookie)?;
                return serde_json::to_value(payload)
                    .map(|payload| ("zalo-qr-success".to_string(), payload))
                    .map_err(|error| error.to_string());
            }

            let payload =
                serde_json::to_value(ZaloQRSuccessPayload { imei }).map_err(|e| e.to_string())?;
            Ok(("zalo-qr-success".to_string(), payload))
        }
    }
}

#[cfg(test)]
pub fn get_zalo_sessions_internal(
    conn: &Connection,
) -> Result<Vec<crate::models::ZaloSession>, String> {
    crate::db::get_all_zalo_sessions(conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn get_managed_zalo_accounts(
    state: tauri::State<DbState>,
) -> Result<Vec<ManagedZaloAccount>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_managed_zalo_accounts_internal(&conn)
}

pub fn get_managed_zalo_accounts_internal(
    conn: &Connection,
) -> Result<Vec<ManagedZaloAccount>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(ca.provider_account_key, zs.zalo_uid, zs.imei),
                    COALESCE(zs.display_name, ca.display_name),
                    COALESCE(zs.avatar, ca.avatar_url),
                    COALESCE(zs.auth_status, 'unknown'), zs.auth_checked_at, zs.auth_error,
                    COALESCE(ca.connection_status, 'disconnected'),
                    ca.last_connected_at, ca.last_error
             FROM zalo_sessions zs
             LEFT JOIN channel_accounts ca
               ON ca.channel = 'zalo' AND (ca.id = zs.channel_account_id OR ca.provider_account_key = zs.imei OR (ca.self_external_id IS NOT NULL AND ca.self_external_id = zs.zalo_uid))
             ORDER BY lower(COALESCE(zs.display_name, ca.display_name, zs.imei)), zs.imei",
        )
        .map_err(|e| format!("Database error: {}", e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(ManagedZaloAccount {
                account_key: row.get(0)?,
                display_name: row.get(1)?,
                avatar: row.get(2)?,
                auth_status: row.get(3)?,
                auth_checked_at: row.get(4)?,
                auth_error: row.get(5)?,
                runtime_status: row.get(6)?,
                last_connected_at: row.get(7)?,
                last_error: row.get(8)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn get_managed_smtp_accounts(
    state: tauri::State<DbState>,
) -> Result<Vec<ManagedSmtpAccount>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    get_managed_smtp_accounts_internal(&conn)
}

pub fn get_managed_smtp_accounts_internal(
    conn: &Connection,
) -> Result<Vec<ManagedSmtpAccount>, String> {
    crate::db::get_smtp_servers(conn)
        .map(|servers| {
            servers
                .into_iter()
                .filter_map(|server| {
                    server.id.map(|id| ManagedSmtpAccount {
                        id,
                        name: server.name,
                        host: server.host,
                        port: server.port,
                        username: server.username,
                        from_email: server.from_email,
                        from_name: server.from_name,
                        security: server.security,
                    })
                })
                .collect()
        })
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub async fn delete_zalo_session(
    app_handle: tauri::AppHandle,
    imei: String,
    state: tauri::State<'_, DbState>,
) -> Result<(), String> {
    if let Some(conv_state) =
        app_handle.try_state::<crate::conversation::commands::ConversationState>()
    {
        let _ = conv_state.zalo_supervisor.stop_account(&imei).await;
    }
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    delete_zalo_session_internal(&conn, &imei)
}

pub fn delete_zalo_session_internal(conn: &Connection, imei: &str) -> Result<(), String> {
    crate::db::delete_zalo_session(conn, imei).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub async fn verify_zalo_session(
    imei: String,
    state: tauri::State<'_, DbState>,
) -> Result<ZaloSessionStatus, String> {
    let session = {
        let conn = state
            .conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        crate::db::get_zalo_session(&conn, &imei)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("Session with IMEI {} not found", imei))?
    };

    let status = verify_zalo_session_internal(session).await?;
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    persist_zalo_verification(&conn, &imei, &status)?;
    if status.is_valid {
        crate::db::update_zalo_session_profile(
            &conn,
            &imei,
            status.display_name.as_deref(),
            status.avatar.as_deref(),
        )
        .map_err(|e| format!("Database error: {}", e))?;
    }
    Ok(status)
}

fn persist_zalo_verification(
    conn: &Connection,
    imei: &str,
    status: &ZaloSessionStatus,
) -> Result<(), String> {
    let auth_status = if status.is_valid { "valid" } else { "expired" };
    crate::db::update_zalo_session_auth_status(
        conn,
        imei,
        auth_status,
        status.error_message.as_deref(),
    )
    .map_err(|e| format!("Database error: {}", e))
}

fn extract_zalo_profile_info(
    val: &serde_json::Value,
) -> (Option<String>, Option<String>, Option<String>) {
    let mut targets = vec![val];
    if let Some(d) = val.get("data") {
        targets.push(d);
    }
    if let Some(u) = val.get("userInfo") {
        targets.push(u);
    }
    if let Some(u) = val.get("data").and_then(|d| d.get("userInfo")) {
        targets.push(u);
    }
    if let Some(p) = val.get("profile") {
        targets.push(p);
    }

    let mut name = None;
    let mut phone = None;
    let mut avatar = None;

    for t in targets {
        if name.is_none() {
            name = t
                .get("zName")
                .or_else(|| t.get("dpName"))
                .or_else(|| t.get("zaloName"))
                .or_else(|| t.get("displayName"))
                .or_else(|| t.get("name"))
                .or_else(|| t.get("display_name"))
                .or_else(|| t.get("username"))
                .or_else(|| t.get("user_name"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }

        if phone.is_none() {
            phone = t
                .get("phone")
                .or_else(|| t.get("phoneNumber"))
                .or_else(|| t.get("phone_number"))
                .or_else(|| t.get("mobile"))
                .or_else(|| t.get("sdT"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }

        if avatar.is_none() {
            avatar = t
                .get("avatar_240")
                .or_else(|| t.get("avatar_120"))
                .or_else(|| t.get("avatar"))
                .or_else(|| t.get("avatarUrl"))
                .or_else(|| t.get("avatar_url"))
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
        }
    }

    (name, phone, avatar)
}

pub fn extract_zalo_uid(
    val: &serde_json::Value,
    cookies: &[zca_rs::context::ZaloCookie],
) -> Option<String> {
    for c in cookies {
        if (c.name == "zalo_id" || c.name == "uid" || c.name == "zalo_uid")
            && !c.value.trim().is_empty()
        {
            return Some(c.value.trim().to_string());
        }
    }

    let mut targets = vec![val];
    if let Some(d) = val.get("data") {
        targets.push(d);
    }
    if let Some(u) = val.get("userInfo") {
        targets.push(u);
    }
    if let Some(u) = val.get("data").and_then(|d| d.get("userInfo")) {
        targets.push(u);
    }
    if let Some(p) = val.get("profile") {
        targets.push(p);
    }

    for t in targets {
        for key in &["userId", "uid", "zaloId", "zalo_id", "zalo_uid"] {
            if let Some(v) = t.get(*key) {
                if let Some(s) = v.as_str() {
                    let trimmed = s.trim();
                    if !trimmed.is_empty() && trimmed != "0" {
                        return Some(trimmed.to_string());
                    }
                } else if let Some(n) = v.as_i64() {
                    if n > 0 {
                        return Some(n.to_string());
                    }
                } else if let Some(n) = v.as_u64() {
                    if n > 0 {
                        return Some(n.to_string());
                    }
                }
            }
        }
    }
    None
}

pub async fn verify_zalo_session_internal(
    session: crate::models::ZaloSession,
) -> Result<ZaloSessionStatus, String> {
    let cookies: Vec<zca_rs::context::ZaloCookie> =
        match serde_json::from_str(&session.cookies_json) {
            Ok(c) => c,
            Err(e) => {
                return Ok(ZaloSessionStatus {
                    imei: session.imei,
                    is_valid: false,
                    display_name: session.display_name,
                    avatar: session.avatar,
                    error_message: Some(format!("Invalid cookies format: {}", e)),
                });
            }
        };

    let options = zalo_api_options();
    match zca_rs::Zalo::login(
        session.imei.clone(),
        session.user_agent.clone(),
        cookies,
        options,
    )
    .await
    {
        Ok(zalo) => {
            let (mut name, mut phone, mut avatar) = match zalo.fetch_account_info().await {
                Ok(info) => {
                    eprintln!("Zalo fetch_account_info SUCCESS: {:?}", info);
                    extract_zalo_profile_info(&info)
                }
                Err(e) => {
                    eprintln!(
                        "fetch_account_info error for IMEI {}: {:?}",
                        session.imei, e
                    );
                    (None, None, None)
                }
            };

            if name.is_none() && phone.is_none() && avatar.is_none() {
                let li_guard = zalo
                    .ctx
                    .login_info
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                eprintln!("Zalo ctx.login_info fallback: {:?}", *li_guard);
                let (n2, p2, a2) = extract_zalo_profile_info(&li_guard);
                name = n2;
                phone = p2;
                avatar = a2;
            }

            let display_name = name.or(phone).or(session.display_name);
            let avatar = avatar.or(session.avatar);

            Ok(ZaloSessionStatus {
                imei: session.imei,
                is_valid: true,
                display_name,
                avatar,
                error_message: None,
            })
        }
        Err(e) => Ok(ZaloSessionStatus {
            imei: session.imei,
            is_valid: false,
            display_name: session.display_name,
            avatar: session.avatar,
            error_message: Some(format!("Login failed: {}", e)),
        }),
    }
}

#[tauri::command]
pub async fn verify_all_zalo_sessions(
    state: tauri::State<'_, DbState>,
) -> Result<Vec<ZaloSessionStatus>, String> {
    let sessions = {
        let conn = state
            .conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        crate::db::get_all_zalo_sessions(&conn).map_err(|e| format!("Database error: {}", e))?
    };

    let mut results = Vec::new();
    for session in sessions {
        let imei = session.imei.clone();
        match verify_zalo_session_internal(session).await {
            Ok(status) => {
                let conn = state
                    .conn
                    .lock()
                    .map_err(|e| format!("Database lock error: {}", e))?;
                persist_zalo_verification(&conn, &imei, &status)?;
                if status.is_valid {
                    crate::db::update_zalo_session_profile(
                        &conn,
                        &imei,
                        status.display_name.as_deref(),
                        status.avatar.as_deref(),
                    )
                    .map_err(|e| format!("Database error: {}", e))?;
                }
                results.push(status);
            }
            Err(e) => {
                let status = ZaloSessionStatus {
                    imei: imei.clone(),
                    is_valid: false,
                    display_name: None,
                    avatar: None,
                    error_message: Some(e),
                };
                let conn = state
                    .conn
                    .lock()
                    .map_err(|error| format!("Database lock error: {}", error))?;
                persist_zalo_verification(&conn, &imei, &status)?;
                results.push(status);
            }
        }
    }
    Ok(results)
}

#[derive(Clone, serde::Serialize)]
pub struct OutreachProgressPayload {
    pub progress: f64,
    pub status: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct OutreachLogPayload {
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct OutreachCompletePayload {
    pub total_sent: usize,
    pub message: String,
}

#[tauri::command]
pub fn get_outreach_templates(
    state: tauri::State<DbState>,
) -> Result<Vec<crate::models::OutreachTemplate>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_outreach_templates(&conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn create_outreach_template(
    name: String,
    content: String,
    image_url: Option<String>,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::create_outreach_template(&conn, &name, &content, image_url.as_deref())
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn update_outreach_template(
    id: i32,
    name: String,
    content: String,
    image_url: Option<String>,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::update_outreach_template(&conn, id, &name, &content, image_url.as_deref())
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_outreach_template(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_outreach_template(&conn, id).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub async fn send_zalo_outreach(
    app_handle: tauri::AppHandle,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let _ = send_zalo_outreach_internal(
            Some(&app_handle_clone),
            &db_state.conn,
            imei,
            lead_ids,
            template_id,
            ZaloOutreachRunOptions::default(),
            OutreachProgressScope::default(),
        )
        .await;
    });
    Ok(())
}

#[tauri::command]
#[expect(
    clippy::too_many_arguments,
    reason = "preserve the existing Tauri Zalo outreach IPC argument contract"
)]
pub async fn start_zalo_outreach(
    app_handle: tauri::AppHandle,
    imeis: Vec<String>,
    template_id: i32,
    lead_ids: Vec<i32>,
    enable_burst_limit: Option<bool>,
    burst_max_messages: Option<u32>,
    burst_delay_seconds: Option<u64>,
    inter_message_delay_seconds: Option<u64>,
) -> Result<(), String> {
    let enable_burst = enable_burst_limit.unwrap_or(false);
    let burst_max = burst_max_messages.unwrap_or(0);
    let burst_delay = burst_delay_seconds.unwrap_or(0);
    let inter_delay = inter_message_delay_seconds.unwrap_or(0);

    let partitions = partition_outreach_leads(&imeis, &lead_ids)?;
    let active_partitions: Vec<(String, Vec<i32>)> = partitions
        .into_iter()
        .filter(|(_, ids)| !ids.is_empty())
        .collect();

    if active_partitions.is_empty() {
        return Ok(());
    }

    let total_leads: usize = active_partitions.iter().map(|(_, ids)| ids.len()).sum();
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let mut processed_offset = 0;
        let mut sent_offset = 0;
        let num_partitions = active_partitions.len();

        for (part_idx, (imei, part_leads)) in active_partitions.into_iter().enumerate() {
            let count = part_leads.len();
            let is_last = part_idx == num_partitions - 1;
            let scope = OutreachProgressScope {
                processed_offset,
                sent_offset,
                total_leads,
                emit_complete: is_last,
            };
            if let Ok(res) = send_zalo_outreach_internal(
                Some(&app_handle_clone),
                &db_state.conn,
                imei,
                part_leads,
                template_id,
                ZaloOutreachRunOptions {
                    enable_burst_limit: enable_burst,
                    burst_max_messages: burst_max,
                    burst_delay_seconds: burst_delay,
                    inter_message_delay_seconds: inter_delay,
                },
                scope,
            )
            .await
            {
                sent_offset += res.total_sent;
            }
            processed_offset += count;
        }
    });
    Ok(())
}

pub fn normalize_phone_digits(phone: &str) -> String {
    let phone_part = match phone.find(|c: char| {
        !c.is_ascii_digit()
            && c != ' '
            && c != '+'
            && c != '-'
            && c != '('
            && c != ')'
            && c != '.'
            && c != ','
            && c != '['
            && c != ']'
    }) {
        Some(idx) => &phone[..idx],
        None => phone,
    };

    let digits: String = phone_part.chars().filter(|c| c.is_ascii_digit()).collect();
    if let Some(rest) = digits.strip_prefix("0084") {
        format!("0{rest}")
    } else if let Some(rest) = digits.strip_prefix("84") {
        format!("0{rest}")
    } else {
        digits
    }
}

fn is_vn_mobile(phone: &str) -> bool {
    let normalized = normalize_phone_digits(phone);
    if normalized.len() != 10 {
        return false;
    }

    let prefix = &normalized[0..2];
    matches!(prefix, "03" | "05" | "07" | "08" | "09")
}

fn render_zalo_template(template: &str, lead_name: &str) -> String {
    template
        .replace("[name]", lead_name)
        .replace("{name}", lead_name)
}

fn should_rotate_zalo_session(error: &zca_rs::ZaloError) -> bool {
    match error {
        zca_rs::ZaloError::ApiError { message, code } => {
            if matches!(code, Some(102 | 221 | 312 | 401 | 403 | 429)) {
                return true;
            }

            let message = message.to_ascii_lowercase();
            [
                "rate limit",
                "too many request",
                "vượt quá số request",
                "tìm số điện thoại quá nhiều lần",
                "hoạt động bất thường",
                "session",
                "login",
                "unauthorized",
                "forbidden",
                "expired",
            ]
            .iter()
            .any(|needle| message.contains(needle))
        }
        zca_rs::ZaloError::Crypto(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("secret key") || message.contains("session")
        }
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ZaloOutreachRunOptions {
    pub enable_burst_limit: bool,
    pub burst_max_messages: u32,
    pub burst_delay_seconds: u64,
    pub inter_message_delay_seconds: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct OutreachProgressScope {
    pub processed_offset: usize,
    pub sent_offset: usize,
    pub total_leads: usize,
    pub emit_complete: bool,
}

impl Default for OutreachProgressScope {
    fn default() -> Self {
        Self {
            processed_offset: 0,
            sent_offset: 0,
            total_leads: 0,
            emit_complete: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZaloOutreachBatchResult {
    pub total_sent: usize,
}

pub fn partition_outreach_leads(
    imeis: &[String],
    lead_ids: &[i32],
) -> Result<Vec<(String, Vec<i32>)>, String> {
    let mut dedup_imeis: Vec<String> = Vec::new();
    for imei in imeis {
        if !dedup_imeis.contains(imei) {
            dedup_imeis.push(imei.clone());
        }
    }

    if dedup_imeis.is_empty() {
        return Err("No valid IMEI provided".to_string());
    }

    let mut dedup_lead_ids: Vec<i32> = Vec::new();
    for &id in lead_ids {
        if !dedup_lead_ids.contains(&id) {
            dedup_lead_ids.push(id);
        }
    }

    let num_imeis = dedup_imeis.len();
    let mut partitions: Vec<Vec<i32>> = vec![Vec::new(); num_imeis];
    for (idx, &lead_id) in dedup_lead_ids.iter().enumerate() {
        partitions[idx % num_imeis].push(lead_id);
    }

    let result = dedup_imeis.into_iter().zip(partitions).collect();
    Ok(result)
}

pub async fn send_zalo_outreach_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    imei: String,
    lead_ids: Vec<i32>,
    template_id: i32,
    options: ZaloOutreachRunOptions,
    progress_scope: OutreachProgressScope,
) -> Result<ZaloOutreachBatchResult, String> {
    let ZaloOutreachRunOptions {
        enable_burst_limit,
        burst_max_messages,
        burst_delay_seconds,
        inter_message_delay_seconds,
    } = options;
    let OutreachProgressScope {
        processed_offset,
        sent_offset: _,
        total_leads,
        emit_complete,
    } = progress_scope;
    let display_total = if total_leads > 0 {
        total_leads
    } else {
        lead_ids.len()
    };
    // Mock flow helper
    if imei == "mock-imei" || imei.starts_with("mock-") {
        let template_content = {
            let conn_guard = conn_mutex.lock().unwrap();
            conn_guard
                .query_row(
                    "SELECT content FROM outreach_templates WHERE id = ?1",
                    [template_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap_or_default()
        };

        let mut total_sent = 0;
        let mut burst_counter: u32 = 0;
        for (idx, lead_id) in lead_ids.iter().enumerate() {
            let (lead_name, phone, lead_zalo_status, existing_zalo_uid) = {
                let conn_guard = conn_mutex.lock().unwrap();
                let res = conn_guard.query_row(
                    "SELECT name, phone, zalo_status, zalo_uid FROM leads WHERE id = ?1",
                    [lead_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    },
                );
                match res {
                    Ok(val) => val,
                    Err(e) => {
                        let msg = format!("Failed to load lead {}: {}", lead_id, e);
                        if let Some(app) = app_handle {
                            let _ = app.emit("outreach-log", OutreachLogPayload { message: msg });
                        }
                        continue;
                    }
                }
            };

            let sanitized_phone: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
            let cached_uid = sanitize_valid_zalo_uid(existing_zalo_uid.as_deref());

            let target_uid = if let Some(uid) = cached_uid {
                if let Some(ref zs) = lead_zalo_status {
                    if zs == "BLOCKED_STRANGER" {
                        let msg = format!(
                            "Skipped Zalo outreach for {} (UID: {}): Previously marked as BLOCKED_STRANGER",
                            lead_name, uid
                        );
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "outreach-log",
                                OutreachLogPayload {
                                    message: msg.clone(),
                                },
                            );
                            let current_num = processed_offset + idx + 1;
                            let progress_val =
                                (current_num as f64 / display_total.max(1) as f64) * 100.0;
                            let _ = app.emit(
                                "outreach-progress",
                                OutreachProgressPayload {
                                    progress: progress_val,
                                    status: "running".to_string(),
                                    message: msg,
                                },
                            );
                        }
                        continue;
                    }
                }
                uid
            } else {
                if let Some(ref zs) = lead_zalo_status {
                    if matches!(
                        zs.as_str(),
                        "BLOCKED_STRANGER" | "VERIFIED_NOT_EXIST" | "NOT_FOUND" | "INVALID_PHONE"
                    ) {
                        let msg = format!(
                            "Skipped Zalo lookup for {} ({}): Previously marked as {}",
                            lead_name, sanitized_phone, zs
                        );
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "outreach-log",
                                OutreachLogPayload {
                                    message: msg.clone(),
                                },
                            );
                            let current_num = processed_offset + idx + 1;
                            let progress_val =
                                (current_num as f64 / display_total.max(1) as f64) * 100.0;
                            let _ = app.emit(
                                "outreach-progress",
                                OutreachProgressPayload {
                                    progress: progress_val,
                                    status: "running".to_string(),
                                    message: msg,
                                },
                            );
                        }
                        continue;
                    }
                }

                if phone.trim().is_empty() {
                    let msg = format!("Lead {} is missing phone number", lead_id);
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }
                    continue;
                }

                if phone != "not-found" && !is_vn_mobile(&phone) {
                    let msg = format!("Skipping non-mobile number: {}", phone);
                    println!("{}", msg);
                    let _ = {
                        let conn_guard = conn_mutex.lock().unwrap();
                        conn_guard.execute(
                            "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                            rusqlite::params![lead_id],
                        )
                    };
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }
                    continue;
                }

                if phone == "not-found" {
                    let msg = format!("Phone number {} not found on Zalo (Mock)", phone);
                    let _ = {
                        let conn_guard = conn_mutex.lock().unwrap();
                        conn_guard.execute(
                            "UPDATE leads SET has_zalo = 0, zalo_status = 'VERIFIED_NOT_EXIST' WHERE id = ?1",
                            rusqlite::params![lead_id],
                        )
                    };
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }
                    continue;
                }

                format!("mock-uid-{sanitized_phone}")
            };

            let formatted_content = render_zalo_template(&template_content, &lead_name);
            let local_request_id = format!("outreach-{}", rand::random::<u128>());
            let conversation_sent_at = crate::conversation::types::utc_now();
            let provider_receipt =
                crate::conversation::outreach::ZaloProviderReceipt::mock(&local_request_id);
            let external_user_id = target_uid.clone();

            total_sent += 1;
            burst_counter += 1;
            let log_msg = format!("Message sent to {} ({}) (Mock)", lead_name, target_uid);
            if let Some(app) = app_handle {
                let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
            }

            let act_res: Result<crate::conversation::outreach::AppliedOutreachMessage, String> = {
                let mut conn_guard = conn_mutex
                    .lock()
                    .map_err(|e| format!("Database lock error: {}", e))?;
                let tx = conn_guard
                    .transaction()
                    .map_err(|e| format!("Database transaction error: {}", e))?;

                let now: String = tx
                    .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                        row.get(0)
                    })
                    .map_err(|e| format!("Database query error: {}", e))?;

                tx.execute(
                    "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![
                        lead_id,
                        "Outreach",
                        format!("Zalo message sent: {}", formatted_content),
                        &now,
                        "System (Zalo Outreach)"
                    ]
                ).map_err(|e| format!("Database query error: {}", e))?;

                tx.execute(
                    "UPDATE leads SET last_activity = ?1, has_zalo = 1, zalo_reached = 1, zalo_uid = ?2, zalo_status = 'VERIFIED_EXIST' WHERE id = ?3",
                    rusqlite::params![&now, &target_uid, lead_id],
                )
                .map_err(|e| format!("Database query error: {}", e))?;

                tx.execute(
                    "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'SUCCESS', NULL, ?3, ?4)",
                    rusqlite::params![lead_id, template_id, &now, &imei],
                )
                .map_err(|e| format!("Database query error: {}", e))?;

                let applied_outreach =
                    crate::conversation::outreach::record_outreach_success_in_transaction(
                        &tx,
                        &crate::conversation::outreach::OutreachConversationRecord {
                            lead_id: i64::from(*lead_id),
                            provider_account_key: imei.clone(),
                            account_display_name: None,
                            external_user_id,
                            user_display_name: Some(lead_name.clone()),
                            user_avatar_url: None,
                            template_id: i64::from(template_id),
                            source: "zalo_outreach".to_string(),
                            text: formatted_content.clone(),
                            local_request_id,
                            provider_receipt,
                            sent_at: conversation_sent_at,
                        },
                    )
                    .map_err(|e| format!("Outreach conversation record error: {}", e))?;

                tx.commit()
                    .map_err(|e| format!("Database transaction commit error: {}", e))?;
                Ok(applied_outreach)
            };

            match act_res {
                Ok(applied_outreach) => {
                    if let Some(app) = app_handle {
                        if let Some(state) =
                            app.try_state::<crate::conversation::commands::ConversationState>()
                        {
                            state.service.publish_outreach_success(&applied_outreach);
                        }
                    }
                }
                Err(e) => {
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: format!(
                                    "Delivery succeeded but local persistence failed: {}",
                                    e
                                ),
                            },
                        );
                    }
                }
            }

            if let Some(app) = app_handle {
                let current_num = processed_offset + idx + 1;
                let progress_val = (current_num as f64 / display_total.max(1) as f64) * 100.0;
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: progress_val,
                        status: "running".to_string(),
                        message: format!("Processed lead {}/{} (Mock)", current_num, display_total),
                    },
                );
            }

            if enable_burst_limit
                && burst_max_messages > 0
                && burst_counter >= burst_max_messages
                && (idx + 1) < lead_ids.len()
            {
                let msg = format!(
                    "[Burst Limit] Sent {} messages. Pausing {} seconds to protect Zalo account...",
                    burst_counter, burst_delay_seconds
                );
                if let Some(app) = app_handle {
                    let _ = app.emit("outreach-log", OutreachLogPayload { message: msg });
                }
                tokio::time::sleep(std::time::Duration::from_secs(burst_delay_seconds)).await;
                burst_counter = 0;
            } else if idx + 1 < lead_ids.len() {
                if inter_message_delay_seconds > 0 {
                    tokio::time::sleep(std::time::Duration::from_secs(inter_message_delay_seconds))
                        .await;
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        }

        let overall_sent = progress_scope.sent_offset + total_sent;
        let final_msg = format!(
            "Outreach complete! Sent {} messages out of {} leads. (Mock)",
            overall_sent, display_total
        );
        if let Some(app) = app_handle {
            if emit_complete {
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 100.0,
                        status: "completed".to_string(),
                        message: final_msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-complete",
                    OutreachCompletePayload {
                        total_sent: overall_sent,
                        message: final_msg,
                    },
                );
            }
        }

        return Ok(ZaloOutreachBatchResult { total_sent });
    }

    // Real Zalo flow
    let session_res = {
        let conn_guard = conn_mutex.lock().unwrap();
        crate::db::get_zalo_session(&conn_guard, &imei)
    };
    let session = match session_res {
        Ok(Some(s)) => s,
        Ok(None) => {
            let msg = format!("Session not found for IMEI: {}", imei);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 0.0,
                        status: "failed".to_string(),
                        message: msg,
                    },
                );
            }
            return Err("Session not found".to_string());
        }
        Err(e) => {
            let msg = format!("Database error loading session: {}", e);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 0.0,
                        status: "failed".to_string(),
                        message: msg,
                    },
                );
            }
            return Err(e.to_string());
        }
    };

    let cookies: Vec<zca_rs::context::ZaloCookie> =
        match serde_json::from_str(&session.cookies_json) {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Failed to parse session cookies: {}", e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: 0.0,
                            status: "failed".to_string(),
                            message: msg,
                        },
                    );
                }
                return Err(e.to_string());
            }
        };

    let (template_content, template_image_url) = {
        let conn_guard = conn_mutex.lock().unwrap();
        let res = conn_guard.query_row(
            "SELECT content, image_url FROM outreach_templates WHERE id = ?1",
            [template_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        );
        match res {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("Failed to load outreach template: {}", e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: 0.0,
                            status: "failed".to_string(),
                            message: msg,
                        },
                    );
                }
                return Err(e.to_string());
            }
        }
    };

    let options = zalo_api_options();
    let zalo = match zca_rs::Zalo::login(
        session.imei.clone(),
        session.user_agent.clone(),
        cookies,
        options,
    )
    .await
    {
        Ok(z) => z,
        Err(e) => {
            let msg = format!("Zalo login failed: {:?}", e);
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: msg.clone(),
                    },
                );
                let _ = app.emit(
                    "outreach-progress",
                    OutreachProgressPayload {
                        progress: 0.0,
                        status: "failed".to_string(),
                        message: msg,
                    },
                );
            }
            return Err(format!("{:?}", e));
        }
    };

    let mut total_sent = 0;
    let mut burst_counter: u32 = 0;
    let mut was_rate_limited = false;
    for (idx, lead_id) in lead_ids.iter().enumerate() {
        let (lead_name, phone, lead_zalo_status, existing_zalo_uid) = {
            let conn_guard = conn_mutex.lock().unwrap();
            let res = conn_guard.query_row(
                "SELECT name, phone, zalo_status, zalo_uid FROM leads WHERE id = ?1",
                [lead_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            );
            match res {
                Ok(val) => val,
                Err(e) => {
                    let msg = format!("Failed to load lead {}: {}", lead_id, e);
                    if let Some(app) = app_handle {
                        let _ = app.emit("outreach-log", OutreachLogPayload { message: msg });
                    }
                    continue;
                }
            }
        };

        let sanitized_phone: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let cached_uid = sanitize_valid_zalo_uid(existing_zalo_uid.as_deref());

        let target_uid = if let Some(uid) = cached_uid {
            if let Some(ref zs) = lead_zalo_status {
                if zs == "BLOCKED_STRANGER" {
                    let msg = format!(
                        "Skipped Zalo outreach for {} (UID: {}): Previously marked as BLOCKED_STRANGER",
                        lead_name, uid
                    );
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }
                    continue;
                }
            }

            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: format!("Using cached Zalo UID for {} (UID: {})", lead_name, uid),
                    },
                );
            }
            uid
        } else {
            if let Some(ref zs) = lead_zalo_status {
                if matches!(
                    zs.as_str(),
                    "BLOCKED_STRANGER" | "VERIFIED_NOT_EXIST" | "NOT_FOUND" | "INVALID_PHONE"
                ) {
                    let msg = format!(
                        "Skipped Zalo lookup for {} ({}): Previously marked as {}",
                        lead_name, sanitized_phone, zs
                    );
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }
                    continue;
                }
            }

            if phone.trim().is_empty() {
                let msg = format!("Lead {} ({}) is missing phone number", lead_name, lead_id);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let current_num = processed_offset + idx + 1;
                    let progress_val = (current_num as f64 / display_total.max(1) as f64) * 100.0;
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: progress_val,
                            status: "running".to_string(),
                            message: msg,
                        },
                    );
                }
                continue;
            }

            if !is_vn_mobile(&phone) {
                let msg = format!("Skipping non-mobile number: {}", phone);
                println!("{}", msg);
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                        rusqlite::params![lead_id],
                    )
                };
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                    let current_num = processed_offset + idx + 1;
                    let progress_val = (current_num as f64 / display_total.max(1) as f64) * 100.0;
                    let _ = app.emit(
                        "outreach-progress",
                        OutreachProgressPayload {
                            progress: progress_val,
                            status: "running".to_string(),
                            message: msg,
                        },
                    );
                }
                continue;
            }

            if let Some(app) = app_handle {
                let _ = app.emit(
                    "outreach-log",
                    OutreachLogPayload {
                        message: format!(
                            "Looking up Zalo UID for {} ({})",
                            lead_name, sanitized_phone
                        ),
                    },
                );
            }

            let user_opt = match zalo.find_user(&sanitized_phone, None).await {
                Ok(opt) => opt,
                Err(e) => {
                    let msg = format!(
                        "Failed to look up phone {} for {}: {:?}",
                        sanitized_phone, lead_name, e
                    );
                    let now: String = chrono::Utc::now().to_rfc3339();
                    let err_str = format!("{:?}", e).to_lowercase();
                    let is_rate_limit = should_rotate_zalo_session(&e);

                    let status_str = if err_str.contains("không tìm thấy")
                        || err_str.contains("not found")
                        || err_str.contains("212")
                    {
                        "VERIFIED_NOT_EXIST"
                    } else if err_str.contains("chặn")
                        || err_str.contains("người lạ")
                        || err_str.contains("stranger")
                        || err_str.contains("privacy")
                        || err_str.contains("122")
                    {
                        "BLOCKED_STRANGER"
                    } else if is_rate_limit {
                        "RATE_LIMITED"
                    } else {
                        "LOOKUP_FAILED"
                    };

                    let conn_guard = conn_mutex.lock();
                    if let Ok(conn) = conn_guard {
                        let _ = conn.execute(
                            "UPDATE leads SET zalo_status = ?1 WHERE id = ?2",
                            rusqlite::params![status_str, lead_id],
                        );
                        if status_str == "VERIFIED_NOT_EXIST" {
                            let _ = conn.execute(
                                "UPDATE leads SET has_zalo = 0 WHERE id = ?1",
                                rusqlite::params![lead_id],
                            );
                        }
                        let _ = conn.execute(
                            "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'FAILED', ?3, ?4, ?5)",
                            rusqlite::params![lead_id, template_id, format!("{:?}", e), &now, &imei],
                        );
                    }

                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }

                    if should_rotate_zalo_session(&e) {
                        let rate_msg = format!("[Zalo Rate Limit Warning] Zalo lookup/message limit reached ({:?}). Pausing campaign...", e);
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "outreach-log",
                                OutreachLogPayload {
                                    message: rate_msg.clone(),
                                },
                            );
                            let current_num = processed_offset + idx + 1;
                            let progress_val =
                                (current_num as f64 / display_total.max(1) as f64) * 100.0;
                            let _ = app.emit(
                                "outreach-progress",
                                OutreachProgressPayload {
                                    progress: progress_val,
                                    status: "rate_limited".to_string(),
                                    message: rate_msg,
                                },
                            );
                        }
                        was_rate_limited = true;
                        break;
                    }

                    continue;
                }
            };

            match user_opt {
                Some(user) => {
                    if let Some(valid_uid) = sanitize_valid_zalo_uid(Some(&user.uid)) {
                        valid_uid
                    } else {
                        let msg = format!(
                            "Invalid Zalo UID returned for {} (phone: {}): {:?}",
                            lead_name, sanitized_phone, user.uid
                        );
                        let now: String = chrono::Utc::now().to_rfc3339();
                        let conn_guard = conn_mutex.lock();
                        if let Ok(conn) = conn_guard {
                            let _ = conn.execute(
                                "UPDATE leads SET zalo_status = 'LOOKUP_FAILED' WHERE id = ?1",
                                rusqlite::params![lead_id],
                            );
                            let _ = conn.execute(
                                "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'FAILED', ?3, ?4, ?5)",
                                rusqlite::params![lead_id, template_id, "Invalid Zalo UID from provider", &now, &imei],
                            );
                        }
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "outreach-log",
                                OutreachLogPayload {
                                    message: msg.clone(),
                                },
                            );
                        }
                        continue;
                    }
                }
                None => {
                    let msg = format!(
                        "Phone number {} for {} not found on Zalo",
                        sanitized_phone, lead_name
                    );
                    let now: String = chrono::Utc::now().to_rfc3339();
                    let conn_guard = conn_mutex.lock();
                    if let Ok(conn) = conn_guard {
                        let _ = conn.execute(
                            "UPDATE leads SET has_zalo = 0, zalo_status = 'VERIFIED_NOT_EXIST' WHERE id = ?1",
                            rusqlite::params![lead_id],
                        );
                        let _ = conn.execute(
                            "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'FAILED', ?3, ?4, ?5)",
                            rusqlite::params![lead_id, template_id, "Phone number not found on Zalo", &now, &imei],
                        );
                    }
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "running".to_string(),
                                message: msg,
                            },
                        );
                    }
                    continue;
                }
            }
        };

        let formatted_content = render_zalo_template(&template_content, &lead_name);

        let attachments = template_image_url
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .map(|url| {
                if url.starts_with("data:image/") {
                    use base64::Engine;
                    let parts: Vec<&str> = url.split(',').collect();
                    let base64_str = parts.get(1).copied().unwrap_or("");
                    let data = base64::engine::general_purpose::STANDARD
                        .decode(base64_str)
                        .unwrap_or_default();
                    let ext = if url.contains("image/png") {
                        "png"
                    } else if url.contains("image/webp") {
                        "webp"
                    } else if url.contains("image/gif") {
                        "gif"
                    } else {
                        "jpg"
                    };
                    vec![zca_rs::models::AttachmentSource::Buffer {
                        data: data.clone(),
                        filename: format!("attachment.{}", ext),
                        metadata: zca_rs::models::AttachmentMetadata {
                            total_size: data.len() as u64,
                            width: None,
                            height: None,
                        },
                    }]
                } else if url.starts_with("http://") || url.starts_with("https://") {
                    vec![zca_rs::models::AttachmentSource::Url(url.clone())]
                } else {
                    vec![zca_rs::models::AttachmentSource::Buffer {
                        data: std::fs::read(url).unwrap_or_default(),
                        filename: std::path::Path::new(url)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("image.jpg")
                            .to_string(),
                        metadata: zca_rs::models::AttachmentMetadata {
                            total_size: std::fs::metadata(url).map(|m| m.len()).unwrap_or(0),
                            width: None,
                            height: None,
                        },
                    }]
                }
            });

        let msg_content = zca_rs::models::MessageContent {
            msg: formatted_content.clone(),
            styles: None,
            urgency: None,
            quote: None,
            mentions: None,
            attachments,
            ttl: None,
        };

        let local_request_id = format!("outreach-{}", rand::random::<u128>());
        let conversation_sent_at = crate::conversation::types::utc_now();

        match zalo
            .send_message(msg_content, &target_uid, zca_rs::models::ThreadType::User)
            .await
        {
            Ok(response) => {
                let provider_receipt =
                    crate::conversation::outreach::ZaloProviderReceipt::from_response(response);
                total_sent += 1;
                burst_counter += 1;
                let log_msg = format!("Message sent to {} ({})", lead_name, target_uid);
                if let Some(app) = app_handle {
                    let _ = app.emit("outreach-log", OutreachLogPayload { message: log_msg });
                }

                let act_res: Result<crate::conversation::outreach::AppliedOutreachMessage, String> = {
                    let mut conn_guard = conn_mutex
                        .lock()
                        .map_err(|e| format!("Database lock error: {}", e))?;
                    let tx = conn_guard
                        .transaction()
                        .map_err(|e| format!("Database transaction error: {}", e))?;

                    let now: String = tx
                        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| {
                            row.get(0)
                        })
                        .map_err(|e| format!("Database query error: {}", e))?;

                    tx.execute(
                        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            lead_id,
                            "Outreach",
                            format!("Zalo message sent: {}", formatted_content),
                            &now,
                            "System (Zalo Outreach)"
                        ]
                    ).map_err(|e| format!("Database query error: {}", e))?;

                    tx.execute(
                        "UPDATE leads SET last_activity = ?1, has_zalo = 1, zalo_reached = 1, zalo_uid = ?2, zalo_status = 'VERIFIED_EXIST' WHERE id = ?3",
                        rusqlite::params![&now, &target_uid, lead_id],
                    )
                    .map_err(|e| format!("Database query error: {}", e))?;

                    tx.execute(
                        "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'SUCCESS', NULL, ?3, ?4)",
                        rusqlite::params![lead_id, template_id, &now, &imei],
                    )
                    .map_err(|e| format!("Database query error: {}", e))?;

                    let applied_outreach =
                        crate::conversation::outreach::record_outreach_success_in_transaction(
                            &tx,
                            &crate::conversation::outreach::OutreachConversationRecord {
                                lead_id: i64::from(*lead_id),
                                provider_account_key: imei.clone(),
                                account_display_name: None,
                                external_user_id: target_uid.clone(),
                                user_display_name: Some(lead_name.clone()),
                                user_avatar_url: None,
                                template_id: i64::from(template_id),
                                source: "zalo_outreach".to_string(),
                                text: formatted_content.clone(),
                                local_request_id,
                                provider_receipt,
                                sent_at: conversation_sent_at,
                            },
                        )
                        .map_err(|e| format!("Outreach conversation record error: {}", e))?;

                    tx.commit()
                        .map_err(|e| format!("Database transaction commit error: {}", e))?;
                    Ok(applied_outreach)
                };

                match act_res {
                    Ok(applied_outreach) => {
                        if let Some(app) = app_handle {
                            if let Some(state) =
                                app.try_state::<crate::conversation::commands::ConversationState>()
                            {
                                state.service.publish_outreach_success(&applied_outreach);
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "outreach-log",
                                OutreachLogPayload {
                                    message: format!(
                                        "Delivery succeeded but local persistence failed: {}",
                                        e
                                    ),
                                },
                            );
                        }
                    }
                }
            }
            Err(e) => {
                let msg = format!(
                    "Failed to send Zalo message to {} ({}): {:?}",
                    lead_name, target_uid, e
                );
                let now: String = chrono::Utc::now().to_rfc3339();
                let err_str = format!("{:?}", e).to_lowercase();
                let status_str = if err_str.contains("người lạ")
                    || err_str.contains("stranger")
                    || err_str.contains("privacy")
                    || err_str.contains("chặn")
                    || err_str.contains("122")
                {
                    "BLOCKED_STRANGER"
                } else if err_str.contains("không tìm thấy")
                    || err_str.contains("not found")
                    || err_str.contains("212")
                {
                    "VERIFIED_NOT_EXIST"
                } else if should_rotate_zalo_session(&e) {
                    "RATE_LIMITED"
                } else {
                    "SEND_FAILED"
                };

                let conn_guard = conn_mutex.lock();
                if let Ok(conn) = conn_guard {
                    let _ = conn.execute(
                        "UPDATE leads SET zalo_status = ?1 WHERE id = ?2",
                        rusqlite::params![status_str, lead_id],
                    );
                    let _ = conn.execute(
                        "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'FAILED', ?3, ?4, ?5)",
                        rusqlite::params![lead_id, template_id, format!("{:?}", e), &now, &imei],
                    );
                }
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "outreach-log",
                        OutreachLogPayload {
                            message: msg.clone(),
                        },
                    );
                }

                if should_rotate_zalo_session(&e) {
                    let rate_msg = format!("[Zalo Rate Limit Warning] Zalo message limit reached ({:?}). Pausing campaign...", e);
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "outreach-log",
                            OutreachLogPayload {
                                message: rate_msg.clone(),
                            },
                        );
                        let current_num = processed_offset + idx + 1;
                        let progress_val =
                            (current_num as f64 / display_total.max(1) as f64) * 100.0;
                        let _ = app.emit(
                            "outreach-progress",
                            OutreachProgressPayload {
                                progress: progress_val,
                                status: "rate_limited".to_string(),
                                message: rate_msg,
                            },
                        );
                    }
                    was_rate_limited = true;
                    break;
                }
            }
        }

        if let Some(app) = app_handle {
            let current_num = processed_offset + idx + 1;
            let progress_val = (current_num as f64 / display_total.max(1) as f64) * 100.0;
            let _ = app.emit(
                "outreach-progress",
                OutreachProgressPayload {
                    progress: progress_val,
                    status: "running".to_string(),
                    message: format!("Processed lead {}/{}", current_num, display_total),
                },
            );
        }

        if enable_burst_limit
            && burst_max_messages > 0
            && burst_counter >= burst_max_messages
            && (idx + 1) < lead_ids.len()
        {
            let msg = format!(
                "[Burst Limit] Sent {} messages. Pausing {} seconds to protect Zalo account...",
                burst_counter, burst_delay_seconds
            );
            if let Some(app) = app_handle {
                let _ = app.emit("outreach-log", OutreachLogPayload { message: msg });
            }
            tokio::time::sleep(std::time::Duration::from_secs(burst_delay_seconds)).await;
            burst_counter = 0;
        } else if idx + 1 < lead_ids.len() {
            if inter_message_delay_seconds > 0 {
                tokio::time::sleep(std::time::Duration::from_secs(inter_message_delay_seconds))
                    .await;
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            }
        }
    }

    let overall_sent = progress_scope.sent_offset + total_sent;
    let final_msg = if was_rate_limited {
        format!(
            "Outreach paused due to Zalo rate limit. Sent {} messages out of {} leads.",
            overall_sent, display_total
        )
    } else {
        format!(
            "Outreach complete! Sent {} messages out of {} leads.",
            overall_sent, display_total
        )
    };
    if let Some(app) = app_handle {
        if emit_complete {
            let _ = app.emit(
                "outreach-progress",
                OutreachProgressPayload {
                    progress: if was_rate_limited {
                        ((processed_offset + total_sent) as f64 / display_total.max(1) as f64)
                            * 100.0
                    } else {
                        100.0
                    },
                    status: if was_rate_limited {
                        "rate_limited".to_string()
                    } else {
                        "completed".to_string()
                    },
                    message: final_msg.clone(),
                },
            );
            let _ = app.emit(
                "outreach-complete",
                OutreachCompletePayload {
                    total_sent: overall_sent,
                    message: final_msg,
                },
            );
        }
    }

    Ok(ZaloOutreachBatchResult { total_sent })
}

/// Retrieves all categories from the database.
#[tauri::command]
pub fn get_categories(state: tauri::State<'_, DbState>) -> Result<Vec<Category>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_categories(&conn).map_err(|e| format!("Database error: {}", e))
}

/// Retrieves categories associated with a specific lead.
#[tauri::command]
pub fn get_categories_for_lead(
    state: tauri::State<'_, DbState>,
    lead_id: i32,
) -> Result<Vec<Category>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_categories_for_lead(&conn, lead_id).map_err(|e| format!("Database error: {}", e))
}

/// Creates a new category in the database.
///
/// Returns an error if the category name is empty or contains only whitespace.
#[tauri::command]
pub fn create_category(state: tauri::State<'_, DbState>, name: String) -> Result<i32, String> {
    if name.trim().is_empty() {
        return Err("Category name cannot be empty".to_string());
    }
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::create_category(&conn, &name).map_err(|e| format!("Database error: {}", e))
}

/// Deletes a category by its ID.
#[tauri::command]
pub fn delete_category(state: tauri::State<'_, DbState>, id: i32) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_category(&conn, id).map_err(|e| format!("Database error: {}", e))
}

/// Associates multiple leads with a category.
///
/// Returns early if the `lead_ids` list is empty.
#[tauri::command]
pub fn add_leads_to_category(
    state: tauri::State<'_, DbState>,
    lead_ids: Vec<i32>,
    category_id: i32,
) -> Result<(), String> {
    if lead_ids.is_empty() {
        return Ok(());
    }
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Transaction error: {}", e))?;

    for lead_id in lead_ids {
        crate::db::add_lead_to_category(&tx, lead_id, category_id)
            .map_err(|e| format!("Database error: {}", e))?;
    }

    tx.commit()
        .map_err(|e| format!("Transaction commit error: {}", e))?;
    Ok(())
}

/// Removes multiple leads from a category.
///
/// Returns early if the `lead_ids` list is empty.
#[tauri::command]
pub fn remove_leads_from_category(
    state: tauri::State<'_, DbState>,
    lead_ids: Vec<i32>,
    category_id: i32,
) -> Result<(), String> {
    if lead_ids.is_empty() {
        return Ok(());
    }
    let mut conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let tx = conn
        .transaction()
        .map_err(|e| format!("Transaction error: {}", e))?;

    for lead_id in lead_ids {
        crate::db::remove_lead_from_category(&tx, lead_id, category_id)
            .map_err(|e| format!("Database error: {}", e))?;
    }

    tx.commit()
        .map_err(|e| format!("Transaction commit error: {}", e))?;
    Ok(())
}

/// Retrieves all leads associated with a specific category.
#[tauri::command]
pub fn get_leads_by_category(
    state: tauri::State<'_, DbState>,
    category_id: i32,
) -> Result<Vec<Lead>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_leads_by_category(&conn, category_id)
        .map_err(|e| format!("Database error: {}", e))
}

#[derive(Clone, serde::Serialize)]
pub struct CampaignProgressPayload {
    pub progress: f64,
    pub status: String,
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct CampaignLogPayload {
    pub message: String,
}

#[derive(Clone, serde::Serialize)]
pub struct CampaignCompletePayload {
    pub total_sent: i32,
    pub message: String,
}

#[tauri::command]
pub async fn send_zalo_campaign(
    app_handle: tauri::AppHandle,
    category_id: i32,
    template_id: i32,
    session_imeis: Vec<String>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let _ = send_zalo_campaign_internal(
            Some(&app_handle_clone),
            &db_state.conn,
            category_id,
            template_id,
            session_imeis,
        )
        .await;
    });
    Ok(())
}

pub async fn send_zalo_campaign_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    category_id: i32,
    template_id: i32,
    session_imeis: Vec<String>,
) -> Result<(), String> {
    let mut is_mock = false;
    if session_imeis.iter().any(|i| i.starts_with("mock-")) {
        is_mock = true;
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "campaign-progress",
            CampaignProgressPayload {
                progress: 0.0,
                status: "running".to_string(),
                message: "Starting campaign...".to_string(),
            },
        );
    }

    let leads = {
        let conn_guard = conn_mutex.lock().unwrap();
        crate::db::get_leads_by_category(&conn_guard, category_id).unwrap_or_default()
    };

    if leads.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: "No leads found in category.".to_string(),
                },
            );
        }
        return Ok(());
    }

    let template_content = {
        let conn_guard = conn_mutex.lock().unwrap();
        conn_guard
            .query_row(
                "SELECT content FROM outreach_templates WHERE id = ?1",
                [template_id],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_default()
    };

    let mut eligible_leads = Vec::new();
    for lead in leads {
        let is_valid_phone = if is_mock {
            lead.phone == "not-found"
                || lead.phone == "mock-rate-limit"
                || lead.phone == "mock-send-fail"
                || is_vn_mobile(&lead.phone)
        } else {
            is_vn_mobile(&lead.phone)
        };
        if !is_valid_phone {
            continue;
        }
        if let Some(status) = &lead.zalo_status {
            if status == "VERIFIED_NOT_EXIST" {
                continue;
            }
            if status == "RATE_LIMITED" {
                if let Some(last_lookup) = &lead.last_lookup_attempt {
                    let recent = if let Ok(parsed_time) =
                        chrono::DateTime::parse_from_rfc3339(last_lookup)
                    {
                        let now = chrono::Utc::now();
                        let diff = now - parsed_time.with_timezone(&chrono::Utc);
                        diff.num_seconds() < 3600
                    } else {
                        false
                    };
                    if recent {
                        continue;
                    }
                }
            }
        }

        let has_success = {
            let conn_guard = conn_mutex.lock().unwrap();
            conn_guard
                .query_row(
                    "SELECT COUNT(*) FROM outreach_history WHERE template_id = ?1 AND lead_id = ?2 AND status = 'SUCCESS'",
                    rusqlite::params![template_id, lead.id.unwrap()],
                    |row| row.get::<_, i32>(0),
                )
                .unwrap_or(0) > 0
        };
        if has_success {
            continue;
        }

        eligible_leads.push(lead);
    }

    if eligible_leads.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: "No eligible leads to process.".to_string(),
                },
            );
        }
        return Ok(());
    }

    let mut active_clients: Vec<zca_rs::Zalo> = Vec::new();
    let mut active_imeis = Vec::new();

    if !is_mock {
        let mut sessions_to_login = Vec::new();
        {
            let conn_guard = conn_mutex.lock().unwrap();
            for imei in &session_imeis {
                if let Ok(Some(session)) = crate::db::get_zalo_session(&conn_guard, imei) {
                    sessions_to_login.push(session);
                }
            }
        }
        for session in sessions_to_login {
            if let Ok(cookies) = serde_json::from_str(&session.cookies_json) {
                let options = zalo_api_options();
                if let Ok(zalo) = zca_rs::Zalo::login(
                    session.imei.clone(),
                    session.user_agent.clone(),
                    cookies,
                    options,
                )
                .await
                {
                    active_clients.push(zalo);
                    active_imeis.push(session.imei.clone());
                }
            }
        }
    } else {
        for imei in &session_imeis {
            active_imeis.push(imei.clone());
        }
    }

    if active_imeis.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "campaign-log",
                CampaignLogPayload {
                    message: "No valid sessions available for campaign".to_string(),
                },
            );
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: 100.0,
                    status: "failed".to_string(),
                    message: "No valid sessions".to_string(),
                },
            );
        }
        return Err("No valid sessions".to_string());
    }

    let mut active_index = 0;
    let mut total_sent = 0;

    for (idx, lead) in eligible_leads.iter().enumerate() {
        if active_imeis.is_empty() {
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "campaign-log",
                    CampaignLogPayload {
                        message: "All sessions exhausted. Stopping campaign.".to_string(),
                    },
                );
            }
            break;
        }

        active_index %= active_imeis.len();
        let current_imei = active_imeis[active_index].clone();

        let sanitized_phone: String = lead.phone.chars().filter(|c| c.is_ascii_digit()).collect();
        let formatted_content = render_zalo_template(&template_content, &lead.name);

        let mut uid = lead.zalo_uid.clone();

        if uid.is_none() || uid.as_deref() == Some("") {
            let mut found = false;
            let mut lookup_failed = false;

            if is_mock {
                if lead.phone == "not-found" {
                    found = false;
                } else if lead.phone == "mock-rate-limit" {
                    lookup_failed = true;
                } else {
                    uid = Some(format!("uid-{}", sanitized_phone));
                    found = true;
                }
            } else {
                let zalo = &active_clients[active_index];
                match zalo.find_user(&sanitized_phone, None).await {
                    Ok(Some(u)) => {
                        uid = Some(u.uid);
                        found = true;
                    }
                    Ok(None) => {
                        found = false;
                    }
                    Err(_) => {
                        lookup_failed = true;
                    }
                }
            }

            let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

            if lookup_failed {
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET zalo_status = 'RATE_LIMITED', last_lookup_attempt = ?1 WHERE id = ?2",
                        rusqlite::params![now, lead.id.unwrap()],
                    )
                };
                let history = crate::models::OutreachHistory {
                    id: None,
                    lead_id: lead.id.unwrap(),
                    template_id,
                    status: "FAILED".to_string(),
                    error_message: Some("Rate limited or lookup error".to_string()),
                    sent_at: now,
                    session_imei: current_imei.clone(),
                };
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    crate::db::log_outreach_attempt(&conn_guard, &history)
                };

                // Evict failed session
                active_imeis.remove(active_index);
                if !is_mock {
                    active_clients.remove(active_index);
                }

                if active_imeis.is_empty() {
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "campaign-log",
                            CampaignLogPayload {
                                message: "All sessions exhausted. Stopping campaign.".to_string(),
                            },
                        );
                    }
                    break;
                }
                active_index %= active_imeis.len();

                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }

            if found {
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET zalo_status = 'VERIFIED_EXIST', zalo_uid = ?1, has_zalo = 1, last_lookup_attempt = ?2 WHERE id = ?3",
                        rusqlite::params![uid, now, lead.id.unwrap()],
                    )
                };
            } else {
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    conn_guard.execute(
                        "UPDATE leads SET zalo_status = 'VERIFIED_NOT_EXIST', has_zalo = 0, last_lookup_attempt = ?1 WHERE id = ?2",
                        rusqlite::params![now, lead.id.unwrap()],
                    )
                };

                let history = crate::models::OutreachHistory {
                    id: None,
                    lead_id: lead.id.unwrap(),
                    template_id,
                    status: "FAILED".to_string(),
                    error_message: Some("User not found on Zalo".to_string()),
                    sent_at: now,
                    session_imei: current_imei.clone(),
                };
                let _ = {
                    let conn_guard = conn_mutex.lock().unwrap();
                    crate::db::log_outreach_attempt(&conn_guard, &history)
                };

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "campaign-log",
                        CampaignLogPayload {
                            message: format!("Lead {} not found on Zalo.", lead.name),
                        },
                    );
                }
                continue;
            }
        }

        let local_request_id = format!("campaign-{}", rand::random::<u128>());
        let send_result: Result<
            crate::conversation::outreach::ZaloProviderReceipt,
            zca_rs::ZaloError,
        > = if is_mock {
            if current_imei == "mock-fail-imei" {
                Err(zca_rs::ZaloError::ApiError {
                    message: "Mock session expired".to_string(),
                    code: Some(102),
                })
            } else if lead.phone == "mock-send-fail" {
                Err(zca_rs::ZaloError::PayloadFormat(
                    "Mock lead send failure".to_string(),
                ))
            } else {
                Ok(crate::conversation::outreach::ZaloProviderReceipt::mock(
                    &local_request_id,
                ))
            }
        } else {
            let zalo = &active_clients[active_index];
            let msg_content = zca_rs::models::MessageContent {
                msg: formatted_content.clone(),
                styles: None,
                urgency: None,
                quote: None,
                mentions: None,
                attachments: None,
                ttl: None,
            };
            if let Some(u) = &uid {
                zalo.send_message(msg_content, u, zca_rs::models::ThreadType::User)
                    .await
                    .map(crate::conversation::outreach::ZaloProviderReceipt::from_response)
            } else {
                Err(zca_rs::ZaloError::PayloadFormat(
                    "Missing Zalo UID for message send".to_string(),
                ))
            }
        };

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        match send_result {
            Ok(provider_receipt) => {
                total_sent += 1;
                let external_user_id = uid.clone().unwrap_or_default();
                let lead_id = lead.id.unwrap();
                let conversation_sent_at = now.clone();

                let act_res = {
                    let mut conn_guard = conn_mutex.lock().unwrap();
                    let tx_res = conn_guard.transaction();
                    match tx_res {
                        Ok(tx) => {
                            if let Err(e) = tx.execute(
                                "INSERT INTO outreach_history (lead_id, template_id, status, error_message, sent_at, session_imei) VALUES (?1, ?2, 'SUCCESS', NULL, ?3, ?4)",
                                rusqlite::params![lead_id, template_id, &now, &current_imei],
                            ) {
                                return Err(format!("Outreach history log error: {}", e));
                            }
                            if let Err(e) = tx.execute(
                                "UPDATE leads SET zalo_reached = 1, has_zalo = 1, last_activity = ?1 WHERE id = ?2",
                                rusqlite::params![now, lead_id],
                            ) {
                                return Err(format!("Lead update error: {}", e));
                            }

                            let applied_res = crate::conversation::outreach::record_outreach_success_in_transaction(
                                &tx,
                                &crate::conversation::outreach::OutreachConversationRecord {
                                    lead_id: i64::from(lead_id),
                                    provider_account_key: current_imei.clone(),
                                    account_display_name: None,
                                    external_user_id,
                                    user_display_name: Some(lead.name.clone()),
                                    user_avatar_url: None,
                                    template_id: i64::from(template_id),
                                    source: "zalo_campaign".to_string(),
                                    text: formatted_content.clone(),
                                    local_request_id,
                                    provider_receipt,
                                    sent_at: conversation_sent_at,
                                },
                            );

                            match applied_res {
                                Ok(applied) => match tx.commit() {
                                    Ok(()) => Ok(applied),
                                    Err(e) => {
                                        Err(format!("Database transaction commit error: {}", e))
                                    }
                                },
                                Err(e) => Err(format!("Outreach conversation record error: {}", e)),
                            }
                        }
                        Err(e) => Err(format!("Failed to begin transaction: {}", e)),
                    }
                };

                match act_res {
                    Ok(applied_outreach) => {
                        if let Some(app) = app_handle {
                            if let Some(state) =
                                app.try_state::<crate::conversation::commands::ConversationState>()
                            {
                                state.service.publish_outreach_success(&applied_outreach);
                            }
                        }
                    }
                    Err(e) => {
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "campaign-log",
                                CampaignLogPayload {
                                    message: format!(
                                        "Delivery succeeded for {} but local persistence failed: {}",
                                        lead.name, e
                                    ),
                                },
                            );
                        }
                    }
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "campaign-log",
                        CampaignLogPayload {
                            message: format!(
                                "Sent to {} using session {}",
                                lead.name, current_imei
                            ),
                        },
                    );
                }

                if !is_mock {
                    use rand::Rng;
                    let sleep_ms = {
                        let mut rng = rand::thread_rng();
                        rng.gen_range(8000..=12000)
                    };
                    tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)).await;
                } else {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }

                active_index = (active_index + 1) % active_imeis.len();
            }
            Err(send_error) => {
                let send_error_message = send_error.to_string();
                let rotate_session = should_rotate_zalo_session(&send_error);
                let history = crate::models::OutreachHistory {
                    id: None,
                    lead_id: lead.id.unwrap(),
                    template_id,
                    status: "FAILED".to_string(),
                    error_message: Some(send_error_message.clone()),
                    sent_at: now,
                    session_imei: current_imei.clone(),
                };
                {
                    let conn_guard = conn_mutex.lock().unwrap();
                    let _ = crate::db::log_outreach_attempt(&conn_guard, &history);
                    let _ = conn_guard.execute(
                        "UPDATE leads SET zalo_reached = 0 WHERE id = ?1",
                        rusqlite::params![lead.id.unwrap()],
                    );
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "campaign-log",
                        CampaignLogPayload {
                            message: format!(
                                "Failed sending to {} using session {}: {}",
                                lead.name, current_imei, send_error_message
                            ),
                        },
                    );
                }

                if rotate_session {
                    active_imeis.remove(active_index);
                    if !is_mock {
                        active_clients.remove(active_index);
                    }

                    if active_imeis.is_empty() {
                        if let Some(app) = app_handle {
                            let _ = app.emit(
                                "campaign-log",
                                CampaignLogPayload {
                                    message: "All sessions exhausted. Stopping campaign."
                                        .to_string(),
                                },
                            );
                        }
                        break;
                    }
                    active_index %= active_imeis.len();
                } else {
                    active_index = (active_index + 1) % active_imeis.len();
                }

                let cooldown_ms = if is_mock { 10 } else { 5000 };
                tokio::time::sleep(tokio::time::Duration::from_millis(cooldown_ms)).await;
            }
        }

        if let Some(app) = app_handle {
            let progress_val = ((idx + 1) as f64 / eligible_leads.len() as f64) * 100.0;
            let _ = app.emit(
                "campaign-progress",
                CampaignProgressPayload {
                    progress: progress_val,
                    status: "running".to_string(),
                    message: format!("Processed {}/{} leads", idx + 1, eligible_leads.len()),
                },
            );
        }
    }

    let final_msg = format!("Campaign completed. Total sent: {}", total_sent);
    if let Some(app) = app_handle {
        let _ = app.emit(
            "campaign-progress",
            CampaignProgressPayload {
                progress: 100.0,
                status: "completed".to_string(),
                message: final_msg.clone(),
            },
        );
        let _ = app.emit(
            "campaign-complete",
            CampaignCompletePayload {
                total_sent,
                message: final_msg,
            },
        );
    }

    Ok(())
}

#[tauri::command]
pub async fn get_campaign_leads(
    state: tauri::State<'_, DbState>,
    category_id: i32,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: Option<String>,
) -> Result<Vec<crate::models::Lead>, String> {
    let conn = state.conn.lock().unwrap();
    let is_email = channel.as_deref() == Some("email");

    let query_str = match segmentation.as_str() {
        "never_sent" => {
            if is_email {
                "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 WHERE lc.category_id = ?1
                   AND l.id NOT IN (SELECT DISTINCT lead_id FROM email_outreach_history WHERE status = 'SUCCESS')"
            } else {
                "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 WHERE lc.category_id = ?1
                   AND COALESCE(l.zalo_reached, 0) = 0
                   AND COALESCE(l.zalo_status, '') NOT IN ('BLOCKED_STRANGER', 'VERIFIED_NOT_EXIST', 'NOT_FOUND', 'INVALID_PHONE')
                   AND l.id NOT IN (SELECT DISTINCT lead_id FROM outreach_history WHERE status = 'SUCCESS')"
            }
        },
        "template_success" => {
            if filter_template_id.is_none() {
                return Err("Template ID is required for success filtering".to_string());
            }
            if is_email {
                "SELECT DISTINCT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 JOIN email_outreach_history oh ON l.id = oh.lead_id
                 WHERE lc.category_id = ?1
                   AND oh.template_id = ?2
                   AND oh.status = 'SUCCESS'"
            } else {
                "SELECT DISTINCT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 JOIN outreach_history oh ON l.id = oh.lead_id
                 WHERE lc.category_id = ?1
                   AND oh.template_id = ?2
                   AND oh.status = 'SUCCESS'"
            }
        },
        "last_failed" => {
            if is_email {
                "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 WHERE lc.category_id = ?1
                   AND l.id IN (
                       SELECT lead_id FROM (
                           SELECT lead_id, status, MAX(sent_at) FROM email_outreach_history GROUP BY lead_id
                       ) WHERE status = 'FAILED'
                   )"
            } else {
                "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
                 JOIN lead_categories lc ON l.id = lc.lead_id
                 WHERE lc.category_id = ?1
                   AND l.id IN (
                       SELECT lead_id FROM (
                           SELECT lead_id, status, MAX(sent_at) FROM outreach_history GROUP BY lead_id
                       ) WHERE status = 'FAILED'
                   )"
            }
        },
        _ => {
            "SELECT l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency FROM leads l
             JOIN lead_categories lc ON l.id = lc.lead_id
             WHERE lc.category_id = ?1"
        }
    };

    let mut leads = Vec::new();
    if segmentation == "template_success" {
        let mut stmt = conn.prepare(query_str).map_err(|e| e.to_string())?;
        let lead_rows = stmt
            .query_map(
                rusqlite::params![category_id, filter_template_id.unwrap()],
                |row| {
                    Ok(crate::models::Lead {
                        id: row.get(0)?,
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
                },
            )
            .map_err(|e| e.to_string())?;
        for row in lead_rows {
            leads.push(row.map_err(|e| e.to_string())?);
        }
    } else {
        let mut stmt = conn.prepare(query_str).map_err(|e| e.to_string())?;
        let lead_rows = stmt
            .query_map([category_id], |row| {
                Ok(crate::models::Lead {
                    id: row.get(0)?,
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
            })
            .map_err(|e| e.to_string())?;
        for row in lead_rows {
            leads.push(row.map_err(|e| e.to_string())?);
        }
    }

    Ok(leads)
}

#[cfg(test)]
pub fn get_smtp_servers(state: tauri::State<DbState>) -> Result<Vec<SmtpServer>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_smtp_servers(&conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn save_smtp_server(server: SmtpServer, state: tauri::State<DbState>) -> Result<i32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::save_smtp_server(&conn, &server).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_smtp_server(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_smtp_server(&conn, id).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub async fn test_smtp_connection(host: String, port: i32) -> Result<String, String> {
    if port <= 0 || port > 65535 {
        return Err("Invalid port number".to_string());
    }
    let addr = format!("{}:{}", host, port);
    let connect_timeout = std::time::Duration::from_secs(5);
    match tokio::time::timeout(connect_timeout, tokio::net::TcpStream::connect(&addr)).await {
        Ok(Ok(_stream)) => Ok(format!("Successfully connected to {}:{}", host, port)),
        Ok(Err(e)) => Err(format!("Failed to connect to {}:{}: {}", host, port, e)),
        Err(_) => Err(format!("Connection to {}:{} timed out", host, port)),
    }
}

#[tauri::command]
pub fn get_email_templates(state: tauri::State<DbState>) -> Result<Vec<EmailTemplate>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::get_email_templates(&conn).map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn create_email_template(
    name: String,
    subject: String,
    content: String,
    state: tauri::State<DbState>,
) -> Result<i32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::create_email_template(&conn, &name, &subject, &content)
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn update_email_template(
    id: i32,
    name: String,
    subject: String,
    content: String,
    state: tauri::State<DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::update_email_template(&conn, id, &name, &subject, &content)
        .map_err(|e| format!("Database error: {}", e))
}

#[tauri::command]
pub fn delete_email_template(id: i32, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;
    crate::db::delete_email_template(&conn, id).map_err(|e| format!("Database error: {}", e))
}

pub fn replace_email_template_variables(template: &str, lead: &Lead) -> String {
    template
        .replace("{{name}}", &lead.name)
        .replace("{{company}}", &lead.company)
        .replace("{{email}}", &lead.email)
        .replace("{{phone}}", &lead.phone)
}

#[tauri::command]
pub async fn send_email_outreach(
    app_handle: tauri::AppHandle,
    smtp_id: i32,
    template_id: i32,
    lead_ids: Vec<i32>,
) -> Result<(), String> {
    let app_handle_clone = app_handle.clone();
    tokio::spawn(async move {
        let db_state = app_handle_clone.state::<DbState>();
        let _ = send_email_outreach_internal(
            Some(&app_handle_clone),
            &db_state.conn,
            smtp_id,
            template_id,
            lead_ids,
        )
        .await;
    });
    Ok(())
}

pub async fn send_email_outreach_internal(
    app_handle: Option<&tauri::AppHandle>,
    conn_mutex: &std::sync::Mutex<Connection>,
    smtp_id: i32,
    template_id: i32,
    lead_ids: Vec<i32>,
) -> Result<(), String> {
    use crate::models::{
        EmailOutreachCompletePayload, EmailOutreachHistory, EmailOutreachLogPayload,
        EmailOutreachProgressPayload,
    };
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    if let Some(app) = app_handle {
        let _ = app.emit(
            "email-outreach-progress",
            EmailOutreachProgressPayload {
                progress: 0.0,
                status: "running".to_string(),
                message: "Starting email outreach...".to_string(),
            },
        );
    }

    let (smtp_server, template) = {
        let conn_guard = conn_mutex
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        let smtp = crate::db::get_smtp_server_by_id(&conn_guard, smtp_id)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("SMTP server with ID {} not found", smtp_id))?;
        let tpl = crate::db::get_email_template_by_id(&conn_guard, template_id)
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| format!("Email template with ID {} not found", template_id))?;
        (smtp, tpl)
    };

    if lead_ids.is_empty() {
        if let Some(app) = app_handle {
            let _ = app.emit(
                "email-outreach-progress",
                EmailOutreachProgressPayload {
                    progress: 100.0,
                    status: "completed".to_string(),
                    message: "No leads selected.".to_string(),
                },
            );
        }
        return Ok(());
    }

    let mut total_sent = 0;

    for (idx, &lead_id) in lead_ids.iter().enumerate() {
        let lead_res = {
            let conn_guard = conn_mutex.lock().unwrap();
            get_lead_details_internal(&conn_guard, lead_id)
        };

        let lead = match lead_res {
            Ok(l) => l,
            Err(e) => {
                let err_msg = format!("Lead ID {} not found: {}", lead_id, e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload {
                            message: err_msg.clone(),
                        },
                    );
                }
                continue;
            }
        };

        let trimmed_email = lead.email.trim();
        if trimmed_email.is_empty() || !trimmed_email.contains('@') {
            let log_msg = format!(
                "Skipping lead '{}' (ID {}): No valid email address",
                lead.name, lead_id
            );
            let history = EmailOutreachHistory {
                id: None,
                lead_id,
                template_id,
                smtp_id,
                status: "SKIPPED".to_string(),
                error_message: Some("No valid email address".to_string()),
                sent_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            };
            {
                let conn_guard = conn_mutex.lock().unwrap();
                let _ = crate::db::log_email_outreach_attempt(&conn_guard, &history);
            }
            if let Some(app) = app_handle {
                let _ = app.emit(
                    "email-outreach-log",
                    EmailOutreachLogPayload { message: log_msg },
                );
                let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
                let _ = app.emit(
                    "email-outreach-progress",
                    EmailOutreachProgressPayload {
                        progress: progress_val,
                        status: "running".to_string(),
                        message: format!("Processed {}/{} leads", idx + 1, lead_ids.len()),
                    },
                );
            }
            continue;
        }

        let interpolated_subject = replace_email_template_variables(&template.subject, &lead);
        let interpolated_body = replace_email_template_variables(&template.content, &lead);

        let from_str = if smtp_server.from_name.trim().is_empty() {
            smtp_server.from_email.clone()
        } else {
            format!("{} <{}>", smtp_server.from_name, smtp_server.from_email)
        };

        let to_str = if lead.name.trim().is_empty() {
            trimmed_email.to_string()
        } else {
            format!("{} <{}>", lead.name, trimmed_email)
        };

        let from_header = match from_str.parse() {
            Ok(h) => h,
            Err(e) => {
                let err_msg = format!("Invalid from header: {}", e);
                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload { message: err_msg },
                    );
                }
                continue;
            }
        };

        let to_header = match to_str.parse() {
            Ok(h) => h,
            Err(_) => match trimmed_email.parse() {
                Ok(h) => h,
                Err(e) => {
                    let err_msg =
                        format!("Invalid recipient email address for {}: {}", lead.name, e);
                    if let Some(app) = app_handle {
                        let _ = app.emit(
                            "email-outreach-log",
                            EmailOutreachLogPayload { message: err_msg },
                        );
                    }
                    continue;
                }
            },
        };

        let email_builder = Message::builder()
            .from(from_header)
            .to(to_header)
            .subject(interpolated_subject)
            .header(ContentType::TEXT_HTML)
            .body(interpolated_body);

        let send_result = match email_builder {
            Ok(email_msg) => {
                let security_upper = smtp_server.security.to_uppercase();
                let creds =
                    Credentials::new(smtp_server.username.clone(), smtp_server.password.clone());

                let transport_res: Result<AsyncSmtpTransport<Tokio1Executor>, String> =
                    if security_upper == "NONE" {
                        Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(
                            &smtp_server.host,
                        )
                        .port(smtp_server.port as u16)
                        .credentials(creds)
                        .build())
                    } else if security_upper == "SSL_TLS"
                        || security_upper == "SSL/TLS"
                        || security_upper == "TLS"
                        || security_upper == "SSL"
                    {
                        AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp_server.host)
                            .map_err(|e| format!("SMTP TLS relay error: {}", e))
                            .map(|b| b.port(smtp_server.port as u16).credentials(creds).build())
                    } else {
                        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp_server.host)
                            .map_err(|e| format!("SMTP STARTTLS relay error: {}", e))
                            .map(|b| b.port(smtp_server.port as u16).credentials(creds).build())
                    };

                match transport_res {
                    Ok(transport) => transport.send(email_msg).await.map_err(|e| e.to_string()),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(format!("Failed to build email message: {}", e)),
        };

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        match send_result {
            Ok(_) => {
                total_sent += 1;
                let history = EmailOutreachHistory {
                    id: None,
                    lead_id,
                    template_id,
                    smtp_id,
                    status: "SUCCESS".to_string(),
                    error_message: None,
                    sent_at: now.clone(),
                };
                {
                    let conn_guard = conn_mutex.lock().unwrap();
                    let _ = crate::db::log_email_outreach_attempt(&conn_guard, &history);
                    let _ = conn_guard.execute(
                        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            lead_id,
                            "Email Outreach",
                            format!("Sent email template '{}' to {}", template.name, lead.email),
                            &now,
                            "System"
                        ],
                    );
                    let _ = conn_guard.execute(
                        "UPDATE leads SET last_activity = ?1 WHERE id = ?2",
                        rusqlite::params![&now, lead_id],
                    );
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload {
                            message: format!(
                                "Successfully sent email to {} ({})",
                                lead.name, lead.email
                            ),
                        },
                    );
                }
            }
            Err(err_msg) => {
                let history = EmailOutreachHistory {
                    id: None,
                    lead_id,
                    template_id,
                    smtp_id,
                    status: "FAILED".to_string(),
                    error_message: Some(err_msg.clone()),
                    sent_at: now.clone(),
                };
                {
                    let conn_guard = conn_mutex.lock().unwrap();
                    let _ = crate::db::log_email_outreach_attempt(&conn_guard, &history);
                    let _ = conn_guard.execute(
                        "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
                        rusqlite::params![
                            lead_id,
                            "Email Outreach",
                            format!("Failed to send email to {}: {}", lead.email, err_msg),
                            &now,
                            "System"
                        ],
                    );
                }

                if let Some(app) = app_handle {
                    let _ = app.emit(
                        "email-outreach-log",
                        EmailOutreachLogPayload {
                            message: format!(
                                "Failed sending email to {} ({}): {}",
                                lead.name, lead.email, err_msg
                            ),
                        },
                    );
                }
            }
        }

        if let Some(app) = app_handle {
            let progress_val = ((idx + 1) as f64 / lead_ids.len() as f64) * 100.0;
            let _ = app.emit(
                "email-outreach-progress",
                EmailOutreachProgressPayload {
                    progress: progress_val,
                    status: "running".to_string(),
                    message: format!("Processed {}/{} leads", idx + 1, lead_ids.len()),
                },
            );
        }
    }

    if let Some(app) = app_handle {
        let _ = app.emit(
            "email-outreach-complete",
            EmailOutreachCompletePayload {
                total_sent,
                message: format!("Email outreach completed. {} emails sent.", total_sent),
            },
        );
    }

    Ok(())
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupCategoryInfo {
    pub id: i32,
    pub name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupMemberDetail {
    pub identity_id: Option<i64>,
    pub external_id: Option<String>,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub participant_role: String,
    pub lead_id: Option<i32>,
    pub lead_name: Option<String>,
    pub categories: Vec<GroupCategoryInfo>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GroupMemberScanResult {
    pub members: Vec<GroupMemberDetail>,
    pub discovered_count: usize,
    pub profile_hydrated_count: usize,
    pub membership_complete: bool,
    pub pages_attempted: usize,
    pub pages_succeeded: usize,
    pub messages_scanned: usize,
    pub stop_reason: Option<String>,
    pub warning: Option<String>,
    pub diagnostics: GroupMemberScanDiagnostics,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberScanWarning {
    pub stage: String,
    pub code: String,
    pub context: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberDirectSnapshotDiagnostics {
    pub attempted: bool,
    pub complete: bool,
    pub member_count: usize,
    pub warning: Option<GroupMemberScanWarning>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberBoardDiagnostics {
    pub attempted: bool,
    pub pages_attempted: usize,
    pub pages_succeeded: usize,
    pub items_inspected: usize,
    pub polls_attempted: usize,
    pub polls_succeeded: usize,
    pub committed_uid_count: usize,
    pub stop_reason: Option<String>,
    pub warnings: Vec<crate::conversation::zalo::client::GroupBoardWarning>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberLocalEvidenceDiagnostics {
    pub messages_scanned: usize,
    pub participant_uid_count: usize,
    pub sender_uid_count: usize,
    pub mention_uid_count: usize,
    pub reaction_uid_count: usize,
    pub quote_owner_uid_count: usize,
    pub total_uid_count: usize,
    pub committed_uid_count: usize,
    pub rejected_uid_count: usize,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberGroupLinkDiagnostics {
    pub attempted: bool,
    pub pages_attempted: usize,
    pub pages_succeeded: usize,
    pub items_inspected: usize,
    pub committed_uid_count: usize,
    pub link_retrieved: bool,
    pub warning: Option<GroupMemberScanWarning>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberProfileHydrationDiagnostics {
    pub attempted: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub warnings: Vec<GroupMemberScanWarning>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMemberScanDiagnostics {
    pub direct_snapshot: GroupMemberDirectSnapshotDiagnostics,
    pub group_link: GroupMemberGroupLinkDiagnostics,
    pub board: GroupMemberBoardDiagnostics,
    pub history_backfill: crate::conversation::zalo::supervisor::GroupMessageBackfillReport,
    pub local_evidence: GroupMemberLocalEvidenceDiagnostics,
    pub profile_hydration: GroupMemberProfileHydrationDiagnostics,
    pub partial_result: bool,
    pub warnings: Vec<GroupMemberScanWarning>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ImportMemberItem {
    pub identity_id: Option<i64>,
    pub external_id: Option<String>,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub phone: Option<String>,
}

#[async_trait::async_trait]
pub(crate) trait GroupMemberScanRuntime: Send + Sync {
    async fn connected_client(
        &self,
        provider_account_key: &str,
    ) -> Option<std::sync::Arc<dyn crate::conversation::zalo::client::ZaloClient>>;

    async fn backfill_group_messages(
        &self,
        provider_account_key: &str,
        timeout: std::time::Duration,
    ) -> crate::conversation::zalo::supervisor::GroupMessageBackfillReport;
}

#[async_trait::async_trait]
impl GroupMemberScanRuntime for crate::conversation::zalo::supervisor::ZaloSessionSupervisor {
    async fn connected_client(
        &self,
        provider_account_key: &str,
    ) -> Option<std::sync::Arc<dyn crate::conversation::zalo::client::ZaloClient>> {
        crate::conversation::zalo::supervisor::ZaloSessionSupervisor::connected_client(
            self,
            provider_account_key,
        )
        .await
    }

    async fn backfill_group_messages(
        &self,
        provider_account_key: &str,
        timeout: std::time::Duration,
    ) -> crate::conversation::zalo::supervisor::GroupMessageBackfillReport {
        crate::conversation::zalo::supervisor::ZaloSessionSupervisor::backfill_group_messages(
            self,
            provider_account_key,
            timeout,
        )
        .await
    }
}

struct CollectedGroupMemberEvidence {
    external_ids: Vec<String>,
    diagnostics: GroupMemberLocalEvidenceDiagnostics,
}

fn normalized_group_member_uid(value: &str) -> Option<String> {
    let normalized = value.trim();
    let lowercase = normalized.to_ascii_lowercase();
    (!normalized.is_empty() && !matches!(lowercase.as_str(), "0" | "-1" | "null" | "undefined"))
        .then(|| normalized.to_string())
}

fn collect_group_member_local_evidence(
    conn: &rusqlite::Connection,
    conversation_id: i64,
) -> Result<CollectedGroupMemberEvidence, String> {
    let mut statement = conn
        .prepare(
            "SELECT source, external_id
             FROM (
                 SELECT 'participant' AS source, ci.external_id AS external_id
                 FROM conversation_participants cp
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE cp.conversation_id = ?1 AND cp.left_at IS NULL

                 UNION ALL

                 SELECT 'sender', ci.external_id
                 FROM messages m
                 JOIN channel_identities ci ON ci.id = m.sender_identity_id
                 WHERE m.conversation_id = ?1 AND m.sender_identity_id IS NOT NULL

                 UNION ALL

                 SELECT 'mention', COALESCE(ci.external_id, mm.external_id)
                 FROM message_mentions mm
                 JOIN messages m ON m.id = mm.message_id
                 LEFT JOIN channel_identities ci ON ci.id = mm.identity_id
                 WHERE m.conversation_id = ?1

                 UNION ALL

                 SELECT 'reaction', mr.actor_external_id
                 FROM message_reactions mr
                 JOIN messages m ON m.id = mr.message_id
                 WHERE m.conversation_id = ?1 AND mr.removed_at IS NULL

                 UNION ALL

                 SELECT 'quote_owner', ci.external_id
                 FROM messages m
                 JOIN channel_identities ci ON ci.id = m.reply_sender_identity_id
                 WHERE m.conversation_id = ?1 AND m.reply_sender_identity_id IS NOT NULL
             )
             WHERE external_id IS NOT NULL AND TRIM(external_id) <> ''",
        )
        .map_err(|error| format!("Database prepare error: {error}"))?;
    let rows = statement
        .query_map([conversation_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("Database query error: {error}"))?;

    let mut participants = HashSet::new();
    let mut senders = HashSet::new();
    let mut mentions = HashSet::new();
    let mut reactions = HashSet::new();
    let mut quote_owners = HashSet::new();
    let mut all = HashSet::new();
    let mut rejected_uid_count = 0;

    for row in rows {
        let (source, external_id) = row.map_err(|error| format!("Row parse error: {error}"))?;
        let Some(external_id) = normalized_group_member_uid(&external_id) else {
            rejected_uid_count += 1;
            continue;
        };
        match source.as_str() {
            "participant" => {
                participants.insert(external_id.clone());
            }
            "sender" => {
                senders.insert(external_id.clone());
            }
            "mention" => {
                mentions.insert(external_id.clone());
            }
            "reaction" => {
                reactions.insert(external_id.clone());
            }
            "quote_owner" => {
                quote_owners.insert(external_id.clone());
            }
            _ => {}
        }
        all.insert(external_id);
    }

    let messages_scanned = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
            [conversation_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("Failed to count local group messages: {error}"))?
        .try_into()
        .map_err(|_| "Local message count exceeds platform limits".to_string())?;
    let mut external_ids = all.into_iter().collect::<Vec<_>>();
    external_ids.sort();

    Ok(CollectedGroupMemberEvidence {
        diagnostics: GroupMemberLocalEvidenceDiagnostics {
            messages_scanned,
            participant_uid_count: participants.len(),
            sender_uid_count: senders.len(),
            mention_uid_count: mentions.len(),
            reaction_uid_count: reactions.len(),
            quote_owner_uid_count: quote_owners.len(),
            total_uid_count: external_ids.len(),
            committed_uid_count: 0,
            rejected_uid_count,
        },
        external_ids,
    })
}

fn collect_group_member_external_ids(
    conn: &rusqlite::Connection,
    conversation_id: i64,
) -> Result<Vec<String>, String> {
    Ok(collect_group_member_local_evidence(conn, conversation_id)?.external_ids)
}

pub fn fetch_group_members_impl(
    conn: &rusqlite::Connection,
    conversation_id: i64,
) -> Result<Vec<GroupMemberDetail>, String> {
    let external_ids = collect_group_member_external_ids(conn, conversation_id)?;
    if external_ids.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (0..external_ids.len())
        .map(|index| format!("?{}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "WITH ranked_identities AS (
             SELECT
                 ci.id,
                 ci.external_id,
                 ci.display_name,
                 ci.avatar_url,
                 ci.lead_id,
                 ROW_NUMBER() OVER (
                     PARTITION BY ci.external_id
                     ORDER BY
                         CASE ci.identity_namespace WHEN 'zalo_uid' THEN 0 WHEN 'user' THEN 1 ELSE 2 END,
                         CASE WHEN ci.display_name IS NOT NULL AND TRIM(ci.display_name) <> '' THEN 0 ELSE 1 END,
                         ci.id
                 ) AS identity_rank
             FROM channel_identities ci
             WHERE ci.channel = 'zalo' AND ci.external_id IN ({placeholders})
         )
         SELECT
             ci.id,
             ci.external_id,
             COALESCE(NULLIF(TRIM(ci.display_name), ''), 'Zalo Member'),
             ci.avatar_url,
             COALESCE((
                 SELECT cp.participant_role
                 FROM conversation_participants cp
                 JOIN channel_identities role_identity ON role_identity.id = cp.identity_id
                 WHERE cp.conversation_id = ?1 AND role_identity.external_id = ci.external_id
                 ORDER BY
                     CASE WHEN cp.left_at IS NULL THEN 0 ELSE 1 END,
                     CASE cp.participant_role WHEN 'admin' THEN 0 WHEN 'deputy' THEN 1 ELSE 2 END
                 LIMIT 1
             ), 'member') AS participant_role,
             l.id,
             l.name
         FROM ranked_identities ci
         LEFT JOIN leads l ON l.id = ci.lead_id
         WHERE ci.identity_rank = 1
         ORDER BY
             CASE participant_role WHEN 'admin' THEN 1 WHEN 'deputy' THEN 2 ELSE 3 END,
             COALESCE(NULLIF(TRIM(ci.display_name), ''), 'Zalo Member') ASC"
    );

    let mut values = Vec::<Value>::with_capacity(external_ids.len() + 1);
    values.push(conversation_id.into());
    values.extend(external_ids.into_iter().map(Value::Text));

    let mut stmt = conn
        .prepare(&query)
        .map_err(|error| format!("Database prepare error: {error}"))?;

    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, Option<i64>>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, Option<i32>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(|e| format!("Database query error: {}", e))?;

    let mut result = Vec::new();
    for row in rows {
        let (id_id, ext_id, name, avt, role, l_id, l_name) =
            row.map_err(|e| format!("Row parse error: {}", e))?;

        let mut categories = Vec::new();
        if let Some(target_lead_id) = l_id {
            let mut cat_stmt = conn
                .prepare(
                    "SELECT c.id, c.name
                     FROM categories c
                     JOIN lead_categories lc ON lc.category_id = c.id
                     WHERE lc.lead_id = ?1",
                )
                .map_err(|e| format!("Category prepare error: {}", e))?;

            let cat_rows = cat_stmt
                .query_map([target_lead_id], |c_row| {
                    Ok(GroupCategoryInfo {
                        id: c_row.get(0)?,
                        name: c_row.get(1)?,
                    })
                })
                .map_err(|e| format!("Category query error: {}", e))?;

            for cat in cat_rows {
                categories.push(cat.map_err(|error| format!("Category row error: {error}"))?);
            }
        }

        result.push(GroupMemberDetail {
            identity_id: id_id,
            external_id: ext_id,
            display_name: name,
            avatar_url: avt,
            participant_role: role,
            lead_id: l_id,
            lead_name: l_name,
            categories,
        });
    }

    Ok(result)
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct BatchImportGroupMembersRequest {
    pub conversation_id: i64,
    pub members: Vec<ImportMemberItem>,
    pub category_ids: Vec<i32>,
    pub new_category_name: Option<String>,
    pub group_title: String,
    pub skip_existing: Option<bool>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct BatchImportGroupMembersResult {
    pub imported_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub assigned_category_ids: Vec<i32>,
}

#[tauri::command]
pub fn get_group_members_unrestricted(
    state: tauri::State<'_, DbState>,
    conversation_id: i64,
) -> Result<Vec<GroupMemberDetail>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    fetch_group_members_impl(&conn, conversation_id)
}

#[tauri::command]
pub fn batch_import_group_members_to_leads(
    conversation_state: tauri::State<'_, ConversationState>,
    request: BatchImportGroupMembersRequest,
) -> Result<BatchImportGroupMembersResult, String> {
    batch_import_group_members_to_leads_impl(conversation_state.service.as_ref(), request)
}

pub(crate) fn batch_import_group_members_to_leads_impl(
    conversation_service: &crate::conversation::service::ConversationService,
    request: BatchImportGroupMembersRequest,
) -> Result<BatchImportGroupMembersResult, String> {
    let BatchImportGroupMembersRequest {
        conversation_id,
        members,
        category_ids,
        new_category_name,
        group_title: _,
        skip_existing,
    } = request;
    let repository_request = crate::conversation::repository::GroupMemberLeadImportRequest {
        conversation_id,
        members: members
            .into_iter()
            .map(
                |member| crate::conversation::repository::GroupMemberLeadImportItem {
                    identity_id: member.identity_id,
                    external_id: member.external_id,
                    phone: member.phone,
                },
            )
            .collect(),
        category_ids,
        new_category_name,
        skip_existing: skip_existing.unwrap_or(false),
    };
    let result = conversation_service
        .import_group_members_to_leads(&repository_request)
        .map_err(|error| format!("Failed to import Zalo group members: {error}"))?;

    Ok(BatchImportGroupMembersResult {
        imported_count: result.imported_count,
        updated_count: result.updated_count,
        skipped_count: result.skipped_count,
        assigned_category_ids: result.assigned_category_ids,
    })
}

const GROUP_MEMBER_BACKFILL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
const GROUP_MEMBER_PROFILE_BATCH_SIZE: usize = 50;

fn group_member_scan_warning(
    stage: &str,
    code: &str,
    context: Option<String>,
    message: impl Into<String>,
) -> GroupMemberScanWarning {
    GroupMemberScanWarning {
        stage: stage.to_string(),
        code: code.to_string(),
        context,
        message: message.into(),
    }
}

fn backfill_warning_code(
    outcome: &crate::conversation::zalo::supervisor::GroupMessageBackfillOutcome,
) -> &'static str {
    use crate::conversation::zalo::supervisor::GroupMessageBackfillOutcome;

    match outcome {
        GroupMessageBackfillOutcome::Completed => "completed",
        GroupMessageBackfillOutcome::TimedOut => "timed_out",
        GroupMessageBackfillOutcome::Unavailable => "unavailable",
        GroupMessageBackfillOutcome::Failed => "failed",
    }
}

#[tauri::command]
pub async fn sync_and_deep_scan_group_members(
    state: tauri::State<'_, DbState>,
    conversation_state: tauri::State<'_, ConversationState>,
    conversation_id: i64,
) -> Result<GroupMemberScanResult, String> {
    sync_and_deep_scan_group_members_impl(
        state.inner(),
        conversation_state.service.as_ref(),
        conversation_state.zalo_supervisor.as_ref(),
        conversation_id,
    )
    .await
}

pub(crate) async fn sync_and_deep_scan_group_members_impl(
    state: &DbState,
    conversation_service: &crate::conversation::service::ConversationService,
    runtime: &dyn GroupMemberScanRuntime,
    conversation_id: i64,
) -> Result<GroupMemberScanResult, String> {
    use crate::conversation::repository::ZaloUidCandidate;
    use crate::conversation::zalo::supervisor::GroupMessageBackfillOutcome;

    let (external_thread_id, provider_account_key) = {
        let conn = state
            .conn
            .lock()
            .map_err(|error| format!("Database lock error: {error}"))?;
        let row = conn
            .query_row(
                "SELECT cc.external_thread_id, ca.provider_account_key, cc.kind, ca.channel
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 WHERE cc.id = ?1",
                [conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| format!("Failed to resolve conversation: {error}"))?
            .ok_or_else(|| format!("Conversation {conversation_id} was not found"))?;
        if row.2 != "group" || row.3 != "zalo" {
            return Err(format!(
                "Conversation {conversation_id} is not a Zalo group"
            ));
        }
        (row.0, row.1)
    };

    let client = runtime.connected_client(&provider_account_key).await;
    let mut warnings = Vec::new();
    let mut direct_snapshot = GroupMemberDirectSnapshotDiagnostics::default();

    if let Some(client) = client.as_ref() {
        direct_snapshot.attempted = true;
        match client.get_group_snapshot(&external_thread_id).await {
            Ok(Some(snapshot)) => {
                if snapshot.provider_account_key != provider_account_key
                    || snapshot.external_thread_id != external_thread_id
                {
                    let warning = group_member_scan_warning(
                        "direct_snapshot",
                        "snapshot_mismatch",
                        Some(format!("conversation_id={conversation_id}")),
                        "Zalo returned a group snapshot for a different account or thread",
                    );
                    direct_snapshot.warning = Some(warning.clone());
                    warnings.push(warning);
                } else {
                    direct_snapshot.complete = snapshot.membership_complete;
                    direct_snapshot.member_count = snapshot.participants.len();
                    conversation_service
                        .apply_group_snapshot(&snapshot)
                        .map_err(|error| {
                            format!("Failed to persist Zalo group snapshot: {error}")
                        })?;
                }
            }
            Ok(None) => {
                let warning = group_member_scan_warning(
                    "direct_snapshot",
                    "snapshot_unavailable",
                    Some(format!("conversation_id={conversation_id}")),
                    "Zalo returned no direct group membership snapshot",
                );
                direct_snapshot.warning = Some(warning.clone());
                warnings.push(warning);
            }
            Err(error) => {
                let warning = group_member_scan_warning(
                    "direct_snapshot",
                    "snapshot_failed",
                    Some(format!("conversation_id={conversation_id}")),
                    format!("Failed to refresh direct group snapshot: {error}"),
                );
                direct_snapshot.warning = Some(warning.clone());
                warnings.push(warning);
            }
        }
    }

    let history_backfill = runtime
        .backfill_group_messages(&provider_account_key, GROUP_MEMBER_BACKFILL_TIMEOUT)
        .await;
    if history_backfill.outcome != GroupMessageBackfillOutcome::Completed
        || history_backfill.warning.is_some()
    {
        warnings.push(group_member_scan_warning(
            "history_backfill",
            backfill_warning_code(&history_backfill.outcome),
            history_backfill.stop_reason.clone(),
            history_backfill.warning.clone().unwrap_or_else(|| {
                format!(
                    "Account-wide group-message backfill ended with {:?}",
                    history_backfill.outcome
                )
            }),
        ));
    }

    let mut board = GroupMemberBoardDiagnostics::default();
    let mut board_scan_failed = false;
    let mut group_link = GroupMemberGroupLinkDiagnostics::default();

    if let Some(client) = client.as_ref() {
        group_link.attempted = true;
        match client.fetch_group_link_detail(&external_thread_id).await {
            Ok(Some(link)) if !link.trim().is_empty() => {
                group_link.link_retrieved = true;
                for page in 1..=10 {
                    group_link.pages_attempted += 1;
                    match client.fetch_group_link_members(&link, page).await {
                        Ok(Some((profiles, has_more))) => {
                            group_link.pages_succeeded += 1;
                            group_link.items_inspected += profiles.len();
                            let candidates = profiles
                                .into_iter()
                                .map(|profile| ZaloUidCandidate {
                                    external_id: profile.external_id,
                                    display_name: profile.display_name,
                                    avatar_url: profile.avatar_url,
                                })
                                .collect::<Vec<_>>();
                            if !candidates.is_empty() {
                                match conversation_service
                                    .repository
                                    .attach_zalo_uids_to_conversation(conversation_id, &candidates)
                                {
                                    Ok(committed) => {
                                        group_link.committed_uid_count += committed.len();
                                    }
                                    Err(error) => {
                                        let warning = group_member_scan_warning(
                                            "group_link",
                                            "persistence_failed",
                                            Some(format!("page={page}")),
                                            format!(
                                                "Failed to persist Group Link Zalo UIDs: {error}"
                                            ),
                                        );
                                        group_link.warning = Some(warning.clone());
                                        warnings.push(warning);
                                        break;
                                    }
                                }
                            }
                            if !has_more {
                                break;
                            }
                        }
                        Ok(None) | Err(_) => {
                            if page == 1 {
                                let warning = group_member_scan_warning(
                                    "group_link",
                                    "link_members_unavailable",
                                    Some(format!("link={link}")),
                                    "Zalo group link returned no member preview",
                                );
                                group_link.warning = Some(warning.clone());
                                warnings.push(warning);
                            }
                            break;
                        }
                    }
                }
            }
            Ok(_) => {}
            Err(error) => {
                let warning = group_member_scan_warning(
                    "group_link",
                    "link_detail_failed",
                    Some(format!("conversation_id={conversation_id}")),
                    format!("Failed to fetch group link detail: {error}"),
                );
                group_link.warning = Some(warning.clone());
                warnings.push(warning);
            }
        }

        if let Ok(Some(invite_profiles)) = client
            .fetch_group_invite_box_members(&external_thread_id, 1)
            .await
        {
            if !invite_profiles.is_empty() {
                let candidates = invite_profiles
                    .into_iter()
                    .map(|profile| ZaloUidCandidate {
                        external_id: profile.external_id,
                        display_name: profile.display_name,
                        avatar_url: profile.avatar_url,
                    })
                    .collect::<Vec<_>>();
                if let Ok(committed) = conversation_service
                    .repository
                    .attach_zalo_uids_to_conversation(conversation_id, &candidates)
                {
                    group_link.committed_uid_count += committed.len();
                }
            }
        }

        match client
            .fetch_group_board_uid_report(&external_thread_id)
            .await
        {
            Ok(report) => {
                board.pages_attempted = report.pages_attempted;
                board.pages_succeeded = report.pages_succeeded;
                board.items_inspected = report.items_inspected;
                board.polls_attempted = report.polls_attempted;
                board.polls_succeeded = report.polls_succeeded;
                board.stop_reason = report.stop_reason.clone();
                board.warnings = report.warnings.clone();

                let candidates = report
                    .uids
                    .iter()
                    .map(|external_id| ZaloUidCandidate {
                        external_id: external_id.clone(),
                        display_name: None,
                        avatar_url: None,
                    })
                    .collect::<Vec<_>>();
                if !candidates.is_empty() {
                    board.committed_uid_count = conversation_service
                        .repository
                        .attach_zalo_uids_to_conversation(conversation_id, &candidates)
                        .map_err(|error| {
                            format!("Failed to persist Board/Poll Zalo UIDs: {error}")
                        })?
                        .len();
                }

                for board_warning in &report.warnings {
                    warnings.push(group_member_scan_warning(
                        &board_warning.source,
                        &board_warning.code,
                        board_warning.context.clone(),
                        board_warning.message.clone(),
                    ));
                }
            }
            Err(error) => {
                board_scan_failed = true;
                warnings.push(group_member_scan_warning(
                    "board",
                    "board_scan_failed",
                    Some(format!("group_id={external_thread_id}")),
                    format!("Failed to scan Group Board/Polls: {error}"),
                ));
            }
        }
    }

    let evidence_before_hydration = {
        let conn = state
            .conn
            .lock()
            .map_err(|error| format!("Database lock error: {error}"))?;
        collect_group_member_local_evidence(&conn, conversation_id)?
    };
    if evidence_before_hydration.diagnostics.rejected_uid_count > 0 {
        warnings.push(group_member_scan_warning(
            "local_evidence",
            "invalid_uid_ignored",
            Some(format!(
                "count={}",
                evidence_before_hydration.diagnostics.rejected_uid_count
            )),
            "Ignored invalid or sentinel UID values already present in local evidence",
        ));
    }

    let local_evidence_candidates = evidence_before_hydration
        .external_ids
        .iter()
        .map(|external_id| ZaloUidCandidate {
            external_id: external_id.clone(),
            display_name: None,
            avatar_url: None,
        })
        .collect::<Vec<_>>();
    let local_committed_uid_count = if local_evidence_candidates.is_empty() {
        0
    } else {
        conversation_service
            .repository
            .attach_zalo_uids_to_conversation(conversation_id, &local_evidence_candidates)
            .map_err(|error| format!("Failed to canonicalize local Zalo UID evidence: {error}"))?
            .len()
    };

    let mut profile_hydration = GroupMemberProfileHydrationDiagnostics::default();
    if let Some(client) = client.as_ref() {
        profile_hydration.attempted = evidence_before_hydration.external_ids.len();
        let mut hydrated_external_ids = HashSet::new();

        for chunk in evidence_before_hydration
            .external_ids
            .chunks(GROUP_MEMBER_PROFILE_BATCH_SIZE)
        {
            let requested_ids = chunk.iter().map(String::as_str).collect::<HashSet<_>>();
            match client.fetch_group_member_profiles(chunk).await {
                Ok(profiles) => {
                    let mut candidates = Vec::new();
                    for profile in profiles {
                        let Some(external_id) = normalized_group_member_uid(&profile.external_id)
                        else {
                            let warning = group_member_scan_warning(
                                "profile_hydration",
                                "invalid_profile_uid",
                                None,
                                "Provider returned a profile with an invalid UID",
                            );
                            profile_hydration.warnings.push(warning.clone());
                            warnings.push(warning);
                            continue;
                        };
                        if !requested_ids.contains(external_id.as_str()) {
                            let warning = group_member_scan_warning(
                                "profile_hydration",
                                "unexpected_profile_uid",
                                Some(format!("uid={external_id}")),
                                "Provider returned a profile outside the requested UID batch",
                            );
                            profile_hydration.warnings.push(warning.clone());
                            warnings.push(warning);
                            continue;
                        }
                        if hydrated_external_ids.insert(external_id.clone()) {
                            candidates.push(ZaloUidCandidate {
                                external_id,
                                display_name: profile.display_name,
                                avatar_url: profile.avatar_url,
                            });
                        }
                    }

                    if !candidates.is_empty() {
                        profile_hydration.succeeded += conversation_service
                            .repository
                            .attach_zalo_uids_to_conversation(conversation_id, &candidates)
                            .map_err(|error| {
                                format!("Failed to persist hydrated Zalo profiles: {error}")
                            })?
                            .len();
                    }
                }
                Err(error) => {
                    let warning = group_member_scan_warning(
                        "profile_hydration",
                        "profile_batch_failed",
                        Some(format!("batch_size={}", chunk.len())),
                        format!("Failed to hydrate Zalo member profiles: {error}"),
                    );
                    profile_hydration.warnings.push(warning.clone());
                    warnings.push(warning);
                }
            }
        }

        profile_hydration.failed = profile_hydration
            .attempted
            .saturating_sub(profile_hydration.succeeded);
        if profile_hydration.failed > 0 && profile_hydration.warnings.is_empty() {
            let warning = group_member_scan_warning(
                "profile_hydration",
                "profiles_incomplete",
                Some(format!("count={}", profile_hydration.failed)),
                "Zalo did not return a profile for every locally evidenced UID",
            );
            profile_hydration.warnings.push(warning.clone());
            warnings.push(warning);
        }
    }

    let (members, local_evidence) = {
        let conn = state
            .conn
            .lock()
            .map_err(|error| format!("Database lock error: {error}"))?;
        let evidence = collect_group_member_local_evidence(&conn, conversation_id)?;
        let members = fetch_group_members_impl(&conn, conversation_id)?;
        let mut diagnostics = evidence.diagnostics;
        diagnostics.committed_uid_count = local_committed_uid_count;
        (members, diagnostics)
    };

    if !direct_snapshot.complete && direct_snapshot.warning.is_none() {
        warnings.push(group_member_scan_warning(
            "overall",
            "membership_snapshot_incomplete",
            Some(format!("conversation_id={conversation_id}")),
            "Results are UIDs evidenced for this conversation, not a complete provider member list",
        ));
    }

    let partial_result = !direct_snapshot.complete
        || history_backfill.outcome != GroupMessageBackfillOutcome::Completed
        || board_scan_failed
        || !board.warnings.is_empty()
        || profile_hydration.failed > 0
        || local_evidence.rejected_uid_count > 0;
    let membership_complete = direct_snapshot.complete;
    let pages_attempted = board.pages_attempted;
    let pages_succeeded = board.pages_succeeded;
    let messages_scanned = local_evidence.messages_scanned;
    let profile_hydrated_count = profile_hydration.succeeded;
    let stop_reason = board
        .stop_reason
        .clone()
        .or_else(|| history_backfill.stop_reason.clone());
    let warning = (!warnings.is_empty()).then(|| {
        warnings
            .iter()
            .map(|warning| warning.message.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    });

    Ok(GroupMemberScanResult {
        discovered_count: members.len(),
        members,
        profile_hydrated_count,
        membership_complete,
        pages_attempted,
        pages_succeeded,
        messages_scanned,
        stop_reason,
        warning,
        diagnostics: GroupMemberScanDiagnostics {
            direct_snapshot,
            group_link,
            board,
            history_backfill,
            local_evidence,
            profile_hydration,
            partial_result,
            warnings,
        },
    })
}

#[derive(Debug, serde::Serialize)]
pub struct HardwareInfo {
    pub hardware_id: String,
    pub hostname: String,
    pub os_info: String,
}

#[tauri::command]
pub fn get_hardware_id() -> Result<HardwareInfo, String> {
    use sha2::{Digest, Sha256};

    let hostname = std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown_host".to_string());

    let os_name = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "unknown_user".to_string());

    let raw_id = format!("{}:{}:{}:{}", os_name, arch, hostname, user);

    let mut hasher = Sha256::new();
    hasher.update(raw_id.as_bytes());
    let hash = format!("{:x}", hasher.finalize());

    let formatted_hwid = format!(
        "HWID-{}-{}-{}-{}",
        &hash[0..4].to_uppercase(),
        &hash[4..8].to_uppercase(),
        &hash[8..12].to_uppercase(),
        &hash[12..16].to_uppercase()
    );

    Ok(HardwareInfo {
        hardware_id: formatted_hwid,
        hostname,
        os_info: format!("{} ({})", os_name, arch),
    })
}

#[cfg(unix)]
fn open_export_file_unix_sync(file_path: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::io::FromRawFd;

    struct FdGuard(libc::c_int);
    impl Drop for FdGuard {
        fn drop(&mut self) {
            if self.0 >= 0 {
                unsafe { libc::close(self.0) };
            }
        }
    }

    let components: Vec<_> = file_path.components().collect();
    if components.is_empty() {
        return Err("Invalid export path".to_string());
    }

    let (parent_components, file_component) = match components.split_last() {
        Some((last, rest)) => (rest, last),
        None => return Err("Invalid export path".to_string()),
    };

    let file_name = match file_component {
        std::path::Component::Normal(name) => name,
        _ => return Err("Export target filename is invalid".to_string()),
    };

    let mut current_fd = -1;
    let mut is_root = false;

    for (i, comp) in parent_components.iter().enumerate() {
        match comp {
            std::path::Component::RootDir => {
                let fd = unsafe {
                    libc::open(
                        c"/".as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err("Failed to open root directory".to_string());
                }
                current_fd = fd;
                is_root = true;
            }
            std::path::Component::Normal(name) => {
                let mut name_c = name.as_bytes().to_vec();
                name_c.push(0);

                let next_fd = if current_fd >= 0 {
                    unsafe {
                        libc::openat(
                            current_fd,
                            name_c.as_ptr() as *const libc::c_char,
                            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                        )
                    }
                } else {
                    unsafe {
                        libc::open(
                            name_c.as_ptr() as *const libc::c_char,
                            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                        )
                    }
                };

                if next_fd < 0 {
                    let err = std::io::Error::last_os_error();
                    let mut comp_stat: libc::stat = unsafe { std::mem::zeroed() };
                    let stat_res = if current_fd >= 0 {
                        unsafe {
                            libc::fstatat(
                                current_fd,
                                name_c.as_ptr() as *const libc::c_char,
                                &mut comp_stat,
                                libc::AT_SYMLINK_NOFOLLOW,
                            )
                        }
                    } else {
                        unsafe {
                            libc::lstat(name_c.as_ptr() as *const libc::c_char, &mut comp_stat)
                        }
                    };

                    if current_fd >= 0 && (i > 0 || is_root) {
                        unsafe { libc::close(current_fd) };
                    }

                    if (stat_res == 0 && (comp_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK)
                        || err.raw_os_error() == Some(libc::ELOOP)
                        || err.raw_os_error() == Some(libc::EMLINK)
                    {
                        return Err(
                            "Export path cannot contain symbolic links in parent directories"
                                .to_string(),
                        );
                    }
                    return Err(format!(
                        "Destination directory does not exist or cannot be accessed ({:?}): {}",
                        name, err
                    ));
                }

                if current_fd >= 0 && (i > 0 || is_root) {
                    unsafe { libc::close(current_fd) };
                }

                let mut dir_stat: libc::stat = unsafe { std::mem::zeroed() };
                if unsafe { libc::fstat(next_fd, &mut dir_stat) } != 0
                    || (dir_stat.st_mode & libc::S_IFMT) != libc::S_IFDIR
                {
                    unsafe { libc::close(next_fd) };
                    return Err(format!(
                        "Destination component {:?} is not a directory",
                        name
                    ));
                }

                current_fd = next_fd;
            }
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if current_fd >= 0 {
                    unsafe { libc::close(current_fd) };
                }
                return Err("Path traversal (..) is not permitted in export path".to_string());
            }
            std::path::Component::Prefix(_) => {}
        }
    }

    if current_fd < 0 {
        let fd = unsafe {
            libc::open(
                c".".as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err("Failed to open current working directory".to_string());
        }
        current_fd = fd;
    }

    let _parent_guard = FdGuard(current_fd);

    let mut file_name_c = file_name.as_bytes().to_vec();
    file_name_c.push(0);

    // Open target non-blocking without O_TRUNC to prevent hangs on existing FIFOs / named pipes
    let file_fd = unsafe {
        libc::openat(
            current_fd,
            file_name_c.as_ptr() as *const libc::c_char,
            libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            0o644,
        )
    };

    if file_fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ELOOP) || err.raw_os_error() == Some(libc::EMLINK) {
            return Err("Export to symbolic links is not permitted".to_string());
        }
        return Err(format!("Failed to open export file for writing: {}", err));
    }

    // Verify opened file descriptor is a regular file before truncation or writing
    let mut file_stat: libc::stat = unsafe { std::mem::zeroed() };
    if unsafe { libc::fstat(file_fd, &mut file_stat) } != 0 {
        unsafe { libc::close(file_fd) };
        return Err("Failed to verify opened file descriptor".to_string());
    }

    if (file_stat.st_mode & libc::S_IFMT) != libc::S_IFREG {
        unsafe { libc::close(file_fd) };
        return Err("Export target must be a regular file".to_string());
    }

    // Safely truncate regular file to 0 bytes
    if unsafe { libc::ftruncate(file_fd, 0) } != 0 {
        unsafe { libc::close(file_fd) };
        return Err("Failed to truncate export file".to_string());
    }

    // Clear O_NONBLOCK flag so standard async writes via tokio/std behave normally
    let flags = unsafe { libc::fcntl(file_fd, libc::F_GETFL) };
    if flags >= 0 {
        unsafe { libc::fcntl(file_fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) };
    }

    let std_file = unsafe { std::fs::File::from_raw_fd(file_fd) };
    Ok(std_file)
}

#[cfg(unix)]
async fn open_export_file_unix(file_path: &std::path::Path) -> Result<tokio::fs::File, String> {
    let path = file_path.to_path_buf();
    let std_file = tokio::task::spawn_blocking(move || open_export_file_unix_sync(&path))
        .await
        .map_err(|e| format!("Filesystem worker failed: {}", e))??;
    Ok(tokio::fs::File::from_std(std_file))
}

#[cfg(windows)]
fn open_export_file_windows_sync(file_path: &std::path::Path) -> Result<std::fs::File, String> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::os::windows::fs::OpenOptionsExt;
    use std::os::windows::io::AsRawHandle;

    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x00200000;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x00000400;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x00000010;
    const FILE_TYPE_DISK: u32 = 0x0001;

    #[repr(C)]
    #[allow(non_snake_case)]
    struct BY_HANDLE_FILE_INFORMATION {
        dwFileAttributes: u32,
        ftCreationTime: [u32; 2],
        ftLastAccessTime: [u32; 2],
        ftLastWriteTime: [u32; 2],
        dwVolumeSerialNumber: u32,
        nFileSizeHigh: u32,
        nFileSizeLow: u32,
        nNumberOfLinks: u32,
        nFileIndexHigh: u32,
        nFileIndexLow: u32,
    }

    extern "system" {
        fn GetFileInformationByHandle(
            hFile: *mut std::ffi::c_void,
            lpFileInformation: *mut BY_HANDLE_FILE_INFORMATION,
        ) -> i32;
        fn GetFileType(hFile: *mut std::ffi::c_void) -> u32;
        fn GetFinalPathNameByHandleW(
            hFile: *mut std::ffi::c_void,
            lpszFilePath: *mut u16,
            cchFilePath: u32,
            dwFlags: u32,
        ) -> u32;
        fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        fn CreateFileW(
            lpFileName: *const u16,
            dwDesiredAccess: u32,
            dwShareMode: u32,
            lpSecurityAttributes: *mut std::ffi::c_void,
            dwCreationDisposition: u32,
            dwFlagsAndAttributes: u32,
            hTemplateFile: *mut std::ffi::c_void,
        ) -> *mut std::ffi::c_void;
    }

    const GENERIC_READ: u32 = 0x80000000;
    const FILE_SHARE_READ: u32 = 1;
    const FILE_SHARE_WRITE: u32 = 2;
    const FILE_SHARE_DELETE: u32 = 4;
    const OPEN_EXISTING: u32 = 3;
    const INVALID_HANDLE_VALUE: *mut std::ffi::c_void = -1isize as *mut std::ffi::c_void;

    // 1. Step-by-step traversal: verify no ancestor directory is a symlink or junction
    let mut current = std::path::PathBuf::new();
    let components: Vec<_> = file_path.components().collect();
    if components.is_empty() {
        return Err("Invalid export path".to_string());
    }

    let (parent_components, _) = match components.split_last() {
        Some((last, rest)) => (rest, last),
        None => return Err("Invalid export path".to_string()),
    };

    for comp in parent_components {
        match comp {
            std::path::Component::ParentDir => {
                return Err("Path traversal (..) is not permitted in export path".to_string());
            }
            std::path::Component::CurDir => {}
            _ => {
                current.push(comp);
                let wide_path: Vec<u16> =
                    current.as_os_str().encode_wide().chain(Some(0)).collect();
                let dir_handle = unsafe {
                    CreateFileW(
                        wide_path.as_ptr(),
                        GENERIC_READ,
                        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                        std::ptr::null_mut(),
                        OPEN_EXISTING,
                        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                        std::ptr::null_mut(),
                    )
                };

                if dir_handle == INVALID_HANDLE_VALUE {
                    let err = std::io::Error::last_os_error();
                    return Err(format!(
                        "Destination directory component {:?} cannot be opened: {}",
                        current, err
                    ));
                }

                let mut dir_info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
                let ok = unsafe { GetFileInformationByHandle(dir_handle, &mut dir_info) };
                unsafe { CloseHandle(dir_handle) };

                if ok == 0 {
                    return Err(format!(
                        "Failed to verify directory information for {:?}",
                        current
                    ));
                }

                if (dir_info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
                    return Err(format!(
                        "Export path cannot contain symbolic links or junctions in parent directory ({:?})",
                        current
                    ));
                }
            }
        }
    }

    // 2. Open target file with FILE_FLAG_OPEN_REPARSE_POINT (0x00200000)
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);

    let std_file = options
        .open(file_path)
        .map_err(|e| format!("Failed to open export file for writing: {}", e))?;

    let raw_handle = std_file.as_raw_handle();

    let file_type = unsafe { GetFileType(raw_handle as *mut std::ffi::c_void) };
    if file_type != FILE_TYPE_DISK {
        return Err("Export target must be a regular disk file".to_string());
    }

    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let ok = unsafe { GetFileInformationByHandle(raw_handle as *mut std::ffi::c_void, &mut info) };
    if ok == 0 {
        return Err("Failed to verify opened file handle information".to_string());
    }

    if (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err("Export to symbolic links or reparse points is not permitted".to_string());
    }

    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) != 0 {
        return Err("Export target cannot be a directory".to_string());
    }

    // 3. Post-open handle verification: query normalized kernel path from handle and ensure it matches expected path
    let mut path_buf = vec![0u16; 1024];
    let len = unsafe {
        GetFinalPathNameByHandleW(
            raw_handle as *mut std::ffi::c_void,
            path_buf.as_mut_ptr(),
            path_buf.len() as u32,
            0,
        )
    };

    if len > 0 && (len as usize) < path_buf.len() {
        let final_path_os = std::ffi::OsString::from_wide(&path_buf[..len as usize]);
        let final_path_str = final_path_os.to_string_lossy();
        let target_str = file_path.to_string_lossy();
        let target_norm = target_str.trim_start_matches(r"\\?\").replace('/', r"\");
        let final_norm = final_path_str
            .trim_start_matches(r"\\?\")
            .replace('/', r"\");
        if !final_norm.eq_ignore_ascii_case(&target_norm) {
            return Err("Export destination was redirected during path resolution".to_string());
        }
    }

    Ok(std_file)
}

#[cfg(windows)]
async fn open_export_file_windows(file_path: &std::path::Path) -> Result<tokio::fs::File, String> {
    let path = file_path.to_path_buf();
    let std_file = tokio::task::spawn_blocking(move || open_export_file_windows_sync(&path))
        .await
        .map_err(|e| format!("Filesystem worker failed: {}", e))??;
    Ok(tokio::fs::File::from_std(std_file))
}

pub async fn write_leads_csv_to_path(
    file_path: &std::path::Path,
    content: &str,
) -> Result<std::path::PathBuf, String> {
    use tokio::io::AsyncWriteExt;

    // 1. Validate file extension
    match file_path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("csv") => {}
        _ => return Err("Only .csv files can be exported".to_string()),
    }

    // 2. Reject path traversal
    if file_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Path traversal (..) is not permitted in export path".to_string());
    }

    // 3. Open file descriptor atomically pinning ancestors with O_NOFOLLOW
    #[cfg(unix)]
    let mut file = open_export_file_unix(file_path).await?;

    #[cfg(windows)]
    let mut file = open_export_file_windows(file_path).await?;

    #[cfg(not(any(unix, windows)))]
    let mut file = {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        options
            .open(file_path)
            .await
            .map_err(|e| format!("Failed to open export file for writing: {}", e))?
    };

    // 4. Write content asynchronously and flush
    file.write_all(content.as_bytes())
        .await
        .map_err(|e| format!("Failed to write export content: {}", e))?;

    file.flush()
        .await
        .map_err(|e| format!("Failed to flush export file: {}", e))?;

    Ok(file_path.to_path_buf())
}

/// Helper that orchestrates dialog resolution and file writing, enabling unit testing of full command workflows
pub async fn export_leads_csv_file_with_dialog<F>(
    _default_filename: &str,
    content: &str,
    save_dialog_fn: F,
) -> Result<Option<String>, String>
where
    F: FnOnce() -> Result<Option<std::path::PathBuf>, String> + Send + 'static,
{
    let selected_path = tokio::task::spawn_blocking(save_dialog_fn)
        .await
        .map_err(|e| format!("Dialog worker failed: {}", e))??;

    let file_path = match selected_path {
        Some(p) => p,
        None => return Ok(None),
    };

    let final_path = write_leads_csv_to_path(&file_path, content).await?;

    Ok(Some(final_path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn export_leads_csv_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    default_filename: String,
    content: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let default_name = default_filename.clone();
    export_leads_csv_file_with_dialog(&default_filename, &content, move || {
        let mut builder = app.dialog().file();
        if !default_name.trim().is_empty() {
            builder = builder.set_file_name(&default_name);
        }
        let res = builder
            .add_filter("CSV File (*.csv)", &["csv"])
            .blocking_save_file();

        match res {
            Some(fp) => fp
                .into_path()
                .map(Some)
                .map_err(|e| format!("Invalid file path selected: {}", e)),
            None => Ok(None),
        }
    })
    .await
}

#[tauri::command]
pub async fn install_portable_linux_update(version: String) -> Result<bool, String> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = version;
        return Err("Portable Linux update is only supported on Linux".to_string());
    }

    #[cfg(target_os = "linux")]
    {
        let clean_version = version.trim().trim_start_matches('v');
        let download_url = format!(
            "https://rustsale.trantrongtri.com/v{}/RustSale%20CRM_{}_amd64.deb",
            clean_version, clean_version
        );

        let random_suffix: u64 = rand::random();
        let temp_dir = std::env::temp_dir().join(format!("rustsale-update-{}", random_suffix));
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temporary directory: {}", e))?;

        let deb_path = temp_dir.join("rustsale.deb");

        // 1. Download package
        let response = reqwest::get(&download_url)
            .await
            .map_err(|e| format!("Failed to download update: {}", e))?;

        if !response.status().is_success() {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(format!(
                "Update download failed with HTTP {}",
                response.status()
            ));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read update body: {}", e))?;

        std::fs::write(&deb_path, bytes)
            .map_err(|e| format!("Failed to write update package: {}", e))?;

        // 2. Extract package
        let extracted_dir = temp_dir.join("extracted");
        std::fs::create_dir_all(&extracted_dir).map_err(|e| e.to_string())?;

        let mut extracted = false;

        // Try 'ar x'
        if std::process::Command::new("ar")
            .args(["x", deb_path.to_str().unwrap_or("")])
            .current_dir(&temp_dir)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            if let Ok(entries) = std::fs::read_dir(&temp_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("data.tar") {
                        let res = std::process::Command::new("tar")
                            .args(["-xf", &name, "-C", extracted_dir.to_str().unwrap_or("")])
                            .current_dir(&temp_dir)
                            .status();
                        if res.map(|s| s.success()).unwrap_or(false) {
                            extracted = true;
                            break;
                        }
                    }
                }
            }
        }

        // Fallback: 7z
        if !extracted
            && std::process::Command::new("7z")
                .args(["x", deb_path.to_str().unwrap_or(""), "-y"])
                .current_dir(&temp_dir)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        {
            if let Ok(entries) = std::fs::read_dir(&temp_dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with("data.tar") {
                        let _ = std::process::Command::new("7z")
                            .args([
                                "x",
                                &name,
                                &format!("-o{}", extracted_dir.to_str().unwrap_or("")),
                                "-y",
                            ])
                            .current_dir(&temp_dir)
                            .status();
                        extracted = true;
                        break;
                    }
                }
            }
        }

        // Fallback: dpkg-deb
        if !extracted
            && std::process::Command::new("dpkg-deb")
                .args([
                    "-x",
                    deb_path.to_str().unwrap_or(""),
                    extracted_dir.to_str().unwrap_or(""),
                ])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        {
            extracted = true;
        }

        if !extracted {
            let _ = std::fs::remove_dir_all(&temp_dir);
            return Err(
                "Failed to extract update package (requires 'ar', '7z', or 'dpkg-deb')".to_string(),
            );
        }

        // 3. Install binaries & icons
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let local_bin = std::path::PathBuf::from(&home).join(".local/bin");
        let local_apps = std::path::PathBuf::from(&home).join(".local/share/applications");
        let local_icons = std::path::PathBuf::from(&home).join(".local/share/icons");
        let _ = std::fs::create_dir_all(&local_bin);
        let _ = std::fs::create_dir_all(&local_apps);
        let _ = std::fs::create_dir_all(&local_icons);

        let src_bin_dir = extracted_dir.join("usr/bin");
        if src_bin_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&src_bin_dir) {
                for entry in entries.flatten() {
                    let file_path = entry.path();
                    if file_path.is_file() {
                        let file_name = entry.file_name();
                        let target_path = local_bin.join(&file_name);

                        // Use install -m 755 to safely overwrite running executable
                        let _ = std::process::Command::new("install")
                            .args([
                                "-m",
                                "755",
                                file_path.to_str().unwrap_or(""),
                                target_path.to_str().unwrap_or(""),
                            ])
                            .status();

                        // Also update current executable path if different and writable
                        if let Ok(current_exe) = std::env::current_exe() {
                            if current_exe.file_name() == Some(&file_name)
                                && current_exe != target_path
                            {
                                let _ = std::process::Command::new("install")
                                    .args([
                                        "-m",
                                        "755",
                                        file_path.to_str().unwrap_or(""),
                                        current_exe.to_str().unwrap_or(""),
                                    ])
                                    .status();
                            }
                        }
                    }
                }
            }
        }

        // Clean up legacy nested directories if present from earlier versions
        let nested_apps = local_apps.join("applications");
        if nested_apps.exists() {
            let _ = std::fs::remove_dir_all(&nested_apps);
        }
        let nested_icons = local_icons.join("icons");
        if nested_icons.exists() {
            let _ = std::fs::remove_dir_all(&nested_icons);
        }

        // Copy icon contents into ~/.local/share/icons
        let src_icons = extracted_dir.join("usr/share/icons");
        if src_icons.exists() {
            if let Ok(entries) = std::fs::read_dir(&src_icons) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let _ = std::process::Command::new("cp")
                        .args([
                            "-r",
                            "-f",
                            "--remove-destination",
                            path.to_str().unwrap_or(""),
                            local_icons.to_str().unwrap_or(""),
                        ])
                        .status();
                }
            }
        }

        // Copy .desktop files into ~/.local/share/applications and patch paths
        let src_apps = extracted_dir.join("usr/share/applications");
        if src_apps.exists() {
            if let Ok(entries) = std::fs::read_dir(&src_apps) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() {
                        let file_name = entry.file_name();
                        let target_desktop = local_apps.join(&file_name);
                        if std::fs::copy(&path, &target_desktop).is_ok() {
                            if let Ok(content) = std::fs::read_to_string(&target_desktop) {
                                let exec_path = local_bin.join("rustsale-crm");
                                let mut new_lines = Vec::new();
                                for line in content.lines() {
                                    if line.starts_with("Exec=") {
                                        new_lines.push(format!("Exec={} %U", exec_path.display()));
                                    } else if line.starts_with("Icon=") {
                                        new_lines.push("Icon=rustsale-crm".to_string());
                                    } else {
                                        new_lines.push(line.to_string());
                                    }
                                }
                                let _ = std::fs::write(&target_desktop, new_lines.join("\n"));
                                #[cfg(unix)]
                                {
                                    use std::os::unix::fs::PermissionsExt;
                                    let _ = std::fs::set_permissions(
                                        &target_desktop,
                                        std::fs::Permissions::from_mode(0o755),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        // Update desktop and icon databases
        let _ = std::process::Command::new("update-desktop-database")
            .arg(local_apps.to_str().unwrap_or(""))
            .status();

        let hicolor_dir = local_icons.join("hicolor");
        if hicolor_dir.exists() {
            let _ = std::process::Command::new("gtk-update-icon-cache")
                .args(["-f", "-t", hicolor_dir.to_str().unwrap_or("")])
                .status();
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);

        Ok(true)
    }
}

pub fn is_public_ip(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            let octets = ipv4.octets();
            if ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_unspecified()
                || ipv4.is_multicast()
            {
                return false;
            }
            // 100.64.0.0/10 (Carrier-Grade NAT)
            if octets[0] == 100 && (octets[1] & 0xC0) == 64 {
                return false;
            }
            // 192.0.0.0/24 (IETF Protocol Assignments) and 192.0.2.0/24 (TEST-NET-1)
            if octets[0] == 192 && octets[1] == 0 && (octets[2] == 0 || octets[2] == 2) {
                return false;
            }
            // 198.51.100.0/24 (TEST-NET-2)
            if octets[0] == 198 && octets[1] == 51 && octets[2] == 100 {
                return false;
            }
            // 203.0.113.0/24 (TEST-NET-3)
            if octets[0] == 203 && octets[1] == 0 && octets[2] == 113 {
                return false;
            }
            // 198.18.0.0/15 (Benchmarking)
            if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
                return false;
            }
            // 240.0.0.0/4 (Reserved)
            if octets[0] >= 240 {
                return false;
            }
            true
        }
        std::net::IpAddr::V6(ipv6) => {
            if ipv6.is_loopback() || ipv6.is_unspecified() || ipv6.is_multicast() {
                return false;
            }
            let segments = ipv6.segments();
            // Unique local (fc00::/7)
            if (segments[0] & 0xfe00) == 0xfc00 {
                return false;
            }
            // Link-local (fe80::/10)
            if (segments[0] & 0xffc0) == 0xfe80 {
                return false;
            }
            // IPv4-mapped IPv6 address (::ffff:0:0/96)
            if let Some(ipv4) = ipv6.to_ipv4_mapped() {
                return is_public_ip(std::net::IpAddr::V4(ipv4));
            }
            true
        }
    }
}

pub const MAX_IMAGE_DOWNLOAD_BYTES: usize = 50 * 1024 * 1024; // 50MB
pub const MAX_BASE64_PAYLOAD_LEN: usize = 70 * 1024 * 1024; // ~50MB decoded

#[allow(dead_code)]
pub async fn validate_public_http_url(url_str: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(url_str).map_err(|e| format!("Invalid image URL: {}", e))?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err("Only http and https schemes are permitted for image download".to_string());
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "URL has no host".to_string())?;

    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        if !is_public_ip(ip) {
            return Err("Access to non-public/private IP address is not permitted".to_string());
        }
    } else {
        let port = parsed
            .port_or_known_default()
            .unwrap_or(if scheme == "https" { 443 } else { 80 });
        let host_port = format!("{}:{}", host, port);
        let addrs = tokio::net::lookup_host(host_port)
            .await
            .map_err(|e| format!("Failed to resolve image host {}: {}", host, e))?;

        let mut has_addr = false;
        for socket_addr in addrs {
            has_addr = true;
            if !is_public_ip(socket_addr.ip()) {
                return Err(format!(
                    "Host {} resolved to non-public address {}",
                    host,
                    socket_addr.ip()
                ));
            }
        }
        if !has_addr {
            return Err(format!("No IP address found for host {}", host));
        }
    }

    Ok(parsed)
}

pub fn detect_image_format(bytes: &[u8]) -> Result<&'static str, String> {
    if let Some(kind) = infer::get(bytes) {
        match kind.extension() {
            "jpg" | "jpeg" => return Ok("jpg"),
            "png" => return Ok("png"),
            "webp" => return Ok("webp"),
            "gif" => return Ok("gif"),
            "bmp" => return Ok("bmp"),
            "ico" => return Ok("ico"),
            "tif" | "tiff" => return Ok("tiff"),
            "avif" => return Ok("avif"),
            _ => {}
        }
    }

    if bytes.len() >= 4 {
        if let Ok(text) = std::str::from_utf8(&bytes[..std::cmp::min(bytes.len(), 2048)]) {
            let trimmed = text.trim_start();
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("<svg")
                || (lower.starts_with("<?xml") && lower.contains("<svg"))
                || (lower.starts_with("<!doctype svg") && lower.contains("<svg"))
            {
                return Ok("svg");
            }
        }
    }

    Err("Payload is not a recognized or supported image format".to_string())
}

pub async fn write_image_bytes_to_path(
    file_path: &std::path::Path,
    bytes: &[u8],
) -> Result<std::path::PathBuf, String> {
    use tokio::io::AsyncWriteExt;

    // 1. Validate file extension
    let valid_extensions = [
        "png", "jpg", "jpeg", "webp", "gif", "bmp", "svg", "ico", "tiff", "tif", "avif",
    ];
    match file_path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if valid_extensions.iter().any(|&e| ext.eq_ignore_ascii_case(e)) => {}
        _ => {
            return Err(
                "Destination file must have a valid image extension (.png, .jpg, .jpeg, .webp, .gif, .bmp, .svg, .ico, .tiff, .tif, .avif)"
                    .to_string(),
            )
        }
    }

    // 2. Reject path traversal
    if file_path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Path traversal (..) is not permitted in destination path".to_string());
    }

    // 3. Write to temporary file adjacent to destination
    let parent = file_path
        .parent()
        .ok_or_else(|| "Invalid destination path: no parent directory".to_string())?;

    let file_stem = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let temp_name = format!(
        ".{}_{}_{}.tmp",
        file_stem,
        std::process::id(),
        rand::random::<u32>()
    );
    let temp_path = parent.join(&temp_name);

    #[cfg(unix)]
    let mut file = open_export_file_unix(&temp_path).await?;

    #[cfg(windows)]
    let mut file = open_export_file_windows(&temp_path).await?;

    #[cfg(not(any(unix, windows)))]
    let mut file = {
        let mut options = tokio::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        options
            .open(&temp_path)
            .await
            .map_err(|e| format!("Failed to open temporary file for writing: {}", e))?
    };

    let write_res: Result<(), String> = async {
        file.write_all(bytes)
            .await
            .map_err(|e| format!("Failed to write image content: {}", e))?;
        file.flush()
            .await
            .map_err(|e| format!("Failed to flush temporary file: {}", e))?;
        drop(file);
        Ok(())
    }
    .await;

    if let Err(e) = write_res {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(e);
    }

    // 4. Atomically rename/replace temp_path -> file_path
    if let Err(e) = tokio::fs::rename(&temp_path, file_path).await {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(format!("Failed to finalize destination image file: {}", e));
    }

    Ok(file_path.to_path_buf())
}

pub async fn download_image_http(initial_url_str: &str) -> Result<Vec<u8>, String> {
    use futures_util::StreamExt;

    let mut current_url =
        url::Url::parse(initial_url_str).map_err(|e| format!("Invalid image URL: {}", e))?;

    let max_redirects = 5;
    for _ in 0..=max_redirects {
        let scheme = current_url.scheme();
        if scheme != "http" && scheme != "https" {
            return Err("Only http and https schemes are permitted for image download".to_string());
        }

        let host = current_url
            .host_str()
            .ok_or_else(|| "URL has no host".to_string())?
            .to_string();

        let port = current_url
            .port_or_known_default()
            .unwrap_or(if scheme == "https" { 443 } else { 80 });

        let host_port = format!("{}:{}", host, port);

        // Resolve DNS and strictly verify ALL resolved IP addresses
        let target_socket_addr = if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            if !is_public_ip(ip) {
                return Err(format!(
                    "Destination IP address {} is non-public/private",
                    ip
                ));
            }
            std::net::SocketAddr::new(ip, port)
        } else {
            let addrs: Vec<std::net::SocketAddr> = tokio::net::lookup_host(&host_port)
                .await
                .map_err(|e| format!("Failed to resolve image host {}: {}", host, e))?
                .collect();

            if addrs.is_empty() {
                return Err(format!("No IP address found for host {}", host));
            }

            for socket_addr in &addrs {
                if !is_public_ip(socket_addr.ip()) {
                    return Err(format!(
                        "Host {} resolved to non-public address {}",
                        host,
                        socket_addr.ip()
                    ));
                }
            }

            addrs[0]
        };

        // Build pinned client with zero auto-redirects
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .resolve(&host, target_socket_addr)
            .build()
            .map_err(|e| format!("Failed to initialize HTTP client: {}", e))?;

        let response = client
            .get(current_url.as_str())
            .send()
            .await
            .map_err(|e| format!("Failed to download image from URL: {}", e))?;

        let status = response.status();
        if status.is_redirection() {
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| format!("Redirect response {} missing Location header", status))?
                .to_str()
                .map_err(|_| "Invalid characters in Location header".to_string())?;

            current_url = current_url
                .join(location)
                .map_err(|e| format!("Invalid redirect target URL '{}': {}", location, e))?;
            continue;
        }

        if !status.is_success() {
            return Err(format!("Image download failed with HTTP status {}", status));
        }

        if let Some(cl) = response.content_length() {
            if cl as usize > MAX_IMAGE_DOWNLOAD_BYTES {
                return Err(format!(
                    "Image file exceeds size limit of {} MB",
                    MAX_IMAGE_DOWNLOAD_BYTES / (1024 * 1024)
                ));
            }
        }

        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(chunk_res) = stream.next().await {
            let chunk =
                chunk_res.map_err(|e| format!("Error while downloading image stream: {}", e))?;
            if bytes.len() + chunk.len() > MAX_IMAGE_DOWNLOAD_BYTES {
                return Err(format!(
                    "Image file exceeds size limit of {} MB",
                    MAX_IMAGE_DOWNLOAD_BYTES / (1024 * 1024)
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        let _ = detect_image_format(&bytes)?;
        return Ok(bytes);
    }

    Err("Too many redirects while downloading image".to_string())
}

pub async fn resolve_image_bytes(
    url: Option<&str>,
    base64_data: Option<&str>,
) -> Result<Vec<u8>, String> {
    use base64::prelude::*;

    if let Some(b64) = base64_data {
        let trimmed = b64.trim();
        if trimmed.len() > MAX_BASE64_PAYLOAD_LEN {
            return Err(format!(
                "Base64 image data exceeds size limit of {} MB",
                MAX_IMAGE_DOWNLOAD_BYTES / (1024 * 1024)
            ));
        }
        let clean_b64 = if let Some(idx) = trimmed.find(',') {
            &trimmed[idx + 1..]
        } else {
            trimmed
        };
        let bytes = BASE64_STANDARD
            .decode(clean_b64.trim())
            .map_err(|e| format!("Failed to decode base64 image data: {}", e))?;
        if bytes.len() > MAX_IMAGE_DOWNLOAD_BYTES {
            return Err(format!(
                "Image payload exceeds size limit of {} MB",
                MAX_IMAGE_DOWNLOAD_BYTES / (1024 * 1024)
            ));
        }
        let _ = detect_image_format(&bytes)?;
        return Ok(bytes);
    }

    if let Some(u) = url {
        let trimmed = u.trim();
        if trimmed.starts_with("data:") {
            if trimmed.len() > MAX_BASE64_PAYLOAD_LEN {
                return Err(format!(
                    "Data URL image exceeds size limit of {} MB",
                    MAX_IMAGE_DOWNLOAD_BYTES / (1024 * 1024)
                ));
            }
            let clean_b64 = if let Some(idx) = trimmed.find(',') {
                &trimmed[idx + 1..]
            } else {
                return Err("Invalid data URL format".to_string());
            };
            let bytes = BASE64_STANDARD
                .decode(clean_b64.trim())
                .map_err(|e| format!("Failed to decode data URL image: {}", e))?;
            if bytes.len() > MAX_IMAGE_DOWNLOAD_BYTES {
                return Err(format!(
                    "Image payload exceeds size limit of {} MB",
                    MAX_IMAGE_DOWNLOAD_BYTES / (1024 * 1024)
                ));
            }
            let _ = detect_image_format(&bytes)?;
            return Ok(bytes);
        }

        if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            return download_image_http(trimmed).await;
        }

        return Err("Unsupported image URL scheme".to_string());
    }

    Err("No image URL or data provided".to_string())
}

pub async fn save_image_file_with_dialog<F>(
    default_filename: Option<String>,
    url: Option<String>,
    base64_data: Option<String>,
    save_dialog_fn: F,
) -> Result<Option<String>, String>
where
    F: FnOnce(&str) -> Result<Option<std::path::PathBuf>, String> + Send + 'static,
{
    // 1. Prepare default filename for the dialog
    let default_name = default_filename
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| {
            if let Some(ref u) = url {
                if let Ok(parsed) = url::Url::parse(u) {
                    if let Some(segments) = parsed.path_segments() {
                        if let Some(last) = segments.last() {
                            let clean_last = last.split('?').next().unwrap_or(last);
                            if !clean_last.is_empty() && clean_last.contains('.') {
                                return clean_last.to_string();
                            }
                        }
                    }
                }
            }
            "image.png".to_string()
        });

    // 2. Open dialog FIRST before network/download side-effects
    let dialog_name = default_name.clone();
    let selected_path = tokio::task::spawn_blocking(move || save_dialog_fn(&dialog_name))
        .await
        .map_err(|e| format!("Dialog worker failed: {}", e))??;

    let file_path = match selected_path {
        Some(p) => p,
        None => return Ok(None), // User cancelled without incurring download
    };

    // 3. Acquire bytes (with bounded streaming, size caps, and SSRF protection)
    let bytes = resolve_image_bytes(url.as_deref(), base64_data.as_deref()).await?;
    if bytes.is_empty() {
        return Err("Image content is empty".to_string());
    }

    // 4. Validate image format and align extension
    let detected_ext = detect_image_format(&bytes)?;
    let target_path = match file_path.extension().and_then(|e| e.to_str()) {
        None => file_path.with_extension(detected_ext),
        Some(ext) => {
            let is_matching = match (detected_ext, ext.to_ascii_lowercase().as_str()) {
                ("jpg", "jpg" | "jpeg") => true,
                ("tiff", "tif" | "tiff") => true,
                (det, e) => det == e,
            };
            if is_matching {
                file_path
            } else {
                // If user selected generic or mismatched extension, normalize to actual detected extension
                file_path.with_extension(detected_ext)
            }
        }
    };

    let final_path = write_image_bytes_to_path(&target_path, &bytes).await?;
    Ok(Some(final_path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn save_image_file<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    default_filename: Option<String>,
    url: Option<String>,
    base64_data: Option<String>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    save_image_file_with_dialog(default_filename, url, base64_data, move |def_name| {
        let mut builder = app.dialog().file();
        if !def_name.trim().is_empty() {
            builder = builder.set_file_name(def_name);
        }
        let res = builder
            .add_filter(
                "Image Files (*.png, *.jpg, *.jpeg, *.webp, *.gif, *.svg, *.bmp, *.ico, *.avif, *.tiff)",
                &[
                    "png", "jpg", "jpeg", "webp", "gif", "svg", "bmp", "ico", "avif", "tiff",
                    "tif",
                ],
            )
            .add_filter("All Files (*.*)", &["*"])
            .blocking_save_file();

        match res {
            Some(fp) => fp
                .into_path()
                .map(Some)
                .map_err(|e| format!("Invalid file path selected: {}", e)),
            None => Ok(None),
        }
    })
    .await
}
