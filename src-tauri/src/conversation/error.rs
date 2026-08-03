use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationErrorCode {
    CapabilityDenied,
    AccountDisconnected,
    StageExpired,
    StageAlreadyClaimed,
    FileTooLarge,
    UnsupportedMedia,
    QuoteNotReady,
    ProviderAuth,
    ProviderRateLimited,
    DeliveryUnknown,
    RecallNotAllowed,
    InvalidRequest,
    ProviderFailure,
    ReactionNotSupported,
    ReactionRemovalNotVerified,
    ReactionDeliveryUnknown,
    ForwardNotSupported,
    ForwardSourceMissingProviderIds,
    ForwardTargetAccountMismatch,
    ForwardTargetLimitExceeded,
    ForwardDeliveryUnknown,
    TemplateLimitExceeded,
    TemplateShortcutConflict,
    InvalidMentionRange,
    MentionParticipantNotFound,
    MentionDirectConversationUnsupported,
}

impl ConversationErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CapabilityDenied => "capability_denied",
            Self::AccountDisconnected => "account_disconnected",
            Self::StageExpired => "stage_expired",
            Self::StageAlreadyClaimed => "stage_already_claimed",
            Self::FileTooLarge => "file_too_large",
            Self::UnsupportedMedia => "unsupported_media",
            Self::QuoteNotReady => "quote_not_ready",
            Self::ProviderAuth => "provider_auth",
            Self::ProviderRateLimited => "provider_rate_limited",
            Self::DeliveryUnknown => "delivery_unknown",
            Self::RecallNotAllowed => "recall_not_allowed",
            Self::InvalidRequest => "invalid_request",
            Self::ProviderFailure => "provider_failure",
            Self::ReactionNotSupported => "reaction_not_supported",
            Self::ReactionRemovalNotVerified => "reaction_removal_not_verified",
            Self::ReactionDeliveryUnknown => "reaction_delivery_unknown",
            Self::ForwardNotSupported => "forward_not_supported",
            Self::ForwardSourceMissingProviderIds => "forward_source_missing_provider_ids",
            Self::ForwardTargetAccountMismatch => "forward_target_account_mismatch",
            Self::ForwardTargetLimitExceeded => "forward_target_limit_exceeded",
            Self::ForwardDeliveryUnknown => "forward_delivery_unknown",
            Self::TemplateLimitExceeded => "template_limit_exceeded",
            Self::TemplateShortcutConflict => "template_shortcut_conflict",
            Self::InvalidMentionRange => "invalid_mention_range",
            Self::MentionParticipantNotFound => "mention_participant_not_found",
            Self::MentionDirectConversationUnsupported => "mention_direct_conversation_unsupported",
        }
    }
}

pub type ConversationResult<T> = Result<T, ConversationError>;

#[derive(Debug, Clone)]
pub struct ConversationError {
    pub code: ConversationErrorCode,
    pub message: String,
}

impl ConversationError {
    pub fn new(code: ConversationErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn command_error(&self) -> ConversationCommandError {
        ConversationCommandError {
            code: self.code,
            message: self.message.clone(),
        }
    }
}

impl fmt::Display for ConversationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for ConversationError {}

impl From<rusqlite::Error> for ConversationError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new(ConversationErrorCode::ProviderFailure, err.to_string())
    }
}

impl From<anyhow::Error> for ConversationError {
    fn from(err: anyhow::Error) -> Self {
        Self::new(ConversationErrorCode::ProviderFailure, err.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationCommandError {
    pub code: ConversationErrorCode,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_serializes_stable_code() {
        let error = ConversationError::new(
            ConversationErrorCode::StageExpired,
            "Attachment staging token expired",
        );
        let value = serde_json::to_value(error.command_error()).unwrap();
        assert_eq!(value["code"], "stage_expired");
        assert_eq!(value["message"], "Attachment staging token expired");
    }
}
