//! Keyset paging shared by every list the UI shows (RS P1).
//!
//! The UI never receives a whole table. A page request carries a filter, a
//! sort and an opaque cursor; the answer is `{ items, total, next_cursor }`.
//! Filtering and sorting run in SQL (or, for the inbox, in Rust over the
//! already-aggregated rows), never in TSX.
//!
//! Guarantees that the tests in each submodule pin down:
//! * the order is total (a unique tie-break key), so pages never overlap or
//!   skip rows even when the sort column has duplicates;
//! * a cursor is *keyset*, not an offset: rows inserted or deleted while the
//!   user scrolls do not shift later pages;
//! * a cursor made for one sort/direction is rejected for another.

pub mod activities;
pub mod comment_logs;
pub mod inbox;
pub mod leads;
pub mod places;

use serde::{Deserialize, Serialize};

/// Page size used when the caller passes 0.
pub const DEFAULT_PAGE_SIZE: u32 = 50;
/// Hard upper bound of a page.
pub const MAX_PAGE_SIZE: u32 = 200;

/// Errors of a paged read. Typed, so the native bridge can map them to stable codes.
#[derive(Debug, thiserror::Error)]
pub enum PageError {
    /// The cursor is not one this core produced for this query.
    #[error("invalid cursor: {0}")]
    InvalidCursor(String),
    /// A filter or sort value is not acceptable.
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    /// The database call failed.
    #[error("database error: {0}")]
    Database(String),
    /// A lock was poisoned or a worker failed.
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<rusqlite::Error> for PageError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}

/// One page of a list.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    /// Rows of this page, already filtered and sorted.
    pub items: Vec<T>,
    /// Rows matching the filter in the whole list (not only this page).
    pub total: u64,
    /// Cursor of the next page; `None` on the last page.
    pub next_cursor: Option<String>,
}

/// Clamps a requested page size to `1..=MAX_PAGE_SIZE` (0 means the default).
pub fn page_size(requested: u32) -> u32 {
    match requested {
        0 => DEFAULT_PAGE_SIZE,
        n => n.min(MAX_PAGE_SIZE),
    }
}

/// A sortable value inside a cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum KeyValue {
    Int(i64),
    Num(f64),
    Text(String),
}

/// The decoded cursor: where the previous page ended.
///
/// `tag` names the sort and direction the cursor was made for (for example
/// `"name:asc"`); `key` is the sort value and `id` the unique tie-break of the
/// last row of the previous page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    pub tag: String,
    pub key: KeyValue,
    pub id: KeyValue,
}

impl Cursor {
    /// The opaque string handed to the UI.
    pub fn encode(&self) -> Result<String, PageError> {
        serde_json::to_string(self).map_err(|error| PageError::Internal(error.to_string()))
    }

    /// Parses a cursor and checks it belongs to `expected_tag`.
    pub fn decode(raw: &str, expected_tag: &str) -> Result<Self, PageError> {
        let cursor: Self = serde_json::from_str(raw)
            .map_err(|error| PageError::InvalidCursor(error.to_string()))?;
        if cursor.tag != expected_tag {
            return Err(PageError::InvalidCursor(format!(
                "made for `{}`, used with `{expected_tag}`",
                cursor.tag
            )));
        }
        Ok(cursor)
    }
}

/// Escapes `%`, `_` and `\` so user text is matched literally by `LIKE ... ESCAPE '\'`.
pub fn like_pattern(search: &str) -> String {
    let mut pattern = String::with_capacity(search.len() + 2);
    pattern.push('%');
    for ch in search.trim().to_lowercase().chars() {
        if matches!(ch, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(ch);
    }
    pattern.push('%');
    pattern
}

/// Newest-first page over a table with an integer `id` primary key.
///
/// `select` is the column list, `from_where` starts at `FROM` and must end
/// with a `WHERE` clause whose first condition is already true (so `AND ...`
/// can follow); `params` are numbered `?1..` inside it.
pub(crate) fn id_desc_page<T>(
    conn: &rusqlite::Connection,
    select: &str,
    from_where: &str,
    params: Vec<rusqlite::types::Value>,
    tag: &str,
    cursor: Option<&str>,
    limit: u32,
    map: impl Fn(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    id_of: impl Fn(&T) -> i64,
) -> Result<Page<T>, PageError> {
    let limit = page_size(limit);
    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) {from_where}"),
        rusqlite::params_from_iter(params.iter()),
        |row| row.get(0),
    )?;
    let mut sql = format!("SELECT {select} {from_where}");
    let mut bound = params;
    if let Some(raw) = cursor {
        let cursor = Cursor::decode(raw, tag)?;
        bound.push(cursor.id.to_sql());
        sql.push_str(&format!(" AND id < ?{}", bound.len()));
    }
    sql.push_str(&format!(" ORDER BY id DESC LIMIT {}", limit + 1));
    let mut stmt = conn.prepare(&sql)?;
    let mut items = stmt
        .query_map(rusqlite::params_from_iter(bound.iter()), map)?
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = items.len() > limit as usize;
    if has_more {
        items.pop();
    }
    let next_cursor = match (has_more, items.last()) {
        (true, Some(last)) => {
            let id = id_of(last);
            Some(
                Cursor {
                    tag: tag.to_string(),
                    key: KeyValue::Int(id),
                    id: KeyValue::Int(id),
                }
                .encode()?,
            )
        }
        _ => None,
    };
    Ok(Page {
        items,
        total: u64::try_from(total).unwrap_or_default(),
        next_cursor,
    })
}

impl KeyValue {
    pub(crate) fn to_sql(&self) -> rusqlite::types::Value {
        match self {
            Self::Int(value) => rusqlite::types::Value::Integer(*value),
            Self::Num(value) => rusqlite::types::Value::Real(*value),
            Self::Text(value) => rusqlite::types::Value::Text(value.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_is_bounded() {
        assert_eq!(page_size(0), DEFAULT_PAGE_SIZE);
        assert_eq!(page_size(1), 1);
        assert_eq!(page_size(10_000), MAX_PAGE_SIZE);
    }

    #[test]
    fn cursor_roundtrips_and_checks_its_tag() {
        let cursor = Cursor {
            tag: "name:asc".into(),
            key: KeyValue::Text("ann".into()),
            id: KeyValue::Int(7),
        };
        let raw = cursor.encode().unwrap();
        assert_eq!(Cursor::decode(&raw, "name:asc").unwrap(), cursor);
        assert!(matches!(
            Cursor::decode(&raw, "name:desc"),
            Err(PageError::InvalidCursor(_))
        ));
        assert!(matches!(
            Cursor::decode("not json", "name:asc"),
            Err(PageError::InvalidCursor(_))
        ));
    }

    #[test]
    fn like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern(" 50%_off\\ "), "%50\\%\\_off\\\\%");
    }
}
