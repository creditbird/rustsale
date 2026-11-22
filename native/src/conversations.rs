//! The conversation inbox, timelines and channel runtime.

use crate::error::CoreApiError;
use crate::runtime::{args, body, limit, on_blocking, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::conversation::commands::{self, ConversationState};
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
    ) -> Result<JsonValue, CoreApiError> {
        let query: InboxQuery = args(query)?;
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(list_conversation_inbox_page(state, query, cursor, limit)?)
        })
        .await
    }

    /// Marks the conversation with inbox `key` read and resolves how many messages changed.
    pub async fn mark_read(key: String) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::mark_conversation_read(state, key)?)
        })
        .await
    }

    /// One page of the timeline of conversation `conversationId`, newest first. `before` is the
    /// `nextBefore` of the previous page (`{ occurredAt, messageId }`), absent for the first.
    pub async fn timeline(
        conversation_id: f64,
        before: Option<JsonValue>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let before = match before {
            None | Some(JsonValue::Null) => None,
            Some(value) => Some(body(value)?),
        };
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_timeline(
                state,
                conversation_id as i64,
                before,
                limit,
            )?)
        })
        .await
    }

    /// Whether each channel account is connected: Zalo, Facebook and WhatsApp.
    pub async fn runtime_status() -> Result<JsonValue, CoreApiError> {
        on_core(|h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_conversation_runtime_status(state).await?)
        })
        .await
    }
}
