//! Paged, filtered and sorted places (scrape results) of one Google Maps job.

use super::{like_pattern, page_size, Cursor, KeyValue, Page, PageError};
use crate::google_maps_jobs::repository::{result_from_row, RESULT_COLUMNS};
use crate::google_maps_jobs::types::ScrapeResult;
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use serde::{Deserialize, Serialize};

/// Columns the UI can sort places by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaceSort {
    /// Order in which the places were found.
    #[default]
    FirstSeen,
    LastSeen,
    Name,
}

impl PlaceSort {
    const fn tag(self) -> &'static str {
        match self {
            Self::FirstSeen => "firstSeen",
            Self::LastSeen => "lastSeen",
            Self::Name => "name",
        }
    }

    const fn key_sql(self) -> &'static str {
        match self {
            Self::FirstSeen => "first_seen_at",
            Self::LastSeen => "last_seen_at",
            Self::Name => "lower(name)",
        }
    }
}

/// Filter and sort of a job's places.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaceQuery {
    /// Case-insensitive text in name, phone, email, address or website.
    pub search: Option<String>,
    /// Exact import status (`pending`, `imported`, ...).
    pub import_status: Option<String>,
    /// `Some(true)`: only places with a phone; `Some(false)`: only without.
    pub has_phone: Option<bool>,
    pub sort: PlaceSort,
    pub descending: bool,
}

impl PlaceQuery {
    fn tag(&self) -> String {
        format!(
            "places:{}:{}",
            self.sort.tag(),
            if self.descending { "desc" } else { "asc" }
        )
    }
}

/// One page of the places of `job_id` matching `query`, after `cursor`.
pub fn list_places_page_internal(
    conn: &Connection,
    job_id: &str,
    query: &PlaceQuery,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Page<ScrapeResult>, PageError> {
    let limit = page_size(limit);
    let tag = query.tag();
    let key_expr = query.sort.key_sql();

    let mut filter = String::from(" FROM google_maps_scrape_results WHERE job_id = ?1");
    let mut params: Vec<Value> = vec![Value::Text(job_id.to_string())];
    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        params.push(Value::Text(like_pattern(search)));
        let n = params.len();
        filter.push_str(&format!(
            " AND (lower(name) LIKE ?{n} ESCAPE '\\' OR lower(COALESCE(phone, '')) LIKE ?{n} ESCAPE '\\' \
             OR lower(COALESCE(email, '')) LIKE ?{n} ESCAPE '\\' OR lower(COALESCE(address, '')) LIKE ?{n} ESCAPE '\\' \
             OR lower(COALESCE(website, '')) LIKE ?{n} ESCAPE '\\')"
        ));
    }
    if let Some(status) = query.import_status.as_deref().filter(|s| !s.is_empty()) {
        params.push(Value::Text(status.to_string()));
        filter.push_str(&format!(" AND import_status = ?{}", params.len()));
    }
    match query.has_phone {
        Some(true) => filter.push_str(" AND phone IS NOT NULL AND trim(phone) <> ''"),
        Some(false) => filter.push_str(" AND (phone IS NULL OR trim(phone) = '')"),
        None => {}
    }

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*){filter}"),
        params_from_iter(params.iter()),
        |row| row.get(0),
    )?;

    let mut sql = format!("SELECT {RESULT_COLUMNS}, {key_expr} AS sort_key{filter}");
    let direction = if query.descending { "DESC" } else { "ASC" };
    if let Some(raw) = cursor {
        let cursor = Cursor::decode(raw, &tag)?;
        let comparison = if query.descending { "<" } else { ">" };
        let base = params.len();
        sql.push_str(&format!(
            " AND ({key_expr}, id) {comparison} (?{}, ?{})",
            base + 1,
            base + 2
        ));
        params.push(cursor.key.to_sql());
        params.push(cursor.id.to_sql());
    }
    sql.push_str(&format!(
        " ORDER BY {key_expr} {direction}, id {direction} LIMIT {}",
        limit + 1
    ));

    let mut stmt = conn.prepare(&sql)?;
    // RESULT_COLUMNS has 25 columns (0..=24); the sort key is column 25.
    let mut items = stmt
        .query_map(params_from_iter(params.iter()), |row| {
            Ok((result_from_row(row)?, row.get::<_, String>(25)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = items.len() > limit as usize;
    if has_more {
        items.pop();
    }
    let next_cursor = match (has_more, items.last()) {
        (true, Some((result, key))) => Some(
            Cursor {
                tag,
                key: KeyValue::Text(key.clone()),
                id: KeyValue::Text(result.id.clone()),
            }
            .encode()?,
        ),
        _ => None,
    };
    Ok(Page {
        items: items.into_iter().map(|(result, _)| result).collect(),
        total: u64::try_from(total).unwrap_or_default(),
        next_cursor,
    })
}

#[cfg(test)]
mod tests;
