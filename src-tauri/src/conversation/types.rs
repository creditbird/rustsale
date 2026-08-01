use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};

pub fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Zalo,
    #[serde(rename = "whatsapp")]
    WhatsApp,
    Messenger,
    Email,
}

impl Channel {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zalo => "zalo",
            Self::WhatsApp => "whatsapp",
            Self::Messenger => "messenger",
            Self::Email => "email",
        }
    }

    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "zalo" => Ok(Self::Zalo),
            "whatsapp" => Ok(Self::WhatsApp),
            "messenger" => Ok(Self::Messenger),
            "email" => Ok(Self::Email),
            _ => Err(format!("unknown channel: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationKind {
    Direct,
    Group,
    EmailThread,
}

impl ConversationKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Group => "group",
            Self::EmailThread => "email_thread",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    Inbound,
    Outbound,
    System,
}

impl MessageDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
            Self::System => "system",
        }
    }

    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "inbound" => Ok(Self::Inbound),
            "outbound" => Ok(Self::Outbound),
            "system" => Ok(Self::System),
            _ => Err(format!("unknown message direction: {s}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageStatus {
    Queued,
    Sending,
    Sent,
    Delivered,
    Read,
    Received,
    Failed,
    Removed,
}

impl MessageStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Delivered => "delivered",
            Self::Read => "read",
            Self::Received => "received",
            Self::Failed => "failed",
            Self::Removed => "removed",
        }
    }

    pub fn from_str(s: &str) -> std::result::Result<Self, String> {
        match s {
            "queued" => Ok(Self::Queued),
            "sending" => Ok(Self::Sending),
            "sent" => Ok(Self::Sent),
            "delivered" => Ok(Self::Delivered),
            "read" => Ok(Self::Read),
            "received" => Ok(Self::Received),
            "failed" => Ok(Self::Failed),
            "removed" => Ok(Self::Removed),
            _ => Err(format!("unknown message status: {s}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineCursor {
    pub occurred_at: String,
    pub message_id: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelBadgeSummary {
    pub channel: Channel,
    pub message_count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReplyTarget {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub channel: Channel,
    pub account_display_name: Option<String>,
    pub conversation_kind: ConversationKind,
    pub last_inbound_at: Option<String>,
    pub last_message_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationParticipantSummary {
    pub identity_id: i64,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub participant_role: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxItemKind {
    Customer,
    Unknown,
    Group,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InboxItem {
    pub key: String,
    pub kind: InboxItemKind,
    pub lead_id: Option<i64>,
    pub conversation_id: Option<i64>,
    pub identity_id: Option<i64>,
    pub title: String,
    pub avatar_url: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_message_at: Option<String>,
    pub channels: Vec<ChannelBadgeSummary>,
    pub reply_targets: Vec<ConversationReplyTarget>,
    pub participant_count: u64,
    pub participants: Vec<ConversationParticipantSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: i64,
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub channel: Channel,
    pub account_display_name: Option<String>,
    pub sender_identity_id: Option<i64>,
    pub sender_display_name: Option<String>,
    pub direction: MessageDirection,
    pub provider_message_id: Option<String>,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content: serde_json::Value,
    pub occurred_at: String,
    pub status: MessageStatus,
    pub removed_at: Option<String>,
    pub error_message: Option<String>,
    pub reply_to_message_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePage {
    pub items: Vec<ConversationMessage>,
    pub next_cursor: Option<TimelineCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkIdentityRequest {
    pub identity_id: i64,
    pub lead_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkIdentityRequest {
    pub identity_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Degraded,
    Invalid,
}

impl AccountConnectionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Connected => "connected",
            Self::Degraded => "degraded",
            Self::Invalid => "invalid",
        }
    }

    pub fn from_db(value: &str) -> rusqlite::Result<Self> {
        match value {
            "disconnected" => Ok(Self::Disconnected),
            "connecting" => Ok(Self::Connecting),
            "connected" => Ok(Self::Connected),
            "degraded" => Ok(Self::Degraded),
            "invalid" => Ok(Self::Invalid),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("invalid account connection status: {other}"),
                )),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationAccountRuntime {
    pub channel_account_id: i64,
    pub channel: Channel,
    pub provider_account_key: String,
    pub display_name: Option<String>,
    pub status: AccountConnectionStatus,
    pub last_connected_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationRuntimeStatus {
    pub accounts: Vec<ConversationAccountRuntime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendConversationMessageRequest {
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub text: String,
    pub reply_to_message_id: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_contract_is_snake_case() {
        assert_eq!(serde_json::to_string(&Channel::Zalo).unwrap(), "\"zalo\"");
        assert_eq!(
            serde_json::to_string(&Channel::WhatsApp).unwrap(),
            "\"whatsapp\""
        );
    }

    #[test]
    fn timeline_cursor_round_trips() {
        let cursor = TimelineCursor {
            occurred_at: "2026-07-31T01:02:03.000Z".to_string(),
            message_id: 42,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        assert_eq!(
            serde_json::from_str::<TimelineCursor>(&json).unwrap(),
            cursor
        );
    }

    #[test]
    fn utc_now_uses_fixed_millisecond_z_format() {
        let value = utc_now();
        assert!(value.ends_with('Z'));
        assert_eq!(value.len(), "2026-07-31T01:02:03.000Z".len());
    }

    #[test]
    fn runtime_status_contract_uses_camel_case_fields() {
        let status = ConversationRuntimeStatus {
            accounts: vec![ConversationAccountRuntime {
                channel_account_id: 1,
                channel: Channel::Zalo,
                provider_account_key: "imei-a".to_string(),
                display_name: Some("Sale 01".to_string()),
                status: AccountConnectionStatus::Connected,
                last_connected_at: Some("2026-07-31T00:00:00.000Z".to_string()),
                last_error: None,
            }],
        };

        let json = serde_json::to_value(status).unwrap();
        assert_eq!(json["accounts"][0]["channelAccountId"], 1);
        assert_eq!(json["accounts"][0]["status"], "connected");
    }
}
