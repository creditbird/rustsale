use crate::conversation::types::{
    ConversationAttachment, ConversationMessage, ConversationReactionActor,
    ConversationReactionGroup, ConversationReplyTarget,
};
use rusqlite::{Connection, Result};
use std::collections::HashMap;

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

    let mut reactions_map: HashMap<i64, Vec<ConversationReactionGroup>> = HashMap::new();
    let react_query = format!(
        "SELECT mr.message_id, mr.reaction, ci.id, ci.display_name, ci.avatar_url
         FROM message_reactions mr
         LEFT JOIN channel_identities ci ON ci.external_id = mr.actor_external_id
         WHERE mr.message_id IN ({placeholders}) AND mr.removed_at IS NULL
         ORDER BY mr.message_id ASC"
    );
    let mut react_stmt = conn.prepare(&react_query)?;
    let mut react_rows = react_stmt.query(params_ref.as_slice())?;
    let mut raw_reactions: HashMap<
        i64,
        HashMap<String, (u32, bool, Vec<ConversationReactionActor>)>,
    > = HashMap::new();
    while let Some(row) = react_rows.next()? {
        let msg_id: i64 = row.get(0)?;
        let icon: String = row.get(1)?;
        let identity_id: Option<i64> = row.get(2)?;
        let display_name: Option<String> = row.get(3)?;
        let avatar_url: Option<String> = row.get(4)?;

        let entry = raw_reactions
            .entry(msg_id)
            .or_default()
            .entry(icon)
            .or_insert_with(|| (0, false, Vec::new()));
        entry.0 += 1;
        if let Some(id) = identity_id {
            entry.2.push(ConversationReactionActor {
                identity_id: id,
                display_name,
                avatar_url,
            });
        }
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
    }

    Ok(())
}
