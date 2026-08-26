use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::phone;
use crate::conversation::repository::{
    NormalizedAttachment, NormalizedIdentity, NormalizedMessage, NormalizedReplyReference,
};
use crate::conversation::types::{Channel, ConversationKind, MessageDirection, MessageStatus};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppAttachment {
    pub media_type: String, // "image", "video", "audio", "file" / "document", "sticker"
    pub mime_type: Option<String>,
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
    pub url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub direct_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppQuote {
    pub stanza_id: String,
    pub participant_jid: String,
    pub text: Option<String>,
    pub media_type: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawWhatsAppMessage {
    pub id: String,
    pub chat_jid: String,
    pub sender_jid: Option<String>,
    pub sender_name: Option<String>,
    pub text: Option<String>,
    pub timestamp: Option<String>,
    pub is_from_me: bool,
    #[serde(default)]
    pub attachments: Vec<RawWhatsAppAttachment>,
    pub quote: Option<RawWhatsAppQuote>,
    pub raw_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppReaction {
    pub message_id: String,
    pub chat_jid: String,
    pub sender_jid: String,
    pub emoji: String,
    pub timestamp: Option<String>,
    pub is_from_me: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawWhatsAppReceipt {
    pub message_ids: Vec<String>,
    pub chat_jid: String,
    pub sender_jid: Option<String>,
    pub receipt_type: String, // "delivered", "read", "played", "sent", "failed"
    pub timestamp: Option<String>,
}

pub struct WhatsAppNormalizer;

impl WhatsAppNormalizer {
    /// Normalizes an incoming timestamp into ISO-8601 UTC string (RFC 3339 with millis).
    pub fn normalize_iso_timestamp(ts_str: Option<&str>) -> String {
        if let Some(ts) = ts_str {
            let ts = ts.trim();
            if !ts.is_empty() {
                if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                    return dt
                        .with_timezone(&Utc)
                        .to_rfc3339_opts(SecondsFormat::Millis, true);
                }
                // Check if numeric unix timestamp in milliseconds or seconds
                if let Ok(num) = ts.parse::<i64>() {
                    if num > 1_000_000_000_000 {
                        // Milliseconds
                        if let Some(dt) = DateTime::from_timestamp_millis(num) {
                            return dt
                                .with_timezone(&Utc)
                                .to_rfc3339_opts(SecondsFormat::Millis, true);
                        }
                    } else if num > 0 {
                        // Seconds
                        if let Some(dt) = DateTime::from_timestamp(num, 0) {
                            return dt
                                .with_timezone(&Utc)
                                .to_rfc3339_opts(SecondsFormat::Millis, true);
                        }
                    }
                }
            }
        }
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    /// Detects canonical message content_type from RawWhatsAppMessage.
    pub fn detect_content_type(msg: &RawWhatsAppMessage) -> &'static str {
        if let Some(first_att) = msg.attachments.first() {
            match first_att.media_type.to_lowercase().as_str() {
                "image" | "photo" => "image",
                "video" => "video",
                "audio" | "voice" | "ptt" => "audio",
                "sticker" => "sticker",
                _ => "file",
            }
        } else {
            "text"
        }
    }

    /// Resolves a CRM lead ID by normalized phone number with strict safety invariants:
    /// - Exactly 1 matching lead -> Some(lead_id)
    /// - 0 matching leads -> None
    /// - >1 matching leads (duplicate phone in CRM) -> None (never guess arbitrarily).
    pub fn resolve_lead_by_phone(conn: &Connection, phone_raw: &str) -> Result<Option<i64>> {
        let phone_raw = phone_raw.trim();
        if phone_raw.is_empty() {
            return Ok(None);
        }

        // Clean user digits or normalized E.164
        let target_e164 =
            phone::normalize_e164(phone_raw, Some("84")).or_else(|| phone::jid_to_phone(phone_raw));

        let target_e164 = match target_e164 {
            Some(p) => p,
            None => return Ok(None),
        };

        // Query leads with non-empty phone
        let mut stmt = conn
            .prepare("SELECT id, phone FROM leads WHERE phone IS NOT NULL AND TRIM(phone) != ''")?;

        let mut matching_lead_ids = HashSet::new();
        let rows = stmt.query_map([], |row| {
            let id: i64 = row.get(0)?;
            let phone: String = row.get(1)?;
            Ok((id, phone))
        })?;

        for item in rows {
            let (id, lead_phone) = item?;
            let lead_phone_clean = lead_phone.trim();
            if lead_phone_clean.is_empty() {
                continue;
            }

            // Test normalized E.164 equality
            if let Some(norm_lead) = phone::normalize_e164(lead_phone_clean, Some("84")) {
                if norm_lead == target_e164 {
                    matching_lead_ids.insert(id);
                    continue;
                }
            }

            // Test domestic/stripped digits equality
            let lead_digits: String = lead_phone_clean
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if !lead_digits.is_empty() {
                let target_digits = &target_e164;
                if lead_digits == *target_digits {
                    matching_lead_ids.insert(id);
                    continue;
                }
                if let Some(rest) = lead_digits.strip_prefix("0") {
                    if format!("84{rest}") == *target_digits {
                        matching_lead_ids.insert(id);
                        continue;
                    }
                }
                if let Some(rest) = target_digits.strip_prefix("84") {
                    if format!("0{rest}") == lead_digits {
                        matching_lead_ids.insert(id);
                        continue;
                    }
                }
            }
        }

        if matching_lead_ids.len() == 1 {
            Ok(matching_lead_ids.into_iter().next())
        } else {
            // 0 or >1 matches (ambiguous) -> None
            Ok(None)
        }
    }

    /// Converts a `RawWhatsAppMessage` into a unified `NormalizedMessage`.
    pub fn normalize_message(
        provider_account_key: &str,
        account_display_name: Option<&str>,
        msg: &RawWhatsAppMessage,
    ) -> Result<NormalizedMessage> {
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let occurred_at = Self::normalize_iso_timestamp(msg.timestamp.as_deref());

        let is_group = phone::is_group_jid(&msg.chat_jid);
        let (conversation_kind, external_thread_id, conversation_title) = if is_group {
            (
                ConversationKind::Group,
                msg.chat_jid.trim().to_string(),
                msg.sender_name.clone(),
            )
        } else {
            let peer_phone = phone::jid_to_phone(&msg.chat_jid)
                .unwrap_or_else(|| msg.chat_jid.trim().to_string());
            let normalized_phone =
                phone::normalize_e164(&peer_phone, Some("84")).unwrap_or(peer_phone);
            (
                ConversationKind::Direct,
                phone::phone_to_direct_jid(&normalized_phone),
                msg.sender_name
                    .clone()
                    .or_else(|| Some(format!("+{}", normalized_phone))),
            )
        };

        let participant = if !msg.is_from_me {
            let sender_jid = msg.sender_jid.as_deref().unwrap_or(msg.chat_jid.as_str());
            let sender_phone = phone::jid_to_phone(sender_jid)
                .or_else(|| phone::normalize_e164(sender_jid, Some("84")));
            let ext_id = sender_phone.unwrap_or_else(|| sender_jid.to_string());
            Some(NormalizedIdentity {
                identity_namespace: "phone".to_string(),
                external_id: ext_id,
                display_name: msg.sender_name.clone(),
                avatar_url: None,
                lead_id: None,
            })
        } else {
            None
        };

        let content_type = Self::detect_content_type(msg).to_string();

        let attachments: Vec<NormalizedAttachment> = msg
            .attachments
            .iter()
            .map(|att| {
                let default_mime = match att.media_type.as_str() {
                    "image" | "photo" => "image/jpeg",
                    "video" => "video/mp4",
                    "audio" | "voice" | "ptt" => "audio/ogg",
                    "sticker" => "image/webp",
                    _ => "application/octet-stream",
                };
                NormalizedAttachment {
                    provider_attachment_id: None,
                    media_type: att.media_type.clone(),
                    mime_type: att
                        .mime_type
                        .clone()
                        .unwrap_or_else(|| default_mime.to_string()),
                    file_name: att.file_name.clone(),
                    size_bytes: att.size_bytes,
                    remote_url: att.url.clone(),
                    thumbnail_url: att.thumbnail_url.clone(),
                    metadata_json: att
                        .direct_path
                        .as_ref()
                        .map(|p| serde_json::json!({ "direct_path": p })),
                }
            })
            .collect();

        let reply = msg.quote.as_ref().map(|q| NormalizedReplyReference {
            provider_message_id: q.stanza_id.clone(),
            provider_client_message_id: String::new(),
            sender_external_id: q.participant_jid.clone(),
            occurred_at: Self::normalize_iso_timestamp(q.timestamp.as_deref()),
            text_content: q.text.clone(),
            content_type: q.media_type.clone().unwrap_or_else(|| "text".to_string()),
            raw_quote: serde_json::to_value(q).unwrap_or_default(),
        });

        let content_json = serde_json::json!({
            "text": msg.text.as_deref().unwrap_or(""),
        });

        Ok(NormalizedMessage {
            channel: Channel::WhatsApp,
            provider_account_key: provider_account_key.to_string(),
            account_display_name: account_display_name.map(str::to_string),
            participant,
            sender_is_participant: !msg.is_from_me,
            external_thread_id,
            conversation_kind,
            conversation_title,
            direction: if msg.is_from_me {
                MessageDirection::Outbound
            } else {
                MessageDirection::Inbound
            },
            provider_message_id: (!msg.id.is_empty()).then_some(msg.id.clone()),
            provider_client_message_id: None,
            local_request_id: None,
            content_type,
            text_content: msg.text.clone(),
            content: content_json,
            attachments,
            reply,
            mentions: vec![],
            raw_payload: msg.raw_json.clone(),
            provider_timestamp: Some(occurred_at),
            ingested_at: now,
            status: if msg.is_from_me {
                MessageStatus::Sent
            } else {
                MessageStatus::Received
            },
        })
    }

    /// Ingests an incoming or synchronized WhatsApp message into the SQLite database.
    /// Atomically creates/updates `channel_conversations`, `channel_identities`,
    /// links phone numbers to `leads` with duplicate safety, and inserts `messages`
    /// and `message_attachments`. Returns the `conversation_id`.
    pub fn ingest_incoming_message(
        conn: &mut Connection,
        channel_account_id: i64,
        msg: &RawWhatsAppMessage,
    ) -> Result<Option<i64>> {
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let event_time = Self::normalize_iso_timestamp(msg.timestamp.as_deref());

        let is_group = phone::is_group_jid(&msg.chat_jid);

        // 1. Thread & Conversation Resolution
        let (external_thread_id, kind, default_title) = if is_group {
            (
                msg.chat_jid.trim().to_string(),
                "group",
                msg.sender_name
                    .clone()
                    .unwrap_or_else(|| "WhatsApp Group".to_string()),
            )
        } else {
            let peer_phone = phone::jid_to_phone(&msg.chat_jid)
                .unwrap_or_else(|| msg.chat_jid.trim().to_string());
            let normalized_phone =
                phone::normalize_e164(&peer_phone, Some("84")).unwrap_or(peer_phone);
            (
                phone::phone_to_direct_jid(&normalized_phone),
                "direct",
                msg.sender_name
                    .clone()
                    .unwrap_or_else(|| format!("+{}", normalized_phone)),
            )
        };

        // Upsert channel_conversations
        tx.execute(
            "INSERT INTO channel_conversations (
                channel_account_id, external_thread_id, kind, title, avatar_url, status, last_message_at, last_synced_at, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, NULL, 'active', ?5, ?6, ?6, ?6)
            ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                title = CASE
                    WHEN EXCLUDED.title IS NOT NULL AND EXCLUDED.title != 'WhatsApp Group' THEN EXCLUDED.title
                    ELSE channel_conversations.title
                END,
                last_message_at = CASE
                    WHEN EXCLUDED.last_message_at IS NOT NULL AND (channel_conversations.last_message_at IS NULL OR EXCLUDED.last_message_at > channel_conversations.last_message_at)
                    THEN EXCLUDED.last_message_at
                    ELSE channel_conversations.last_message_at
                END,
                last_synced_at = EXCLUDED.last_synced_at,
                updated_at = EXCLUDED.updated_at",
            params![
                channel_account_id,
                external_thread_id,
                kind,
                default_title,
                event_time,
                now_str
            ],
        )?;

        let conversation_id: i64 = tx.query_row(
            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
            params![channel_account_id, external_thread_id, kind],
            |row| row.get(0),
        )?;

        // 2. Identity & Lead Resolution
        let mut sender_identity_id: Option<i64> = None;

        if !is_group {
            let peer_phone = phone::jid_to_phone(&msg.chat_jid)
                .unwrap_or_else(|| msg.chat_jid.trim().to_string());
            let normalized_phone =
                phone::normalize_e164(&peer_phone, Some("84")).unwrap_or(peer_phone);

            let lead_id = Self::resolve_lead_by_phone(&tx, &normalized_phone)?;
            let link_source = if lead_id.is_some() {
                "phone_match"
            } else {
                "unlinked"
            };

            let display_name = msg
                .sender_name
                .clone()
                .unwrap_or_else(|| format!("+{}", normalized_phone));

            tx.execute(
                "INSERT INTO channel_identities (
                    lead_id, channel, channel_account_id, identity_namespace, external_id,
                    normalized_address, display_name, link_source, created_at, updated_at
                ) VALUES (
                    ?1, 'whatsapp', ?2, 'phone', ?3,
                    ?4, ?5, ?6, ?7, ?7
                )
                ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                    lead_id = CASE WHEN EXCLUDED.lead_id IS NOT NULL THEN EXCLUDED.lead_id ELSE channel_identities.lead_id END,
                    display_name = COALESCE(EXCLUDED.display_name, channel_identities.display_name),
                    link_source = CASE WHEN EXCLUDED.lead_id IS NOT NULL THEN EXCLUDED.link_source ELSE channel_identities.link_source END,
                    updated_at = EXCLUDED.updated_at",
                params![
                    lead_id,
                    channel_account_id,
                    normalized_phone,
                    normalized_phone,
                    display_name,
                    link_source,
                    now_str
                ],
            )?;

            let identity_id: i64 = tx.query_row(
                "SELECT id FROM channel_identities
                 WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND identity_namespace = 'phone' AND external_id = ?2",
                params![channel_account_id, normalized_phone],
                |row| row.get(0),
            )?;

            // Ensure participant
            tx.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', ?3)
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![conversation_id, identity_id, now_str],
            )?;

            if !msg.is_from_me {
                sender_identity_id = Some(identity_id);
            }
        } else if let Some(ref sender_jid) = msg.sender_jid {
            if !msg.is_from_me {
                let sender_phone = phone::jid_to_phone(sender_jid)
                    .or_else(|| phone::normalize_e164(sender_jid, Some("84")));
                let ext_id = sender_phone.clone().unwrap_or_else(|| sender_jid.clone());

                let lead_id = sender_phone
                    .as_deref()
                    .and_then(|p| Self::resolve_lead_by_phone(&tx, p).ok().flatten());
                let link_source = if lead_id.is_some() {
                    "phone_match"
                } else {
                    "unlinked"
                };

                let display_name = msg
                    .sender_name
                    .clone()
                    .unwrap_or_else(|| format!("+{}", ext_id));

                tx.execute(
                    "INSERT INTO channel_identities (
                        lead_id, channel, channel_account_id, identity_namespace, external_id,
                        normalized_address, display_name, link_source, created_at, updated_at
                    ) VALUES (
                        ?1, 'whatsapp', ?2, 'phone', ?3,
                        ?4, ?5, ?6, ?7, ?7
                    )
                    ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                        lead_id = CASE WHEN EXCLUDED.lead_id IS NOT NULL THEN EXCLUDED.lead_id ELSE channel_identities.lead_id END,
                        display_name = COALESCE(EXCLUDED.display_name, channel_identities.display_name),
                        link_source = CASE WHEN EXCLUDED.lead_id IS NOT NULL THEN EXCLUDED.link_source ELSE channel_identities.link_source END,
                        updated_at = EXCLUDED.updated_at",
                    params![
                        lead_id,
                        channel_account_id,
                        ext_id,
                        ext_id,
                        display_name,
                        link_source,
                        now_str
                    ],
                )?;

                let member_identity_id: i64 = tx.query_row(
                    "SELECT id FROM channel_identities
                     WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND identity_namespace = 'phone' AND external_id = ?2",
                    params![channel_account_id, ext_id],
                    |row| row.get(0),
                )?;

                tx.execute(
                    "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                     VALUES (?1, ?2, 'member', ?3)
                     ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                    params![conversation_id, member_identity_id, now_str],
                )?;

                sender_identity_id = Some(member_identity_id);
            }
        }

        // 3. Message Ingestion
        let (direction, status) = if msg.is_from_me {
            ("outbound", "sent")
        } else {
            ("inbound", "received")
        };

        let content_type = Self::detect_content_type(msg);
        let text_content = msg.text.as_deref().unwrap_or("");
        let raw_payload = msg
            .raw_json
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string()));

        let mut reply_to_message_id: Option<i64> = None;
        let mut reply_sender_identity_id: Option<i64> = None;

        if let Some(ref q) = msg.quote {
            reply_to_message_id = tx
                .query_row(
                    "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                    params![conversation_id, q.stanza_id],
                    |r| r.get(0),
                )
                .optional()?;

            if !q.participant_jid.trim().is_empty() {
                let q_phone = phone::jid_to_phone(&q.participant_jid)
                    .unwrap_or_else(|| q.participant_jid.trim().to_string());
                reply_sender_identity_id = tx
                    .query_row(
                        "SELECT id FROM channel_identities WHERE channel = 'whatsapp' AND channel_account_id = ?1 AND external_id = ?2",
                        params![channel_account_id, q_phone],
                        |r| r.get(0),
                    )
                    .optional()?;
            }
        }

        let existing_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![conversation_id, msg.id],
                |row| row.get(0),
            )
            .optional()?;

        let _message_id = match existing_id {
            Some(id) => id,
            None => {
                tx.execute(
                    "INSERT INTO messages (
                        conversation_id, sender_identity_id, direction, provider_message_id,
                        content_type, text_content, content_json, raw_payload_json,
                        provider_timestamp, ingested_at, status, reply_to_message_id, reply_sender_identity_id
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}', ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![
                        conversation_id,
                        sender_identity_id,
                        direction,
                        msg.id,
                        content_type,
                        text_content,
                        raw_payload,
                        event_time,
                        now_str,
                        status,
                        reply_to_message_id,
                        reply_sender_identity_id
                    ],
                )?;
                let mid = tx.last_insert_rowid();

                // Insert attachments
                for att in &msg.attachments {
                    let att_type = match att.media_type.to_lowercase().as_str() {
                        "image" | "photo" => "image",
                        "video" => "video",
                        "audio" | "voice" | "ptt" => "audio",
                        "sticker" => "sticker",
                        _ => "file",
                    };
                    let default_mime = match att_type {
                        "image" => "image/jpeg",
                        "video" => "video/mp4",
                        "audio" => "audio/ogg",
                        "sticker" => "image/webp",
                        _ => "application/octet-stream",
                    };
                    let mime = att.mime_type.as_deref().unwrap_or(default_mime);
                    let metadata = att
                        .direct_path
                        .as_ref()
                        .map(|p| serde_json::json!({ "direct_path": p }).to_string())
                        .unwrap_or_else(|| "{}".to_string());

                    tx.execute(
                        "INSERT INTO message_attachments (
                            message_id, provider_attachment_id, attachment_type, file_name,
                            mime_type, size_bytes, remote_url, local_path, thumbnail_url, metadata_json, created_at
                        ) VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                        ON CONFLICT(message_id, remote_url) WHERE remote_url IS NOT NULL DO NOTHING",
                        params![
                            mid,
                            att_type,
                            att.file_name,
                            mime,
                            att.size_bytes,
                            att.url,
                            att.direct_path,
                            att.thumbnail_url,
                            metadata,
                            now_str
                        ],
                    )?;
                }

                // Check and apply any pending reactions
                let mut pending_stmt = tx.prepare(
                    "SELECT id, actor_external_id, reaction, removed, occurred_at
                     FROM pending_message_reactions
                     WHERE channel_account_id = ?1 AND provider_message_id = ?2",
                )?;

                let pending_reactions: Vec<(i64, String, String, i64, String)> = pending_stmt
                    .query_map(params![channel_account_id, msg.id], |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        ))
                    })?
                    .filter_map(Result::ok)
                    .collect();

                for (pending_id, actor, emoji, removed, reacted_at) in pending_reactions {
                    if removed == 1 || emoji.trim().is_empty() {
                        tx.execute(
                            "DELETE FROM message_reactions WHERE message_id = ?1 AND actor_external_id = ?2",
                            params![mid, actor],
                        )?;
                    } else {
                        tx.execute(
                            "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
                             VALUES (?1, ?2, ?3, 1, ?4, NULL)
                             ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                                 reacted_at = EXCLUDED.reacted_at,
                                 removed_at = NULL",
                            params![mid, actor, emoji, reacted_at],
                        )?;
                    }
                    tx.execute(
                        "DELETE FROM pending_message_reactions WHERE id = ?1",
                        params![pending_id],
                    )?;
                }

                mid
            }
        };

        tx.commit()?;
        Ok(Some(conversation_id))
    }

    /// Ingests a WhatsApp reaction event.
    pub fn ingest_reaction(
        conn: &mut Connection,
        channel_account_id: i64,
        reaction: &RawWhatsAppReaction,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        let now_str = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let occurred_at = Self::normalize_iso_timestamp(reaction.timestamp.as_deref());

        let is_group = phone::is_group_jid(&reaction.chat_jid);
        let external_thread_id = if is_group {
            reaction.chat_jid.trim().to_string()
        } else {
            let phone = phone::jid_to_phone(&reaction.chat_jid)
                .unwrap_or_else(|| reaction.chat_jid.trim().to_string());
            let normalized = phone::normalize_e164(&phone, Some("84")).unwrap_or(phone);
            phone::phone_to_direct_jid(&normalized)
        };

        let actor_phone = phone::jid_to_phone(&reaction.sender_jid)
            .unwrap_or_else(|| reaction.sender_jid.trim().to_string());

        let target_message_id: Option<i64> = tx
            .query_row(
                "SELECT m.id FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 WHERE cc.channel_account_id = ?1 AND m.provider_message_id = ?2
                 LIMIT 1",
                params![channel_account_id, reaction.message_id],
                |r| r.get(0),
            )
            .optional()?;

        let is_removed = reaction.emoji.trim().is_empty();

        if let Some(msg_id) = target_message_id {
            if is_removed {
                tx.execute(
                    "DELETE FROM message_reactions WHERE message_id = ?1 AND actor_external_id = ?2",
                    params![msg_id, actor_phone],
                )?;
            } else {
                tx.execute(
                    "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
                     VALUES (?1, ?2, ?3, 1, ?4, NULL)
                     ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                         reacted_at = EXCLUDED.reacted_at,
                         removed_at = NULL",
                    params![msg_id, actor_phone, reaction.emoji, occurred_at],
                )?;
            }
        } else {
            // Save as pending reaction
            let conversation_kind = if is_group { "group" } else { "direct" };
            tx.execute(
                "INSERT INTO pending_message_reactions (
                    channel_account_id, external_thread_id, conversation_kind,
                    provider_event_id, provider_message_id, provider_client_message_id,
                    actor_external_id, reaction, removed, occurred_at, raw_payload_json, created_at, updated_at
                ) VALUES (
                    ?1, ?2, ?3, '', ?4, '', ?5, ?6, ?7, ?8, '{}', ?9, ?9
                )
                ON CONFLICT DO NOTHING",
                params![
                    channel_account_id,
                    external_thread_id,
                    conversation_kind,
                    reaction.message_id,
                    actor_phone,
                    reaction.emoji,
                    if is_removed { 1 } else { 0 },
                    occurred_at,
                    now_str
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    /// Ingests a WhatsApp delivery or read receipt.
    pub fn ingest_receipt(
        conn: &mut Connection,
        channel_account_id: i64,
        receipt: &RawWhatsAppReceipt,
    ) -> Result<()> {
        let tx = conn.transaction()?;
        let occurred_at = Self::normalize_iso_timestamp(receipt.timestamp.as_deref());

        let recipient_phone = receipt
            .sender_jid
            .as_deref()
            .and_then(phone::jid_to_phone)
            .unwrap_or_default();

        for mid in &receipt.message_ids {
            let msg_id: Option<i64> = tx
                .query_row(
                    "SELECT m.id FROM messages m
                     JOIN channel_conversations cc ON cc.id = m.conversation_id
                     WHERE cc.channel_account_id = ?1 AND m.provider_message_id = ?2
                     LIMIT 1",
                    params![channel_account_id, mid],
                    |r| r.get(0),
                )
                .optional()?;

            if let Some(id) = msg_id {
                let r_type = receipt.receipt_type.to_lowercase();
                if r_type == "read" || r_type == "played" {
                    tx.execute(
                        "UPDATE messages SET status = 'read' WHERE id = ?1 AND status != 'read'",
                        params![id],
                    )?;
                } else if r_type == "delivered" {
                    tx.execute(
                        "UPDATE messages SET status = 'delivered' WHERE id = ?1 AND status NOT IN ('delivered', 'read')",
                        params![id],
                    )?;
                }

                tx.execute(
                    "INSERT INTO message_receipts (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                     VALUES (?1, ?2, ?3, ?4, '{}')
                     ON CONFLICT(message_id, recipient_external_id, receipt_type) DO NOTHING",
                    params![id, recipient_phone, r_type, occurred_at],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }
}
