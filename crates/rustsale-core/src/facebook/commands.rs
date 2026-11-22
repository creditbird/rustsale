use crate::host::State;
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

pub(crate) fn resolve_facebook_channel_account_id(
    conn: &rusqlite::Connection,
    requested_id: i64,
    page_id: &str,
) -> Result<i64, String> {
    if requested_id > 0 {
        let matches: bool = conn
            .query_row(
                "SELECT 1 FROM channel_accounts WHERE id = ?1 AND provider_account_key = ?2 AND (channel = 'messenger' OR channel = 'facebook')",
                rusqlite::params![requested_id, page_id],
                |_| Ok(true),
            )
            .unwrap_or(false);
        if matches {
            return Ok(requested_id);
        }
    }

    conn.query_row(
        "SELECT id FROM channel_accounts WHERE (channel = 'messenger' OR channel = 'facebook') AND provider_account_key = ?1",
        rusqlite::params![page_id],
        |r| r.get(0),
    )
    .map_err(|_| {
        format!(
            "No linked Facebook channel account found for page_id '{}'. Please connect your Fanpage in Settings first.",
            page_id
        )
    })
}

pub fn schedule_facebook_post_impl(
    conn: &rusqlite::Connection,
    request: CreateFacebookPostRequest,
) -> Result<i64, String> {
    let draft_group_id = request
        .draft_group_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let media_paths_json = serde_json::to_string(&request.media_paths.unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let idempotency_key = uuid::Uuid::new_v4().to_string();

    let (scheduling_mode, initial_status) = if let Some(ref ts) = request.scheduled_publish_time {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
            let now = chrono::Utc::now();
            let diff = dt.signed_duration_since(now).num_seconds();
            if diff > 30 * 86400 {
                ("local_queued".to_string(), "queued")
            } else if diff >= 600 {
                ("meta".to_string(), "queued")
            } else if diff >= 0 {
                // Within 10 minutes: Meta rejects <10min API schedules, so queue locally until exact target time
                ("local_queued".to_string(), "queued")
            } else {
                // Scheduled time is in the past
                ("local_queued".to_string(), "overdue_prompt")
            }
        } else {
            return Err("Invalid RFC3339 scheduled_publish_time format".to_string());
        }
    } else {
        // Immediate post created in draft state
        ("meta".to_string(), "draft")
    };

    let channel_account_id =
        resolve_facebook_channel_account_id(conn, request.channel_account_id, &request.page_id)?;

    conn.execute(
        "INSERT INTO facebook_scheduled_posts (
            draft_group_id, channel_account_id, page_id, post_type, message, link_url,
            media_paths_json, scheduling_mode, scheduled_publish_time, idempotency_key, status,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, datetime('now'), datetime('now'))",
        params![
            draft_group_id,
            channel_account_id,
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

pub fn schedule_facebook_post(
    state: State<DbState>,
    request: CreateFacebookPostRequest,
) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    schedule_facebook_post_impl(&conn, request)
}

pub fn publish_facebook_post_now_impl(
    conn: &rusqlite::Connection,
    post_id: i64,
) -> Result<bool, String> {
    let rows = conn
        .execute(
            "UPDATE facebook_scheduled_posts
             SET status = 'queued',
                 scheduled_publish_time = NULL,
                 scheduling_mode = 'meta',
                 owner_id = NULL,
                 claim_deadline = NULL,
                 next_retry_at = NULL,
                 publish_attempted_at = datetime('now'),
                 updated_at = datetime('now')
             WHERE id = ?1 AND status IN ('draft', 'failed', 'queued', 'overdue_prompt')",
            params![post_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(rows > 0)
}

pub fn publish_facebook_post_now(state: State<DbState>, post_id: i64) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    publish_facebook_post_now_impl(&conn, post_id)
}

pub fn list_facebook_scheduled_posts_impl(
    conn: &rusqlite::Connection,
    page_id: Option<String>,
) -> Result<Vec<FacebookScheduledPostDto>, String> {
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

pub fn list_facebook_scheduled_posts(
    state: State<DbState>,
    page_id: Option<String>,
) -> Result<Vec<FacebookScheduledPostDto>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    list_facebook_scheduled_posts_impl(&conn, page_id)
}

pub fn cancel_facebook_scheduled_post_impl(
    conn: &rusqlite::Connection,
    post_id: i64,
) -> Result<bool, String> {
    let rows = conn
        .execute(
            "DELETE FROM facebook_scheduled_posts WHERE id = ?1 AND status IN ('draft', 'queued', 'overdue_prompt')",
            params![post_id],
        )
        .map_err(|e| e.to_string())?;
    Ok(rows > 0)
}

pub fn cancel_facebook_scheduled_post(state: State<DbState>, post_id: i64) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    cancel_facebook_scheduled_post_impl(&conn, post_id)
}

// ---------------- Comment Automations & Logs ----------------

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCommentAutomationRequest {
    pub channel_account_id: i64,
    pub page_id: String,
    pub rule_name: String,
    pub post_ids: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub match_mode: Option<String>,
    pub auto_like: Option<bool>,
    pub public_reply_template: Option<String>,
    pub private_reply_template: Option<String>,
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateCommentAutomationRequest {
    pub id: i64,
    pub rule_name: String,
    pub post_ids: Option<Vec<String>>,
    pub keywords: Option<Vec<String>>,
    pub match_mode: Option<String>,
    pub auto_like: Option<bool>,
    pub public_reply_template: Option<String>,
    pub private_reply_template: Option<String>,
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentAutomationDto {
    pub id: i64,
    pub channel_account_id: i64,
    pub page_id: String,
    pub rule_name: String,
    pub post_ids: Vec<String>,
    pub keywords: Vec<String>,
    pub match_mode: String,
    pub auto_like: bool,
    pub public_reply_template: Option<String>,
    pub private_reply_template: Option<String>,
    pub is_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommentLogDto {
    pub id: i64,
    pub comment_id: String,
    pub page_id: String,
    pub post_id: Option<String>,
    pub sender_psid: String,
    pub sender_name: Option<String>,
    pub comment_text: Option<String>,
    pub automation_id: Option<i64>,
    pub like_status: String,
    pub public_reply_status: String,
    pub private_reply_status: String,
    pub skip_reason: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PageHealthDto {
    pub page_id: String,
    pub token_status: String,
    pub granted_scopes: Vec<String>,
    pub active_scheduled_count: i64,
    pub active_automations_count: i64,
    pub updated_at: String,
}

pub fn create_comment_automation_impl(
    conn: &rusqlite::Connection,
    req: CreateCommentAutomationRequest,
) -> Result<i64, String> {
    let post_ids_json = serde_json::to_string(&req.post_ids.unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let keywords_json = serde_json::to_string(&req.keywords.unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let match_mode = req.match_mode.unwrap_or_else(|| "contains".to_string());
    let auto_like = if req.auto_like.unwrap_or(true) { 1 } else { 0 };
    let is_enabled = if req.is_enabled.unwrap_or(true) { 1 } else { 0 };

    let channel_account_id =
        resolve_facebook_channel_account_id(conn, req.channel_account_id, &req.page_id)?;

    conn.execute(
        "INSERT INTO facebook_comment_automations
         (channel_account_id, page_id, rule_name, post_ids_json, keywords_json, match_mode, auto_like, public_reply_template, private_reply_template, is_enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now'), datetime('now'))",
        params![
            channel_account_id,
            req.page_id,
            req.rule_name,
            post_ids_json,
            keywords_json,
            match_mode,
            auto_like,
            req.public_reply_template,
            req.private_reply_template,
            is_enabled,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(conn.last_insert_rowid())
}

pub fn create_comment_automation(
    state: State<DbState>,
    req: CreateCommentAutomationRequest,
) -> Result<i64, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    create_comment_automation_impl(&conn, req)
}

pub fn list_comment_automations_impl(
    conn: &rusqlite::Connection,
    page_id: Option<String>,
) -> Result<Vec<CommentAutomationDto>, String> {
    let mut sql = "SELECT id, channel_account_id, page_id, rule_name, post_ids_json, keywords_json, match_mode, auto_like, public_reply_template, private_reply_template, is_enabled, created_at, updated_at FROM facebook_comment_automations".to_string();
    if page_id.is_some() {
        sql.push_str(" WHERE page_id = ?1");
    }
    sql.push_str(" ORDER BY id DESC");

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let map_row = |r: &rusqlite::Row| {
        let post_ids_raw: String = r.get(4)?;
        let keywords_raw: String = r.get(5)?;
        Ok(CommentAutomationDto {
            id: r.get(0)?,
            channel_account_id: r.get(1)?,
            page_id: r.get(2)?,
            rule_name: r.get(3)?,
            post_ids: serde_json::from_str(&post_ids_raw).unwrap_or_default(),
            keywords: serde_json::from_str(&keywords_raw).unwrap_or_default(),
            match_mode: r.get(6)?,
            auto_like: r.get::<_, i64>(7)? == 1,
            public_reply_template: r.get(8)?,
            private_reply_template: r.get(9)?,
            is_enabled: r.get::<_, i64>(10)? == 1,
            created_at: r.get(11)?,
            updated_at: r.get(12)?,
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

pub fn list_comment_automations(
    state: State<DbState>,
    page_id: Option<String>,
) -> Result<Vec<CommentAutomationDto>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    list_comment_automations_impl(&conn, page_id)
}

pub fn update_comment_automation_impl(
    conn: &rusqlite::Connection,
    req: UpdateCommentAutomationRequest,
) -> Result<bool, String> {
    let post_ids_json = serde_json::to_string(&req.post_ids.unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let keywords_json = serde_json::to_string(&req.keywords.unwrap_or_default())
        .unwrap_or_else(|_| "[]".to_string());
    let match_mode = req.match_mode.unwrap_or_else(|| "contains".to_string());
    let auto_like = if req.auto_like.unwrap_or(true) { 1 } else { 0 };
    let is_enabled = if req.is_enabled.unwrap_or(true) { 1 } else { 0 };

    let rows = conn.execute(
        "UPDATE facebook_comment_automations
         SET rule_name = ?1, post_ids_json = ?2, keywords_json = ?3, match_mode = ?4, auto_like = ?5, public_reply_template = ?6, private_reply_template = ?7, is_enabled = ?8, updated_at = datetime('now')
         WHERE id = ?9",
        params![
            req.rule_name,
            post_ids_json,
            keywords_json,
            match_mode,
            auto_like,
            req.public_reply_template,
            req.private_reply_template,
            is_enabled,
            req.id,
        ],
    ).map_err(|e| e.to_string())?;

    Ok(rows > 0)
}

pub fn update_comment_automation(
    state: State<DbState>,
    req: UpdateCommentAutomationRequest,
) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    update_comment_automation_impl(&conn, req)
}

pub fn delete_comment_automation_impl(
    conn: &rusqlite::Connection,
    id: i64,
) -> Result<bool, String> {
    let rows = conn
        .execute(
            "DELETE FROM facebook_comment_automations WHERE id = ?1",
            params![id],
        )
        .map_err(|e| e.to_string())?;
    Ok(rows > 0)
}

pub fn delete_comment_automation(state: State<DbState>, id: i64) -> Result<bool, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    delete_comment_automation_impl(&conn, id)
}

pub fn list_comment_logs_impl(
    conn: &rusqlite::Connection,
    page_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<CommentLogDto>, String> {
    let l = limit.unwrap_or(50).clamp(1, 200);
    let mut sql = "SELECT id, comment_id, page_id, post_id, sender_psid, sender_name, comment_text, automation_id, like_status, public_reply_status, private_reply_status, skip_reason, error_message, created_at, updated_at FROM facebook_comment_logs".to_string();
    if page_id.is_some() {
        sql.push_str(" WHERE page_id = ?1");
        sql.push_str(" ORDER BY id DESC LIMIT ?2");
    } else {
        sql.push_str(" ORDER BY id DESC LIMIT ?1");
    }

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let map_row = |r: &rusqlite::Row| {
        Ok(CommentLogDto {
            id: r.get(0)?,
            comment_id: r.get(1)?,
            page_id: r.get(2)?,
            post_id: r.get(3)?,
            sender_psid: r.get(4)?,
            sender_name: r.get(5)?,
            comment_text: r.get(6)?,
            automation_id: r.get(7)?,
            like_status: r.get(8)?,
            public_reply_status: r.get(9)?,
            private_reply_status: r.get(10)?,
            skip_reason: r.get(11)?,
            error_message: r.get(12)?,
            created_at: r.get(13)?,
            updated_at: r.get(14)?,
        })
    };

    let rows = if let Some(ref pid) = page_id {
        stmt.query_map(params![pid, l], map_row)
            .map_err(|e| e.to_string())?
    } else {
        stmt.query_map(params![l], map_row)
            .map_err(|e| e.to_string())?
    };

    let mut result = Vec::new();
    for r in rows {
        result.push(r.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

pub fn list_comment_logs(
    state: State<DbState>,
    page_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<CommentLogDto>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    list_comment_logs_impl(&conn, page_id, limit)
}

pub fn get_facebook_page_health_impl(
    conn: &rusqlite::Connection,
    page_id: &str,
) -> Result<PageHealthDto, String> {
    let (token_status, scopes_json, updated_at): (String, String, String) = conn
        .query_row(
            "SELECT token_status, granted_scopes_json, updated_at FROM facebook_page_tokens WHERE page_id = ?1",
            params![page_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("Page token not found: {}", e))?;

    let granted_scopes: Vec<String> = serde_json::from_str(&scopes_json).unwrap_or_default();

    let active_scheduled_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM facebook_scheduled_posts WHERE page_id = ?1 AND status IN ('queued', 'publishing', 'scheduled_on_meta')",
            params![page_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let active_automations_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM facebook_comment_automations WHERE page_id = ?1 AND is_enabled = 1",
            params![page_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    Ok(PageHealthDto {
        page_id: page_id.to_string(),
        token_status,
        granted_scopes,
        active_scheduled_count,
        active_automations_count,
        updated_at,
    })
}

pub fn get_facebook_page_health(
    state: State<DbState>,
    page_id: String,
) -> Result<PageHealthDto, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    get_facebook_page_health_impl(&conn, &page_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, OptionalExtension};

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE channel_accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel TEXT NOT NULL,
                provider_account_key TEXT NOT NULL,
                name TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active'
            );
            INSERT INTO channel_accounts (id, channel, provider_account_key, name)
            VALUES (1, 'facebook', 'page_123', 'Test Page');

            CREATE TABLE facebook_scheduled_posts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                draft_group_id TEXT NOT NULL,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL,
                post_type TEXT NOT NULL CHECK (post_type IN ('status', 'link', 'photo', 'multi_photo', 'video', 'reel')),
                message TEXT NOT NULL,
                link_url TEXT,
                media_paths_json TEXT NOT NULL DEFAULT '[]',
                scheduling_mode TEXT NOT NULL DEFAULT 'meta' CHECK (scheduling_mode IN ('meta', 'local_queued')),
                scheduled_publish_time TEXT,
                next_retry_at TEXT,
                remote_post_id TEXT,
                upload_session_id TEXT,
                idempotency_key TEXT NOT NULL UNIQUE,
                status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'queued', 'publishing', 'scheduled_on_meta', 'published', 'failed', 'delivery_unknown', 'overdue_prompt')),
                retry_count INTEGER NOT NULL DEFAULT 0,
                owner_id TEXT,
                claim_deadline TEXT,
                error_code TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
            );
            CREATE TABLE facebook_page_tokens (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL UNIQUE,
                access_token_encrypted TEXT NOT NULL,
                token_status TEXT NOT NULL DEFAULT 'valid' CHECK (token_status IN ('valid', 'invalid', 'expired', 'revoked')),
                granted_scopes_json TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE facebook_comment_automations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                channel_account_id INTEGER NOT NULL,
                page_id TEXT NOT NULL,
                rule_name TEXT NOT NULL,
                post_ids_json TEXT NOT NULL DEFAULT '[]',
                keywords_json TEXT NOT NULL DEFAULT '[]',
                match_mode TEXT NOT NULL DEFAULT 'contains' CHECK (match_mode IN ('contains', 'exact', 'regex', 'all')),
                auto_like INTEGER NOT NULL DEFAULT 1 CHECK (auto_like IN (0, 1)),
                public_reply_template TEXT,
                private_reply_template TEXT,
                is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(channel_account_id) REFERENCES channel_accounts(id) ON DELETE CASCADE
            );
            CREATE TABLE facebook_comment_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                comment_id TEXT NOT NULL UNIQUE,
                page_id TEXT NOT NULL,
                post_id TEXT,
                sender_psid TEXT NOT NULL,
                sender_name TEXT,
                comment_text TEXT,
                automation_id INTEGER,
                like_status TEXT NOT NULL DEFAULT 'none',
                public_reply_status TEXT NOT NULL DEFAULT 'none',
                private_reply_status TEXT NOT NULL DEFAULT 'none',
                skip_reason TEXT,
                error_message TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        ).unwrap();
        conn
    }

    #[test]
    fn test_schedule_post_immediate_creates_valid_draft() {
        let conn = setup_test_db();
        let req = CreateFacebookPostRequest {
            draft_group_id: None,
            channel_account_id: 1,
            page_id: "page_123".to_string(),
            post_type: "status".to_string(),
            message: "Hello world immediate".to_string(),
            link_url: None,
            media_paths: None,
            scheduled_publish_time: None,
        };

        let post_id = schedule_facebook_post_impl(&conn, req)
            .expect("Must insert immediate post without CHECK violation");
        assert!(post_id > 0);

        let (status, scheduling_mode): (String, String) = conn
            .query_row(
                "SELECT status, scheduling_mode FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "draft");
        assert_eq!(scheduling_mode, "meta");
    }

    #[test]
    fn test_schedule_post_meta_and_local_queued_boundaries() {
        let conn = setup_test_db();

        // 1. > 30 days -> local_queued
        let future_35d = (chrono::Utc::now() + chrono::Duration::days(35)).to_rfc3339();
        let post_id_35d = schedule_facebook_post_impl(
            &conn,
            CreateFacebookPostRequest {
                draft_group_id: None,
                channel_account_id: 1,
                page_id: "page_123".to_string(),
                post_type: "status".to_string(),
                message: "Future post".to_string(),
                link_url: None,
                media_paths: None,
                scheduled_publish_time: Some(future_35d),
            },
        )
        .unwrap();

        // 2. 2 hours -> meta
        let future_2h = (chrono::Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
        let post_id_2h = schedule_facebook_post_impl(
            &conn,
            CreateFacebookPostRequest {
                draft_group_id: None,
                channel_account_id: 1,
                page_id: "page_123".to_string(),
                post_type: "status".to_string(),
                message: "Meta scheduled post".to_string(),
                link_url: None,
                media_paths: None,
                scheduled_publish_time: Some(future_2h),
            },
        )
        .unwrap();

        // 3. 5 minutes (<10m) -> local_queued
        let future_5m = (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339();
        let post_id_5m = schedule_facebook_post_impl(
            &conn,
            CreateFacebookPostRequest {
                draft_group_id: None,
                channel_account_id: 1,
                page_id: "page_123".to_string(),
                post_type: "status".to_string(),
                message: "Sub-10m post".to_string(),
                link_url: None,
                media_paths: None,
                scheduled_publish_time: Some(future_5m),
            },
        )
        .unwrap();

        let mode_35d: String = conn
            .query_row(
                "SELECT scheduling_mode FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id_35d],
                |r| r.get(0),
            )
            .unwrap();
        let mode_2h: String = conn
            .query_row(
                "SELECT scheduling_mode FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id_2h],
                |r| r.get(0),
            )
            .unwrap();
        let mode_5m: String = conn
            .query_row(
                "SELECT scheduling_mode FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id_5m],
                |r| r.get(0),
            )
            .unwrap();

        assert_eq!(mode_35d, "local_queued");
        assert_eq!(mode_2h, "meta");
        assert_eq!(mode_5m, "local_queued");
    }

    #[test]
    fn test_cancel_facebook_scheduled_post_succeeds() {
        let conn = setup_test_db();
        let post_id = schedule_facebook_post_impl(
            &conn,
            CreateFacebookPostRequest {
                draft_group_id: None,
                channel_account_id: 1,
                page_id: "page_123".to_string(),
                post_type: "status".to_string(),
                message: "To be cancelled".to_string(),
                link_url: None,
                media_paths: None,
                scheduled_publish_time: None,
            },
        )
        .unwrap();

        let cancelled = cancel_facebook_scheduled_post_impl(&conn, post_id).unwrap();
        assert!(cancelled);

        let exists: Option<i64> = conn
            .query_row(
                "SELECT 1 FROM facebook_scheduled_posts WHERE id = ?1",
                [post_id],
                |r| r.get(0),
            )
            .optional()
            .unwrap();
        assert!(exists.is_none());
    }

    #[test]
    fn test_comment_automation_crud_and_logs() {
        let conn = setup_test_db();

        // 1. Create rule
        let rule_id = create_comment_automation_impl(
            &conn,
            CreateCommentAutomationRequest {
                channel_account_id: 1,
                page_id: "page_123".to_string(),
                rule_name: "Auto Price Reply".to_string(),
                post_ids: Some(vec!["post_1".to_string()]),
                keywords: Some(vec!["giá".to_string(), "price".to_string()]),
                match_mode: Some("contains".to_string()),
                auto_like: Some(true),
                public_reply_template: Some("Chào bạn, shop đã inbox!".to_string()),
                private_reply_template: Some("Giá sản phẩm là 299k ạ!".to_string()),
                is_enabled: Some(true),
            },
        )
        .unwrap();

        assert!(rule_id > 0);

        // 2. List rules
        let rules = list_comment_automations_impl(&conn, Some("page_123".to_string())).unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_name, "Auto Price Reply");
        assert!(rules[0].auto_like);
        assert_eq!(rules[0].keywords, vec!["giá", "price"]);

        // 3. Update rule
        let updated = update_comment_automation_impl(
            &conn,
            UpdateCommentAutomationRequest {
                id: rule_id,
                rule_name: "Updated Auto Reply".to_string(),
                post_ids: None,
                keywords: Some(vec!["inbox".to_string()]),
                match_mode: Some("exact".to_string()),
                auto_like: Some(false),
                public_reply_template: None,
                private_reply_template: Some("Updated private".to_string()),
                is_enabled: Some(true),
            },
        )
        .unwrap();
        assert!(updated);

        let rules_after =
            list_comment_automations_impl(&conn, Some("page_123".to_string())).unwrap();
        assert_eq!(rules_after[0].rule_name, "Updated Auto Reply");
        assert!(!rules_after[0].auto_like);

        // 4. Comment logs
        conn.execute(
            "INSERT INTO facebook_comment_logs (comment_id, page_id, post_id, sender_psid, sender_name, comment_text, automation_id, like_status, public_reply_status, private_reply_status, created_at, updated_at)
             VALUES ('comm_1', 'page_123', 'post_1', 'psid_1', 'User 1', 'How much?', ?1, 'success', 'success', 'success', datetime('now'), datetime('now'))",
            params![rule_id],
        ).unwrap();

        let logs = list_comment_logs_impl(&conn, Some("page_123".to_string()), Some(10)).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].comment_id, "comm_1");
        assert_eq!(logs[0].like_status, "success");

        // Without a page filter the limit is the only bound parameter.
        let all_logs = list_comment_logs_impl(&conn, None, Some(10)).unwrap();
        assert_eq!(all_logs.len(), 1);
        assert_eq!(all_logs[0].comment_id, "comm_1");

        // 5. Delete rule
        let deleted = delete_comment_automation_impl(&conn, rule_id).unwrap();
        assert!(deleted);
        let rules_empty =
            list_comment_automations_impl(&conn, Some("page_123".to_string())).unwrap();
        assert_eq!(rules_empty.len(), 0);
    }

    #[test]
    fn test_get_facebook_page_health() {
        let conn = setup_test_db();
        conn.execute(
            "INSERT INTO facebook_page_tokens (channel_account_id, page_id, access_token_encrypted, token_status, granted_scopes_json, created_at, updated_at)
             VALUES (1, 'page_123', 'enc_token', 'valid', '[\"pages_manage_posts\",\"pages_messaging\"]', datetime('now'), datetime('now'))",
            [],
        ).unwrap();

        let health = get_facebook_page_health_impl(&conn, "page_123").unwrap();
        assert_eq!(health.page_id, "page_123");
        assert_eq!(health.token_status, "valid");
        assert_eq!(health.granted_scopes.len(), 2);
        assert_eq!(health.active_scheduled_count, 0);
        assert_eq!(health.active_automations_count, 0);
    }

    #[test]
    fn test_channel_account_resolution_fallback_and_fail_closed() {
        let conn = setup_test_db();

        // 1. Direct valid ID resolves
        let res_direct = resolve_facebook_channel_account_id(&conn, 1, "page_123");
        assert_eq!(res_direct.unwrap(), 1);

        // 2. ID 0 with matching provider_account_key falls back to account id 1
        let res_fallback = resolve_facebook_channel_account_id(&conn, 0, "page_123");
        assert_eq!(res_fallback.unwrap(), 1);

        // 3. Unknown page_id fails closed with explicit error
        let res_err = resolve_facebook_channel_account_id(&conn, 0, "page_unknown");
        assert!(res_err.is_err());
        assert!(res_err
            .unwrap_err()
            .contains("No linked Facebook channel account found for page_id 'page_unknown'"));

        // 4. Invalid channel_account_id and unknown page_id fails closed
        let res_err2 = resolve_facebook_channel_account_id(&conn, 999, "page_unknown");
        assert!(res_err2.is_err());

        // 5. Cross-account mismatch: valid account ID 1 (page_123) with page_456
        conn.execute(
            "INSERT INTO channel_accounts (id, channel, provider_account_key, name)
             VALUES (2, 'facebook', 'page_456', 'Second Page')",
            [],
        )
        .unwrap();

        // When passing account 1 with page_456, it must resolve to account 2 (the true owner of page_456)
        let res_mismatch = resolve_facebook_channel_account_id(&conn, 1, "page_456");
        assert_eq!(res_mismatch.unwrap(), 2);

        // When passing account 1 with unknown page_999, it must fail closed
        let res_mismatch_err = resolve_facebook_channel_account_id(&conn, 1, "page_999");
        assert!(res_mismatch_err.is_err());
    }
}
