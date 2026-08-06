use chrono::SecondsFormat;
use serde::{Deserialize, Deserializer, Serialize};

pub fn utc_now() -> String {
    chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

const fn default_true() -> bool {
    true
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationReactionCode {
    Heart,
    Like,
    Haha,
    Wow,
    Cry,
    Angry,
}

impl ConversationReactionCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Heart => "heart",
            Self::Like => "like",
            Self::Haha => "haha",
            Self::Wow => "wow",
            Self::Cry => "cry",
            Self::Angry => "angry",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMentionInput {
    pub pos: usize,
    pub len: usize,
    pub target_identity_id: Option<i64>,
    pub is_all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMention {
    pub pos: usize,
    pub len: usize,
    pub target_identity_id: Option<i64>,
    pub target_external_id: Option<String>,
    pub display_name: Option<String>,
    pub is_all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMentionCandidate {
    pub identity_id: Option<i64>,
    pub external_id: Option<String>,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMentionCapabilities {
    pub enabled: bool,
    #[serde(rename = "maxPerMessage", alias = "maxMentionsPerMessage")]
    pub max_mentions_per_message: usize,
    pub supports_all_mention: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetConversationReactionRequest {
    pub message_id: i64,
    pub reaction: ConversationReactionCode,
    pub active: bool,
    pub client_mutation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionMutationStatus {
    Running,
    Succeeded,
    Failed,
    DeliveryUnknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReactionMutationResult {
    pub client_mutation_id: String,
    pub message_id: i64,
    pub reaction: ConversationReactionCode,
    pub requested_active: bool,
    pub status: ReactionMutationStatus,
    #[serde(default)]
    pub reactions: Vec<ConversationReactionGroup>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationReactionCapabilities {
    pub read: bool,
    pub write: bool,
    pub supported: Vec<ConversationReactionCode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConversationReactionCapabilitiesWire {
    #[serde(default)]
    read: Option<bool>,
    #[serde(default)]
    write: Option<bool>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    supported: Vec<ConversationReactionCode>,
}

impl<'de> Deserialize<'de> for ConversationReactionCapabilities {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ConversationReactionCapabilitiesWire::deserialize(deserializer)?;
        let legacy_enabled = wire.enabled.unwrap_or(false);
        Ok(Self {
            read: wire.read.unwrap_or(legacy_enabled),
            write: wire.write.unwrap_or(legacy_enabled),
            supported: wire.supported,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardConversationMessageRequest {
    pub client_operation_id: String,
    pub source_message_id: i64,
    pub target_conversation_ids: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardOperationStatus {
    Queued,
    Sending,
    Partial,
    Sent,
    Failed,
    DeliveryUnknown,
}

impl ForwardOperationStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForwardOperationStatus::Queued => "queued",
            ForwardOperationStatus::Sending => "sending",
            ForwardOperationStatus::Partial => "partial",
            ForwardOperationStatus::Sent => "sent",
            ForwardOperationStatus::Failed => "failed",
            ForwardOperationStatus::DeliveryUnknown => "delivery_unknown",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "queued" => ForwardOperationStatus::Queued,
            "sending" => ForwardOperationStatus::Sending,
            "partial" => ForwardOperationStatus::Partial,
            "sent" => ForwardOperationStatus::Sent,
            "failed" => ForwardOperationStatus::Failed,
            "delivery_unknown" => ForwardOperationStatus::DeliveryUnknown,
            _ => ForwardOperationStatus::Queued,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardTargetStatus {
    Queued,
    Sending,
    Sent,
    Failed,
    DeliveryUnknown,
}

impl ForwardTargetStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForwardTargetStatus::Queued => "queued",
            ForwardTargetStatus::Sending => "sending",
            ForwardTargetStatus::Sent => "sent",
            ForwardTargetStatus::Failed => "failed",
            ForwardTargetStatus::DeliveryUnknown => "delivery_unknown",
        }
    }

    pub fn from_db_str(s: &str) -> Self {
        match s {
            "queued" => ForwardTargetStatus::Queued,
            "sending" => ForwardTargetStatus::Sending,
            "sent" => ForwardTargetStatus::Sent,
            "failed" => ForwardTargetStatus::Failed,
            "delivery_unknown" => ForwardTargetStatus::DeliveryUnknown,
            _ => ForwardTargetStatus::Queued,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationForwardTargetResult {
    pub target_conversation_id: i64,
    pub client_batch_id: String,
    pub status: ForwardTargetStatus,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardConversationMessageResult {
    pub client_operation_id: String,
    pub source_message_id: i64,
    pub status: ForwardOperationStatus,
    pub targets: Vec<ConversationForwardTargetResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationForwardCapabilities {
    pub enabled: bool,
    #[serde(rename = "maxTargets", alias = "maxTargetsPerForward")]
    pub max_targets_per_forward: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationQuickTemplate {
    pub id: i64,
    pub shortcut: String,
    pub title: String,
    #[serde(rename = "bodyText", alias = "content")]
    pub content: String,
    pub category: Option<String>,
    pub is_active: bool,
    pub usage_count: u32,
    pub last_used_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConversationQuickTemplateRequest {
    pub shortcut: String,
    pub title: String,
    #[serde(rename = "bodyText", alias = "content")]
    pub content: String,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateConversationQuickTemplateRequest {
    pub id: i64,
    pub shortcut: String,
    pub title: String,
    #[serde(rename = "bodyText", alias = "content")]
    pub content: String,
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationQuickTemplatesRequest {
    pub category: Option<String>,
    pub search_query: Option<String>,
    #[serde(default)]
    pub include_inactive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConversationQuickTemplateRequest {
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkConversationQuickTemplateUsedRequest {
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationMentionCandidatesRequest {
    pub conversation_id: i64,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryConversationForwardTargetRequest {
    pub client_operation_id: String,
    pub target_conversation_id: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationQuickTemplateCapabilities {
    pub enabled: bool,
    pub max_templates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessageActions {
    pub copy: bool,
    pub forward: bool,
    pub react: bool,
    pub recall: bool,
    pub reply: bool,
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
    #[serde(skip_serializing)]
    pub provider_message_id: Option<String>,
    #[serde(skip_serializing)]
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
    #[serde(default)]
    pub mentions: Vec<ConversationMention>,
    #[serde(default)]
    pub actions: ConversationMessageActions,
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
    pub reactions: ConversationReactionCapabilities,
    pub forward: ConversationForwardCapabilities,
    pub mentions: ConversationMentionCapabilities,
    pub quick_templates: ConversationQuickTemplateCapabilities,
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
            reactions: ConversationReactionCapabilities {
                read: false,
                write: false,
                supported: Vec::new(),
            },
            forward: ConversationForwardCapabilities {
                enabled: false,
                max_targets_per_forward: 0,
            },
            mentions: ConversationMentionCapabilities {
                enabled: false,
                max_mentions_per_message: 0,
                supports_all_mention: false,
            },
            quick_templates: ConversationQuickTemplateCapabilities {
                enabled: false,
                max_templates: 0,
            },
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
            reactions: ConversationReactionCapabilities {
                read: true,
                write: true,
                supported: vec![
                    ConversationReactionCode::Heart,
                    ConversationReactionCode::Like,
                    ConversationReactionCode::Haha,
                    ConversationReactionCode::Wow,
                    ConversationReactionCode::Cry,
                    ConversationReactionCode::Angry,
                ],
            },
            forward: ConversationForwardCapabilities {
                enabled: true,
                max_targets_per_forward: 10,
            },
            mentions: ConversationMentionCapabilities {
                enabled: true,
                max_mentions_per_message: 20,
                supports_all_mention: true,
            },
            quick_templates: ConversationQuickTemplateCapabilities {
                enabled: true,
                max_templates: 500,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConversationSendPartContent {
    #[serde(rename_all = "camelCase")]
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        mentions: Vec<ConversationMentionInput>,
    },
    #[serde(rename_all = "camelCase")]
    Sticker {
        sticker_id: u32,
        cate_id: u32,
        sticker_type: u32,
    },
    #[serde(rename_all = "camelCase")]
    Attachment { staged_attachment_id: String },
    #[serde(rename_all = "camelCase")]
    Forward { source_message_id: i64 },
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

    #[test]
    fn phase2a_contract_capabilities() {
        let zalo = ConversationCapabilities::zalo_phase_one();
        assert!(zalo.reactions.read);
        assert!(zalo.reactions.write);
        assert_eq!(zalo.reactions.supported.len(), 6);
        assert!(zalo
            .reactions
            .supported
            .contains(&ConversationReactionCode::Heart));
        assert_eq!(zalo.forward.max_targets_per_forward, 10);
        assert_eq!(zalo.mentions.max_mentions_per_message, 20);
        assert_eq!(zalo.quick_templates.max_templates, 500);

        let text_only = ConversationCapabilities::text_only();
        assert!(!text_only.reactions.read);
        assert!(!text_only.reactions.write);
        assert!(text_only.reactions.supported.is_empty());
        assert!(!text_only.forward.enabled);
        assert_eq!(text_only.forward.max_targets_per_forward, 0);
        assert!(!text_only.mentions.enabled);
        assert_eq!(text_only.mentions.max_mentions_per_message, 0);
        assert!(!text_only.quick_templates.enabled);
        assert_eq!(text_only.quick_templates.max_templates, 0);
    }

    #[test]
    fn phase2a_contract_serialization() {
        let req = SetConversationReactionRequest {
            message_id: 101,
            reaction: ConversationReactionCode::Heart,
            active: true,
            client_mutation_id: "mut-101".into(),
        };
        let val = serde_json::to_value(&req).unwrap();
        assert_eq!(val["messageId"], 101);
        assert_eq!(val["reaction"], "heart");
        assert_eq!(val["active"], true);
        assert_eq!(val["clientMutationId"], "mut-101");
        assert!(val.get("reactionCode").is_none());

        let parsed: SetConversationReactionRequest = serde_json::from_value(serde_json::json!({
            "messageId": 101,
            "reaction": "heart",
            "active": false,
            "clientMutationId": "mut-102"
        }))
        .unwrap();
        assert_eq!(parsed.reaction, ConversationReactionCode::Heart);
        assert!(!parsed.active);
        assert_eq!(parsed.client_mutation_id, "mut-102");

        let reaction_result = ConversationReactionMutationResult {
            client_mutation_id: "mut-101".into(),
            message_id: 101,
            reaction: ConversationReactionCode::Heart,
            requested_active: true,
            status: ReactionMutationStatus::Running,
            reactions: Vec::new(),
            error_code: None,
            error_message: None,
        };
        let reaction_result_val = serde_json::to_value(reaction_result).unwrap();
        assert_eq!(reaction_result_val["clientMutationId"], "mut-101");
        assert_eq!(reaction_result_val["reaction"], "heart");
        assert_eq!(reaction_result_val["requestedActive"], true);
        assert_eq!(reaction_result_val["status"], "running");

        let fwd_req = ForwardConversationMessageRequest {
            client_operation_id: "op-1".into(),
            source_message_id: 50,
            target_conversation_ids: vec![1, 2, 3],
        };
        let fwd_val = serde_json::to_value(&fwd_req).unwrap();
        assert_eq!(fwd_val["clientOperationId"], "op-1");
        assert_eq!(fwd_val["sourceMessageId"], 50);
        assert_eq!(fwd_val["targetConversationIds"][0], 1);

        let target_result = ConversationForwardTargetResult {
            target_conversation_id: 1,
            client_batch_id: "op-1:1".into(),
            status: ForwardTargetStatus::Queued,
            error_code: None,
            error_message: None,
        };
        let target_result_val = serde_json::to_value(target_result).unwrap();
        assert_eq!(target_result_val["targetConversationId"], 1);
        assert_eq!(target_result_val["clientBatchId"], "op-1:1");
        assert!(target_result_val.get("conversationId").is_none());
        assert!(target_result_val.get("forwardedMessageId").is_none());

        let capabilities =
            serde_json::to_value(ConversationCapabilities::zalo_phase_one()).unwrap();
        assert_eq!(capabilities["forward"]["maxTargets"], 10);
        assert!(capabilities["forward"]
            .get("maxTargetsPerForward")
            .is_none());
        assert_eq!(capabilities["mentions"]["maxPerMessage"], 20);
        assert!(capabilities["mentions"]
            .get("maxMentionsPerMessage")
            .is_none());

        let actions = serde_json::to_value(ConversationMessageActions::default()).unwrap();
        assert_eq!(actions["copy"], false);

        let tmpl_req = CreateConversationQuickTemplateRequest {
            shortcut: "hello".into(),
            title: "Hello Template".into(),
            content: "Hello there!".into(),
            category: Some("greeting".into()),
        };
        let tmpl_val = serde_json::to_value(&tmpl_req).unwrap();
        assert_eq!(tmpl_val["shortcut"], "hello");

        let send_text_with_mentions = ConversationSendPartRequest {
            client_part_id: "part-text-1".into(),
            content: ConversationSendPartContent::Text {
                text: "Hello @User".into(),
                mentions: vec![ConversationMentionInput {
                    pos: 6,
                    len: 5,
                    target_identity_id: Some(42),
                    is_all: false,
                }],
            },
        };
        let part_val = serde_json::to_value(&send_text_with_mentions).unwrap();
        assert_eq!(part_val["type"], "text");
        assert_eq!(part_val["mentions"][0]["targetIdentityId"], 42);

        let send_forward = ConversationSendPartRequest {
            client_part_id: "part-fwd-1".into(),
            content: ConversationSendPartContent::Forward {
                source_message_id: 99,
            },
        };
        let fwd_part_val = serde_json::to_value(&send_forward).unwrap();
        assert_eq!(fwd_part_val["type"], "forward");
        assert_eq!(fwd_part_val["sourceMessageId"], 99);
    }

    #[test]
    fn quick_template_frontend_contract_serialization() {
        let create = serde_json::from_value::<CreateConversationQuickTemplateRequest>(
            serde_json::json!({
                "shortcut": "hello",
                "title": "Greeting",
                "bodyText": "Hello world",
                "category": "general"
            }),
        );
        assert!(create.is_ok());

        let update = serde_json::from_value::<UpdateConversationQuickTemplateRequest>(
            serde_json::json!({
                "id": 5,
                "shortcut": "hello",
                "title": "Greeting",
                "bodyText": "Hello world",
                "category": "general",
                "isActive": false
            }),
        );
        assert!(update.is_ok());

        let list = serde_json::from_value::<ListConversationQuickTemplatesRequest>(
            serde_json::json!({ "includeInactive": true }),
        )
        .unwrap();
        let list_value = serde_json::to_value(list).unwrap();
        assert_eq!(list_value["includeInactive"], true);

        let template = serde_json::from_value::<ConversationQuickTemplate>(serde_json::json!({
            "id": 5,
            "shortcut": "hello",
            "title": "Greeting",
            "bodyText": "Hello world",
            "category": "general",
            "isActive": true,
            "usageCount": 0,
            "lastUsedAt": null,
            "createdAt": "2026-08-03T00:00:00Z",
            "updatedAt": "2026-08-03T00:00:00Z"
        }))
        .unwrap();
        let template_value = serde_json::to_value(template).unwrap();
        assert_eq!(template_value["bodyText"], "Hello world");
        assert_eq!(template_value["isActive"], true);
    }

    #[test]
    fn phase2a_legacy_reaction_capability_json_maps_enabled_to_read_and_write() {
        let capabilities: ConversationReactionCapabilities =
            serde_json::from_value(serde_json::json!({
                "enabled": true,
                "supported": ["heart", "like"]
            }))
            .unwrap();

        assert!(capabilities.read);
        assert!(capabilities.write);
        assert_eq!(capabilities.supported.len(), 2);
    }

    #[test]
    fn conversation_message_json_keeps_provider_ids_backend_owned() {
        let message = ConversationMessage {
            id: 1,
            conversation_id: 2,
            channel_account_id: 3,
            channel: Channel::Zalo,
            account_display_name: Some("Sales".into()),
            sender_identity_id: None,
            sender_display_name: Some("Peer".into()),
            direction: MessageDirection::Inbound,
            provider_message_id: Some("provider-message".into()),
            provider_client_message_id: Some("provider-client-message".into()),
            content_type: "text".into(),
            text_content: Some("Hello".into()),
            content: serde_json::json!({"text": "Hello"}),
            occurred_at: "2026-08-03T10:00:00.000Z".into(),
            status: MessageStatus::Received,
            removed_at: None,
            error_message: None,
            reply_to_message_id: Some(9),
            attachments: Vec::new(),
            reply_target: Some(ConversationReplyTarget {
                message_id: Some(9),
                sender_display_name: Some("Peer".into()),
                content_type: "text".into(),
                text_content: Some("Previous".into()),
                media_type: None,
                occurred_at: Some("2026-08-03T09:59:00.000Z".into()),
            }),
            reactions: Vec::new(),
            batch_index: None,
            send_batch: None,
            mentions: Vec::new(),
            actions: ConversationMessageActions::default(),
        };

        let value = serde_json::to_value(message).unwrap();
        assert!(value.get("providerMessageId").is_none());
        assert!(value.get("providerClientMessageId").is_none());
        assert!(value["replyTarget"].get("providerMessageId").is_none());
    }
}
