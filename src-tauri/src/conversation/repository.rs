use crate::conversation::types::{Channel, ConversationKind, MessageDirection, MessageStatus};
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
}
