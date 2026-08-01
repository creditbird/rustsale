use crate::conversation::repository::{NormalizedIdentity, NormalizedMessage};
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

fn content_parts(message_type: &str, content: &Value) -> (String, Option<String>, Value) {
    if let Some(text) = content.as_str() {
        return (
            "text".to_string(),
            Some(text.to_string()),
            serde_json::json!({ "text": text }),
        );
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
    (content_type.to_string(), None, content.clone())
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
                display_name: (!message.data.d_name.is_empty())
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
    let (content_type, text_content, content) = content_parts(&base.msg_type, &base.content);

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
        content_type,
        text_content,
        content,
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
