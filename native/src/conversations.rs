//! The conversation inbox, timelines and channel runtime.

use crate::error::CoreApiError;
use crate::runtime::{args, body, checked_id64, id64, limit, on_blocking, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::conversation::commands::{self, ConversationState};
use rustsale_core::conversation::types::TimelineCursor;
use rustsale_core::paging::inbox::{InboxQuery, list_conversation_inbox_page};

/// Conversations of the four channels: inbox, timelines and channel runtime.
#[module]
pub struct Conversations;

#[module]
impl Conversations {
    /// One page of the inbox, filtered and sorted in the core. `query` is `{ search?, kind?,
    /// channel?, unreadOnly?, sort?, reverse? }`. Resolves `{ items, total, nextCursor }`.
    pub async fn inbox_page(
        query: Option<JsonValue>,
        cursor: Option<String>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let query: InboxQuery = args(query)?;
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(list_conversation_inbox_page(state, query, cursor, limit)?)
        })
        .await
    }

    /// Marks the conversation with inbox `key` read and resolves how many messages changed. Emits
    /// one `conversation-invalidated` event (reason `metadata`) per conversation whose unread count
    /// changed, so inbox and badges refresh; nothing is emitted when no message changed.
    pub async fn mark_read(key: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::mark_conversation_read(state, key)?)
        })
        .await
    }

    /// One page of the timeline of conversation `conversationId`. A page holds the newest `limit`
    /// messages older than `before`, listed **oldest first** (the last item is the newest, ready to
    /// be drawn top to bottom). Resolves `{ items, nextCursor, snapshotSequence }`: `nextCursor`
    /// (`{ occurredAt, messageId }`) is the oldest message of a full page and is passed back as
    /// `before` to fetch the page before it; it is null when there is nothing older. `before` is
    /// absent for the newest page. Older pages are therefore prepended to what is on screen.
    pub async fn timeline(
        conversation_id: JsonValue,
        before: Option<JsonValue>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let before: Option<TimelineCursor> = match before {
            None | Some(JsonValue::Null) => None,
            Some(value) => {
                let mut cursor: TimelineCursor = body(value)?;
                cursor.message_id = checked_id64(cursor.message_id)?;
                Some(cursor)
            }
        };
        let limit = limit(limit_);
        let conversation = id64(conversation_id)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_timeline(
                state,
                conversation,
                before,
                limit,
            )?)
        })
        .await
    }

    /// Whether each channel account is connected: Zalo, Facebook and WhatsApp.
    pub async fn runtime_status() -> Result<JsonValue, JsError> {
        on_core(|h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_runtime_status(state).await?)
        })
        .await
    }
}
