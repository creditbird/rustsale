use crate::conversation::repository::{
    NormalizedAttachment, NormalizedIdentity, NormalizedMessage, NormalizedReplyReference,
};
use crate::conversation::types::{Channel, ConversationKind, MessageDirection, MessageStatus};
use anyhow::Result;
use chrono::{SecondsFormat, TimeZone, Utc};
use serde_json::Value;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{DeliveredMessage, Message, Reaction, SeenMessage};

#[derive(Debug, Clone)]
pub struct NormalizedReaction {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_event_id: Option<String>,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub actor_external_id: String,
    pub reaction: String,
    pub removed: bool,
    pub occurred_at: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct NormalizedUndo {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub occurred_at: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptKind {
    Delivered,
    Read,
}

#[derive(Debug, Clone)]
pub struct NormalizedReceipt {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub recipient_external_ids: Vec<String>,
    pub kind: ReceiptKind,
    pub occurred_at: String,
    pub raw_payload: Value,
}

#[derive(Debug, Clone)]
pub struct NormalizedGroupParticipant {
    pub identity: NormalizedIdentity,
    pub participant_role: String,
}

#[derive(Debug, Clone)]
pub struct NormalizedGroupSnapshot {
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub title: String,
    pub avatar_url: Option<String>,
    pub participants: Vec<NormalizedGroupParticipant>,
    pub membership_complete: bool,
    pub occurred_at: String,
}

struct NormalizedContentParts {
    content_type: String,
    text_content: Option<String>,
    content: Value,
    attachments: Vec<NormalizedAttachment>,
    reply: Option<NormalizedReplyReference>,
}

#[derive(Debug, Clone)]
pub enum ZaloMutation {
    Message(NormalizedMessage),
    Reaction(NormalizedReaction),
    Undo(NormalizedUndo),
    Receipt(NormalizedReceipt),
    GroupSnapshot(NormalizedGroupSnapshot),
    ReconcileRequired { scope: &'static str },
}

fn provider_time(value: &str) -> String {
    value
        .parse::<i64>()
        .ok()
        .and_then(|millis| Utc.timestamp_millis_opt(millis).single())
        .map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_else(crate::conversation::types::utc_now)
}

fn provider_time_u64(millis: u64) -> String {
    Utc.timestamp_millis_opt(millis as i64)
        .single()
        .map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_else(crate::conversation::types::utc_now)
}

fn content_parts(
    message_type: &str,
    content: &Value,
    quote_obj: Option<&Value>,
) -> NormalizedContentParts {
    if let Some(text) = content.as_str() {
        let reply = extract_reply(quote_obj);
        return NormalizedContentParts {
            content_type: "text".to_string(),
            text_content: Some(text.to_string()),
            content: serde_json::json!({ "text": text }),
            attachments: Vec::new(),
            reply,
        };
    }
    let content_type = match message_type {
        "chat.photo" => "image",
        "chat.video.msg" => "video",
        "chat.voice" => "audio",
        "share.file" => "file",
        "chat.sticker" => "sticker",
        "chat.link" => "link",
        _ => "unsupported",
    };

    let text_content = content
        .get("caption")
        .or_else(|| content.get("description"))
        .or_else(|| content.get("title"))
        .or_else(|| content.get("text"))
        .or_else(|| content.get("message"))
        .or_else(|| content.get("subTitle"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());

    let mut attachments = Vec::new();
    match content_type {
        "image" => {
            let url = content
                .get("url")
                .or_else(|| content.get("href"))
                .or_else(|| content.get("hdUrl"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let thumb = content
                .get("thumbUrl")
                .or_else(|| content.get("thumb_url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if url.is_some() || thumb.is_some() {
                attachments.push(NormalizedAttachment {
                    provider_attachment_id: None,
                    media_type: "image".to_string(),
                    mime_type: "image/jpeg".to_string(),
                    file_name: None,
                    size_bytes: None,
                    remote_url: url,
                    thumbnail_url: thumb,
                    metadata_json: Some(content.clone()),
                });
            }
        }
        "video" => {
            let url = content
                .get("href")
                .or_else(|| content.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let thumb = content
                .get("thumbUrl")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let name = content
                .get("file_name")
                .or_else(|| content.get("fileName"))
                .or_else(|| content.get("title"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let size = content
                .get("fileSize")
                .or_else(|| content.get("size"))
                .and_then(|v| v.as_u64());
            if url.is_some() || thumb.is_some() {
                attachments.push(NormalizedAttachment {
                    provider_attachment_id: None,
                    media_type: "video".to_string(),
                    mime_type: "video/mp4".to_string(),
                    file_name: name,
                    size_bytes: size,
                    remote_url: url,
                    thumbnail_url: thumb,
                    metadata_json: Some(content.clone()),
                });
            }
        }
        "audio" => {
            let url = content
                .get("href")
                .or_else(|| content.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if let Some(url) = url {
                attachments.push(NormalizedAttachment {
                    provider_attachment_id: None,
                    media_type: "audio".to_string(),
                    mime_type: "audio/aac".to_string(),
                    file_name: None,
                    size_bytes: None,
                    remote_url: Some(url),
                    thumbnail_url: None,
                    metadata_json: Some(content.clone()),
                });
            }
        }
        "file" => {
            let url = content
                .get("href")
                .or_else(|| content.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let name = content
                .get("title")
                .or_else(|| content.get("file_name"))
                .or_else(|| content.get("fileName"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let size = content
                .get("fileSize")
                .or_else(|| content.get("size"))
                .and_then(|v| v.as_u64());
            if url.is_some() || name.is_some() {
                attachments.push(NormalizedAttachment {
                    provider_attachment_id: None,
                    media_type: "file".to_string(),
                    mime_type: "application/octet-stream".to_string(),
                    file_name: name,
                    size_bytes: size,
                    remote_url: url,
                    thumbnail_url: None,
                    metadata_json: Some(content.clone()),
                });
            }
        }
        "sticker" => {
            let url = content
                .get("url")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            attachments.push(NormalizedAttachment {
                provider_attachment_id: content
                    .get("id")
                    .and_then(|v| v.as_u64())
                    .map(|id| id.to_string()),
                media_type: "sticker".to_string(),
                mime_type: "image/webp".to_string(),
                file_name: None,
                size_bytes: None,
                remote_url: url,
                thumbnail_url: None,
                metadata_json: Some(content.clone()),
            });
        }
        _ => {}
    }

    let reply = extract_reply(quote_obj);

    NormalizedContentParts {
        content_type: content_type.to_string(),
        text_content,
        content: content.clone(),
        attachments,
        reply,
    }
}

fn extract_reply(quote_obj: Option<&Value>) -> Option<NormalizedReplyReference> {
    let quote = quote_obj?;
    if quote.is_null() {
        return None;
    }
    let p_msg_id = quote
        .get("globalMsgId")
        .or_else(|| quote.get("msgId"))
        .and_then(|v| v.as_str())?
        .to_string();
    let cli_msg_id = quote
        .get("cliMsgId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let sender = quote
        .get("ownerId")
        .or_else(|| quote.get("uidFrom"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let text = quote
        .get("msg")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let c_type = quote
        .get("msgType")
        .and_then(|v| v.as_str())
        .unwrap_or("text")
        .to_string();
    let ts = quote
        .get("ts")
        .and_then(|v| v.as_str())
        .map(provider_time)
        .unwrap_or_else(crate::conversation::types::utc_now);

    Some(NormalizedReplyReference {
        provider_message_id: p_msg_id,
        provider_client_message_id: cli_msg_id,
        sender_external_id: sender,
        occurred_at: ts,
        text_content: text,
        content_type: c_type,
        raw_quote: quote.clone(),
    })
}

fn normalize_message(
    provider_account_key: &str,
    account_display_name: Option<&str>,
    message: Message,
) -> Result<NormalizedMessage> {
    let now = crate::conversation::types::utc_now();
    let (base, external_thread_id, is_self, kind, participant) = match message {
        Message::User(message) => {
            let participant = Some(NormalizedIdentity {
                identity_namespace: "zalo_uid".to_string(),
                external_id: message.thread_id.clone(),
                display_name: (!message.is_self && !message.data.d_name.is_empty())
                    .then(|| message.data.d_name.clone()),
                avatar_url: None,
                lead_id: None,
            });
            (
                message.data,
                message.thread_id,
                message.is_self,
                ConversationKind::Direct,
                participant,
            )
        }
        Message::Group(message) => {
            let participant = (!message.is_self).then(|| NormalizedIdentity {
                identity_namespace: "zalo_uid".to_string(),
                external_id: message.data.base.uid_from.clone(),
                display_name: (!message.data.base.d_name.is_empty())
                    .then(|| message.data.base.d_name.clone()),
                avatar_url: None,
                lead_id: None,
            });
            (
                message.data.base,
                message.thread_id,
                message.is_self,
                ConversationKind::Group,
                participant,
            )
        }
    };
    let quote_val = base
        .quote
        .as_ref()
        .and_then(|q| serde_json::to_value(q).ok());
    let parts = content_parts(&base.msg_type, &base.content, quote_val.as_ref());

    Ok(NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: provider_account_key.to_string(),
        account_display_name: account_display_name.map(str::to_string),
        participant,
        sender_is_participant: !is_self,
        external_thread_id,
        conversation_kind: kind,
        conversation_title: None,
        direction: if is_self {
            MessageDirection::Outbound
        } else {
            MessageDirection::Inbound
        },
        provider_message_id: (!base.msg_id.is_empty()).then_some(base.msg_id.clone()),
        provider_client_message_id: (!base.cli_msg_id.is_empty())
            .then_some(base.cli_msg_id.clone()),
        local_request_id: None,
        content_type: parts.content_type,
        text_content: parts.text_content,
        content: parts.content,
        attachments: parts.attachments,
        reply: parts.reply,
        raw_payload: Some(serde_json::to_value(&base)?),
        provider_timestamp: Some(provider_time(&base.ts)),
        ingested_at: now,
        status: if is_self {
            MessageStatus::Sent
        } else {
            MessageStatus::Received
        },
    })
}

fn normalize_reaction(provider_account_key: &str, reaction: &Reaction) -> Vec<ZaloMutation> {
    let reaction_str = serde_json::to_value(&reaction.data.content.r_icon)
        .ok()
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| {
            serde_json::to_string(&reaction.data.content.r_icon)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string()
        });

    let occurred_at = provider_time(&reaction.data.ts);
    let raw_payload = serde_json::to_value(&reaction.data).unwrap_or_default();

    reaction
        .data
        .content
        .r_msg
        .iter()
        .map(|r_msg| {
            ZaloMutation::Reaction(NormalizedReaction {
                provider_account_key: provider_account_key.to_string(),
                external_thread_id: reaction.thread_id.clone(),
                is_group: reaction.is_group,
                provider_event_id: (!reaction.data.msg_id.is_empty())
                    .then_some(reaction.data.msg_id.clone()),
                provider_message_id: r_msg.g_msg_id.clone(),
                provider_client_message_id: r_msg.c_msg_id.clone(),
                actor_external_id: reaction.data.uid_from.clone(),
                reaction: reaction_str.clone(),
                removed: reaction.data.content.r_type == 0,
                occurred_at: occurred_at.clone(),
                raw_payload: raw_payload.clone(),
            })
        })
        .collect()
}

pub fn normalize_listener_event(
    provider_account_key: &str,
    account_display_name: Option<&str>,
    event: ListenerEvent,
) -> Result<Vec<ZaloMutation>> {
    match event {
        ListenerEvent::Message(message) => {
            let msg = normalize_message(provider_account_key, account_display_name, *message)?;
            Ok(vec![ZaloMutation::Message(msg)])
        }
        ListenerEvent::OldMessages { messages, .. } => {
            let mut result = Vec::with_capacity(messages.len());
            for msg in messages {
                result.push(ZaloMutation::Message(normalize_message(
                    provider_account_key,
                    account_display_name,
                    msg,
                )?));
            }
            Ok(result)
        }
        ListenerEvent::Reaction(reaction) => {
            Ok(normalize_reaction(provider_account_key, &reaction))
        }
        ListenerEvent::OldReactions { reactions, .. } => {
            let mut result = Vec::new();
            for reaction in reactions {
                result.extend(normalize_reaction(provider_account_key, &reaction));
            }
            Ok(result)
        }
        ListenerEvent::Undo(undo) => {
            let norm_undo = NormalizedUndo {
                provider_account_key: provider_account_key.to_string(),
                external_thread_id: undo.thread_id.clone(),
                is_group: undo.is_group,
                provider_message_id: undo.data.msg_id.clone(),
                provider_client_message_id: undo.data.cli_msg_id.clone(),
                occurred_at: provider_time(&undo.data.ts),
                raw_payload: serde_json::to_value(&undo.data)?,
            };
            Ok(vec![ZaloMutation::Undo(norm_undo)])
        }
        ListenerEvent::DeliveredMessages(delivered_vec) => {
            let mut result = Vec::with_capacity(delivered_vec.len());
            for delivered in delivered_vec {
                match delivered {
                    DeliveredMessage::User(user_msg) => {
                        let norm_receipt = NormalizedReceipt {
                            provider_account_key: provider_account_key.to_string(),
                            external_thread_id: user_msg.thread_id.clone(),
                            is_group: false,
                            provider_message_id: user_msg.data.msg_id.clone(),
                            recipient_external_ids: user_msg.data.delivered_uids.clone(),
                            kind: ReceiptKind::Delivered,
                            occurred_at: provider_time_u64(user_msg.data.m_s_ts),
                            raw_payload: serde_json::to_value(&user_msg.data)?,
                        };
                        result.push(ZaloMutation::Receipt(norm_receipt));
                    }
                    DeliveredMessage::Group(group_msg) => {
                        let norm_receipt = NormalizedReceipt {
                            provider_account_key: provider_account_key.to_string(),
                            external_thread_id: group_msg.thread_id.clone(),
                            is_group: true,
                            provider_message_id: group_msg.data.base.msg_id.clone(),
                            recipient_external_ids: group_msg.data.base.delivered_uids.clone(),
                            kind: ReceiptKind::Delivered,
                            occurred_at: provider_time_u64(group_msg.data.base.m_s_ts),
                            raw_payload: serde_json::to_value(&group_msg.data)?,
                        };
                        result.push(ZaloMutation::Receipt(norm_receipt));
                    }
                }
            }
            Ok(result)
        }
        ListenerEvent::SeenMessages(seen_vec) => {
            let now = crate::conversation::types::utc_now();
            let mut result = Vec::with_capacity(seen_vec.len());
            for seen in seen_vec {
                match seen {
                    SeenMessage::User(user_msg) => {
                        let norm_receipt = NormalizedReceipt {
                            provider_account_key: provider_account_key.to_string(),
                            external_thread_id: user_msg.thread_id.clone(),
                            is_group: false,
                            provider_message_id: user_msg.data.msg_id.clone(),
                            recipient_external_ids: vec![user_msg.data.id_to.clone()],
                            kind: ReceiptKind::Read,
                            occurred_at: now.clone(),
                            raw_payload: serde_json::to_value(&user_msg.data)?,
                        };
                        result.push(ZaloMutation::Receipt(norm_receipt));
                    }
                    SeenMessage::Group(group_msg) => {
                        let norm_receipt = NormalizedReceipt {
                            provider_account_key: provider_account_key.to_string(),
                            external_thread_id: group_msg.thread_id.clone(),
                            is_group: true,
                            provider_message_id: group_msg.data.msg_id.clone(),
                            recipient_external_ids: group_msg.data.seen_uids.clone(),
                            kind: ReceiptKind::Read,
                            occurred_at: now.clone(),
                            raw_payload: serde_json::to_value(&group_msg.data)?,
                        };
                        result.push(ZaloMutation::Receipt(norm_receipt));
                    }
                }
            }
            Ok(result)
        }
        ListenerEvent::Disconnected { .. }
        | ListenerEvent::Closed { .. }
        | ListenerEvent::Error(_) => Ok(vec![ZaloMutation::ReconcileRequired { scope: "all" }]),
        ListenerEvent::PacketError { .. } => {
            Ok(vec![ZaloMutation::ReconcileRequired { scope: "all" }])
        }
        ListenerEvent::GroupEvent(_) => Ok(vec![ZaloMutation::ReconcileRequired {
            scope: "group_metadata",
        }]),
        ListenerEvent::Connected
        | ListenerEvent::Typing(_)
        | ListenerEvent::UploadAttachment(_)
        | ListenerEvent::FriendEvent(_) => Ok(vec![]),
    }
}
