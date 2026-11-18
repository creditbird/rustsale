//! REST handlers: wiring + mapping only. Default responses keep the original
//! (full) shape for compatibility; `?view=compact` returns the intake views.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;

use super::clients::ClientStore;
use super::error::{ApiError, ApiResult};
use super::intake;
use super::rate_limit::RateLimiter;
use crate::conversation::commands::ConversationState;
use crate::conversation::types::TimelineCursor;

const FULL_TIMELINE_DEFAULT: u32 = 50;
const FULL_TIMELINE_MAX: u32 = 100;

#[derive(Clone)]
pub struct ApiState {
    pub conversation: Arc<ConversationState>,
    pub clients: Arc<ClientStore>,
    pub limiter: Arc<RateLimiter>,
}

#[derive(Debug, Deserialize)]
pub struct ViewQuery {
    pub view: Option<String>,
    pub limit: Option<u32>,
    pub before: Option<String>,
}

impl ViewQuery {
    fn compact(&self) -> ApiResult<bool> {
        match self.view.as_deref() {
            None | Some("full") => Ok(false),
            Some("compact") => Ok(true),
            Some(_) => Err(ApiError::bad_request("view must be 'compact' or 'full'")),
        }
    }
}

pub async fn get_inbox(
    State(state): State<ApiState>,
    Query(query): Query<ViewQuery>,
) -> ApiResult<Response> {
    if query.compact()? {
        let items = intake::inbox_unread(&state.conversation, query.limit)?;
        return Ok(Json(items).into_response());
    }
    Ok(Json(state.conversation.service.list_inbox()?).into_response())
}

pub async fn get_timeline(
    State(state): State<ApiState>,
    Path(conversation_id): Path<i64>,
    Query(query): Query<ViewQuery>,
) -> ApiResult<Response> {
    if query.compact()? {
        let view = intake::read_timeline(
            &state.conversation,
            conversation_id,
            query.limit,
            query.before.as_deref(),
        )?;
        return Ok(Json(view).into_response());
    }
    let limit = intake::check_limit(query.limit, FULL_TIMELINE_DEFAULT, FULL_TIMELINE_MAX)?;
    // Full view keeps the legacy JSON-encoded cursor `{"occurredAt":..,"messageId":..}`.
    let before = match query.before.as_deref().map(str::trim) {
        Some(raw) if !raw.is_empty() => Some(
            serde_json::from_str::<TimelineCursor>(raw)
                .map_err(|_| ApiError::bad_request("invalid 'before' cursor"))?,
        ),
        _ => None,
    };
    intake::ensure_conversation(&state.conversation, conversation_id)?;
    let page = state.conversation.service.get_conversation_timeline(
        conversation_id,
        before.as_ref(),
        limit,
    )?;
    Ok(Json(page).into_response())
}

pub async fn mark_read(
    State(state): State<ApiState>,
    Path(conversation_id): Path<i64>,
) -> ApiResult<Response> {
    let marked = intake::mark_read(&state.conversation, conversation_id)?;
    Ok(Json(serde_json::json!({ "rowsMarked": marked })).into_response())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageBody {
    #[serde(default)]
    pub channel_account_id: Option<i64>,
    pub text: String,
    #[serde(default)]
    pub reply_to_message_id: Option<i64>,
}

pub async fn send_message(
    State(state): State<ApiState>,
    Path(conversation_id): Path<i64>,
    Json(body): Json<SendMessageBody>,
) -> ApiResult<Response> {
    let message = intake::send_message(
        &state.conversation,
        conversation_id,
        body.text,
        body.channel_account_id,
        body.reply_to_message_id,
    )?;
    Ok((StatusCode::CREATED, Json(message)).into_response())
}
