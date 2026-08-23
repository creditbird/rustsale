use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::commands::DbState;

#[derive(Debug, Serialize, Deserialize)]
pub struct FacebookScheduledPostDto {
    pub id: i64,
    pub draft_group_id: String,
    pub channel_account_id: i64,
    pub page_id: String,
    pub post_type: String,
    pub message: String,
    pub link_url: Option<String>,
    pub media_paths: Vec<String>,
    pub scheduling_mode: String,
    pub scheduled_publish_time: Option<String>,
    pub remote_post_id: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateFacebookPostRequest {
    pub draft_group_id: Option<String>,
    pub channel_account_id: i64,
    pub page_id: String,
    pub post_type: String,
    pub message: String,
    pub link_url: Option<String>,
    pub media_paths: Option<Vec<String>>,
    pub scheduled_publish_time: Option<String>,
}

#[tauri::command]
pub fn schedule_facebook_post(
    state: tauri::State<DbState>,
    request: CreateFacebookPostRequest,
) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let draft_group_id = request
        .draft_group_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let media_paths_json = serde_json::to_string(&request.media_paths.unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let idempotency_key = uuid::Uuid::new_v4().to_string();

    let scheduling_mode = if let Some(ref ts) = request.scheduled_publish_time {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
            let now = chrono::Utc::now();
            let diff = dt.signed_duration_since(now).num_seconds();
            if diff > 30 * 86400 {
                "local_queued".to_string()
            } else {
                "meta".to_string()
            }
        } else {
            return Err("Invalid RFC3339 scheduled_publish_time format".to_string());
        }
    } else {
        "immediate".to_string()
    };

    let initial_status = if request.scheduled_publish_time.is_some() {
        "queued"
    } else {
        "draft"
    };

    conn.execute(
        "INSERT INTO facebook_scheduled_posts (
            draft_group_id, channel_account_id, page_id, post_type, message, link_url,
            media_paths_json, scheduling_mode, scheduled_publish_time, idempotency_key, status,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'), datetime('now'))",
        params![
            draft_group_id,
            request.channel_account_id,
            request.page_id,
            request.post_type,
            request.message,
            request.link_url,
            media_paths_json,
            scheduling_mode,
            request.scheduled_publish_time,
            idempotency_key,
            initial_status,
        ],
    )
    .map_err(|e| e.to_string())?;

    let post_id = conn.last_insert_rowid();
    Ok(post_id)
}

#[tauri::command]
pub fn publish_facebook_post_now(
    state: tauri::State<DbState>,
    post_id: i64,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = conn
        .execute(
            "UPDATE facebook_scheduled_posts
             SET status = 'queued',
                 scheduled_publish_time = NULL,
                 owner_id = NULL,
                 claim_deadline = NULL,
                 next_retry_at = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1 AND status IN ('draft', 'failed', 'queued')",
            params![post_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(rows > 0)
}

#[tauri::command]
pub fn list_facebook_scheduled_posts(
    state: tauri::State<DbState>,
    page_id: Option<String>,
) -> Result<Vec<FacebookScheduledPostDto>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut sql = "SELECT id, draft_group_id, channel_account_id, page_id, post_type, message, link_url, media_paths_json, scheduling_mode, scheduled_publish_time, remote_post_id, status, error_code, error_message, created_at, updated_at FROM facebook_scheduled_posts".to_string();
    if page_id.is_some() {
        sql.push_str(" WHERE page_id = ?1");
    }
    sql.push_str(" ORDER BY created_at DESC LIMIT 100");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let map_row = |r: &rusqlite::Row| {
        let media_json: String = r.get(7)?;
        let media_paths: Vec<String> = serde_json::from_str(&media_json).unwrap_or_default();
        Ok(FacebookScheduledPostDto {
            id: r.get(0)?,
            draft_group_id: r.get(1)?,
            channel_account_id: r.get(2)?,
            page_id: r.get(3)?,
            post_type: r.get(4)?,
            message: r.get(5)?,
            link_url: r.get(6)?,
            media_paths,
            scheduling_mode: r.get(8)?,
            scheduled_publish_time: r.get(9)?,
            remote_post_id: r.get(10)?,
            status: r.get(11)?,
            error_code: r.get(12)?,
            error_message: r.get(13)?,
            created_at: r.get(14)?,
            updated_at: r.get(15)?,
        })
    };

    let rows = if let Some(ref pid) = page_id {
        stmt.query_map(params![pid], map_row)
            .map_err(|e| e.to_string())?
    } else {
        stmt.query_map([], map_row).map_err(|e| e.to_string())?
    };

    let mut result = Vec::new();
    for r in rows {
        result.push(r.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn cancel_facebook_scheduled_post(
    state: tauri::State<DbState>,
    post_id: i64,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let rows = conn
        .execute(
            "UPDATE facebook_scheduled_posts SET status = 'cancelled', updated_at = datetime('now') WHERE id = ?1 AND status IN ('draft', 'queued')",
            params![post_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(rows > 0)
}
