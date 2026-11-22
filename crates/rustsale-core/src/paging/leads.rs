//! Paged, filtered and sorted leads for the leads table.
//!
//! New commands next to the v2 `get_leads` / `get_leads_by_category`, which stay
//! unchanged for the Tauri adapter. Sorting is by SQLite binary collation of the
//! lower-cased ASCII column (`lower()` does not fold Vietnamese capitals); the
//! unique `l.id` tie-break makes the order total.

use super::{like_pattern, page_size, Cursor, KeyValue, Page, PageError};
use crate::commands::DbState;
use crate::host::State;
use crate::models::Lead;
use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection, Row};
use serde::{Deserialize, Serialize};

/// Columns of the leads table the UI can sort by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LeadSort {
    #[default]
    Id,
    Name,
    Company,
    Status,
    Value,
    Owner,
    LastActivity,
}

impl LeadSort {
    const fn tag(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Name => "name",
            Self::Company => "company",
            Self::Status => "status",
            Self::Value => "value",
            Self::Owner => "owner",
            Self::LastActivity => "lastActivity",
        }
    }

    /// SQL expression of the sort key (never null) and how to read it back.
    const fn key_sql(self) -> (&'static str, KeyKind) {
        match self {
            Self::Id => ("l.id", KeyKind::Int),
            Self::Name => ("lower(COALESCE(l.name, ''))", KeyKind::Text),
            Self::Company => ("lower(COALESCE(l.company, ''))", KeyKind::Text),
            Self::Status => ("lower(COALESCE(l.status, ''))", KeyKind::Text),
            Self::Value => ("COALESCE(l.value, 0.0)", KeyKind::Num),
            Self::Owner => ("lower(COALESCE(l.owner, ''))", KeyKind::Text),
            Self::LastActivity => ("COALESCE(l.last_activity, '')", KeyKind::Text),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum KeyKind {
    Int,
    Num,
    Text,
}

/// Filter and sort of the leads table.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LeadQuery {
    /// Case-insensitive text in name, company, email, phone or owner.
    pub search: Option<String>,
    /// Exact pipeline status.
    pub status: Option<String>,
    /// Only leads of this category.
    pub category_id: Option<i32>,
    pub sort: LeadSort,
    pub descending: bool,
}

impl LeadQuery {
    fn tag(&self) -> String {
        format!(
            "leads:{}:{}",
            self.sort.tag(),
            if self.descending { "desc" } else { "asc" }
        )
    }
}

const LEAD_COLUMNS: &str = "l.id, l.name, l.company, l.status, l.value, l.last_activity, l.owner, l.email, l.phone, l.about, l.address, l.has_zalo, l.zalo_reached, l.zalo_uid, l.zalo_status, l.relationship_status, l.last_lookup_attempt, l.currency";

fn filter_sql(query: &LeadQuery) -> (String, Vec<Value>) {
    let mut sql = String::from(" FROM leads l WHERE 1 = 1");
    let mut params: Vec<Value> = Vec::new();
    if let Some(search) = query
        .search
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        params.push(Value::Text(like_pattern(search)));
        let n = params.len();
        sql.push_str(&format!(
            " AND (lower(l.name) LIKE ?{n} ESCAPE '\\' OR lower(l.company) LIKE ?{n} ESCAPE '\\' \
             OR lower(l.email) LIKE ?{n} ESCAPE '\\' OR lower(l.phone) LIKE ?{n} ESCAPE '\\' \
             OR lower(l.owner) LIKE ?{n} ESCAPE '\\')"
        ));
    }
    if let Some(status) = query.status.as_deref().filter(|s| !s.is_empty()) {
        params.push(Value::Text(status.to_string()));
        sql.push_str(&format!(" AND l.status = ?{}", params.len()));
    }
    if let Some(category_id) = query.category_id {
        params.push(Value::Integer(i64::from(category_id)));
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM lead_categories lc WHERE lc.lead_id = l.id AND lc.category_id = ?{})",
            params.len()
        ));
    }
    (sql, params)
}

fn lead_from_row(row: &Row<'_>) -> rusqlite::Result<Lead> {
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
            .or_else(|| Some("VND".to_string())),
    })
}

/// One page of leads matching `query`, after `cursor`.
pub fn get_leads_page_internal(
    conn: &Connection,
    query: &LeadQuery,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Page<Lead>, PageError> {
    let limit = page_size(limit);
    let tag = query.tag();
    let (key_expr, kind) = query.sort.key_sql();
    let (filter, filter_params) = filter_sql(query);

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*){filter}"),
        params_from_iter(filter_params.iter()),
        |row| row.get(0),
    )?;

    let mut sql = format!("SELECT {LEAD_COLUMNS}, {key_expr} AS sort_key{filter}");
    let mut params = filter_params;
    let direction = if query.descending { "DESC" } else { "ASC" };
    if let Some(raw) = cursor {
        let cursor = Cursor::decode(raw, &tag)?;
        let comparison = if query.descending { "<" } else { ">" };
        let base = params.len();
        sql.push_str(&format!(
            " AND ({key_expr}, l.id) {comparison} (?{}, ?{})",
            base + 1,
            base + 2
        ));
        params.push(cursor.key.to_sql());
        params.push(cursor.id.to_sql());
    }
    sql.push_str(&format!(
        " ORDER BY {key_expr} {direction}, l.id {direction} LIMIT {}",
        limit + 1
    ));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(params.iter()), |row| {
        let lead = lead_from_row(row)?;
        let key = match kind {
            KeyKind::Int => KeyValue::Int(row.get(18)?),
            KeyKind::Num => KeyValue::Num(row.get(18)?),
            KeyKind::Text => KeyValue::Text(row.get(18)?),
        };
        Ok((lead, key))
    })?;
    let mut items = rows.collect::<Result<Vec<_>, _>>()?;

    let has_more = items.len() > limit as usize;
    if has_more {
        items.pop();
    }
    let next_cursor = match (has_more, items.last()) {
        (true, Some((lead, key))) => Some(
            Cursor {
                tag,
                key: key.clone(),
                id: KeyValue::Int(i64::from(lead.id.unwrap_or_default())),
            }
            .encode()?,
        ),
        _ => None,
    };
    Ok(Page {
        items: items.into_iter().map(|(lead, _)| lead).collect(),
        total: u64::try_from(total).unwrap_or_default(),
        next_cursor,
    })
}

/// New command: the leads table, filtered, sorted and paged in SQL.
pub fn get_leads_page(
    state: State<DbState>,
    query: LeadQuery,
    cursor: Option<String>,
    limit: u32,
) -> Result<Page<Lead>, PageError> {
    let conn = state
        .conn
        .lock()
        .map_err(|error| PageError::Internal(format!("database lock: {error}")))?;
    get_leads_page_internal(&conn, &query, cursor.as_deref(), limit)
}

/// New command: like [`get_leads_page`] for one category.
pub fn get_leads_by_category_page(
    state: State<DbState>,
    category_id: i32,
    mut query: LeadQuery,
    cursor: Option<String>,
    limit: u32,
) -> Result<Page<Lead>, PageError> {
    query.category_id = Some(category_id);
    get_leads_page(state, query, cursor, limit)
}

#[cfg(test)]
mod tests;
