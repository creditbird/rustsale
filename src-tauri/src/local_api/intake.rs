//! Compact, token-cheap views shared by REST (`?view=compact`) and MCP tools.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use serde::Serialize;

use super::error::{ApiError, ApiResult};
use crate::conversation::caption::text_or_caption;
use crate::conversation::commands::ConversationState;
use crate::conversation::types::{
    ConversationMessage, InboxItem, InboxItemKind, MessageDirection, MessageStatus,
    SendConversationMessageRequest, TimelineCursor,
};

pub const INBOX_DEFAULT: u32 = 20;
pub const INBOX_MAX: u32 = 50;
pub const TIMELINE_DEFAULT: u32 = 10;
pub const TIMELINE_MAX: u32 = 50;
pub const TEXT_MAX_CHARS: usize = 2000;
const PREVIEW_CHARS: usize = 80;
const MESSAGE_CHARS: usize = 1000;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InboxEntry {
    pub id: i64,
    pub name: String,
    pub unread: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    pub channel_account_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TimelineEntry {
    pub id: i64,
    pub dir: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub at: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub att: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub st: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub struct TimelineView {
    pub items: Vec<TimelineEntry>,
    pub next: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MarkReadView {
    pub ok: bool,
    pub marked: u64,
}

#[derive(Debug, Serialize)]
pub struct SendView {
    pub ok: bool,
    pub id: i64,
    pub status: &'static str,
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((idx, _)) => format!("{}…", &text[..idx]),
        None => text.to_string(),
    }
}

pub fn check_limit(limit: Option<u32>, default: u32, max: u32) -> ApiResult<u32> {
    match limit {
        None => Ok(default),
        Some(n) if (1..=max).contains(&n) => Ok(n),
        Some(_) => Err(ApiError::bad_request(format!(
            "limit must be between 1 and {max}"
        ))),
    }
}

pub fn encode_cursor(cursor: &TimelineCursor) -> String {
    URL_SAFE_NO_PAD.encode(format!("{}|{}", cursor.message_id, cursor.occurred_at))
}

pub fn decode_cursor(raw: &str) -> ApiResult<TimelineCursor> {
    let invalid = || ApiError::bad_request("invalid cursor");
    let bytes = URL_SAFE_NO_PAD.decode(raw.trim()).map_err(|_| invalid())?;
    let text = String::from_utf8(bytes).map_err(|_| invalid())?;
    let (id, at) = text.split_once('|').ok_or_else(invalid)?;
    Ok(TimelineCursor {
        occurred_at: at.to_string(),
        message_id: id.parse().map_err(|_| invalid())?,
    })
}

/// Pure mapping, unit-tested: direct (non-group) chats with unread messages, newest first.
pub fn compact_inbox(items: Vec<InboxItem>, limit: u32) -> Vec<InboxEntry> {
    let mut out: Vec<InboxEntry> = items
        .into_iter()
        .filter(|item| item.kind != InboxItemKind::Group && item.unread_count > 0)
        .filter_map(|item| {
            // A lead can span several chats; reply on the most recently active one.
            let target = item
                .reply_targets
                .iter()
                .max_by(|a, b| a.last_message_at.cmp(&b.last_message_at));
            let id = target.map(|t| t.conversation_id).or(item.conversation_id)?;
            Some(InboxEntry {
                id,
                name: item.title,
                unread: item.unread_count,
                preview: item
                    .last_message_preview
                    .as_deref()
                    .map(|p| truncate(p, PREVIEW_CHARS)),
                channel_account_id: target.map(|t| t.channel_account_id),
                at: item.last_message_at,
            })
        })
        .collect();
    out.sort_by(|a, b| b.at.cmp(&a.at));
    out.truncate(limit as usize);
    out
}

fn compact_message(message: ConversationMessage) -> TimelineEntry {
    let dir = match message.direction {
        MessageDirection::Inbound => "in",
        MessageDirection::Outbound => "out",
        MessageDirection::System => "sys",
    };
    let st = match (message.direction, message.status) {
        (
            MessageDirection::Outbound,
            s @ (MessageStatus::Queued | MessageStatus::Sending | MessageStatus::Failed),
        ) => Some(s.as_str()),
        _ => None,
    };
    TimelineEntry {
        id: message.id,
        dir,
        text: text_or_caption(
            &message.content_type,
            message.text_content.as_deref().filter(|t| !t.is_empty()),
            &message.content,
            MESSAGE_CHARS + 1,
        )
        .map(|t| truncate(&t, MESSAGE_CHARS)),
        at: message.occurred_at,
        att: message
            .attachments
            .into_iter()
            .map(|a| a.media_type)
            .collect(),
        st,
    }
}

pub fn ensure_conversation(state: &ConversationState, conversation_id: i64) -> ApiResult<i64> {
    state
        .service
        .repository
        .get_conversation_channel_account_id(conversation_id)?
        .ok_or_else(|| ApiError::not_found(format!("conversation {conversation_id} not found")))
}

pub fn inbox_unread(state: &ConversationState, limit: Option<u32>) -> ApiResult<Vec<InboxEntry>> {
    let limit = check_limit(limit, INBOX_DEFAULT, INBOX_MAX)?;
    Ok(compact_inbox(state.service.list_inbox()?, limit))
}

pub fn read_timeline(
    state: &ConversationState,
    conversation_id: i64,
    limit: Option<u32>,
    before: Option<&str>,
) -> ApiResult<TimelineView> {
    let limit = check_limit(limit, TIMELINE_DEFAULT, TIMELINE_MAX)?;
    let before = before
        .filter(|b| !b.trim().is_empty())
        .map(decode_cursor)
        .transpose()?;
    ensure_conversation(state, conversation_id)?;
    let page = state
        .service
        .get_conversation_timeline(conversation_id, before.as_ref(), limit)?;
    let mut items: Vec<TimelineEntry> = page.items.into_iter().map(compact_message).collect();
    items.sort_by(|a, b| a.at.cmp(&b.at).then(a.id.cmp(&b.id)));
    Ok(TimelineView {
        items,
        next: page.next_cursor.as_ref().map(encode_cursor),
    })
}

pub fn mark_read(state: &ConversationState, conversation_id: i64) -> ApiResult<u64> {
    ensure_conversation(state, conversation_id)?;
    Ok(state
        .service
        .mark_conversation_read(&format!("conversation:{conversation_id}"))?)
}

/// Validates, resolves the channel account when omitted, enqueues and wakes the senders.
pub fn send_message(
    state: &ConversationState,
    conversation_id: i64,
    text: String,
    channel_account_id: Option<i64>,
    reply_to_message_id: Option<i64>,
) -> ApiResult<ConversationMessage> {
    let chars = text.trim().chars().count();
    if chars == 0 || text.chars().count() > TEXT_MAX_CHARS {
        return Err(ApiError::bad_request(format!(
            "text must be 1..{TEXT_MAX_CHARS} characters"
        )));
    }
    let owner = ensure_conversation(state, conversation_id)?;
    if let Some(given) = channel_account_id {
        if given != owner {
            return Err(ApiError::bad_request(
                "channelAccountId does not match the conversation",
            ));
        }
    }
    let message = state
        .service
        .enqueue_message(&SendConversationMessageRequest {
            conversation_id,
            channel_account_id: owner,
            text,
            reply_to_message_id,
        })?;
    state.zalo_supervisor.notify_outbox();
    state.whatsapp_supervisor.notify_outbox();
    Ok(message)
}

#[cfg(test)]
mod compact_message_tests {
    use super::*;
    use crate::conversation::types::{Channel, ConversationMessageActions};
    use serde_json::json;

    fn message(kind: &str, text: Option<&str>, content: serde_json::Value) -> ConversationMessage {
        ConversationMessage {
            id: 1,
            conversation_id: 2,
            channel_account_id: 3,
            channel: Channel::Zalo,
            account_display_name: None,
            sender_identity_id: None,
            sender_external_id: None,
            sender_display_name: None,
            sender_avatar_url: None,
            direction: MessageDirection::Inbound,
            provider_message_id: None,
            provider_client_message_id: None,
            content_type: kind.to_string(),
            text_content: text.map(str::to_string),
            content,
            occurred_at: "2026-11-19T09:42:00Z".to_string(),
            status: MessageStatus::Received,
            removed_at: None,
            error_message: None,
            reply_to_message_id: None,
            attachments: Vec::new(),
            reply_target: None,
            reactions: Vec::new(),
            batch_index: None,
            send_batch: None,
            mentions: Vec::new(),
            actions: ConversationMessageActions::default(),
        }
    }

    fn text_of(message: ConversationMessage) -> Option<String> {
        compact_message(message).text
    }

    #[test]
    fn image_caption_fills_empty_text() {
        let m = message("image", None, json!({ "title": " cont này " }));
        assert_eq!(text_of(m).as_deref(), Some("cont này"));
        let m = message("image", Some(""), json!({ "title": "cap" }));
        assert_eq!(text_of(m).as_deref(), Some("cap"));
    }

    #[test]
    fn image_without_caption_has_no_text() {
        assert_eq!(text_of(message("image", None, json!({}))), None);
        assert_eq!(
            text_of(message("image", None, json!({ "title": " " }))),
            None
        );
    }

    #[test]
    fn plain_text_is_unchanged() {
        let m = message("text", Some("xin chào"), json!({ "title": "ignored" }));
        assert_eq!(text_of(m).as_deref(), Some("xin chào"));
    }

    #[test]
    fn video_caption_is_used() {
        let m = message("video", None, json!({ "title": "clip" }));
        assert_eq!(text_of(m).as_deref(), Some("clip"));
    }

    #[test]
    fn unsupported_title_is_not_used() {
        let m = message(
            "unsupported",
            None,
            json!({ "title": "https://example.test" }),
        );
        assert_eq!(text_of(m), None);
    }

    #[test]
    fn long_caption_is_truncated_with_ellipsis() {
        let m = message(
            "image",
            None,
            json!({ "title": "a".repeat(MESSAGE_CHARS + 10) }),
        );
        let text = text_of(m).unwrap_or_default();
        assert_eq!(text.chars().count(), MESSAGE_CHARS + 1);
        assert!(text.ends_with('…'));
    }

    #[test]
    fn json_shape_is_unchanged() {
        let mut m = message("image", None, json!({ "title": "cap" }));
        m.attachments
            .push(crate::conversation::types::ConversationAttachment {
                id: 9,
                media_type: "image".to_string(),
                mime_type: "image/jpeg".to_string(),
                file_name: None,
                size_bytes: None,
                local_path: None,
                remote_url: None,
                thumbnail_url: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        let value = serde_json::to_value(compact_message(m)).unwrap_or_default();
        let mut keys: Vec<_> = value
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        keys.sort();
        assert_eq!(keys, ["at", "att", "dir", "id", "text"]);
        assert_eq!(value["att"], json!(["image"]));
    }
}

/// Manual check against a COPY of `rustsale.db` (never the live file):
/// `RUSTSALE_CAPTION_CHECK_DB=/tmp/copy/rustsale.db RUSTSALE_CAPTION_CHECK_IDS=92836,92842 \
///  RUSTSALE_CAPTION_CHECK_CONV=2250596 cargo test --lib caption_check_on_db_copy -- --ignored --nocapture`
#[cfg(test)]
mod db_copy_check {
    use super::*;
    use crate::conversation::repository::ConversationRepository;
    use crate::conversation::zalo::webhook::{build_payload, ZaloWebhookMessage};

    #[test]
    #[ignore = "needs RUSTSALE_CAPTION_CHECK_DB pointing at a copy of rustsale.db"]
    fn caption_check_on_db_copy() {
        let (Ok(path), Ok(ids), Ok(conv)) = (
            std::env::var("RUSTSALE_CAPTION_CHECK_DB"),
            std::env::var("RUSTSALE_CAPTION_CHECK_IDS"),
            std::env::var("RUSTSALE_CAPTION_CHECK_CONV"),
        ) else {
            panic!("set RUSTSALE_CAPTION_CHECK_DB, _IDS and _CONV");
        };
        let live = std::env::var("HOME")
            .map(|home| format!("{home}/.local/share/com.rustsale.crm"))
            .unwrap_or_default();
        assert!(
            !path.starts_with(&live),
            "refusing to open the live database; pass a copy"
        );
        let wanted: Vec<i64> = ids
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        let conversation_id: i64 = conv.trim().parse().expect("numeric conversation id");
        let repository = ConversationRepository::new(path);
        let page = repository
            .get_conversation_timeline(conversation_id, None, 500)
            .expect("timeline");
        let mut found = 0;
        for message in page.items.into_iter().filter(|m| wanted.contains(&m.id)) {
            found += 1;
            let webhook = ZaloWebhookMessage {
                conversation_id,
                channel_account_id: message.channel_account_id,
                sender_uid: None,
                sender_name: None,
                content_type: message.content_type.clone(),
                text: crate::conversation::caption::text_or_caption(
                    &message.content_type,
                    message.text_content.as_deref(),
                    &message.content,
                    2000,
                ),
                ts: message.occurred_at.clone(),
                is_group: false,
            };
            let webhook_text = build_payload(conversation_id, vec![webhook])
                .map(|p| p.text)
                .unwrap_or_default();
            let id = message.id;
            let kind = message.content_type.clone();
            let timeline = compact_message(message).text;
            println!("id={id} type={kind}\n  webhook : {webhook_text}\n  timeline: {timeline:?}");
        }
        assert_eq!(found, wanted.len(), "not every requested id was found");
    }
}
