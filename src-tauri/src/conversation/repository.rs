use crate::conversation::types::{
    utc_now, Channel, ChannelBadgeSummary, ConversationKind, ConversationMessage,
    ConversationParticipantSummary, ConversationReplyTarget, InboxItem, InboxItemKind,
    MessageDirection, MessageStatus, TimelineCursor, TimelinePage,
};
use crate::db::open_db_connection;
use rusqlite::{params, OptionalExtension, Result};

#[derive(Debug, Clone)]
pub struct NormalizedIdentity {
    pub identity_namespace: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub lead_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct NormalizedMessage {
    pub channel: Channel,
    pub provider_account_key: String,
    pub account_display_name: Option<String>,
    pub participant: Option<NormalizedIdentity>,
    pub sender_is_participant: bool,
    pub external_thread_id: String,
    pub conversation_kind: ConversationKind,
    pub conversation_title: Option<String>,
    pub direction: MessageDirection,
    pub provider_message_id: Option<String>,
    pub provider_client_message_id: Option<String>,
    pub local_request_id: Option<String>,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content: serde_json::Value,
    pub raw_payload: Option<serde_json::Value>,
    pub provider_timestamp: Option<String>,
    pub ingested_at: String,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMessage {
    pub channel_account_id: i64,
    pub identity_id: Option<i64>,
    pub conversation_id: i64,
    pub message_id: i64,
    pub duplicate: bool,
}

#[derive(Debug, Clone)]
pub struct ConversationRepository {
    db_path: String,
}

impl ConversationRepository {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    pub fn apply_normalized_message(&self, message: &NormalizedMessage) -> Result<AppliedMessage> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = &message.ingested_at;

        let channel_account_id: i64 = tx.query_row(
            "INSERT INTO channel_accounts
             (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'disconnected', '{}', ?4, ?4)
             ON CONFLICT(channel, provider_account_key) DO UPDATE SET
                 display_name = COALESCE(excluded.display_name, channel_accounts.display_name),
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                message.channel.as_str(),
                message.provider_account_key,
                message.account_display_name,
                now
            ],
            |row| row.get(0),
        )?;

        let identity_id = if let Some(identity) = &message.participant {
            Some(tx.query_row(
                "INSERT INTO channel_identities
                 (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                 ON CONFLICT(channel, identity_namespace, external_id) DO UPDATE SET
                     lead_id = COALESCE(channel_identities.lead_id, excluded.lead_id),
                     display_name = COALESCE(excluded.display_name, channel_identities.display_name),
                     avatar_url = COALESCE(excluded.avatar_url, channel_identities.avatar_url),
                     updated_at = excluded.updated_at
                 RETURNING id",
                params![
                    identity.lead_id,
                    message.channel.as_str(),
                    identity.identity_namespace,
                    identity.external_id,
                    identity.display_name,
                    identity.avatar_url,
                    if identity.lead_id.is_some() { "provider_match" } else { "unlinked" },
                    now
                ],
                |row| row.get::<_, i64>(0),
            )?)
        } else {
            None
        };

        let conversation_id: i64 = tx.query_row(
            "INSERT INTO channel_conversations
             (channel_account_id, external_thread_id, kind, title, status, last_message_at, last_synced_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, ?6, ?6)
             ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                 title = COALESCE(excluded.title, channel_conversations.title),
                 last_message_at = CASE
                     WHEN channel_conversations.last_message_at IS NULL
                          OR excluded.last_message_at > channel_conversations.last_message_at
                     THEN excluded.last_message_at
                     ELSE channel_conversations.last_message_at
                 END,
                 last_synced_at = excluded.last_synced_at,
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                channel_account_id,
                message.external_thread_id,
                message.conversation_kind.as_str(),
                message.conversation_title,
                message.provider_timestamp.as_deref().unwrap_or(now),
                now
            ],
            |row| row.get(0),
        )?;

        if let Some(identity_id) = identity_id {
            tx.execute(
                "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role)
                 VALUES (?1, ?2, 'member')
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![conversation_id, identity_id],
            )?;
        }

        let sender_identity_id = message
            .sender_is_participant
            .then_some(identity_id)
            .flatten();

        let existing_message_id = if let Some(provider_message_id) = &message.provider_message_id {
            tx.query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![conversation_id, provider_message_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        } else if let Some(local_request_id) = &message.local_request_id {
            tx.query_row(
                "SELECT id FROM messages WHERE local_request_id = ?1",
                [local_request_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        } else {
            None
        };

        if let Some(message_id) = existing_message_id {
            tx.commit()?;
            return Ok(AppliedMessage {
                channel_account_id,
                identity_id,
                conversation_id,
                message_id,
                duplicate: true,
            });
        }

        tx.execute(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id,
              provider_client_message_id, local_request_id, content_type, text_content,
              content_json, raw_payload_json, provider_timestamp, ingested_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                conversation_id,
                sender_identity_id,
                message.direction.as_str(),
                message.provider_message_id,
                message.provider_client_message_id,
                message.local_request_id,
                message.content_type,
                message.text_content,
                message.content.to_string(),
                message
                    .raw_payload
                    .as_ref()
                    .map(serde_json::Value::to_string),
                message.provider_timestamp,
                message.ingested_at,
                message.status.as_str()
            ],
        )?;
        let message_id = tx.last_insert_rowid();
        tx.commit()?;

        Ok(AppliedMessage {
            channel_account_id,
            identity_id,
            conversation_id,
            message_id,
            duplicate: false,
        })
    }

    pub fn list_inbox(&self) -> Result<Vec<InboxItem>> {
        let conn = open_db_connection(&self.db_path)?;
        let mut items = Vec::new();

        // 1. Customer rows: Linked direct/email conversations grouped by lead_id (key: "lead:{lead_id}")
        let lead_ids: Vec<i64> = {
            let mut stmt = conn.prepare(
                "SELECT ci.lead_id, MAX(COALESCE(cc.last_message_at, cc.created_at)) as max_at
                 FROM channel_conversations cc
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE ci.lead_id IS NOT NULL
                   AND cc.kind IN ('direct', 'email_thread')
                 GROUP BY ci.lead_id
                 ORDER BY max_at DESC
                 LIMIT 500",
            )?;
            let rows = stmt.query_map([], |row| row.get(0))?;
            rows.collect::<Result<Vec<i64>>>()?
        };

        for lead_id in lead_ids {
            let title: String = conn
                .query_row("SELECT name FROM leads WHERE id = ?1", [lead_id], |row| {
                    row.get(0)
                })
                .optional()?
                .flatten()
                .unwrap_or_else(|| format!("Lead {lead_id}"));

            let avatar_url: Option<String> = conn
                .query_row(
                    "SELECT avatar_url FROM channel_identities WHERE lead_id = ?1 AND avatar_url IS NOT NULL LIMIT 1",
                    [lead_id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();

            let mut stmt = conn.prepare(
                "SELECT DISTINCT cc.id, cc.channel_account_id, ca.channel, ca.display_name, cc.kind, cc.last_message_at
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE ci.lead_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 ORDER BY cc.last_message_at DESC",
            )?;

            let conv_rows = stmt
                .query_map([lead_id], |row| {
                    let channel_str: String = row.get(2)?;
                    let channel = Channel::from_str(&channel_str).unwrap_or(Channel::Zalo);
                    let kind_str: String = row.get(4)?;
                    let kind = match kind_str.as_str() {
                        "group" => ConversationKind::Group,
                        "email_thread" => ConversationKind::EmailThread,
                        _ => ConversationKind::Direct,
                    };
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        channel,
                        row.get::<_, Option<String>>(3)?,
                        kind,
                        row.get::<_, Option<String>>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>>>()?;

            let mut reply_targets = Vec::new();
            for (conv_id, account_id, channel, acc_name, kind, last_msg_at) in &conv_rows {
                let last_inbound_at: Option<String> = conn
                    .query_row(
                        "SELECT COALESCE(provider_timestamp, ingested_at) FROM messages
                         WHERE conversation_id = ?1 AND direction = 'inbound'
                         ORDER BY COALESCE(provider_timestamp, ingested_at) DESC, id DESC LIMIT 1",
                        [conv_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .flatten();

                reply_targets.push(ConversationReplyTarget {
                    conversation_id: *conv_id,
                    channel_account_id: *account_id,
                    channel: *channel,
                    account_display_name: acc_name.clone(),
                    conversation_kind: *kind,
                    last_inbound_at,
                    last_message_at: last_msg_at.clone(),
                });
            }

            let (last_message_preview, last_message_at) = conn
                .query_row(
                    "SELECT m.text_content, COALESCE(m.provider_timestamp, m.ingested_at)
                     FROM messages m
                     JOIN channel_conversations cc ON cc.id = m.conversation_id
                     JOIN conversation_participants cp ON cp.conversation_id = cc.id
                     JOIN channel_identities ci ON ci.id = cp.identity_id
                     WHERE ci.lead_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                     ORDER BY COALESCE(m.provider_timestamp, m.ingested_at) DESC, m.id DESC
                     LIMIT 1",
                    [lead_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .optional()?
                .unwrap_or((
                    None,
                    reply_targets
                        .first()
                        .and_then(|t| t.last_message_at.clone()),
                ));

            let mut badge_stmt = conn.prepare(
                "SELECT ca.channel, COUNT(m.id)
                 FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE ci.lead_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 GROUP BY ca.channel",
            )?;
            let channels = badge_stmt
                .query_map([lead_id], |row| {
                    let channel_str: String = row.get(0)?;
                    let channel = Channel::from_str(&channel_str).unwrap_or(Channel::Zalo);
                    let count: i64 = row.get(1)?;
                    Ok(ChannelBadgeSummary {
                        channel,
                        message_count: count as u64,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            items.push(InboxItem {
                key: format!("lead:{lead_id}"),
                kind: InboxItemKind::Customer,
                lead_id: Some(lead_id),
                conversation_id: None,
                identity_id: None,
                title,
                avatar_url,
                last_message_preview,
                last_message_at,
                channels,
                reply_targets,
                participant_count: 0,
                participants: Vec::new(),
            });
        }

        // 2. Unknown rows: Unlinked direct conversations grouped by identity_id (key: "identity:{identity_id}")
        let unlinked_identities: Vec<(i64, Option<String>, Option<String>, String)> = {
            let mut stmt = conn.prepare(
                "SELECT ci.id, ci.display_name, ci.avatar_url, ci.external_id, MAX(COALESCE(cc.last_message_at, cc.created_at)) as max_at
                 FROM channel_conversations cc
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE ci.lead_id IS NULL
                   AND cc.kind IN ('direct', 'email_thread')
                 GROUP BY ci.id
                 ORDER BY max_at DESC
                 LIMIT 500",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>>>()?
        };

        for (identity_id, display_name, avatar_url, external_id) in unlinked_identities {
            let title = display_name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("Unknown ({external_id})"));

            let mut stmt = conn.prepare(
                "SELECT DISTINCT cc.id, cc.channel_account_id, ca.channel, ca.display_name, cc.kind, cc.last_message_at
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 ORDER BY cc.last_message_at DESC",
            )?;

            let conv_rows = stmt
                .query_map([identity_id], |row| {
                    let channel_str: String = row.get(2)?;
                    let channel = Channel::from_str(&channel_str).unwrap_or(Channel::Zalo);
                    let kind_str: String = row.get(4)?;
                    let kind = match kind_str.as_str() {
                        "group" => ConversationKind::Group,
                        "email_thread" => ConversationKind::EmailThread,
                        _ => ConversationKind::Direct,
                    };
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        channel,
                        row.get::<_, Option<String>>(3)?,
                        kind,
                        row.get::<_, Option<String>>(5)?,
                    ))
                })?
                .collect::<Result<Vec<_>>>()?;

            let mut reply_targets = Vec::new();
            for (conv_id, account_id, channel, acc_name, kind, last_msg_at) in &conv_rows {
                let last_inbound_at: Option<String> = conn
                    .query_row(
                        "SELECT COALESCE(provider_timestamp, ingested_at) FROM messages
                         WHERE conversation_id = ?1 AND direction = 'inbound'
                         ORDER BY COALESCE(provider_timestamp, ingested_at) DESC, id DESC LIMIT 1",
                        [conv_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .flatten();

                reply_targets.push(ConversationReplyTarget {
                    conversation_id: *conv_id,
                    channel_account_id: *account_id,
                    channel: *channel,
                    account_display_name: acc_name.clone(),
                    conversation_kind: *kind,
                    last_inbound_at,
                    last_message_at: last_msg_at.clone(),
                });
            }

            let (last_message_preview, last_message_at) = conn
                .query_row(
                    "SELECT m.text_content, COALESCE(m.provider_timestamp, m.ingested_at)
                     FROM messages m
                     JOIN channel_conversations cc ON cc.id = m.conversation_id
                     JOIN conversation_participants cp ON cp.conversation_id = cc.id
                     WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                     ORDER BY COALESCE(m.provider_timestamp, m.ingested_at) DESC, m.id DESC
                     LIMIT 1",
                    [identity_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .optional()?
                .unwrap_or((
                    None,
                    reply_targets
                        .first()
                        .and_then(|t| t.last_message_at.clone()),
                ));

            let mut badge_stmt = conn.prepare(
                "SELECT ca.channel, COUNT(m.id)
                 FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 GROUP BY ca.channel",
            )?;
            let channels = badge_stmt
                .query_map([identity_id], |row| {
                    let channel_str: String = row.get(0)?;
                    let channel = Channel::from_str(&channel_str).unwrap_or(Channel::Zalo);
                    let count: i64 = row.get(1)?;
                    Ok(ChannelBadgeSummary {
                        channel,
                        message_count: count as u64,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            items.push(InboxItem {
                key: format!("identity:{identity_id}"),
                kind: InboxItemKind::Unknown,
                lead_id: None,
                conversation_id: None,
                identity_id: Some(identity_id),
                title,
                avatar_url,
                last_message_preview,
                last_message_at,
                channels,
                reply_targets,
                participant_count: 0,
                participants: Vec::new(),
            });
        }

        // 3. Group rows: Group conversations (key: "conversation:{conversation_id}")
        let group_convs: Vec<(
            i64,
            i64,
            Channel,
            Option<String>,
            ConversationKind,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = {
            let mut stmt = conn.prepare(
                "SELECT cc.id, cc.channel_account_id, ca.channel, ca.display_name, cc.kind, cc.title, cc.avatar_url, cc.last_message_at
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 WHERE cc.kind = 'group'
                 ORDER BY COALESCE(cc.last_message_at, cc.created_at) DESC
                 LIMIT 500",
            )?;
            let rows = stmt.query_map([], |row| {
                let channel_str: String = row.get(2)?;
                let channel = Channel::from_str(&channel_str).unwrap_or(Channel::Zalo);
                let kind_str: String = row.get(4)?;
                let kind = match kind_str.as_str() {
                    "group" => ConversationKind::Group,
                    "email_thread" => ConversationKind::EmailThread,
                    _ => ConversationKind::Direct,
                };
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    channel,
                    row.get::<_, Option<String>>(3)?,
                    kind,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>>>()?
        };

        for (conv_id, account_id, channel, acc_name, kind, title_opt, avatar_url, last_msg_at) in
            group_convs
        {
            let title = title_opt
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "Group Conversation".to_string());

            let last_inbound_at: Option<String> = conn
                .query_row(
                    "SELECT COALESCE(provider_timestamp, ingested_at) FROM messages
                     WHERE conversation_id = ?1 AND direction = 'inbound'
                     ORDER BY COALESCE(provider_timestamp, ingested_at) DESC, id DESC LIMIT 1",
                    [conv_id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();

            let reply_target = ConversationReplyTarget {
                conversation_id: conv_id,
                channel_account_id: account_id,
                channel,
                account_display_name: acc_name,
                conversation_kind: kind,
                last_inbound_at,
                last_message_at: last_msg_at.clone(),
            };

            let (last_message_preview, msg_last_at) = conn
                .query_row(
                    "SELECT text_content, COALESCE(provider_timestamp, ingested_at)
                     FROM messages
                     WHERE conversation_id = ?1
                     ORDER BY COALESCE(provider_timestamp, ingested_at) DESC, id DESC
                     LIMIT 1",
                    [conv_id],
                    |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?,
                            row.get::<_, Option<String>>(1)?,
                        ))
                    },
                )
                .optional()?
                .unwrap_or((None, last_msg_at.clone()));

            let msg_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                [conv_id],
                |row| row.get(0),
            )?;

            let channels = vec![ChannelBadgeSummary {
                channel,
                message_count: msg_count as u64,
            }];

            let participant_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = ?1",
                [conv_id],
                |row| row.get(0),
            )?;

            let mut part_stmt = conn.prepare(
                "SELECT cp.identity_id, ci.display_name, ci.avatar_url, cp.participant_role
                 FROM conversation_participants cp
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE cp.conversation_id = ?1
                 ORDER BY CASE WHEN cp.participant_role IN ('admin', 'owner') THEN 0 ELSE 1 END,
                          ci.display_name ASC
                 LIMIT 5",
            )?;
            let participants = part_stmt
                .query_map([conv_id], |row| {
                    Ok(ConversationParticipantSummary {
                        identity_id: row.get(0)?,
                        display_name: row.get(1)?,
                        avatar_url: row.get(2)?,
                        participant_role: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            items.push(InboxItem {
                key: format!("conversation:{conv_id}"),
                kind: InboxItemKind::Group,
                lead_id: None,
                conversation_id: Some(conv_id),
                identity_id: None,
                title,
                avatar_url,
                last_message_preview,
                last_message_at: msg_last_at.or(last_msg_at),
                channels,
                reply_targets: vec![reply_target],
                participant_count: participant_count as u64,
                participants,
            });
        }

        // Sort all aggregated rows by last_message_at DESC
        items.sort_by(|a, b| match (&b.last_message_at, &a.last_message_at) {
            (Some(b_at), Some(a_at)) => b_at.cmp(a_at),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });

        Ok(items)
    }

    pub fn link_identity(&self, identity_id: i64, lead_id: i64) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE channel_identities
             SET lead_id = ?1, link_source = 'manual', link_confidence = 1.0, verified_at = ?2, updated_at = ?2
             WHERE id = ?3",
            params![lead_id, utc_now(), identity_id],
        )?;
        let conversation_id = tx
            .query_row(
                "SELECT cp.conversation_id
                 FROM conversation_participants cp
                 JOIN channel_conversations cc ON cc.id = cp.conversation_id
                 WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 ORDER BY cc.last_message_at DESC, cc.id DESC
                 LIMIT 1",
                [identity_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        tx.commit()?;
        Ok(conversation_id)
    }

    pub fn get_contact_timeline(
        &self,
        lead_id: i64,
        channels: &[Channel],
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        if channels.is_empty() {
            return Ok(TimelinePage {
                items: Vec::new(),
                next_cursor: None,
            });
        }
        let conn = open_db_connection(&self.db_path)?;
        let channel_placeholders = std::iter::repeat("?")
            .take(channels.len())
            .collect::<Vec<_>>()
            .join(", ");
        let before_at = before.map(|cursor| cursor.occurred_at.as_str());
        let before_id = before.map(|cursor| cursor.message_id);
        let sql = format!(
            "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                    ca.display_name, m.sender_identity_id, ci.display_name, m.direction,
                    m.provider_message_id, m.content_type, m.text_content, m.content_json,
                    COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
             WHERE EXISTS (
                   SELECT 1
                   FROM conversation_participants cp
                   JOIN channel_identities participant ON participant.id = cp.identity_id
                   WHERE cp.conversation_id = cc.id AND participant.lead_id = ?
               )
               AND cc.kind IN ('direct', 'email_thread')
               AND ca.channel IN ({channel_placeholders})
               AND (? IS NULL OR COALESCE(m.provider_timestamp, m.ingested_at) < ?
                    OR (COALESCE(m.provider_timestamp, m.ingested_at) = ? AND m.id < ?))
             ORDER BY occurred_at DESC, m.id DESC
             LIMIT ?"
        );
        let mut values = Vec::<rusqlite::types::Value>::new();
        values.push(lead_id.into());
        values.extend(
            channels
                .iter()
                .map(|channel| rusqlite::types::Value::Text(channel.as_str().to_string())),
        );
        values.push(before_at.map_or(rusqlite::types::Value::Null, |value| {
            value.to_string().into()
        }));
        values.push(before_at.map_or(rusqlite::types::Value::Null, |value| {
            value.to_string().into()
        }));
        values.push(before_at.map_or(rusqlite::types::Value::Null, |value| {
            value.to_string().into()
        }));
        values.push(before_id.map_or(rusqlite::types::Value::Null, Into::into));
        let clamped_limit = limit.clamp(1, 100);
        values.push(i64::from(clamped_limit).into());
        let mut statement = conn.prepare(&sql)?;
        let rows = statement
            .query_map(
                rusqlite::params_from_iter(values.iter()),
                map_conversation_message,
            )?
            .collect::<Result<Vec<_>>>()?;
        Ok(page_from_descending_rows(rows, clamped_limit))
    }

    pub fn get_conversation_timeline(
        &self,
        conversation_id: i64,
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        let conn = open_db_connection(&self.db_path)?;
        let before_at = before.map(|cursor| cursor.occurred_at.as_str());
        let before_id = before.map(|cursor| cursor.message_id);
        let sql = "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                    ca.display_name, m.sender_identity_id, ci.display_name, m.direction,
                    m.provider_message_id, m.content_type, m.text_content, m.content_json,
                    COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
             WHERE m.conversation_id = ?
               AND (? IS NULL OR COALESCE(m.provider_timestamp, m.ingested_at) < ?
                    OR (COALESCE(m.provider_timestamp, m.ingested_at) = ? AND m.id < ?))
             ORDER BY occurred_at DESC, m.id DESC
             LIMIT ?";
        let clamped_limit = limit.clamp(1, 100);
        let mut values = Vec::<rusqlite::types::Value>::new();
        values.push(conversation_id.into());
        values.push(before_at.map_or(rusqlite::types::Value::Null, |value| {
            value.to_string().into()
        }));
        values.push(before_at.map_or(rusqlite::types::Value::Null, |value| {
            value.to_string().into()
        }));
        values.push(before_at.map_or(rusqlite::types::Value::Null, |value| {
            value.to_string().into()
        }));
        values.push(before_id.map_or(rusqlite::types::Value::Null, Into::into));
        values.push(i64::from(clamped_limit).into());
        let mut statement = conn.prepare(sql)?;
        let rows = statement
            .query_map(
                rusqlite::params_from_iter(values.iter()),
                map_conversation_message,
            )?
            .collect::<Result<Vec<_>>>()?;
        Ok(page_from_descending_rows(rows, clamped_limit))
    }
}

fn map_conversation_message(row: &rusqlite::Row) -> Result<ConversationMessage> {
    let raw_channel: String = row.get(3)?;
    let channel = Channel::from_str(&raw_channel).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            3,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    let raw_direction: String = row.get(7)?;
    let direction = MessageDirection::from_str(&raw_direction).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            7,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    let raw_content_json: String = row.get(11)?;
    let content: serde_json::Value =
        serde_json::from_str(&raw_content_json).unwrap_or_else(|_| serde_json::json!({}));

    let raw_status: String = row.get(13)?;
    let status = MessageStatus::from_str(&raw_status).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            13,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        )
    })?;

    Ok(ConversationMessage {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        channel_account_id: row.get(2)?,
        channel,
        account_display_name: row.get(4)?,
        sender_identity_id: row.get(5)?,
        sender_display_name: row.get(6)?,
        direction,
        provider_message_id: row.get(8)?,
        content_type: row.get(9)?,
        text_content: row.get(10)?,
        content,
        occurred_at: row.get(12)?,
        status,
        removed_at: row.get(14)?,
        error_message: row.get(15)?,
        reply_to_message_id: row.get(16)?,
    })
}

fn page_from_descending_rows(mut rows: Vec<ConversationMessage>, limit: u32) -> TimelinePage {
    let limit_usize = limit.clamp(1, 100) as usize;
    let next_cursor = if rows.len() == limit_usize {
        rows.last().map(|msg| TimelineCursor {
            occurred_at: msg.occurred_at.clone(),
            message_id: msg.id,
        })
    } else {
        None
    };

    rows.reverse();

    TimelinePage {
        items: rows,
        next_cursor,
    }
}
