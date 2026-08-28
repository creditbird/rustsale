use crate::conversation::types::{
    resolve_attachment_mime_type, ConversationAttachment, ConversationMention, ConversationMessage,
    ConversationReactionActor, ConversationReactionGroup, ConversationReplyTarget,
    ConversationSendBatchSummary, MessageSendBatchStatus,
};
use rusqlite::{Connection, Result};
use std::collections::HashMap;

type ReactionAggregate = (u32, bool, Vec<ConversationReactionActor>);
type ReactionGroupsByIcon = HashMap<String, ReactionAggregate>;

fn is_within_recall_window(occurred_at: &str, window_seconds: Option<u64>) -> bool {
    let Some(window_seconds) = window_seconds else {
        return true;
    };
    let Ok(occurred_at) = chrono::DateTime::parse_from_rfc3339(occurred_at) else {
        return false;
    };
    let elapsed = chrono::Utc::now().signed_duration_since(occurred_at.with_timezone(&chrono::Utc));
    elapsed.num_seconds() >= 0 && elapsed.num_seconds() <= window_seconds as i64
}

pub fn hydrate_messages(conn: &Connection, messages: &mut [ConversationMessage]) -> Result<()> {
    if messages.is_empty() {
        return Ok(());
    }

    let message_ids: Vec<i64> = messages.iter().map(|m| m.id).collect();
    let placeholders = message_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    let mut attachments_map: HashMap<i64, Vec<ConversationAttachment>> = HashMap::new();
    let query = format!(
        "SELECT message_id, id, attachment_type, mime_type, file_name, size_bytes,
                local_path, remote_url, thumbnail_url, metadata_json
         FROM message_attachments
         WHERE message_id IN ({placeholders})
         ORDER BY id ASC"
    );
    let mut stmt = conn.prepare(&query)?;
    let params_ref: Vec<&dyn rusqlite::ToSql> = message_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    let mut rows = stmt.query(params_ref.as_slice())?;
    while let Some(row) = rows.next()? {
        let msg_id: i64 = row.get(0)?;
        let metadata_str: Option<String> = row.get(9)?;
        let metadata: Option<serde_json::Value> = metadata_str
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());
        let width = metadata
            .as_ref()
            .and_then(|m| m.get("width"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let height = metadata
            .as_ref()
            .and_then(|m| m.get("height"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let duration_seconds = metadata
            .as_ref()
            .and_then(|m| m.get("duration_seconds"))
            .and_then(|v| v.as_f64());

        let media_type: String = row.get(2)?;
        let raw_mime: Option<String> = row.get(3)?;
        let mime_type = resolve_attachment_mime_type(&media_type, raw_mime.as_deref());

        let att = ConversationAttachment {
            id: row.get(1)?,
            media_type,
            mime_type,
            file_name: row.get(4)?,
            size_bytes: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
            local_path: row.get(6)?,
            remote_url: row.get(7)?,
            thumbnail_url: row.get(8)?,
            width,
            height,
            duration_seconds,
        };
        attachments_map.entry(msg_id).or_default().push(att);
    }

    let reply_ids: Vec<i64> = messages
        .iter()
        .filter_map(|m| m.reply_to_message_id)
        .collect();
    let mut reply_targets_map: HashMap<i64, ConversationReplyTarget> = HashMap::new();
    if !reply_ids.is_empty() {
        let reply_placeholders = reply_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let reply_query = format!(
            "SELECT m.id, ci.display_name, m.text_content, m.content_type, m.provider_timestamp
             FROM messages m
             LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
             WHERE m.id IN ({reply_placeholders})"
        );
        let mut r_stmt = conn.prepare(&reply_query)?;
        let r_params: Vec<&dyn rusqlite::ToSql> = reply_ids
            .iter()
            .map(|id| id as &dyn rusqlite::ToSql)
            .collect();
        let mut r_rows = r_stmt.query(r_params.as_slice())?;
        while let Some(row) = r_rows.next()? {
            let id: i64 = row.get(0)?;
            let sender_name: Option<String> = row.get(1)?;
            let text: Option<String> = row.get(2)?;
            let c_type: String = row.get(3)?;
            let occurred_at: Option<String> = row.get(4)?;

            reply_targets_map.insert(
                id,
                ConversationReplyTarget {
                    message_id: Some(id),
                    sender_display_name: sender_name,
                    content_type: c_type,
                    text_content: text,
                    media_type: None,
                    occurred_at,
                },
            );
        }
    }

    let batch_query = format!(
        "SELECT m.id, mb.id, mb.status, mb.part_count,
                COALESCE(SUM(CASE WHEN batch_message.status = 'sent' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN batch_message.status = 'failed' THEN 1 ELSE 0 END), 0)
         FROM messages m
         JOIN message_send_batches mb ON mb.id = m.send_batch_id
         LEFT JOIN messages batch_message ON batch_message.send_batch_id = mb.id
         WHERE m.id IN ({placeholders})
         GROUP BY m.id, mb.id, mb.status, mb.part_count"
    );
    let mut batch_stmt = conn.prepare(&batch_query)?;
    let mut batch_rows = batch_stmt.query(params_ref.as_slice())?;
    let mut batch_map: HashMap<i64, ConversationSendBatchSummary> = HashMap::new();
    while let Some(row) = batch_rows.next()? {
        let message_id: i64 = row.get(0)?;
        let status_raw: String = row.get(2)?;
        let status = match status_raw.as_str() {
            "sending" => MessageSendBatchStatus::Sending,
            "sent" => MessageSendBatchStatus::Sent,
            "partial" => MessageSendBatchStatus::Partial,
            "failed" => MessageSendBatchStatus::Failed,
            _ => MessageSendBatchStatus::Queued,
        };
        batch_map.insert(
            message_id,
            ConversationSendBatchSummary {
                send_batch_id: row.get(1)?,
                status,
                total_parts: row.get(3)?,
                sent_parts: row.get(4)?,
                failed_parts: row.get(5)?,
            },
        );
    }

    let mut reactions_map: HashMap<i64, Vec<ConversationReactionGroup>> = HashMap::new();
    let react_query = format!(
        "SELECT mr.message_id, mr.reaction, mr.actor_external_id, COALESCE(mr.count, 1),
                ci.id, ci.display_name, ci.avatar_url, ca.self_external_id
         FROM message_reactions mr
         JOIN messages m ON m.id = mr.message_id
         JOIN channel_conversations cc ON cc.id = m.conversation_id
         JOIN channel_accounts ca ON ca.id = cc.channel_account_id
         LEFT JOIN channel_identities ci
           ON ci.channel = ca.channel
          AND ci.identity_namespace = 'zalo_uid'
          AND ci.external_id = mr.actor_external_id
         WHERE mr.message_id IN ({placeholders})
           AND mr.removed_at IS NULL
           AND lower(trim(mr.reaction)) <> 'unknown'
         ORDER BY mr.message_id ASC"
    );
    let mut react_stmt = conn.prepare(&react_query)?;
    let mut react_rows = react_stmt.query(params_ref.as_slice())?;
    let mut raw_reactions: HashMap<i64, ReactionGroupsByIcon> = HashMap::new();
    while let Some(row) = react_rows.next()? {
        let msg_id: i64 = row.get(0)?;
        let icon: String = row.get(1)?;
        let icon = canonical_reaction_icon(&icon);
        let actor_external_id: String = row.get(2)?;
        let count_val: u32 = row.get::<_, Option<u32>>(3)?.unwrap_or(1).max(1);
        let identity_id: Option<i64> = row.get(4)?;
        let display_name: Option<String> = row.get(5)?;
        let avatar_url: Option<String> = row.get(6)?;
        let self_external_id: Option<String> = row.get(7)?;

        let entry = raw_reactions
            .entry(msg_id)
            .or_default()
            .entry(icon)
            .or_insert_with(|| (0, false, Vec::new()));
        entry.0 += count_val;
        entry.1 |= self_external_id.as_deref() == Some(actor_external_id.as_str());
        entry.2.push(ConversationReactionActor {
            identity_id,
            external_id: actor_external_id,
            display_name,
            avatar_url,
        });
    }
    for (msg_id, groups_map) in raw_reactions {
        let groups = groups_map
            .into_iter()
            .map(
                |(reaction_icon, (count, includes_me, actors))| ConversationReactionGroup {
                    reaction_icon,
                    count,
                    includes_me,
                    actors,
                },
            )
            .collect();
        reactions_map.insert(msg_id, groups);
    }

    let mut mentions_map: HashMap<i64, Vec<ConversationMention>> = HashMap::new();
    let mentions_query = format!(
        "SELECT mm.message_id, mm.start_utf16, mm.length_utf16, mm.identity_id, mm.external_id,
                COALESCE(ci.display_name, mm.display_text) AS display_name
         FROM message_mentions mm
         LEFT JOIN channel_identities ci ON ci.id = mm.identity_id
         WHERE mm.message_id IN ({placeholders})
         ORDER BY mm.message_id ASC, mm.start_utf16 ASC"
    );
    let mut mentions_stmt = conn.prepare(&mentions_query)?;
    let mut mentions_rows = mentions_stmt.query(params_ref.as_slice())?;
    while let Some(row) = mentions_rows.next()? {
        let msg_id: i64 = row.get(0)?;
        let pos: i64 = row.get(1)?;
        let len: i64 = row.get(2)?;
        let target_identity_id: Option<i64> = row.get(3)?;
        let target_external_id: Option<String> = row.get(4)?;
        let display_name: Option<String> = row.get(5)?;
        let is_all = target_external_id.as_deref() == Some("all")
            || (target_identity_id.is_none()
                && target_external_id.is_none()
                && display_name.as_deref() == Some("all"));

        mentions_map
            .entry(msg_id)
            .or_default()
            .push(ConversationMention {
                pos: pos as usize,
                len: len as usize,
                target_identity_id,
                target_external_id,
                display_name,
                is_all,
            });
    }

    let mut caps_map: HashMap<i64, (crate::conversation::types::ConversationCapabilities, bool)> =
        HashMap::new();
    let caps_query = format!(
        "SELECT DISTINCT ca.id, ca.channel, ca.connection_status, ca.capabilities_json
         FROM channel_accounts ca
         JOIN channel_conversations cc ON cc.channel_account_id = ca.id
         JOIN messages m ON m.conversation_id = cc.id
         WHERE m.id IN ({placeholders})"
    );
    let mut caps_stmt = conn.prepare(&caps_query)?;
    let mut caps_rows = caps_stmt.query(params_ref.as_slice())?;
    while let Some(row) = caps_rows.next()? {
        let acc_id: i64 = row.get(0)?;
        let channel: String = row.get(1)?;
        let connection_status: String = row.get(2)?;
        let json_str: String = row.get(3)?;
        let caps = if json_str.trim().is_empty() || json_str.trim() == "{}" {
            if channel == "zalo" {
                crate::conversation::types::ConversationCapabilities::zalo_phase_one()
            } else if channel == "whatsapp" {
                crate::conversation::types::ConversationCapabilities::whatsapp()
            } else {
                crate::conversation::types::ConversationCapabilities::text_only()
            }
        } else {
            crate::conversation::types::ConversationCapabilities::from_json(&json_str)
        };
        caps_map.insert(acc_id, (caps, connection_status == "connected"));
    }

    for msg in messages.iter_mut() {
        if let Some(atts) = attachments_map.remove(&msg.id) {
            msg.attachments = atts;
        }

        let is_sticker_candidate = msg.content_type == "sticker"
            || msg.text_content.as_deref() == Some("[Sticker]")
            || msg.content.get("stickerId").is_some()
            || msg.content.get("cateId").is_some();

        if is_sticker_candidate {
            let extracted_url = msg
                .content
                .get("url")
                .or_else(|| msg.content.get("spriteUrl"))
                .or_else(|| msg.content.get("sprite_url"))
                .or_else(|| msg.content.get("stickerSpriteUrl"))
                .or_else(|| msg.content.get("sticker_sprite_url"))
                .or_else(|| msg.content.get("staticUrl"))
                .or_else(|| msg.content.get("static_url"))
                .or_else(|| msg.content.get("webpUrl"))
                .or_else(|| msg.content.get("webp_url"))
                .or_else(|| msg.content.get("stickerWebpUrl"))
                .or_else(|| msg.content.get("sticker_webp_url"))
                .or_else(|| msg.content.get("webUrl"))
                .or_else(|| msg.content.get("web_url"))
                .or_else(|| msg.content.get("stickerUrl"))
                .or_else(|| msg.content.get("sticker_url"))
                .or_else(|| msg.content.get("thumbUrl"))
                .or_else(|| msg.content.get("thumb_url"))
                .or_else(|| msg.content.get("thumbnailUrl"))
                .or_else(|| msg.content.get("thumbnail_url"))
                .or_else(|| msg.content.get("previewUrl"))
                .or_else(|| msg.content.get("preview_url"))
                .or_else(|| msg.content.get("src"))
                .or_else(|| msg.content.get("href"))
                .or_else(|| msg.content.get("path"))
                .and_then(|v| v.as_str())
                .map(str::to_string);

            if msg.content_type != "sticker"
                && (extracted_url.is_some() || msg.content.get("stickerId").is_some())
            {
                msg.content_type = "sticker".to_string();
            }

            if let Some(url) = extracted_url {
                if msg.attachments.is_empty() {
                    msg.attachments.push(ConversationAttachment {
                        id: 0,
                        media_type: "sticker".to_string(),
                        mime_type: "image/webp".to_string(),
                        file_name: None,
                        size_bytes: None,
                        local_path: None,
                        remote_url: Some(url),
                        thumbnail_url: None,
                        width: None,
                        height: None,
                        duration_seconds: None,
                    });
                } else if msg.attachments[0].remote_url.is_none() {
                    msg.attachments[0].remote_url = Some(url);
                }
            }
        }
        if let Some(tid) = msg.reply_to_message_id {
            if let Some(hydrated) = reply_targets_map.get(&tid) {
                msg.reply_target = Some(hydrated.clone());
            }
        }
        if let Some(reacts) = reactions_map.remove(&msg.id) {
            msg.reactions = reacts;
        }
        if let Some(batch) = batch_map.remove(&msg.id) {
            msg.send_batch = Some(batch);
        }
        if let Some(mentions) = mentions_map.remove(&msg.id) {
            msg.mentions = mentions;
        }

        let has_provider_ids = msg
            .provider_message_id
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty())
            || msg
                .provider_client_message_id
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
        let is_not_removed = msg.removed_at.is_none();
        let account = caps_map.get(&msg.channel_account_id);
        let caps = account.map(|(caps, _)| caps);
        let account_connected = account.is_none_or(|(_, connected)| *connected);

        let reaction_read_enabled = caps.is_none_or(|c| c.supports_reactions && c.reactions.read);
        let react_enabled =
            account_connected && caps.is_none_or(|c| c.supports_reactions && c.reactions.write);
        if !reaction_read_enabled {
            msg.reactions.clear();
        }
        let forward_enabled = account_connected && caps.is_none_or(|c| c.forward.enabled);
        let reply_enabled = account_connected && caps.is_none_or(|c| c.supports_quote_reply);
        let recall_enabled = account_connected && caps.is_none_or(|c| c.supports_recall);
        let recall_window_ok =
            is_within_recall_window(&msg.occurred_at, caps.and_then(|c| c.recall_window_seconds));
        let recall_status_ok = matches!(
            msg.status,
            crate::conversation::types::MessageStatus::Sent
                | crate::conversation::types::MessageStatus::Delivered
                | crate::conversation::types::MessageStatus::Read
        );

        msg.actions = crate::conversation::types::ConversationMessageActions {
            copy: is_not_removed,
            reply: reply_enabled && is_not_removed && has_provider_ids,
            recall: recall_enabled
                && is_not_removed
                && msg.direction == crate::conversation::types::MessageDirection::Outbound
                && has_provider_ids
                && recall_status_ok
                && recall_window_ok,
            forward: forward_enabled
                && is_not_removed
                && has_provider_ids
                && msg.content_type == "text",
            react: react_enabled && is_not_removed && has_provider_ids,
        };
    }

    Ok(())
}

fn canonical_reaction_icon(icon: &str) -> String {
    match icon.trim().to_ascii_lowercase().as_str() {
        "❤️" | "❤" | "/-heart" | "/:heart" | "heart" => "❤️".to_string(),
        "👍" | "/-strong" | "/:strong" | "/-like" | "/:like" | "like" => "👍".to_string(),
        "😆" | "😄" | ":>" | "/-haha" | "/:haha" | "haha" => "😆".to_string(),
        "😮" | ":o" | "/-wow" | "/:wow" | "wow" => "😮".to_string(),
        ":-(" | ":-[" | "/-sad" | "/:sad" => "😭".to_string(),
        "😭" | "😢" | ":-((" | ":((" | "--b" | "/-cry" | "/:cry" | "cry" | "sad" => {
            "😭".to_string()
        }
        "😡" | ":-h" | "/-angry" | "/:angry" | "angry" => "😡".to_string(),
        _ => icon.to_string(),
    }
}

pub fn load_message_reaction_groups(
    conn: &rusqlite::Connection,
    message_id: i64,
) -> rusqlite::Result<Vec<ConversationReactionGroup>> {
    let react_query = "SELECT mr.reaction, mr.actor_external_id,
                COALESCE(mr.count, 1), ci.id, ci.display_name, ci.avatar_url, ca.self_external_id
         FROM message_reactions mr
         JOIN messages m ON m.id = mr.message_id
         JOIN channel_conversations cc ON cc.id = m.conversation_id
         JOIN channel_accounts ca ON ca.id = cc.channel_account_id
         LEFT JOIN channel_identities ci
           ON ci.channel = ca.channel
          AND ci.identity_namespace = 'zalo_uid'
          AND ci.external_id = mr.actor_external_id
         WHERE mr.message_id = ?1
           AND mr.removed_at IS NULL
           AND lower(trim(mr.reaction)) <> 'unknown'
         ORDER BY mr.reacted_at ASC";

    let mut stmt = conn.prepare(react_query)?;
    let mut rows = stmt.query(rusqlite::params![message_id])?;

    let mut raw_reactions: std::collections::BTreeMap<
        String,
        (u32, bool, Vec<ConversationReactionActor>),
    > = std::collections::BTreeMap::new();

    while let Some(row) = rows.next()? {
        let icon: String = row.get(0)?;
        let icon = canonical_reaction_icon(&icon);
        let actor_external_id: String = row.get(1)?;
        let count_val: u32 = row.get::<_, Option<u32>>(2)?.unwrap_or(1).max(1);
        let identity_id: Option<i64> = row.get(3)?;
        let display_name: Option<String> = row.get(4)?;
        let avatar_url: Option<String> = row.get(5)?;
        let self_external_id: Option<String> = row.get(6)?;

        let entry = raw_reactions
            .entry(icon)
            .or_insert_with(|| (0, false, Vec::new()));
        entry.0 += count_val;
        entry.1 |= self_external_id.as_deref() == Some(actor_external_id.as_str());
        if !entry.2.iter().any(|a| a.external_id == actor_external_id) {
            entry.2.push(ConversationReactionActor {
                identity_id,
                external_id: actor_external_id,
                display_name,
                avatar_url,
            });
        }
    }

    let groups = raw_reactions
        .into_iter()
        .map(
            |(reaction_icon, (count, includes_me, actors))| ConversationReactionGroup {
                reaction_icon,
                count,
                includes_me,
                actors,
            },
        )
        .collect();

    Ok(groups)
}
