use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetaErrorKind {
    /// OAuth token expired, invalid session, revoked permissions (code 190, 102, 401 Unauthorized)
    AuthFailure,
    /// Rate limit reached (HTTP 429, code 4, 17, 32, 613)
    RateLimited,
    /// Transient / temporary server error (code 2, 500, 502, 503, 504, is_transient: true)
    Transient,
    /// Policy violations or message window blocks (code 10, 2018278, etc.)
    PolicyViolation,
    /// Other Graph API errors
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaGraphError {
    pub http_status: u16,
    pub code: Option<i64>,
    pub subcode: Option<i64>,
    pub message: String,
    pub error_type: Option<String>,
    pub fbtrace_id: Option<String>,
    pub is_transient: bool,
    pub kind: MetaErrorKind,
}

impl MetaGraphError {
    pub fn parse(http_status: u16, body: &serde_json::Value) -> Self {
        let err_obj = body.get("error").unwrap_or(body);
        let message = err_obj
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown Meta API error")
            .to_string();
        let code = err_obj.get("code").and_then(|c| c.as_i64());
        let subcode = err_obj.get("error_subcode").and_then(|c| c.as_i64());
        let error_type = err_obj
            .get("type")
            .and_then(|t| t.as_str())
            .map(String::from);
        let fbtrace_id = err_obj
            .get("fbtrace_id")
            .and_then(|t| t.as_str())
            .map(String::from);
        let is_transient = err_obj
            .get("is_transient")
            .and_then(|t| t.as_bool())
            .unwrap_or(false);

        let kind = if http_status == 401
            || code == Some(190)
            || code == Some(102)
            || message.contains("Error validating access token")
            || message.contains("Session has expired")
            || message.contains("The access token could not be decrypted")
        {
            MetaErrorKind::AuthFailure
        } else if is_transient
            || code == Some(2)
            || http_status == 500
            || http_status == 502
            || http_status == 503
            || http_status == 504
        {
            MetaErrorKind::Transient
        } else if http_status == 429
            || code == Some(4)
            || code == Some(17)
            || code == Some(32)
            || code == Some(613)
        {
            MetaErrorKind::RateLimited
        } else if code == Some(10) || code == Some(2018278) || message.contains("24 hours") {
            MetaErrorKind::PolicyViolation
        } else {
            MetaErrorKind::Other
        };

        Self {
            http_status,
            code,
            subcode,
            message,
            error_type,
            fbtrace_id,
            is_transient,
            kind,
        }
    }

    pub fn is_auth_failure(&self) -> bool {
        self.kind == MetaErrorKind::AuthFailure
    }

    pub fn is_transient(&self) -> bool {
        self.kind == MetaErrorKind::Transient || self.is_transient
    }
}

impl fmt::Display for MetaGraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = self.code {
            write!(f, "Meta Graph API error (code {}): {}", code, self.message)
        } else {
            write!(
                f,
                "Meta Graph API error (status {}): {}",
                self.http_status, self.message
            )
        }
    }
}

impl std::error::Error for MetaGraphError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_429_without_a_code_is_rate_limited() {
        // Seen in the v2 production log: status 429 with no error code in the body.
        let body = serde_json::json!({
            "error": { "message": "An error has occurred. No further information is available." }
        });
        let err = MetaGraphError::parse(429, &body);
        assert_eq!(err.kind, MetaErrorKind::RateLimited);
        assert!(!err.is_auth_failure());
    }

    #[test]
    fn auth_failure_still_wins_over_rate_limit_classification() {
        let body =
            serde_json::json!({ "error": { "message": "Session has expired", "code": 190 } });
        assert_eq!(
            MetaGraphError::parse(400, &body).kind,
            MetaErrorKind::AuthFailure
        );
    }
}
