//! Filtered, sorted, cursor-paged inbox.
//!
//! The inbox rows are aggregated from several queries (customers, unknown
//! identities, groups), so paging runs in Rust over that list rather than in
//! one SQL statement. The order is total: sort key, then the unique row `key`
//! (`lead:7`, `identity:3`, `conversation:9`).

use super::{page_size, Cursor, KeyValue, Page, PageError};
use crate::conversation::commands::ConversationState;
use crate::conversation::types::{Channel, InboxItem, InboxItemKind};
use crate::host::State;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// What the inbox is ordered by. Each sort has a natural direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InboxSort {
    /// Latest message first.
    #[default]
    Recent,
    /// Most unread messages first.
    Unread,
    /// Title A to Z.
    Title,
}

impl InboxSort {
    const fn tag(self) -> &'static str {
        match self {
            Self::Recent => "recent",
            Self::Unread => "unread",
            Self::Title => "title",
        }
    }

    /// Natural direction: newest, most unread, A to Z.
    const fn natural_descending(self) -> bool {
        !matches!(self, Self::Title)
    }
}

/// Filter and sort of the inbox.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InboxQuery {
    /// Case-insensitive text in the title or the last message preview.
    pub search: Option<String>,
    pub kind: Option<InboxItemKind>,
    /// Only rows that have this channel.
    pub channel: Option<Channel>,
    pub unread_only: bool,
    pub sort: InboxSort,
    /// Flips the natural direction of `sort`.
    pub reverse: bool,
}

impl InboxQuery {
    fn descending(&self) -> bool {
        self.sort.natural_descending() != self.reverse
    }

    fn tag(&self) -> String {
        format!(
            "inbox:{}:{}",
            self.sort.tag(),
            if self.descending() { "desc" } else { "asc" }
        )
    }

    fn matches(&self, item: &InboxItem) -> bool {
        if self.kind.is_some_and(|kind| kind != item.kind) {
            return false;
        }
        if self.unread_only && item.unread_count == 0 {
            return false;
        }
        if let Some(channel) = self.channel {
            if !item.channels.iter().any(|badge| badge.channel == channel) {
                return false;
            }
        }
        match self.search.as_deref().map(str::trim) {
            Some(text) if !text.is_empty() => {
                let needle = text.to_lowercase();
                item.title.to_lowercase().contains(&needle)
                    || item
                        .last_message_preview
                        .as_deref()
                        .is_some_and(|preview| preview.to_lowercase().contains(&needle))
            }
            _ => true,
        }
    }
}

fn sort_key(item: &InboxItem, sort: InboxSort) -> KeyValue {
    match sort {
        InboxSort::Recent => KeyValue::Text(item.last_message_at.clone().unwrap_or_default()),
        InboxSort::Unread => KeyValue::Int(i64::try_from(item.unread_count).unwrap_or(i64::MAX)),
        InboxSort::Title => KeyValue::Text(item.title.to_lowercase()),
    }
}

fn compare_key(a: &KeyValue, b: &KeyValue) -> Ordering {
    match (a, b) {
        (KeyValue::Int(a), KeyValue::Int(b)) => a.cmp(b),
        (KeyValue::Num(a), KeyValue::Num(b)) => a.total_cmp(b),
        (KeyValue::Text(a), KeyValue::Text(b)) => a.cmp(b),
        // Cursors are tag-checked, so mixed kinds cannot happen; keep the order total anyway.
        (KeyValue::Int(_), _) => Ordering::Less,
        (_, KeyValue::Int(_)) => Ordering::Greater,
        (KeyValue::Num(_), _) => Ordering::Less,
        (_, KeyValue::Num(_)) => Ordering::Greater,
    }
}

fn compare(a: (&KeyValue, &str), b: (&KeyValue, &str), descending: bool) -> Ordering {
    let ordering = compare_key(a.0, b.0).then_with(|| a.1.cmp(b.1));
    if descending {
        ordering.reverse()
    } else {
        ordering
    }
}

/// One page of `items` matching `query`, after `cursor`.
pub fn page_inbox(
    items: Vec<InboxItem>,
    query: &InboxQuery,
    cursor: Option<&str>,
    limit: u32,
) -> Result<Page<InboxItem>, PageError> {
    let limit = page_size(limit) as usize;
    let tag = query.tag();
    let descending = query.descending();
    let after = cursor.map(|raw| Cursor::decode(raw, &tag)).transpose()?;
    let after_id = match &after {
        Some(cursor) => match &cursor.id {
            KeyValue::Text(id) => Some(id.as_str()),
            _ => return Err(PageError::InvalidCursor("row key is not text".into())),
        },
        None => None,
    };

    let mut rows: Vec<(KeyValue, InboxItem)> = items
        .into_iter()
        .filter(|item| query.matches(item))
        .map(|item| (sort_key(&item, query.sort), item))
        .collect();
    let total = rows.len() as u64;
    rows.sort_by(|a, b| compare((&a.0, &a.1.key), (&b.0, &b.1.key), descending));

    let mut rest = rows
        .into_iter()
        .filter(|(key, item)| match (&after, after_id) {
            (Some(cursor), Some(id)) => {
                compare((key, &item.key), (&cursor.key, id), descending) == Ordering::Greater
            }
            _ => true,
        });
    let mut page: Vec<(KeyValue, InboxItem)> = rest.by_ref().take(limit + 1).collect();
    let has_more = page.len() > limit;
    if has_more {
        page.pop();
    }
    let next_cursor = match (has_more, page.last()) {
        (true, Some((key, item))) => Some(
            Cursor {
                tag,
                key: key.clone(),
                id: KeyValue::Text(item.key.clone()),
            }
            .encode()?,
        ),
        _ => None,
    };
    Ok(Page {
        items: page.into_iter().map(|(_, item)| item).collect(),
        total,
        next_cursor,
    })
}

/// New command: the inbox, filtered, sorted and paged in core.
pub fn list_conversation_inbox_page(
    state: State<'_, ConversationState>,
    query: InboxQuery,
    cursor: Option<String>,
    limit: u32,
) -> Result<Page<InboxItem>, PageError> {
    let items = state
        .service
        .list_inbox()
        .map_err(|error| PageError::Database(error.to_string()))?;
    page_inbox(items, &query, cursor.as_deref(), limit)
}

#[cfg(test)]
mod tests;
