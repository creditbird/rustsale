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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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

    #[allow(clippy::should_implement_trait)]
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
pub struct ConversationAttachment {
    pub id: i64,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing)]
    pub local_path: Option<String>,
    pub remote_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReplyTarget {
    pub message_id: Option<i64>,
    pub provider_message_id: Option<String>,
    pub sender_display_name: Option<String>,
    pub content_type: String,
    pub text_content: Option<String>,
    pub media_type: Option<String>,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReactionActor {
    pub identity_id: Option<i64>,
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReactionGroup {
    pub reaction_icon: String,
    pub count: u32,
    pub includes_me: bool,
    pub actors: Vec<ConversationReactionActor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageSendBatchStatus {
    Queued,
    Sending,
    Sent,
    Partial,
    Failed,
}

impl MessageSendBatchStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sending => "sending",
            Self::Sent => "sent",
            Self::Partial => "partial",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSendBatchSummary {
    pub send_batch_id: String,
    pub status: MessageSendBatchStatus,
    pub total_parts: u32,
    pub sent_parts: u32,
    pub failed_parts: u32,
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
pub struct InboxReplyTarget {
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
    pub reply_targets: Vec<InboxReplyTarget>,
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
    pub provider_client_message_id: Option<String>,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content: serde_json::Value,
    pub occurred_at: String,
    pub status: MessageStatus,
    pub removed_at: Option<String>,
    pub error_message: Option<String>,
    pub reply_to_message_id: Option<i64>,
    pub attachments: Vec<ConversationAttachment>,
    pub reply_target: Option<ConversationReplyTarget>,
    pub reactions: Vec<ConversationReactionGroup>,
    pub batch_index: Option<u32>,
    pub send_batch: Option<ConversationSendBatchSummary>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkLeadRequest {
    pub lead_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelinkLeadRequest {
    pub source_lead_id: i64,
    pub target_lead_id: i64,
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
    pub capabilities: ConversationCapabilities,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConversationRequest {
    pub conversation_id: Option<i64>,
    pub identity_id: Option<i64>,
    pub lead_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartDirectConversationRequest {
    pub channel_account_id: i64,
    pub phone_or_external_id: String,
    pub display_name: Option<String>,
    pub initial_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationCapabilities {
    pub max_text_length: usize,
    pub max_attachments_per_batch: usize,
    pub max_file_size_bytes: u64,
    pub supported_image_types: Vec<String>,
    pub supported_video_types: Vec<String>,
    pub supported_audio_types: Vec<String>,
    pub supported_doc_types: Vec<String>,
    pub supports_stickers: bool,
    pub supports_quote_reply: bool,
    pub supports_recall: bool,
    pub recall_window_seconds: Option<u64>,
    pub supports_reactions: bool,
}

impl ConversationCapabilities {
    pub fn text_only() -> Self {
        Self {
            max_text_length: 2000,
            max_attachments_per_batch: 0,
            max_file_size_bytes: 0,
            supported_image_types: Vec::new(),
            supported_video_types: Vec::new(),
            supported_audio_types: Vec::new(),
            supported_doc_types: Vec::new(),
            supports_stickers: false,
            supports_quote_reply: false,
            supports_recall: false,
            recall_window_seconds: None,
            supports_reactions: false,
        }
    }

    pub fn from_json(value: &str) -> Self {
        serde_json::from_str(value).unwrap_or_else(|_| Self::text_only())
    }

    pub fn zalo_phase_one() -> Self {
        Self {
            max_text_length: 2000,
            max_attachments_per_batch: 10,
            max_file_size_bytes: 52_428_800,
            supported_image_types: vec![
                "image/jpeg".into(),
                "image/png".into(),
                "image/webp".into(),
                "image/gif".into(),
            ],
            supported_video_types: vec!["video/mp4".into()],
            supported_audio_types: vec!["audio/aac".into(), "audio/m4a".into(), "audio/mp3".into()],
            supported_doc_types: vec![
                "application/pdf".into(),
                "application/zip".into(),
                "text/plain".into(),
            ],
            supports_stickers: true,
            supports_quote_reply: true,
            supports_recall: true,
            recall_window_seconds: Some(86400),
            supports_reactions: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConversationSendPartContent {
    #[serde(rename_all = "camelCase")]
    Text { text: String },
    #[serde(rename_all = "camelCase")]
    Sticker {
        sticker_id: u32,
        cate_id: u32,
        sticker_type: u32,
    },
    #[serde(rename_all = "camelCase")]
    Attachment { staged_attachment_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSendPartRequest {
    pub client_part_id: String,
    #[serde(flatten)]
    pub content: ConversationSendPartContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendConversationBatchRequest {
    pub conversation_id: i64,
    pub client_batch_id: String,
    pub reply_to_message_id: Option<i64>,
    pub parts: Vec<ConversationSendPartRequest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PromotedStagedAttachment {
    pub staged_attachment_id: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub local_path: String,
    pub thumbnail_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendConversationBatchResult {
    pub batch_id: String,
    pub client_batch_id: String,
    pub messages: Vec<ConversationMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendConversationBatchResponse {
    pub batch_id: String,
    pub client_batch_id: String,
    pub queued_messages: Vec<ConversationMessage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedAttachmentResponse {
    pub staged_attachment_id: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
    pub expires_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloStickerCate {
    pub id: u32,
    pub name: String,
    pub icon_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloSticker {
    pub id: u32,
    pub cate_id: u32,
    pub sticker_type: u32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloStickerPack {
    pub category: ZaloStickerCate,
    pub stickers: Vec<ZaloSticker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallMessageRequest {
    pub message_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageConversationClipboardImageRequest {
    pub conversation_id: i64,
    pub file_name: String,
    pub mime_type: String,
    pub base64_data: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_request_uses_tagged_camel_case_parts() {
        let request = SendConversationBatchRequest {
            conversation_id: 31,
            client_batch_id: "batch-1".into(),
            reply_to_message_id: Some(9),
            parts: vec![ConversationSendPartRequest {
                client_part_id: "part-1".into(),
                content: ConversationSendPartContent::Attachment {
                    staged_attachment_id: "stage-1".into(),
                },
            }],
        };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["conversationId"], 31);
        assert_eq!(value["parts"][0]["clientPartId"], "part-1");
        assert_eq!(value["parts"][0]["type"], "attachment");
        assert_eq!(value["parts"][0]["stagedAttachmentId"], "stage-1");
    }

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
                capabilities: ConversationCapabilities::zalo_phase_one(),
                status: AccountConnectionStatus::Connected,
                last_connected_at: Some("2026-07-31T00:00:00.000Z".to_string()),
                last_error: None,
            }],
        };

        let json = serde_json::to_value(status).unwrap();
        assert_eq!(json["accounts"][0]["channelAccountId"], 1);
        assert_eq!(json["accounts"][0]["status"], "connected");
    }

    #[test]
    fn conversation_attachment_does_not_serialize_local_spool_path() {
        let attachment = ConversationAttachment {
            id: 1,
            media_type: "image".to_string(),
            mime_type: "image/png".to_string(),
            file_name: Some("safe.png".to_string()),
            size_bytes: Some(4),
            local_path: Some("/home/user/private/attachments-spool/stage-a.dat".to_string()),
            remote_url: Some("https://cdn.example/safe.png".to_string()),
            thumbnail_url: None,
            width: Some(1),
            height: Some(1),
            duration_seconds: None,
        };

        let json = serde_json::to_value(attachment).unwrap();
        assert!(json.get("localPath").is_none());
        assert_eq!(json["remoteUrl"], "https://cdn.example/safe.png");
    }
}
