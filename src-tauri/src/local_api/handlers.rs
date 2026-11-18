use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::conversation::commands::ConversationState;
use crate::conversation::types::{SendConversationMessageRequest, TimelineCursor};

#[derive(Clone)]
pub struct ApiState {
    pub conversation: Arc<ConversationState>,
    pub token: Arc<str>,
}

#[derive(Serialize)]
struct ApiErrorBody {
    code: &'static str,
    message: String,
}

fn error_response(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(ApiErrorBody {
            code,
            message: message.into(),
        }),
    )
        .into_response()
}

fn conversation_not_found(conversation_id: i64) -> Response {
    error_response(
        StatusCode::NOT_FOUND,
        "not_found",
        format!("conversation {conversation_id} not found"),
    )
}

/// The conversation service collapses structured errors into `anyhow::Error`
/// at this boundary (e.g. `ConversationService::enqueue_message` maps its
/// `ConversationError` down with `anyhow::anyhow!(e.message)`), so there is
/// no error code left to switch on here. This is a best-effort classification
/// of the resulting message text, matching substrings the service layer is
/// known to produce today; anything unrecognized is treated as internal.
fn classify_service_error(error: &anyhow::Error) -> Response {
    let message = error.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("not found") {
        error_response(StatusCode::NOT_FOUND, "not_found", message)
    } else if lower.contains("disconnected") || lower.contains("not connected") {
        error_response(StatusCode::CONFLICT, "conflict", message)
    } else if lower.contains("must not be empty")
        || lower.contains("invalid")
        || lower.contains("unsupported")
    {
        error_response(StatusCode::BAD_REQUEST, "bad_request", message)
    } else {
        error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
    }
}

fn ensure_conversation_exists(state: &ApiState, conversation_id: i64) -> Option<Response> {
    match state
        .conversation
        .service
        .repository
        .get_conversation_account_key(conversation_id)
    {
        Ok(Some(_)) => None,
        Ok(None) => Some(conversation_not_found(conversation_id)),
        Err(error) => Some(classify_service_error(&anyhow::Error::from(error))),
    }
}

pub async fn get_inbox(State(state): State<ApiState>) -> Response {
    match state.conversation.service.list_inbox() {
        Ok(items) => Json(items).into_response(),
        Err(error) => classify_service_error(&error),
    }
}

#[derive(Debug, Deserialize)]
pub struct TimelineQuery {
    /// JSON-encoded `TimelineCursor`, e.g. `{"occurredAt":"...","messageId":123}`.
    pub before: Option<String>,
    pub limit: Option<u32>,
}

pub async fn get_timeline(
    State(state): State<ApiState>,
    Path(conversation_id): Path<i64>,
    Query(query): Query<TimelineQuery>,
) -> Response {
    if let Some(response) = ensure_conversation_exists(&state, conversation_id) {
        return response;
    }

    let before = match query.before.as_deref().map(str::trim) {
        Some(raw) if !raw.is_empty() => match serde_json::from_str::<TimelineCursor>(raw) {
            Ok(cursor) => Some(cursor),
            Err(error) => {
                return error_response(
                    StatusCode::BAD_REQUEST,
                    "bad_request",
                    format!("invalid 'before' cursor: {error}"),
                );
            }
        },
        _ => None,
    };
    let limit = query.limit.unwrap_or(50);

    match state.conversation.service.get_conversation_timeline(
        conversation_id,
        before.as_ref(),
        limit,
    ) {
        Ok(page) => Json(page).into_response(),
        Err(error) => classify_service_error(&error),
    }
}

pub async fn mark_read(
    State(state): State<ApiState>,
    Path(conversation_id): Path<i64>,
) -> Response {
    if let Some(response) = ensure_conversation_exists(&state, conversation_id) {
        return response;
    }

    let key = format!("conversation:{conversation_id}");
    match state.conversation.service.mark_conversation_read(&key) {
        Ok(rows_marked) => Json(serde_json::json!({ "rowsMarked": rows_marked })).into_response(),
        Err(error) => classify_service_error(&error),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendMessageBody {
    pub channel_account_id: i64,
    pub text: String,
    #[serde(default)]
    pub reply_to_message_id: Option<i64>,
}

pub async fn send_message(
    State(state): State<ApiState>,
    Path(conversation_id): Path<i64>,
    Json(body): Json<SendMessageBody>,
) -> Response {
    if body.text.trim().is_empty() {
        return error_response(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "text must not be empty",
        );
    }
    if let Some(response) = ensure_conversation_exists(&state, conversation_id) {
        return response;
    }

    let request = SendConversationMessageRequest {
        conversation_id,
        channel_account_id: body.channel_account_id,
        text: body.text,
        reply_to_message_id: body.reply_to_message_id,
    };

    match state.conversation.service.enqueue_message(&request) {
        Ok(message) => {
            state.conversation.zalo_supervisor.notify_outbox();
            state.conversation.whatsapp_supervisor.notify_outbox();
            (StatusCode::CREATED, Json(message)).into_response()
        }
        Err(error) => classify_service_error(&error),
    }
}
