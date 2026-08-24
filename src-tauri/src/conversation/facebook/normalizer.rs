use anyhow::Result;
use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::client::{
    FacebookAttachmentNode, FacebookConversationThread, FacebookMessageNode,
    FacebookParticipantNode,
};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IngestSummary {
    pub conversations_synced: usize,
    pub messages_inserted: usize,
    pub new_messages_count: usize,
    pub affected_conversation_ids: Vec<i64>,
}

pub struct FacebookMessengerNormalizer;

impl FacebookMessengerNormalizer {
    pub fn normalize_iso_timestamp(meta_time: &str) -> String {
        if meta_time.trim().is_empty() {
            return Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        }
        // Meta format: "2026-08-24T02:00:00+0000" or ISO 8601
        if let Ok(dt) = DateTime::parse_from_str(meta_time, "%Y-%m-%dT%H:%M:%S%z") {
            return dt.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Millis, true);
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(meta_time) {
            return dt.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Millis, true);
        }
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    pub fn find_customer_participant<'a>(
        participants: &'a [FacebookParticipantNode],
        page_id: &str,
    ) -> Option<&'a FacebookParticipantNode> {
        participants
            .iter()
            .find(|p| p.id != page_id && !p.id.trim().is_empty())
    }

    pub fn find_customer_from_messages(
        messages: &[FacebookMessageNode],
        page_id: &str,
    ) -> Option<(String, Option<String>)> {
        for msg in messages {
            if let Some(ref from) = msg.from {
                if from.id != page_id && !from.id.trim().is_empty() {
                    return Some((from.id.trim().to_string(), from.name.clone()));
                }
            }
            if let Some(ref to) = msg.to {
                for recipient in &to.data {
                    if recipient.id != page_id && !recipient.id.trim().is_empty() {
                        return Some((recipient.id.trim().to_string(), recipient.name.clone()));
                    }
                }
            }
        }
        None
    }

    pub fn detect_attachment_type(att: &FacebookAttachmentNode) -> &'static str {
        if att.image_data.is_some() {
            return "image";
        }
        if att.video_data.is_some() {
            return "video";
        }
        if let Some(ref mime) = att.mime_type {
            if mime.starts_with("image/") {
                return "image";
            }
            if mime.starts_with("video/") {
                return "video";
            }
            if mime.starts_with("audio/") {
                return "audio";
            }
        }
        "file"
    }

    pub fn ingest_conversations_batch(
        conn: &mut Connection,
        channel_account_id: i64,
        page_id: &str,
        threads: &[FacebookConversationThread],
    ) -> Result<IngestSummary> {
        let tx = conn.transaction()?;
        let mut summary = IngestSummary::default();
        let now_str = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

        for thread in threads {
            let participants = thread
                .participants
                .as_ref()
                .map(|p| p.data.as_slice())
                .unwrap_or(&[]);

            let messages_slice = thread
                .messages
                .as_ref()
                .map(|m| m.data.as_slice())
                .unwrap_or(&[]);

            let customer = Self::find_customer_participant(participants, page_id);
            let (customer_psid, customer_name) = match customer {
                Some(c) if !c.id.trim().is_empty() => {
                    let name = c
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Facebook User {}", c.id.trim()));
                    (c.id.trim().to_string(), name)
                }
                _ => match Self::find_customer_from_messages(messages_slice, page_id) {
                    Some((psid, name_opt)) => {
                        let name = name_opt.unwrap_or_else(|| format!("Facebook User {}", psid));
                        (psid, name)
                    }
                    None => {
                        tracing::warn!(
                            "Skipping Facebook thread {} for page {}: unable to resolve customer PSID",
                            thread.id,
                            page_id
                        );
                        continue;
                    }
                },
            };

            // 1. Upsert channel_identities
            tx.execute(
                "INSERT INTO channel_identities (
                    channel, channel_account_id, identity_namespace, external_id, display_name, created_at, updated_at
                ) VALUES ('messenger', ?1, 'psid', ?2, ?3, ?4, ?4)
                ON CONFLICT(channel, channel_account_id, identity_namespace, external_id) WHERE channel_account_id IS NOT NULL DO UPDATE SET
                    display_name = COALESCE(EXCLUDED.display_name, channel_identities.display_name),
                    updated_at = EXCLUDED.updated_at",
                params![channel_account_id, customer_psid, customer_name, now_str],
            )?;

            let identity_id: i64 = tx.query_row(
                "SELECT id FROM channel_identities 
                 WHERE channel = 'messenger' AND channel_account_id = ?1 AND identity_namespace = 'psid' AND external_id = ?2",
                params![channel_account_id, customer_psid],
                |row| row.get(0),
            )?;

            // 2. Upsert channel_conversations
            let thread_updated_time = Self::normalize_iso_timestamp(&thread.updated_time);
            tx.execute(
                "INSERT INTO channel_conversations (
                    channel_account_id, external_thread_id, kind, title, avatar_url, status, last_message_at, last_synced_at, created_at, updated_at
                ) VALUES (?1, ?2, 'direct', ?3, NULL, 'active', ?4, ?5, ?5, ?5)
                ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                    title = COALESCE(EXCLUDED.title, channel_conversations.title),
                    last_message_at = CASE
                        WHEN EXCLUDED.last_message_at IS NOT NULL AND (channel_conversations.last_message_at IS NULL OR EXCLUDED.last_message_at > channel_conversations.last_message_at)
                        THEN EXCLUDED.last_message_at
                        ELSE channel_conversations.last_message_at
                    END,
                    last_synced_at = EXCLUDED.last_synced_at,
                    updated_at = EXCLUDED.updated_at",
                params![channel_account_id, thread.id, customer_name, thread_updated_time, now_str],
            )?;

            let conversation_id: i64 = tx.query_row(
                "SELECT id FROM channel_conversations
                 WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'direct'",
                params![channel_account_id, thread.id],
                |row| row.get(0),
            )?;

            if !summary.affected_conversation_ids.contains(&conversation_id) {
                summary.affected_conversation_ids.push(conversation_id);
            }

            // 3. Ensure conversation_participants
            tx.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', ?3)
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![conversation_id, identity_id, now_str],
            )?;

            summary.conversations_synced += 1;

            // 4. Ingest messages
            if let Some(ref messages_container) = thread.messages {
                for msg in &messages_container.data {
                    let msg_created_time = Self::normalize_iso_timestamp(&msg.created_time);
                    let is_outbound = msg
                        .from
                        .as_ref()
                        .map(|f| f.id == page_id)
                        .unwrap_or(false);

                    let (direction, sender_id, status) = if is_outbound {
                        ("outbound", None, "sent")
                    } else {
                        ("inbound", Some(identity_id), "received")
                    };

                    let text_content = msg.message.as_deref().unwrap_or("");
                    let raw_json = serde_json::to_string(msg).unwrap_or_else(|_| "{}".to_string());

                    let attachments = msg
                        .attachments
                        .as_ref()
                        .map(|a| a.data.as_slice())
                        .unwrap_or(&[]);

                    let content_type = if !attachments.is_empty() {
                        Self::detect_attachment_type(&attachments[0])
                    } else {
                        "text"
                    };

                    // Check if message already exists
                    let existing_id: Option<i64> = tx
                        .query_row(
                            "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                            params![conversation_id, msg.id],
                            |row| row.get(0),
                        )
                        .optional()?;

                    let message_id = match existing_id {
                        Some(id) => id,
                        None => {
                            tx.execute(
                                "INSERT INTO messages (
                                    conversation_id, sender_identity_id, direction, provider_message_id,
                                    content_type, text_content, content_json, raw_payload_json,
                                    provider_timestamp, ingested_at, status
                                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, '{}', ?7, ?8, ?9, ?10)",
                                params![
                                    conversation_id,
                                    sender_id,
                                    direction,
                                    msg.id,
                                    content_type,
                                    text_content,
                                    raw_json,
                                    msg_created_time,
                                    now_str,
                                    status
                                ],
                            )?;
                            summary.new_messages_count += 1;
                            tx.last_insert_rowid()
                        }
                    };

                    summary.messages_inserted += 1;

                    // Insert attachments
                    for att in attachments {
                        let att_type = Self::detect_attachment_type(att);
                        let remote_url = att
                            .file_url
                            .as_deref()
                            .or_else(|| att.image_data.as_ref().and_then(|i| i.url.as_deref()))
                            .or_else(|| att.video_data.as_ref().and_then(|v| v.url.as_deref()));

                        let thumbnail_url = att
                            .image_data
                            .as_ref()
                            .and_then(|i| i.preview_url.as_deref())
                            .or_else(|| att.video_data.as_ref().and_then(|v| v.preview_url.as_deref()));

                        let att_json = serde_json::to_string(att).unwrap_or_else(|_| "{}".to_string());

                        if let Some(url) = remote_url {
                            tx.execute(
                                "INSERT INTO message_attachments (
                                    message_id, provider_attachment_id, attachment_type, file_name,
                                    mime_type, size_bytes, remote_url, thumbnail_url, metadata_json, created_at
                                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                                ON CONFLICT(message_id, remote_url) WHERE remote_url IS NOT NULL DO NOTHING",
                                params![
                                    message_id,
                                    att.id,
                                    att_type,
                                    att.name,
                                    att.mime_type,
                                    att.size,
                                    url,
                                    thumbnail_url,
                                    att_json,
                                    now_str
                                ],
                            )?;
                        }
                    }
                }
            }
        }

        // Advance checkpoint
        tx.execute(
            "INSERT INTO channel_sync_checkpoints (channel_account_id, scope, cursor, last_event_at, last_success_at, updated_at)
             VALUES (?1, 'meta_messenger_sync', ?2, ?2, ?3, ?3)
             ON CONFLICT(channel_account_id, scope) DO UPDATE SET
                 cursor = COALESCE(EXCLUDED.cursor, channel_sync_checkpoints.cursor),
                 last_event_at = MAX(COALESCE(channel_sync_checkpoints.last_event_at, ''), COALESCE(EXCLUDED.last_event_at, '')),
                 last_success_at = EXCLUDED.last_success_at,
                 updated_at = EXCLUDED.updated_at",
            params![channel_account_id, now_str, now_str],
        )?;

        tx.commit()?;
        Ok(summary)
    }
}
