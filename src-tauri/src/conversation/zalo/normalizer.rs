use crate::conversation::repository::{
    NormalizedAttachment, NormalizedIdentity, NormalizedMention, NormalizedMessage,
    NormalizedReplyReference,
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
#[allow(clippy::large_enum_variant)]
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
    let parsed_content_buf = content
        .as_str()
        .filter(|s| s.trim().starts_with('{'))
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let effective_content = parsed_content_buf.as_ref().unwrap_or(content);

    let is_rich_media_type = matches!(
        message_type,
        "chat.photo" | "chat.video.msg" | "chat.voice" | "share.file" | "chat.sticker"
    );

    if let Some(text) = effective_content.as_str() {
        if !is_rich_media_type {
            let reply = extract_reply(quote_obj);
            return NormalizedContentParts {
                content_type: "text".to_string(),
                text_content: Some(text.to_string()),
                content: serde_json::json!({ "text": text }),
                attachments: Vec::new(),
                reply,
            };
        }
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

    let text_content = effective_content
        .get("caption")
        .or_else(|| effective_content.get("description"))
        .or_else(|| effective_content.get("title"))
        .or_else(|| effective_content.get("text"))
        .or_else(|| effective_content.get("message"))
        .or_else(|| effective_content.get("subTitle"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string());

    let mut attachments = Vec::new();
    match content_type {
        "image" => {
            let url = effective_content
                .get("url")
                .or_else(|| effective_content.get("href"))
                .or_else(|| effective_content.get("hdUrl"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let thumb = effective_content
                .get("thumbUrl")
                .or_else(|| effective_content.get("thumb_url"))
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
                    metadata_json: Some(effective_content.clone()),
                });
            }
        }
        "video" => {
            let url = effective_content
                .get("href")
                .or_else(|| effective_content.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let thumb = effective_content
                .get("thumbUrl")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let name = effective_content
                .get("file_name")
                .or_else(|| effective_content.get("fileName"))
                .or_else(|| effective_content.get("title"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let size = effective_content
                .get("fileSize")
                .or_else(|| effective_content.get("size"))
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
                    metadata_json: Some(effective_content.clone()),
                });
            }
        }
        "audio" => {
            let url = effective_content
                .get("href")
                .or_else(|| effective_content.get("url"))
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
                    metadata_json: Some(effective_content.clone()),
                });
            }
        }
        "file" => {
            let url = effective_content
                .get("href")
                .or_else(|| effective_content.get("url"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let name = effective_content
                .get("title")
                .or_else(|| effective_content.get("file_name"))
                .or_else(|| effective_content.get("fileName"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let size = effective_content
                .get("fileSize")
                .or_else(|| effective_content.get("size"))
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
                    metadata_json: Some(effective_content.clone()),
                });
            }
        }
        "sticker" => {
            let url = effective_content
                .get("url")
                .or_else(|| effective_content.get("spriteUrl"))
                .or_else(|| effective_content.get("sprite_url"))
                .or_else(|| effective_content.get("stickerSpriteUrl"))
                .or_else(|| effective_content.get("sticker_sprite_url"))
                .or_else(|| effective_content.get("staticUrl"))
                .or_else(|| effective_content.get("static_url"))
                .or_else(|| effective_content.get("webpUrl"))
                .or_else(|| effective_content.get("webp_url"))
                .or_else(|| effective_content.get("stickerWebpUrl"))
                .or_else(|| effective_content.get("sticker_webp_url"))
                .or_else(|| effective_content.get("webUrl"))
                .or_else(|| effective_content.get("web_url"))
                .or_else(|| effective_content.get("stickerUrl"))
                .or_else(|| effective_content.get("sticker_url"))
                .or_else(|| effective_content.get("thumbUrl"))
                .or_else(|| effective_content.get("thumb_url"))
                .or_else(|| effective_content.get("thumbnailUrl"))
                .or_else(|| effective_content.get("thumbnail_url"))
                .or_else(|| effective_content.get("previewUrl"))
                .or_else(|| effective_content.get("preview_url"))
                .or_else(|| effective_content.get("src"))
                .or_else(|| effective_content.get("href"))
                .or_else(|| effective_content.get("path"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let provider_attachment_id = effective_content
                .get("id")
                .or_else(|| effective_content.get("stickerId"))
                .or_else(|| effective_content.get("sticker_id"))
                .and_then(|v| {
                    v.as_u64()
                        .map(|id| id.to_string())
                        .or_else(|| v.as_str().map(str::to_string))
                });
            attachments.push(NormalizedAttachment {
                provider_attachment_id,
                media_type: "sticker".to_string(),
                mime_type: "image/webp".to_string(),
                file_name: None,
                size_bytes: None,
                remote_url: url,
                thumbnail_url: None,
                metadata_json: Some(effective_content.clone()),
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
            let display_name = if !message.is_self && !message.data.d_name.trim().is_empty() {
                let d_name = message.data.d_name.trim();
                let matches_account = account_display_name
                    .map(|acc| acc.trim().eq_ignore_ascii_case(d_name))
                    .unwrap_or(false);
                if matches_account {
                    None
                } else {
                    Some(d_name.to_string())
                }
            } else {
                None
            };
            let participant = Some(NormalizedIdentity {
                identity_namespace: "zalo_uid".to_string(),
                external_id: message.thread_id.clone(),
                display_name,
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
    let mentions = extract_zalo_mentions(parts.text_content.as_deref(), &parts.content);

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
        mentions,
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

fn extract_zalo_mentions(text: Option<&str>, content: &Value) -> Vec<NormalizedMention> {
    let mut mentions = Vec::new();
    let mut mentions_arr = content.get("mentions").and_then(Value::as_array).cloned();

    if mentions_arr.as_ref().is_none_or(|items| items.is_empty()) {
        let ext_val = content
            .get("propertyExt")
            .or_else(|| content.get("property_ext"))
            .and_then(|p| p.get("ext"));
        if let Some(ext) = ext_val {
            let parsed_ext = if let Some(s) = ext.as_str() {
                serde_json::from_str::<Value>(s).ok()
            } else {
                Some(ext.clone())
            };
            if let Some(pe) = parsed_ext {
                if let Some(arr) = pe.get("mentions").and_then(Value::as_array) {
                    mentions_arr = Some(arr.clone());
                }
            }
        }
    }

    let mentions_arr = match mentions_arr {
        Some(arr) if !arr.is_empty() => arr,
        _ => return mentions,
    };

    for item in mentions_arr {
        let obj = match item.as_object() {
            Some(o) => o,
            None => continue,
        };

        let target_uid = obj
            .get("target_uid")
            .or_else(|| obj.get("uid"))
            .or_else(|| obj.get("targetUid"))
            .or_else(|| obj.get("userId"))
            .or_else(|| obj.get("target_external_id"))
            .and_then(Value::as_str);

        let pos = obj
            .get("utf16_offset")
            .or_else(|| obj.get("offset"))
            .or_else(|| obj.get("pos"))
            .or_else(|| obj.get("start_utf16"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;

        let len = obj
            .get("utf16_len")
            .or_else(|| obj.get("len"))
            .or_else(|| obj.get("length_utf16"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;

        let display_text = obj
            .get("mention_text")
            .or_else(|| obj.get("display_text"))
            .or_else(|| obj.get("text"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| {
                text.and_then(|txt| crate::conversation::mentions::utf16_slice(txt, pos, len))
            })
            .unwrap_or_default();

        let is_all = target_uid == Some("-1")
            || target_uid == Some("all")
            || obj.get("is_all").and_then(Value::as_bool).unwrap_or(false);

        let external_id = if is_all {
            Some("all".to_string())
        } else {
            target_uid.map(String::from)
        };

        mentions.push(NormalizedMention {
            identity_id: None,
            external_id,
            display_text,
            start_utf16: pos,
            length_utf16: len,
        });
    }

    mentions
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
