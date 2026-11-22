//! Newest-first, cursor-paged Facebook comment log (all pages, or one page).

use super::{id_desc_page, Page, PageError};
use crate::commands::DbState;
use crate::facebook::commands::CommentLogDto;
use crate::host::State;
use rusqlite::types::Value;
use rusqlite::Connection;

const TAG: &str = "commentLogs:desc";

/// One page of comment logs, newest first, optionally of one Facebook page.
pub fn list_comment_logs_page_internal(
    conn: &Connection,
    page_id: Option<&str>,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Page<CommentLogDto>, PageError> {
    let mut params = Vec::new();
    let mut from_where = String::from("FROM facebook_comment_logs WHERE 1 = 1");
    if let Some(page_id) = page_id.filter(|p| !p.is_empty()) {
        params.push(Value::Text(page_id.to_string()));
        from_where.push_str(" AND page_id = ?1");
    }
    id_desc_page(
        conn,
        "id, comment_id, page_id, post_id, sender_psid, sender_name, comment_text, automation_id, like_status, public_reply_status, private_reply_status, skip_reason, error_message, created_at, updated_at",
        &from_where,
        params,
        TAG,
        cursor,
        limit,
        |row| {
            Ok(CommentLogDto {
                id: row.get(0)?,
                comment_id: row.get(1)?,
                page_id: row.get(2)?,
                post_id: row.get(3)?,
                sender_psid: row.get(4)?,
                sender_name: row.get(5)?,
                comment_text: row.get(6)?,
                automation_id: row.get(7)?,
                like_status: row.get(8)?,
                public_reply_status: row.get(9)?,
                private_reply_status: row.get(10)?,
                skip_reason: row.get(11)?,
                error_message: row.get(12)?,
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
            })
        },
        |log| log.id,
    )
}

/// New command: cursor-paged comment log.
pub fn list_comment_logs_page(
    state: State<DbState>,
    page_id: Option<String>,
    cursor: Option<String>,
    limit: u32,
) -> Result<Page<CommentLogDto>, PageError> {
    let conn = state
        .conn
        .lock()
        .map_err(|error| PageError::Internal(format!("database lock: {error}")))?;
    list_comment_logs_page_internal(&conn, page_id.as_deref(), cursor.as_deref(), limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seeded(rows: usize) -> Connection {
        let conn = Connection::open_in_memory().expect("db");
        conn.execute_batch(include_str!("../conversation/schema.sql"))
            .expect("schema");
        for i in 0..rows {
            conn.execute(
                "INSERT INTO facebook_comment_logs
                 (comment_id, page_id, sender_psid, like_status, public_reply_status, private_reply_status, created_at, updated_at)
                 VALUES (?1, ?2, 'psid', 'none', 'none', 'none', '2026-01-01', '2026-01-01')",
                rusqlite::params![format!("c{i}"), if i % 2 == 0 { "pageA" } else { "pageB" }],
            )
            .expect("log");
        }
        conn
    }

    fn walk(conn: &Connection, page_id: Option<&str>, limit: u32) -> Vec<i64> {
        let mut ids = Vec::new();
        let mut cursor = None;
        loop {
            let page = list_comment_logs_page_internal(conn, page_id, cursor.as_deref(), limit)
                .expect("page");
            ids.extend(page.items.iter().map(|l| l.id));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return ids,
            }
        }
    }

    #[test]
    fn newest_first_without_gap_or_repeat() {
        let conn = seeded(23);
        let expected: Vec<i64> = (1..=23).rev().collect();
        assert_eq!(walk(&conn, None, 5), expected);
    }

    #[test]
    fn page_filter_applies_to_total_and_pages() {
        let conn = seeded(20);
        let page = list_comment_logs_page_internal(&conn, Some("pageA"), None, 3).expect("page");
        assert_eq!(page.total, 10);
        let ids = walk(&conn, Some("pageA"), 3);
        assert_eq!(ids.len(), 10);
        assert!(ids.windows(2).all(|w| w[0] > w[1]));
        assert_eq!(
            walk(&conn, Some(""), 50).len(),
            20,
            "blank filter means all"
        );
    }

    #[test]
    fn foreign_cursor_is_rejected() {
        let conn = seeded(3);
        assert!(matches!(
            list_comment_logs_page_internal(&conn, None, Some("nope"), 2),
            Err(PageError::InvalidCursor(_))
        ));
    }
}
