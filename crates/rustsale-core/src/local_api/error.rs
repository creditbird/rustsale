//! Single error type for the local API. Every error leaves as JSON `{code, message}`.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::conversation::error::{ConversationError, ConversationErrorCode};

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub retry_after: Option<u64>,
}

#[derive(Serialize)]
pub struct ErrorBody<'a> {
    pub code: &'a str,
    pub message: &'a str,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", message)
    }
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", message)
    }
    /// Attachment kinds, ids and sources the bot can reason about (`read_attachment`).
    pub fn attachment_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "ATTACHMENT_NOT_FOUND",
            "attachment not found",
        )
    }
    pub fn unsupported_kind(kind: &str) -> Self {
        Self::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "UNSUPPORTED_KIND",
            format!("only image attachments can be read (this one is '{kind}')"),
        )
    }
    pub fn link_expired() -> Self {
        Self::new(
            StatusCode::GONE,
            "LINK_EXPIRED",
            "the provider no longer serves this attachment (link expired)",
        )
    }
    pub fn download_failed(reason: &'static str) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, "DOWNLOAD_FAILED", reason)
    }
    pub fn too_large(max_bytes: u64) -> Self {
        Self::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "TOO_LARGE",
            format!("attachment is larger than {max_bytes} bytes"),
        )
    }
    pub fn rate_limited(retry_after: u64) -> Self {
        Self {
            retry_after: Some(retry_after),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                "too many requests",
            )
        }
    }
    /// Logs the detail; the client only sees a generic message.
    pub fn internal(detail: impl std::fmt::Display) -> Self {
        tracing::error!(target: "local_api", error = %detail, "internal error");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "internal error",
        )
    }
    pub fn body(&self) -> ErrorBody<'_> {
        ErrorBody {
            code: self.code,
            message: &self.message,
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(error)
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        Self::internal(error)
    }
}

impl From<ConversationError> for ApiError {
    fn from(error: ConversationError) -> Self {
        use ConversationErrorCode as C;
        let status = match error.code {
            C::InvalidRequest
            | C::UnsupportedMedia
            | C::InvalidMentionRange
            | C::MentionParticipantNotFound
            | C::MentionDirectConversationUnsupported
            | C::ForwardSourceMissingProviderIds
            | C::ForwardTargetAccountMismatch
            | C::ForwardTargetLimitExceeded
            | C::TemplateLimitExceeded => StatusCode::BAD_REQUEST,
            C::NotFound => StatusCode::NOT_FOUND,
            C::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            C::CapabilityDenied
            | C::AccountDisconnected
            | C::StageExpired
            | C::StageAlreadyClaimed
            | C::QuoteNotReady
            | C::RecallNotAllowed
            | C::ReactionNotSupported
            | C::ForwardNotSupported
            | C::TemplateShortcutConflict => StatusCode::CONFLICT,
            C::ProviderRateLimited => StatusCode::SERVICE_UNAVAILABLE,
            C::ProviderAuth
            | C::DeliveryUnknown
            | C::ReactionDeliveryUnknown
            | C::ReactionRemovalNotVerified
            | C::ForwardDeliveryUnknown
            | C::MediaMetadataUnavailable => StatusCode::BAD_GATEWAY,
            // Also produced by DB errors (`From<rusqlite::Error>`), so never echo it.
            C::ProviderFailure => return Self::internal(error),
        };
        if status.is_server_error() {
            tracing::warn!(target: "local_api", code = error.code.as_str(), error = %error.message, "provider error");
            return Self::new(status, error.code.as_str(), "messaging provider error");
        }
        Self::new(status, error.code.as_str(), error.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.body())).into_response();
        if let Some(secs) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(secs));
        }
        response
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

/// Generic code for a bare status produced by axum itself (extractor rejections, fallback, 405).
pub fn code_for_status(status: StatusCode) -> &'static str {
    match status {
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY => "bad_request",
        StatusCode::UNAUTHORIZED => "unauthorized",
        StatusCode::FORBIDDEN => "forbidden",
        StatusCode::NOT_FOUND => "not_found",
        StatusCode::METHOD_NOT_ALLOWED => "method_not_allowed",
        StatusCode::PAYLOAD_TOO_LARGE => "payload_too_large",
        StatusCode::UNSUPPORTED_MEDIA_TYPE => "unsupported_media_type",
        StatusCode::TOO_MANY_REQUESTS => "rate_limited",
        _ if status.is_server_error() => "internal_error",
        _ => "error",
    }
}
