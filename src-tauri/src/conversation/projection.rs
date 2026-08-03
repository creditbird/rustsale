use crate::conversation::types::{
    ConversationAttachment, ConversationMention, ConversationMessage, ConversationReactionActor,
    ConversationReactionGroup, ConversationReplyTarget, ConversationSendBatchSummary,
    MessageSendBatchStatus,
};
use rusqlite::{Connection, Result};
use std::collections::HashMap;

type ReactionAggregate = (u32, bool, Vec<ConversationReactionActor>);
type ReactionGroupsByIcon = HashMap<String, ReactionAggregate>;

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

        let att = ConversationAttachment {
            id: row.get(1)?,
            media_type: row.get(2)?,
            mime_type: row.get(3)?,
            file_name: row.get(4)?,
            size_bytes: row.get(5)?,
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
            "SELECT m.id, m.provider_message_id, ci.display_name, m.text_content, m.content_type, m.provider_timestamp
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
            let p_msg_id: Option<String> = row.get(1)?;
            let sender_name: Option<String> = row.get(2)?;
            let text: Option<String> = row.get(3)?;
            let c_type: String = row.get(4)?;
            let occurred_at: Option<String> = row.get(5)?;

            reply_targets_map.insert(
                id,
                ConversationReplyTarget {
                    message_id: Some(id),
                    provider_message_id: p_msg_id,
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
        "SELECT mr.message_id, mr.reaction, mr.actor_external_id,
                ci.id, ci.display_name, ci.avatar_url, ca.self_external_id
         FROM message_reactions mr
         JOIN messages m ON m.id = mr.message_id
         JOIN channel_conversations cc ON cc.id = m.conversation_id
         JOIN channel_accounts ca ON ca.id = cc.channel_account_id
         LEFT JOIN channel_identities ci
           ON ci.channel = ca.channel
          AND ci.identity_namespace = 'zalo_uid'
          AND ci.external_id = mr.actor_external_id
         WHERE mr.message_id IN ({placeholders}) AND mr.removed_at IS NULL
         ORDER BY mr.message_id ASC"
    );
    let mut react_stmt = conn.prepare(&react_query)?;
    let mut react_rows = react_stmt.query(params_ref.as_slice())?;
    let mut raw_reactions: HashMap<i64, ReactionGroupsByIcon> = HashMap::new();
    while let Some(row) = react_rows.next()? {
        let msg_id: i64 = row.get(0)?;
        let icon: String = row.get(1)?;
        let actor_external_id: String = row.get(2)?;
        let identity_id: Option<i64> = row.get(3)?;
        let display_name: Option<String> = row.get(4)?;
        let avatar_url: Option<String> = row.get(5)?;
        let self_external_id: Option<String> = row.get(6)?;

        let entry = raw_reactions
            .entry(msg_id)
            .or_default()
            .entry(icon)
            .or_insert_with(|| (0, false, Vec::new()));
        entry.0 += 1;
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

    let mut caps_map: HashMap<i64, crate::conversation::types::ConversationCapabilities> =
        HashMap::new();
    let caps_query = format!(
        "SELECT DISTINCT ca.id, ca.capabilities_json
         FROM channel_accounts ca
         JOIN channel_conversations cc ON cc.channel_account_id = ca.id
         JOIN messages m ON m.conversation_id = cc.id
         WHERE m.id IN ({placeholders})"
    );
    let mut caps_stmt = conn.prepare(&caps_query)?;
    let mut caps_rows = caps_stmt.query(params_ref.as_slice())?;
    while let Some(row) = caps_rows.next()? {
        let acc_id: i64 = row.get(0)?;
        let json_str: String = row.get(1)?;
        let caps = if json_str.trim().is_empty() || json_str.trim() == "{}" {
            crate::conversation::types::ConversationCapabilities::zalo_phase_one()
        } else {
            crate::conversation::types::ConversationCapabilities::from_json(&json_str)
        };
        caps_map.insert(acc_id, caps);
    }

    let forwardable_types = [
        "text", "image", "video", "audio", "file", "sticker", "link", "forward",
    ];

    for msg in messages.iter_mut() {
        if let Some(atts) = attachments_map.remove(&msg.id) {
            msg.attachments = atts;
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
            && msg
                .provider_client_message_id
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
        let is_not_removed = msg.removed_at.is_none();
        let caps = caps_map.get(&msg.channel_account_id);

        let forward_enabled = caps.is_none_or(|c| c.forward.enabled);
        let react_enabled = caps.is_none_or(|c| c.supports_reactions && c.reactions.enabled);
        let reply_enabled = caps.is_none_or(|c| c.supports_quote_reply);
        let recall_enabled = caps.is_none_or(|c| c.supports_recall);

        msg.actions = crate::conversation::types::ConversationMessageActions {
            reply: reply_enabled && is_not_removed && has_provider_ids,
            recall: recall_enabled
                && is_not_removed
                && msg.direction == crate::conversation::types::MessageDirection::Outbound
                && has_provider_ids,
            forward: forward_enabled
                && is_not_removed
                && has_provider_ids
                && forwardable_types.contains(&msg.content_type.as_str()),
            react: react_enabled && is_not_removed && has_provider_ids,
        };
    }

    Ok(())
}

pub fn load_message_reaction_groups(
    conn: &rusqlite::Connection,
    message_id: i64,
) -> rusqlite::Result<Vec<ConversationReactionGroup>> {
    let react_query = "SELECT mr.reaction, mr.actor_external_id,
                ci.id, ci.display_name, ci.avatar_url, ca.self_external_id
         FROM message_reactions mr
         JOIN messages m ON m.id = mr.message_id
         JOIN channel_conversations cc ON cc.id = m.conversation_id
         JOIN channel_accounts ca ON ca.id = cc.channel_account_id
         LEFT JOIN channel_identities ci
           ON ci.channel = ca.channel
          AND ci.identity_namespace = 'zalo_uid'
          AND ci.external_id = mr.actor_external_id
         WHERE mr.message_id = ?1 AND mr.removed_at IS NULL
         ORDER BY mr.reacted_at ASC";

    let mut stmt = conn.prepare(react_query)?;
    let mut rows = stmt.query(rusqlite::params![message_id])?;

    let mut raw_reactions: std::collections::BTreeMap<
        String,
        (u32, bool, Vec<ConversationReactionActor>),
    > = std::collections::BTreeMap::new();

    while let Some(row) = rows.next()? {
        let icon: String = row.get(0)?;
        let actor_external_id: String = row.get(1)?;
        let identity_id: Option<i64> = row.get(2)?;
        let display_name: Option<String> = row.get(3)?;
        let avatar_url: Option<String> = row.get(4)?;
        let self_external_id: Option<String> = row.get(5)?;

        let entry = raw_reactions
            .entry(icon)
            .or_insert_with(|| (0, false, Vec::new()));
        entry.0 += 1;
        entry.1 |= self_external_id.as_deref() == Some(actor_external_id.as_str());
        entry.2.push(ConversationReactionActor {
            identity_id,
            external_id: actor_external_id,
            display_name,
            avatar_url,
        });
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
