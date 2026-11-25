//! Newest-first, cursor-paged activity log (all leads, or one lead).

use super::{id_desc_page, Page, PageError};
use crate::commands::DbState;
use crate::host::State;
use crate::models::Activity;
use rusqlite::types::Value;
use rusqlite::Connection;

const TAG: &str = "activities:desc";

/// One page of activities, newest first, optionally of one lead.
pub fn get_activities_page_internal(
    conn: &Connection,
    lead_id: Option<i32>,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Page<Activity>, PageError> {
    let mut params = Vec::new();
    let mut from_where = String::from("FROM activities WHERE 1 = 1");
    if let Some(lead_id) = lead_id {
        params.push(Value::Integer(i64::from(lead_id)));
        from_where.push_str(" AND lead_id = ?1");
    }
    id_desc_page(
        conn,
        "id, lead_id, activity_type, content, created_at, performed_by",
        &from_where,
        params,
        TAG,
        cursor,
        limit,
        |row| {
            Ok(Activity {
                id: Some(row.get(0)?),
                lead_id: row.get(1)?,
                activity_type: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
                performed_by: row.get(5)?,
            })
        },
        |activity| i64::from(activity.id.unwrap_or_default()),
    )
}

/// New command: cursor-paged activity log.
pub fn get_activities_page(
    state: State<DbState>,
    lead_id: Option<i32>,
    cursor: Option<String>,
    limit: u32,
) -> Result<Page<Activity>, PageError> {
    let conn = state
        .conn
        .lock()
        .map_err(|error| PageError::Internal(format!("database lock: {error}")))?;
    get_activities_page_internal(&conn, lead_id, cursor.as_deref(), limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::init_db;
    use crate::paging::MAX_PAGE_SIZE;

    fn seeded(leads: i64, per_lead: i64) -> Connection {
        let conn = init_db(":memory:").expect("db");
        for lead in 0..leads {
            conn.execute(
                "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone)
                 VALUES (?1, 'c', 'New', 0, 'x', 'o', 'e', 'p')",
                [format!("lead {lead}")],
            )
            .expect("lead");
        }
        for n in 0..per_lead {
            for lead in 1..=leads {
                conn.execute(
                    "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by)
                     VALUES (?1, 'note', ?2, '2026-01-01', 'me')",
                    rusqlite::params![lead, format!("n{n}")],
                )
                .expect("activity");
            }
        }
        conn
    }

    fn walk(conn: &Connection, lead: Option<i32>, limit: u32) -> Vec<i32> {
        let mut ids = Vec::new();
        let mut cursor = None;
        loop {
            let page =
                get_activities_page_internal(conn, lead, cursor.as_deref(), limit).expect("page");
            ids.extend(page.items.iter().filter_map(|a| a.id));
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return ids,
            }
        }
    }

    #[test]
    fn newest_first_without_gap_or_repeat() {
        let conn = seeded(3, 9);
        let ids = walk(&conn, None, 4);
        let expected: Vec<i32> = (1..=27).rev().collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn filter_by_lead_counts_only_that_lead() {
        let conn = seeded(3, 5);
        let page = get_activities_page_internal(&conn, Some(2), None, 2).expect("page");
        assert_eq!(page.total, 5);
        assert_eq!(page.items.len(), 2);
        assert!(page.items.iter().all(|a| a.lead_id == 2));
        assert_eq!(walk(&conn, Some(2), 2).len(), 5);
    }

    #[test]
    fn new_rows_while_scrolling_do_not_shift_the_next_page() {
        let conn = seeded(1, 10);
        let first = get_activities_page_internal(&conn, None, None, 4).expect("first");
        conn.execute(
            "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by)
             VALUES (1, 'note', 'late', '2026-01-02', 'me')",
            [],
        )
        .expect("insert");
        let second = get_activities_page_internal(&conn, None, first.next_cursor.as_deref(), 4)
            .expect("second");
        let ids: Vec<_> = second.items.iter().filter_map(|a| a.id).collect();
        assert_eq!(ids, vec![6, 5, 4, 3]);
    }

    #[test]
    fn foreign_cursor_is_rejected_and_size_is_clamped() {
        let conn = seeded(1, 3);
        assert!(matches!(
            get_activities_page_internal(&conn, None, Some("{}"), 2),
            Err(PageError::InvalidCursor(_))
        ));
        let page =
            get_activities_page_internal(&conn, None, None, MAX_PAGE_SIZE + 500).expect("page");
        assert_eq!(page.items.len(), 3);
        assert!(page.next_cursor.is_none());
    }
}
