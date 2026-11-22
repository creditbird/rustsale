//! The one error TSX sees: `{ code, message }`, thrown as an `Error` whose
//! `code` is the stable string and whose `message` is for people.

use nsh_core::element::JsError;
use rustsale_core::conversation::error::ConversationCommandError;
use rustsale_core::paging::PageError;

/// What a call into the core fails with.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct CoreApiError {
    /// Stable machine-readable code (`invalid_cursor`, `not_found`, ...).
    pub code: String,
    /// Human-readable detail; never holds secrets.
    pub message: String,
}

impl CoreApiError {
    /// An error with `code` and `message`.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    /// The core was not started yet.
    pub fn not_started() -> Self {
        Self::new(
            "unavailable",
            "the core is not started: call Core.start() first",
        )
    }

    /// A worker thread or lock failed.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("internal", message)
    }

    /// An argument TSX passed is not acceptable.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }
}

/// Core commands report plain `String` errors. They carry no code, so the
/// message is sorted into the few codes TSX can act on; everything else is
/// `internal`.
impl From<String> for CoreApiError {
    fn from(message: String) -> Self {
        let lower = message.to_lowercase();
        let code = if lower.contains("not found") || lower.contains("no such") {
            "not_found"
        } else if lower.contains("invalid")
            || lower.contains("must not be empty")
            || lower.contains("required")
            || lower.contains("unsupported")
        {
            "invalid_request"
        } else if lower.contains("not logged in")
            || lower.contains("disconnected")
            || lower.contains("unavailable")
            || lower.contains("not running")
        {
            "unavailable"
        } else if lower.contains("already")
            || lower.contains("conflict")
            || lower.contains("duplicate")
        {
            "conflict"
        } else {
            "internal"
        };
        Self {
            code: code.to_owned(),
            message,
        }
    }
}

impl From<&str> for CoreApiError {
    fn from(message: &str) -> Self {
        Self::from(message.to_owned())
    }
}

impl From<ConversationCommandError> for CoreApiError {
    fn from(error: ConversationCommandError) -> Self {
        Self {
            code: serde_code(&error),
            message: error.message,
        }
    }
}

fn serde_code(error: &ConversationCommandError) -> String {
    // The core's own stable snake_case code, unchanged.
    serde_json::to_value(error.code)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "internal".to_owned())
}

impl From<PageError> for CoreApiError {
    fn from(error: PageError) -> Self {
        match error {
            PageError::InvalidCursor(m) => Self::new("invalid_cursor", m),
            PageError::InvalidRequest(m) => Self::new("invalid_request", m),
            PageError::Database(m) => Self::new("database", m),
            PageError::Internal(m) => Self::new("internal", m),
        }
    }
}

impl From<CoreApiError> for JsError {
    fn from(error: CoreApiError) -> Self {
        JsError::Error(error.message).with_code(&error.code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustsale_core::conversation::error::{ConversationError, ConversationErrorCode};

    #[test]
    fn string_errors_are_sorted_by_meaning() {
        assert_eq!(CoreApiError::from("Lead not found").code, "not_found");
        assert_eq!(CoreApiError::from("invalid status").code, "invalid_request");
        assert_eq!(
            CoreApiError::from("account disconnected").code,
            "unavailable"
        );
        assert_eq!(CoreApiError::from("duplicate phone").code, "conflict");
        assert_eq!(CoreApiError::from("disk exploded").code, "internal");
    }

    #[test]
    fn message_is_kept_verbatim() {
        let error = CoreApiError::from("Lead 7 not found");
        assert_eq!(error.message, "Lead 7 not found");
    }

    #[test]
    fn conversation_codes_pass_through() {
        let command =
            ConversationError::new(ConversationErrorCode::StageExpired, "gone").command_error();
        let error = CoreApiError::from(command);
        assert_eq!(error.code, "stage_expired");
        assert_eq!(error.message, "gone");
    }

    #[test]
    fn page_errors_map_to_stable_codes() {
        assert_eq!(
            CoreApiError::from(PageError::InvalidCursor("x".into())).code,
            "invalid_cursor"
        );
        assert_eq!(
            CoreApiError::from(PageError::InvalidRequest("x".into())).code,
            "invalid_request"
        );
        assert_eq!(
            CoreApiError::from(PageError::Database("x".into())).code,
            "database"
        );
        assert_eq!(
            CoreApiError::from(PageError::Internal("x".into())).code,
            "internal"
        );
    }

    #[test]
    fn js_error_carries_code_and_message() {
        let js: JsError = CoreApiError::new("not_found", "no lead 9").into();
        assert_eq!(js.message(), "no lead 9");
    }
}
