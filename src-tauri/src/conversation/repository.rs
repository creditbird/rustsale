use crate::conversation::error::{ConversationError, ConversationErrorCode};
use crate::conversation::reactions::{canonical_reaction_code, mark_reaction_removed};
pub use crate::conversation::types::AccountConnectionStatus;
use crate::conversation::types::{
    utc_now, Channel, ChannelBadgeSummary, ConversationAccountRuntime, ConversationKind,
    ConversationMessage, ConversationMessageActions, ConversationParticipantSummary,
    ConversationReplyTarget, ConversationRuntimeStatus, ConversationSendPartContent, InboxItem,
    InboxItemKind, MessageDirection, MessageStatus, SendConversationBatchRequest,
    SendConversationBatchResult, TimelineCursor, TimelinePage,
};
pub use crate::conversation::zalo::normalizer::{
    NormalizedGroupParticipant, NormalizedGroupSnapshot, NormalizedReaction, NormalizedReceipt,
    NormalizedUndo, ReceiptKind,
};
use crate::db::open_db_connection;
use chrono::Utc;
use rusqlite::{params, OptionalExtension, Result};
use std::sync::atomic::{AtomicU64, Ordering};

static LAST_PROVIDER_CLIENT_ID: AtomicU64 = AtomicU64::new(0);

type UnlinkedIdentityRow = (
    i64,
    Option<String>,
    Option<String>,
    String,
    Option<String>,
    bool,
);
type GroupConversationRow = (
    i64,
    i64,
    Channel,
    Option<String>,
    ConversationKind,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub fn next_provider_client_message_id() -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let mut observed = LAST_PROVIDER_CLIENT_ID.load(Ordering::Relaxed);
    loop {
        let candidate = now_ms.max(observed.saturating_add(1));
        match LAST_PROVIDER_CLIENT_ID.compare_exchange_weak(
            observed,
            candidate,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => return candidate.to_string(),
            Err(actual) => observed = actual,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedIdentity {
    pub identity_namespace: String,
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub lead_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedAttachment {
    pub provider_attachment_id: Option<String>,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
    pub remote_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedReplyReference {
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub sender_external_id: String,
    pub occurred_at: String,
    pub text_content: Option<String>,
    pub content_type: String,
    pub raw_quote: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedMention {
    pub identity_id: Option<i64>,
    pub external_id: Option<String>,
    pub display_text: String,
    pub start_utf16: usize,
    pub length_utf16: usize,
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
    pub attachments: Vec<NormalizedAttachment>,
    pub reply: Option<NormalizedReplyReference>,
    pub mentions: Vec<NormalizedMention>,
    pub raw_payload: Option<serde_json::Value>,
    pub provider_timestamp: Option<String>,
    pub ingested_at: String,
    pub status: MessageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncCheckpointDetail {
    pub cursor: Option<String>,
    pub last_event_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMessage {
    pub channel_account_id: i64,
    pub identity_id: Option<i64>,
    pub conversation_id: i64,
    pub message_id: i64,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedAttachment {
    pub media_type: String,
    pub mime_type: String,
    pub file_name: Option<String>,
    pub size_bytes: Option<u64>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ClaimedOutboxJob {
    pub job_id: String,
    pub message_id: i64,
    pub conversation_id: i64,
    pub channel_account_id: i64,
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub content_type: String,
    pub text_content: Option<String>,
    pub content_json: serde_json::Value,
    pub provider_client_message_id: String,
    pub attachment: Option<ClaimedAttachment>,
    pub quote: Option<crate::conversation::zalo::client::ProviderQuoteContext>,
    pub send_batch_id: Option<String>,
    pub batch_index: Option<u32>,
    pub attempt_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDirectRecipient {
    pub conversation_id: i64,
    pub external_thread_id: String,
}

pub struct ResolveDirectPhoneRecipientRequest<'a> {
    pub message_id: i64,
    pub channel_account_id: i64,
    pub phone: &'a str,
    pub resolved_uid: &'a str,
    pub display_name: Option<&'a str>,
    pub avatar_url: Option<&'a str>,
    pub now: &'a str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StagedAttachmentRecord {
    pub id: String,
    pub conversation_id: i64,
    pub staged_attachment_id: String,
    pub media_type: String,
    pub mime_type: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub local_path: String,
    pub thumbnail_path: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_seconds: Option<f64>,
    pub status: String,
    pub expires_at: String,
    pub created_at: String,
    pub claimed_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConversationRepository {
    db_path: String,
}

impl ConversationRepository {
    pub fn new(db_path: String) -> Self {
        Self { db_path }
    }

    pub fn db_path(&self) -> &str {
        &self.db_path
    }

    pub fn update_account_identity_and_capabilities(
        &self,
        account_id: i64,
        self_external_id: Option<&str>,
        capabilities_json: &str,
        now: &str,
    ) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        conn.execute(
            "UPDATE channel_accounts
             SET self_external_id = COALESCE(?2, self_external_id),
                 capabilities_json = ?3,
                 connection_status = 'connected',
                 last_connected_at = ?4,
                 updated_at = ?4
             WHERE id = ?1",
            params![account_id, self_external_id, capabilities_json, now],
        )?;
        Ok(())
    }

    pub fn insert_staged_attachment(&self, record: &StagedAttachmentRecord) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        conn.execute(
            "INSERT INTO conversation_staged_attachments
             (id, conversation_id, staged_attachment_id, media_type, mime_type, file_name, file_size_bytes, local_path, thumbnail_path, width, height, duration_seconds, status, expires_at, created_at, claimed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                record.id,
                record.conversation_id,
                record.staged_attachment_id,
                record.media_type,
                record.mime_type,
                record.file_name,
                record.file_size_bytes,
                record.local_path,
                record.thumbnail_path,
                record.width,
                record.height,
                record.duration_seconds,
                record.status,
                record.expires_at,
                record.created_at,
                record.claimed_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_staged_attachment(
        &self,
        staged_attachment_id: &str,
    ) -> Result<Option<StagedAttachmentRecord>> {
        let conn = open_db_connection(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, staged_attachment_id, media_type, mime_type, file_name, file_size_bytes, local_path, thumbnail_path, width, height, duration_seconds, status, expires_at, created_at, claimed_at
             FROM conversation_staged_attachments
             WHERE staged_attachment_id = ?1",
        )?;
        let mut rows = stmt.query(params![staged_attachment_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(StagedAttachmentRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                staged_attachment_id: row.get(2)?,
                media_type: row.get(3)?,
                mime_type: row.get(4)?,
                file_name: row.get(5)?,
                file_size_bytes: row.get(6)?,
                local_path: row.get(7)?,
                thumbnail_path: row.get(8)?,
                width: row.get(9)?,
                height: row.get(10)?,
                duration_seconds: row.get(11)?,
                status: row.get(12)?,
                expires_at: row.get(13)?,
                created_at: row.get(14)?,
                claimed_at: row.get(15)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn mark_staged_attachment_discarded(&self, staged_attachment_id: &str) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        conn.execute(
            "UPDATE conversation_staged_attachments
             SET status = 'discarded'
             WHERE staged_attachment_id = ?1",
            params![staged_attachment_id],
        )?;
        Ok(())
    }

    pub fn list_staging_cleanup_candidates(
        &self,
        now: &str,
    ) -> Result<Vec<StagedAttachmentRecord>> {
        let conn = open_db_connection(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, staged_attachment_id, media_type, mime_type, file_name, file_size_bytes, local_path, thumbnail_path, width, height, duration_seconds, status, expires_at, created_at, claimed_at
             FROM conversation_staged_attachments
             WHERE status = 'discarded' OR (status = 'staged' AND expires_at <= ?1)",
        )?;
        let rows = stmt.query_map(params![now], |row| {
            Ok(StagedAttachmentRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                staged_attachment_id: row.get(2)?,
                media_type: row.get(3)?,
                mime_type: row.get(4)?,
                file_name: row.get(5)?,
                file_size_bytes: row.get(6)?,
                local_path: row.get(7)?,
                thumbnail_path: row.get(8)?,
                width: row.get(9)?,
                height: row.get(10)?,
                duration_seconds: row.get(11)?,
                status: row.get(12)?,
                expires_at: row.get(13)?,
                created_at: row.get(14)?,
                claimed_at: row.get(15)?,
            })
        })?;
        let mut results = Vec::new();
        for r in rows {
            results.push(r?);
        }
        Ok(results)
    }

    pub fn active_spool_bytes(&self) -> Result<u64> {
        let conn = open_db_connection(&self.db_path)?;
        let bytes: i64 = conn.query_row(
            "SELECT COALESCE(SUM(file_size_bytes), 0)
             FROM conversation_staged_attachments
             WHERE status IN ('staged', 'claimed')",
            [],
            |r| r.get(0),
        )?;
        Ok(bytes as u64)
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
        } else {
            None
        };

        let existing_message_id = match existing_message_id {
            Some(id) => Some(id),
            None => {
                if let Some(cli_id) = &message.provider_client_message_id {
                    if !cli_id.trim().is_empty() {
                        tx.query_row(
                            "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_client_message_id = ?2",
                            params![conversation_id, cli_id],
                            |row| row.get::<_, i64>(0),
                        )
                        .optional()?
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };

        let existing_message_id = match existing_message_id {
            Some(id) => Some(id),
            None => {
                if let Some(local_req_id) = &message.local_request_id {
                    tx.query_row(
                        "SELECT id FROM messages WHERE local_request_id = ?1",
                        [local_req_id],
                        |row| row.get::<_, i64>(0),
                    )
                    .optional()?
                } else {
                    None
                }
            }
        };

        if let Some(message_id) = existing_message_id {
            tx.execute(
                "UPDATE messages
                 SET provider_message_id = COALESCE(provider_message_id, ?2),
                     provider_client_message_id = COALESCE(provider_client_message_id, ?3),
                     provider_timestamp = COALESCE(provider_timestamp, ?4),
                     raw_payload_json = COALESCE(?5, raw_payload_json),
                     status = CASE WHEN status IN ('sending', 'queued', 'failed') THEN 'sent' ELSE status END
                 WHERE id = ?1",
                params![
                    message_id,
                    message.provider_message_id,
                    message.provider_client_message_id,
                    message.provider_timestamp,
                    message.raw_payload.as_ref().map(serde_json::Value::to_string),
                ],
            )?;

            tx.execute(
                "UPDATE message_outbox_jobs SET status = 'completed', completed_at = ?2, updated_at = ?2 WHERE message_id = ?1 AND status IN ('pending', 'running', 'retry_wait', 'failed')",
                params![message_id, now],
            )?;

            let send_batch_id: Option<String> = tx
                .query_row(
                    "SELECT send_batch_id FROM messages WHERE id = ?1",
                    [message_id],
                    |r| r.get(0),
                )
                .optional()?
                .flatten();

            if let Some(batch_id) = send_batch_id {
                Self::recalculate_send_batch_status_tx(&tx, &batch_id, now)?;

                let fwd_target_info: Option<(String, i64)> = tx
                    .query_row(
                        "SELECT client_operation_id, target_conversation_id
                         FROM message_forward_targets
                         WHERE send_batch_id = ?1",
                        [&batch_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .optional()?;

                if let Some((op_id, target_conv_id)) = fwd_target_info {
                    crate::conversation::forwarding::update_forward_target_status(
                        &tx,
                        &op_id,
                        target_conv_id,
                        crate::conversation::types::ForwardTargetStatus::Sent,
                        None,
                        None,
                        now,
                    )
                    .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
                }
            }

            for att in &message.attachments {
                let updated = tx.execute(
                    "UPDATE message_attachments
                     SET provider_attachment_id = COALESCE(provider_attachment_id, ?2),
                         mime_type = COALESCE(?4, mime_type),
                         file_name = COALESCE(?5, file_name),
                         size_bytes = COALESCE(?6, size_bytes),
                         remote_url = COALESCE(?7, remote_url),
                         thumbnail_url = COALESCE(?8, thumbnail_url),
                         metadata_json = COALESCE(?9, metadata_json)
                     WHERE message_id = ?1
                       AND (
                           provider_attachment_id = ?2
                           OR (provider_attachment_id IS NULL AND attachment_type = ?3)
                       )",
                    params![
                        message_id,
                        att.provider_attachment_id,
                        att.media_type,
                        att.mime_type,
                        att.file_name,
                        att.size_bytes,
                        att.remote_url,
                        att.thumbnail_url,
                        att.metadata_json.as_ref().map(serde_json::Value::to_string),
                    ],
                )?;

                if updated == 0 {
                    tx.execute(
                        "INSERT INTO message_attachments
                         (message_id, provider_attachment_id, attachment_type, mime_type, file_name, size_bytes, remote_url, thumbnail_url, metadata_json, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![
                            message_id,
                            att.provider_attachment_id,
                            att.media_type,
                            att.mime_type,
                            att.file_name,
                            att.size_bytes,
                            att.remote_url,
                            att.thumbnail_url,
                            att.metadata_json.as_ref().map(serde_json::Value::to_string),
                            now,
                        ],
                    )?;
                }
            }

            Self::save_message_mentions_tx(
                &tx,
                message_id,
                message.channel.as_str(),
                &message.mentions,
                now,
            )?;

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

        for att in &message.attachments {
            tx.execute(
                "INSERT INTO message_attachments
                 (message_id, provider_attachment_id, attachment_type, mime_type, file_name, size_bytes, remote_url, thumbnail_url, metadata_json, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    message_id,
                    att.provider_attachment_id,
                    att.media_type,
                    att.mime_type,
                    att.file_name,
                    att.size_bytes,
                    att.remote_url,
                    att.thumbnail_url,
                    att.metadata_json.as_ref().map(serde_json::Value::to_string),
                    now,
                ],
            )?;
        }

        Self::save_message_mentions_tx(
            &tx,
            message_id,
            message.channel.as_str(),
            &message.mentions,
            now,
        )?;

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
            let title: String = {
                let lead_name: Option<String> = conn
                    .query_row("SELECT name FROM leads WHERE id = ?1", [lead_id], |row| {
                        row.get(0)
                    })
                    .optional()?
                    .flatten();

                let identity_name: Option<String> = conn
                    .query_row(
                        "SELECT display_name FROM channel_identities WHERE lead_id = ?1 AND display_name IS NOT NULL AND TRIM(display_name) != '' ORDER BY id ASC LIMIT 1",
                        [lead_id],
                        |row| row.get(0),
                    )
                    .optional()?
                    .flatten();

                match (lead_name, identity_name) {
                    (Some(l), Some(i))
                        if (l == "Hello World" || l.starts_with("Lead "))
                            && !i.trim().is_empty() =>
                    {
                        i
                    }
                    (Some(l), _) if !l.trim().is_empty() => l,
                    (_, Some(i)) => i,
                    _ => format!("Lead {lead_id}"),
                }
            };

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

                reply_targets.push(crate::conversation::types::InboxReplyTarget {
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
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 LEFT JOIN messages m ON m.conversation_id = cc.id
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
        let unlinked_identities: Vec<UnlinkedIdentityRow> = {
            let mut stmt = conn.prepare(
                "SELECT ci.id,
                        ci.display_name,
                        ci.avatar_url,
                        ci.external_id,
                        (
                            SELECT CASE
                                WHEN json_valid(m.raw_payload_json)
                                THEN json_extract(m.raw_payload_json, '$.dName')
                                ELSE NULL
                            END
                            FROM conversation_participants cp2
                            JOIN messages m ON m.conversation_id = cp2.conversation_id
                            WHERE cp2.identity_id = ci.id
                              AND m.direction = 'inbound'
                              AND NULLIF(TRIM(CASE
                                  WHEN json_valid(m.raw_payload_json)
                                  THEN json_extract(m.raw_payload_json, '$.dName')
                                  ELSE NULL
                              END), '') IS NOT NULL
                            ORDER BY COALESCE(m.provider_timestamp, m.ingested_at) DESC, m.id DESC
                            LIMIT 1
                        ) AS latest_inbound_display_name,
                        EXISTS (
                            SELECT 1
                            FROM conversation_participants cp3
                            JOIN channel_conversations cc3 ON cc3.id = cp3.conversation_id
                            JOIN channel_accounts ca3 ON ca3.id = cc3.channel_account_id
                            WHERE cp3.identity_id = ci.id
                              AND ci.display_name IS NOT NULL
                              AND TRIM(ci.display_name) = TRIM(ca3.display_name)
                        ) AS matches_account_name,
                        MAX(COALESCE(cc.last_message_at, cc.created_at)) as max_at
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
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)? != 0,
                ))
            })?;
            rows.collect::<Result<Vec<_>>>()?
        };

        for (
            identity_id,
            display_name,
            avatar_url,
            external_id,
            latest_inbound_display_name,
            matches_account_name,
        ) in unlinked_identities
        {
            let stored_display_name = if matches_account_name {
                None
            } else {
                display_name
            };
            let title = latest_inbound_display_name
                .filter(|s| !s.trim().is_empty())
                .or_else(|| stored_display_name.filter(|s| !s.trim().is_empty()))
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

                reply_targets.push(crate::conversation::types::InboxReplyTarget {
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
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 LEFT JOIN messages m ON m.conversation_id = cc.id
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
        let group_convs: Vec<GroupConversationRow> = {
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

            let reply_target = crate::conversation::types::InboxReplyTarget {
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
                "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = ?1 AND left_at IS NULL",
                [conv_id],
                |row| row.get(0),
            )?;

            let mut part_stmt = conn.prepare(
                "SELECT cp.identity_id, ci.display_name, ci.avatar_url, cp.participant_role
                 FROM conversation_participants cp
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE cp.conversation_id = ?1 AND cp.left_at IS NULL
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

    pub fn unlink_identity(&self, identity_id: i64) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE channel_identities
             SET lead_id = NULL, link_source = 'unlinked', link_confidence = 0.0, verified_at = NULL, updated_at = ?1
             WHERE id = ?2",
            params![utc_now(), identity_id],
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

    pub fn unlink_lead(&self, lead_id: i64) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let conversation_id = tx
            .query_row(
                "SELECT cp.conversation_id
                 FROM conversation_participants cp
                 JOIN channel_conversations cc ON cc.id = cp.conversation_id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE ci.lead_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 ORDER BY cc.last_message_at DESC, cc.id DESC
                 LIMIT 1",
                [lead_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        tx.execute(
            "UPDATE channel_identities
             SET lead_id = NULL, link_source = 'unlinked', link_confidence = 0.0, verified_at = NULL, updated_at = ?1
             WHERE lead_id = ?2",
            params![utc_now(), lead_id],
        )?;
        tx.commit()?;
        Ok(conversation_id)
    }

    pub fn relink_lead(&self, source_lead_id: i64, target_lead_id: i64) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let conversation_id = tx
            .query_row(
                "SELECT cp.conversation_id
                 FROM conversation_participants cp
                 JOIN channel_conversations cc ON cc.id = cp.conversation_id
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE ci.lead_id = ?1 AND cc.kind IN ('direct', 'email_thread')
                 ORDER BY cc.last_message_at DESC, cc.id DESC
                 LIMIT 1",
                [source_lead_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        let now = utc_now();
        tx.execute(
            "UPDATE channel_identities
             SET lead_id = ?1, link_source = 'manual', link_confidence = 1.0, verified_at = ?2, updated_at = ?2
             WHERE lead_id = ?3",
            params![target_lead_id, now, source_lead_id],
        )?;
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
        let channel_placeholders = std::iter::repeat_n("?", channels.len())
            .collect::<Vec<_>>()
            .join(", ");
        let before_at = before.map(|cursor| cursor.occurred_at.as_str());
        let before_id = before.map(|cursor| cursor.message_id);
        let sql = format!(
            "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                    ca.display_name, m.sender_identity_id,
                    CASE
                        WHEN m.direction = 'inbound'
                         AND json_valid(m.raw_payload_json)
                         AND NULLIF(TRIM(json_extract(m.raw_payload_json, '$.dName')), '') IS NOT NULL
                        THEN json_extract(m.raw_payload_json, '$.dName')
                        ELSE ci.display_name
                    END,
                    m.direction,
                    m.provider_message_id, m.content_type, m.text_content, m.content_json,
                    COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                    m.provider_client_message_id, m.batch_index
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

    pub fn get_identity_timeline(
        &self,
        identity_id: i64,
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        let conn = open_db_connection(&self.db_path)?;
        let before_at = before.map(|cursor| cursor.occurred_at.as_str());
        let before_id = before.map(|cursor| cursor.message_id);
        let sql = "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                    ca.display_name, m.sender_identity_id,
                    CASE
                        WHEN m.direction = 'inbound'
                         AND json_valid(m.raw_payload_json)
                         AND NULLIF(TRIM(json_extract(m.raw_payload_json, '$.dName')), '') IS NOT NULL
                        THEN json_extract(m.raw_payload_json, '$.dName')
                        ELSE ci.display_name
                    END,
                    m.direction,
                    m.provider_message_id, m.content_type, m.text_content, m.content_json,
                    COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                    m.provider_client_message_id, m.batch_index
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
             WHERE EXISTS (
                   SELECT 1
                   FROM conversation_participants cp
                   WHERE cp.conversation_id = cc.id AND cp.identity_id = ?
               )
               AND cc.kind IN ('direct', 'email_thread')
               AND (? IS NULL OR COALESCE(m.provider_timestamp, m.ingested_at) < ?
                    OR (COALESCE(m.provider_timestamp, m.ingested_at) = ? AND m.id < ?))
             ORDER BY occurred_at DESC, m.id DESC
             LIMIT ?";
        let clamped_limit = limit.clamp(1, 100);
        let mut values = Vec::<rusqlite::types::Value>::new();
        values.push(identity_id.into());
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
        let mut page = page_from_descending_rows(rows, clamped_limit);
        crate::conversation::projection::hydrate_messages(&conn, &mut page.items)?;
        Ok(page)
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
                    ca.display_name, m.sender_identity_id,
                    CASE
                        WHEN m.direction = 'inbound'
                         AND json_valid(m.raw_payload_json)
                         AND NULLIF(TRIM(json_extract(m.raw_payload_json, '$.dName')), '') IS NOT NULL
                        THEN json_extract(m.raw_payload_json, '$.dName')
                        ELSE ci.display_name
                    END,
                    m.direction,
                    m.provider_message_id, m.content_type, m.text_content, m.content_json,
                    COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                    m.provider_client_message_id, m.batch_index
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
        let mut page = page_from_descending_rows(rows, clamped_limit);
        crate::conversation::projection::hydrate_messages(&conn, &mut page.items)?;
        Ok(page)
    }

    pub fn apply_reaction(&self, reaction: &NormalizedReaction) -> Result<Option<i64>> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let message_id = match resolve_message_id(
            &tx,
            &reaction.provider_account_key,
            &reaction.external_thread_id,
            reaction.is_group,
            &reaction.provider_message_id,
            &reaction.provider_client_message_id,
        )? {
            Some(id) => id,
            None => return Ok(None),
        };

        let canonical_code = canonical_reaction_code(&reaction.reaction);
        let stored_reaction = canonical_code.unwrap_or(reaction.reaction.trim());

        if reaction.removed {
            mark_reaction_removed(
                &tx,
                message_id,
                &reaction.actor_external_id,
                canonical_code,
                &reaction.occurred_at,
            )?;
        } else {
            tx.execute(
                "INSERT INTO message_reactions (message_id, actor_external_id, reaction, reacted_at, removed_at)
                 VALUES (?1, ?2, ?3, ?4, NULL)
                 ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                     reacted_at = COALESCE(message_reactions.reacted_at, excluded.reacted_at),
                     removed_at = NULL",
                params![
                    message_id,
                    reaction.actor_external_id,
                    stored_reaction,
                    reaction.occurred_at,
                ],
            )?;
        }

        let active_flag = if reaction.removed { 0 } else { 1 };
        let now = utc_now();
        if let Some(reaction_code) = canonical_code {
            tx.execute(
                "UPDATE reaction_mutations
                 SET status = 'succeeded', updated_at = ?1
                 WHERE message_id = ?2 AND reaction_code = ?3 AND active = ?4 AND status IN ('running', 'delivery_unknown')",
                params![now, message_id, reaction_code, active_flag],
            )?;
        } else if reaction.removed {
            tx.execute(
                "UPDATE reaction_mutations
                 SET status = 'succeeded', updated_at = ?1
                 WHERE message_id = ?2 AND active = 0 AND status IN ('running', 'delivery_unknown')",
                params![now, message_id],
            )?;
        }

        tx.commit()?;
        Ok(Some(message_id))
    }

    pub fn apply_undo(&self, undo: &NormalizedUndo) -> Result<Option<i64>> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let message_id = match resolve_message_id(
            &tx,
            &undo.provider_account_key,
            &undo.external_thread_id,
            undo.is_group,
            &undo.provider_message_id,
            &undo.provider_client_message_id,
        )? {
            Some(id) => id,
            None => return Ok(None),
        };

        tx.execute(
            "UPDATE messages
             SET status = 'removed',
                 removed_at = COALESCE(removed_at, ?1)
             WHERE id = ?2",
            params![undo.occurred_at, message_id],
        )?;

        tx.commit()?;
        Ok(Some(message_id))
    }

    pub fn apply_receipt(&self, receipt: &NormalizedReceipt) -> Result<Option<i64>> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let message_id = match resolve_message_id(
            &tx,
            &receipt.provider_account_key,
            &receipt.external_thread_id,
            receipt.is_group,
            &receipt.provider_message_id,
            "",
        )? {
            Some(id) => id,
            None => return Ok(None),
        };

        let receipt_type_str = match receipt.kind {
            ReceiptKind::Delivered => "delivered",
            ReceiptKind::Read => "read",
        };

        let recipients = if receipt.recipient_external_ids.is_empty() {
            vec!["".to_string()]
        } else {
            receipt.recipient_external_ids.clone()
        };

        let payload_str = receipt.raw_payload.to_string();

        for recipient_id in &recipients {
            tx.execute(
                "INSERT INTO message_receipts
                 (message_id, recipient_external_id, receipt_type, occurred_at, raw_payload_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(message_id, recipient_external_id, receipt_type) DO UPDATE SET
                     occurred_at = excluded.occurred_at,
                     raw_payload_json = excluded.raw_payload_json",
                params![
                    message_id,
                    recipient_id,
                    receipt_type_str,
                    receipt.occurred_at,
                    payload_str,
                ],
            )?;
        }

        let current_status: String = tx.query_row(
            "SELECT status FROM messages WHERE id = ?1",
            [message_id],
            |row| row.get(0),
        )?;

        if current_status != "failed" && current_status != "removed" {
            let current_rank = status_rank(&current_status);
            let target_rank = status_rank(receipt_type_str);
            if target_rank > current_rank {
                tx.execute(
                    "UPDATE messages SET status = ?1 WHERE id = ?2",
                    params![receipt_type_str, message_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(Some(message_id))
    }

    pub fn apply_group_snapshot(&self, snapshot: &NormalizedGroupSnapshot) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = &snapshot.occurred_at;

        let default_caps = serde_json::to_string(
            &crate::conversation::types::ConversationCapabilities::zalo_phase_one(),
        )
        .unwrap_or_else(|_| "{}".to_string());
        let channel_account_id: i64 = tx.query_row(
            "INSERT INTO channel_accounts
             (channel, provider_account_key, connection_status, capabilities_json, created_at, updated_at)
             VALUES ('zalo', ?1, 'disconnected', ?2, ?3, ?3)
             ON CONFLICT(channel, provider_account_key) DO UPDATE SET
                 updated_at = excluded.updated_at
             RETURNING id",
            params![snapshot.provider_account_key, default_caps, now],
            |row| row.get(0),
        )?;

        let conversation_id: i64 = tx.query_row(
            "INSERT INTO channel_conversations
             (channel_account_id, external_thread_id, kind, title, avatar_url, status, last_synced_at, created_at, updated_at)
             VALUES (?1, ?2, 'group', ?3, ?4, 'active', ?5, ?5, ?5)
             ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET
                 title = excluded.title,
                 avatar_url = COALESCE(excluded.avatar_url, channel_conversations.avatar_url),
                 last_synced_at = excluded.last_synced_at,
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                channel_account_id,
                snapshot.external_thread_id,
                snapshot.title,
                snapshot.avatar_url,
                now
            ],
            |row| row.get(0),
        )?;

        let mut active_identity_ids = Vec::new();

        for participant in &snapshot.participants {
            let identity_id: i64 = tx.query_row(
                "INSERT INTO channel_identities
                 (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
                 VALUES (?1, 'zalo', ?2, ?3, ?4, ?5, ?6, ?7, ?7)
                 ON CONFLICT(channel, identity_namespace, external_id) DO UPDATE SET
                     lead_id = COALESCE(channel_identities.lead_id, excluded.lead_id),
                     display_name = COALESCE(excluded.display_name, channel_identities.display_name),
                     avatar_url = COALESCE(excluded.avatar_url, channel_identities.avatar_url),
                     updated_at = excluded.updated_at
                 RETURNING id",
                params![
                    participant.identity.lead_id,
                    participant.identity.identity_namespace,
                    participant.identity.external_id,
                    participant.identity.display_name,
                    participant.identity.avatar_url,
                    if participant.identity.lead_id.is_some() { "provider_match" } else { "unlinked" },
                    now
                ],
                |row| row.get(0),
            )?;

            active_identity_ids.push(identity_id);

            tx.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at, left_at)
                 VALUES (?1, ?2, ?3, ?4, NULL)
                 ON CONFLICT(conversation_id, identity_id) DO UPDATE SET
                     participant_role = excluded.participant_role,
                     joined_at = COALESCE(conversation_participants.joined_at, excluded.joined_at),
                     left_at = NULL",
                params![
                    conversation_id,
                    identity_id,
                    participant.participant_role,
                    now
                ],
            )?;
        }

        if snapshot.membership_complete {
            if active_identity_ids.is_empty() {
                tx.execute(
                    "UPDATE conversation_participants
                     SET left_at = ?1
                     WHERE conversation_id = ?2 AND left_at IS NULL",
                    params![now, conversation_id],
                )?;
            } else {
                let query = format!(
                    "UPDATE conversation_participants
                     SET left_at = ?1
                     WHERE conversation_id = ?2
                       AND left_at IS NULL
                       AND identity_id NOT IN ({})",
                    active_identity_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                );
                tx.execute(&query, params![now, conversation_id])?;
            }
        }

        tx.commit()?;
        Ok(conversation_id)
    }

    pub fn update_checkpoint(
        &self,
        channel_account_id: i64,
        scope: &str,
        cursor: Option<&str>,
        event_at: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        let now = utc_now();
        let last_success = if error.is_none() {
            Some(event_at.unwrap_or(&now))
        } else {
            None
        };

        conn.execute(
            "INSERT INTO channel_sync_checkpoints
             (channel_account_id, scope, cursor, last_event_at, last_success_at, last_error, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(channel_account_id, scope) DO UPDATE SET
                 cursor = CASE
                     WHEN excluded.cursor IS NULL THEN channel_sync_checkpoints.cursor
                     WHEN channel_sync_checkpoints.last_event_at IS NULL OR excluded.last_event_at >= channel_sync_checkpoints.last_event_at THEN excluded.cursor
                     ELSE channel_sync_checkpoints.cursor
                 END,
                 last_event_at = CASE
                     WHEN excluded.last_event_at IS NULL THEN channel_sync_checkpoints.last_event_at
                     WHEN channel_sync_checkpoints.last_event_at IS NULL OR excluded.last_event_at >= channel_sync_checkpoints.last_event_at THEN excluded.last_event_at
                     ELSE channel_sync_checkpoints.last_event_at
                 END,
                 last_success_at = COALESCE(excluded.last_success_at, channel_sync_checkpoints.last_success_at),
                 last_error = excluded.last_error,
                 updated_at = excluded.updated_at",
            params![
                channel_account_id,
                scope,
                cursor,
                event_at,
                last_success,
                error,
                now
            ],
        )?;

        Ok(())
    }

    pub fn update_history_cursor(
        &self,
        channel_account_id: i64,
        scope: &str,
        cursor: &str,
    ) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        let now = utc_now();

        conn.execute(
            "INSERT INTO channel_sync_checkpoints
             (channel_account_id, scope, cursor, last_event_at, last_success_at, last_error, updated_at)
             VALUES (?1, ?2, ?3, NULL, ?4, NULL, ?4)
             ON CONFLICT(channel_account_id, scope) DO UPDATE SET
                 cursor = excluded.cursor,
                 last_success_at = excluded.last_success_at,
                 last_error = NULL,
                 updated_at = excluded.updated_at",
            params![channel_account_id, scope, cursor, now],
        )?;

        Ok(())
    }

    pub fn get_checkpoint(&self, channel_account_id: i64, scope: &str) -> Result<Option<String>> {
        let conn = open_db_connection(&self.db_path)?;
        let cursor: Option<String> = conn
            .query_row(
                "SELECT cursor FROM channel_sync_checkpoints WHERE channel_account_id = ?1 AND scope = ?2",
                params![channel_account_id, scope],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();
        Ok(cursor)
    }

    pub fn get_checkpoint_detail(
        &self,
        channel_account_id: i64,
        scope: &str,
    ) -> Result<Option<SyncCheckpointDetail>> {
        let conn = open_db_connection(&self.db_path)?;
        let detail = conn
            .query_row(
                "SELECT cursor, last_event_at, last_success_at, last_error FROM channel_sync_checkpoints WHERE channel_account_id = ?1 AND scope = ?2",
                params![channel_account_id, scope],
                |row| {
                    Ok(SyncCheckpointDetail {
                        cursor: row.get(0)?,
                        last_event_at: row.get(1)?,
                        last_success_at: row.get(2)?,
                        last_error: row.get(3)?,
                    })
                },
            )
            .optional()?;
        Ok(detail)
    }

    pub fn set_account_status(
        &self,
        provider_account_key: &str,
        status: AccountConnectionStatus,
        error: Option<&str>,
    ) -> Result<i64> {
        let conn = open_db_connection(&self.db_path)?;
        let now = utc_now();
        let status_str = status.as_str();
        let last_connected = if status == AccountConnectionStatus::Connected {
            Some(now.as_str())
        } else {
            None
        };

        let default_caps = serde_json::to_string(
            &crate::conversation::types::ConversationCapabilities::zalo_phase_one(),
        )
        .unwrap_or_else(|_| "{}".to_string());
        let id: i64 = conn.query_row(
            "INSERT INTO channel_accounts
             (channel, provider_account_key, connection_status, last_error, last_connected_at, capabilities_json, created_at, updated_at)
             VALUES ('zalo', ?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(channel, provider_account_key) DO UPDATE SET
                 connection_status = excluded.connection_status,
                 last_error = excluded.last_error,
                 last_connected_at = COALESCE(excluded.last_connected_at, channel_accounts.last_connected_at),
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                provider_account_key,
                status_str,
                error,
                last_connected,
                default_caps,
                now,
            ],
            |row| row.get(0),
        )?;

        Ok(id)
    }

    pub fn lead_id_for_identity(&self, identity_id: i64) -> Result<Option<i64>> {
        let conn = open_db_connection(&self.db_path)?;
        let lead_id: Option<i64> = conn
            .query_row(
                "SELECT lead_id FROM channel_identities WHERE id = ?1",
                [identity_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(lead_id)
    }

    pub fn lookup_message_invalidation_info(
        &self,
        message_id: i64,
    ) -> Result<Option<(i64, Option<i64>, Channel)>> {
        let conn = open_db_connection(&self.db_path)?;
        let row: Option<(i64, Option<i64>, String)> = conn
            .query_row(
                "SELECT m.conversation_id, ci.lead_id, ca.channel
                 FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 LEFT JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 LEFT JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE m.id = ?1
                 LIMIT 1",
                [message_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        if let Some((conversation_id, lead_id, channel_str)) = row {
            let channel = Channel::from_str(&channel_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
            Ok(Some((conversation_id, lead_id, channel)))
        } else {
            Ok(None)
        }
    }

    pub fn lookup_message_thread_info(&self, message_id: i64) -> Result<(String, String, bool)> {
        let conn = open_db_connection(&self.db_path)?;
        let (account_key, external_thread_id, kind_str): (String, String, String) = conn
            .query_row(
                "SELECT ca.provider_account_key, cc.external_thread_id, cc.kind
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             WHERE m.id = ?1",
                [message_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )?;
        let is_group = kind_str == "group";
        Ok((account_key, external_thread_id, is_group))
    }

    pub fn mark_message_recalled(&self, message_id: i64, now: &str) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        conn.execute(
            "UPDATE messages SET status = 'removed', removed_at = COALESCE(removed_at, ?2) WHERE id = ?1",
            params![message_id, now],
        )?;
        Ok(())
    }

    pub fn get_message_by_id(&self, message_id: i64) -> Result<Option<ConversationMessage>> {
        let conn = open_db_connection(&self.db_path)?;
        let sql = "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                    ca.display_name, m.sender_identity_id, ci.display_name,
                    m.direction, m.provider_message_id, m.content_type, m.text_content,
                    m.content_json, COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                    m.provider_client_message_id, m.batch_index
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
             WHERE m.id = ?1";
        let msg: Option<ConversationMessage> = conn
            .query_row(sql, [message_id], map_conversation_message)
            .optional()?;
        if let Some(m) = msg {
            let mut list = vec![m];
            crate::conversation::projection::hydrate_messages(&conn, &mut list)?;
            Ok(list.pop())
        } else {
            Ok(None)
        }
    }

    pub fn get_conversation_account_key(&self, conversation_id: i64) -> Result<Option<String>> {
        let conn = open_db_connection(&self.db_path)?;
        let key: Option<String> = conn
            .query_row(
                "SELECT ca.provider_account_key
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 WHERE cc.id = ?1",
                [conversation_id],
                |row| row.get(0),
            )
            .optional()?;
        Ok(key)
    }

    pub fn get_channel_account_id(&self, provider_account_key: &str) -> Result<Option<i64>> {
        let conn = open_db_connection(&self.db_path)?;
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM channel_accounts WHERE provider_account_key = ?1 AND channel = 'zalo'",
                [provider_account_key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id)
    }

    pub fn get_runtime_status(&self) -> Result<ConversationRuntimeStatus> {
        let conn = open_db_connection(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT id, channel, provider_account_key, display_name, connection_status, last_connected_at, last_error, capabilities_json
             FROM channel_accounts
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let channel_str: String = row.get(1)?;
            let channel = Channel::from_str(&channel_str).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
                )
            })?;
            let status_str: String = row.get(4)?;
            let status = AccountConnectionStatus::from_db(&status_str)?;
            Ok(ConversationAccountRuntime {
                channel_account_id: row.get(0)?,
                channel,
                provider_account_key: row.get(2)?,
                display_name: row.get(3)?,
                capabilities: crate::conversation::types::ConversationCapabilities::from_json(
                    &row.get::<_, String>(7)?,
                ),
                status,
                last_connected_at: row.get(5)?,
                last_error: row.get(6)?,
            })
        })?;

        let mut accounts = Vec::new();
        for account in rows {
            accounts.push(account?);
        }

        Ok(ConversationRuntimeStatus { accounts })
    }

    pub fn message_count(&self) -> u64 {
        let conn = match open_db_connection(&self.db_path) {
            Ok(c) => c,
            Err(_) => return 0,
        };
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
            .unwrap_or(0);
        count as u64
    }

    pub fn account_status(&self, imei: &str) -> String {
        let conn = match open_db_connection(&self.db_path) {
            Ok(c) => c,
            Err(_) => return "unknown".to_string(),
        };
        let status: String = conn
            .query_row(
                "SELECT connection_status FROM channel_accounts WHERE provider_account_key = ?1 AND channel = 'zalo'",
                [imei],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "unknown".to_string());
        status
    }

    pub fn enqueue_outbound_message(
        &self,
        request: &crate::conversation::types::SendConversationMessageRequest,
        local_request_id: &str,
        job_id: &str,
        now: &str,
    ) -> Result<ConversationMessage> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let belongs: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM channel_conversations WHERE id = ?1 AND channel_account_id = ?2",
                params![request.conversation_id, request.channel_account_id],
                |row| row.get(0),
            )
            .optional()?;

        if belongs.is_none() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }

        let content_json = serde_json::json!({ "text": request.text }).to_string();
        let provider_client_message_id = next_provider_client_message_id();
        let message_id: i64 = tx.query_row(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id, provider_client_message_id,
              local_request_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp,
              ingested_at, sent_at, edited_at, removed_at, status, error_code, error_message, reply_to_message_id)
             VALUES (?1, NULL, 'outbound', NULL, ?2, ?3, 'text', ?4, ?5, NULL, NULL, ?6, NULL, NULL, NULL, 'queued', NULL, NULL, ?7)
             RETURNING id",
            params![
                request.conversation_id,
                provider_client_message_id,
                local_request_id,
                request.text,
                content_json,
                now,
                request.reply_to_message_id,
            ],
            |row| row.get(0),
        )?;

        tx.execute(
            "INSERT INTO message_outbox_jobs
             (id, message_id, channel_account_id, status, attempt_count, next_attempt_at, last_error, created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, 'pending', 0, ?4, NULL, ?4, ?4, NULL)",
            params![job_id, message_id, request.channel_account_id, now],
        )?;

        let message = tx.query_row(
            "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                    ca.display_name, m.sender_identity_id, ci.display_name, m.direction,
                    m.provider_message_id, m.content_type, m.text_content, m.content_json,
                    COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                    m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                    m.provider_client_message_id, m.batch_index
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
             WHERE m.id = ?1",
            [message_id],
            map_conversation_message,
        )?;

        tx.commit()?;
        Ok(message)
    }

    fn save_message_mentions_tx(
        tx: &rusqlite::Transaction<'_>,
        message_id: i64,
        channel: &str,
        mentions: &[NormalizedMention],
        now: &str,
    ) -> Result<()> {
        tx.execute(
            "DELETE FROM message_mentions WHERE message_id = ?1",
            [message_id],
        )?;
        for m in mentions {
            let identity_id = if let Some(id) = m.identity_id {
                Some(id)
            } else if let Some(ext_id) = &m.external_id {
                tx.query_row(
                    "SELECT id FROM channel_identities
                     WHERE channel = ?1 AND identity_namespace = 'zalo_uid' AND external_id = ?2",
                    params![channel, ext_id],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?
            } else {
                None
            };

            tx.execute(
                "INSERT INTO message_mentions
                 (message_id, identity_id, external_id, display_text, start_utf16, length_utf16, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(message_id, start_utf16, length_utf16) DO UPDATE SET
                     identity_id = excluded.identity_id,
                     external_id = excluded.external_id,
                     display_text = excluded.display_text",
                params![
                    message_id,
                    identity_id,
                    m.external_id,
                    m.display_text,
                    m.start_utf16 as i64,
                    m.length_utf16 as i64,
                    now,
                ],
            )?;
        }
        Ok(())
    }

    fn recalculate_send_batch_status_tx(
        tx: &rusqlite::Transaction<'_>,
        send_batch_id: &str,
        now: &str,
    ) -> Result<()> {
        let (total, sent_count, failed_count, active_count): (i32, i32, i32, i32) = tx.query_row(
            "SELECT
            COUNT(*),
            SUM(CASE WHEN status = 'sent' THEN 1 ELSE 0 END),
            SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END),
            SUM(CASE WHEN status IN ('queued', 'sending') THEN 1 ELSE 0 END)
         FROM messages WHERE send_batch_id = ?1",
            [send_batch_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        let status = if sent_count == total {
            "sent"
        } else if failed_count == total {
            "failed"
        } else if sent_count > 0 && (sent_count + failed_count == total) {
            "partial"
        } else if active_count > 0 && sent_count > 0 {
            "sending"
        } else {
            "queued"
        };

        let completed_at = if status == "sent" || status == "failed" || status == "partial" {
            Some(now)
        } else {
            None
        };

        tx.execute(
            "UPDATE message_send_batches
         SET status = ?2, updated_at = ?3, completed_at = COALESCE(completed_at, ?4)
         WHERE id = ?1",
            params![send_batch_id, status, now, completed_at],
        )?;

        Ok(())
    }

    pub fn enqueue_outbound_batch(
        &self,
        request: &SendConversationBatchRequest,
        now: &str,
    ) -> Result<SendConversationBatchResult, ConversationError> {
        let mut conn = open_db_connection(&self.db_path).map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
        })?;
        let tx = conn.transaction().map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
        })?;

        let (channel_account_id, kind): (i64, String) = tx
            .query_row(
                "SELECT channel_account_id, kind FROM channel_conversations WHERE id = ?1",
                [request.conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?
            .ok_or_else(|| {
                ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    "Target conversation does not exist",
                )
            })?;

        let participants = if kind == "group" {
            let mut part_stmt = tx
                .prepare(
                    "SELECT cp.identity_id, ci.external_id, ci.display_name, CASE WHEN cp.left_at IS NULL THEN 1 ELSE 0 END AS is_active
                     FROM conversation_participants cp
                     JOIN channel_identities ci ON ci.id = cp.identity_id
                     WHERE cp.conversation_id = ?1",
                )
                .map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

            let rows = part_stmt
                .query_map([request.conversation_id], |row| {
                    let identity_id: i64 = row.get(0)?;
                    let external_id: Option<String> = row.get(1)?;
                    let display_name: String = row.get(2)?;
                    let is_active_int: i32 = row.get(3)?;
                    Ok(crate::conversation::mentions::MentionParticipant {
                        identity_id,
                        external_id,
                        display_name,
                        is_active: is_active_int != 0,
                    })
                })
                .map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?;

            let mut list = Vec::new();
            for r in rows {
                list.push(r.map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?);
            }
            drop(part_stmt);
            list
        } else {
            Vec::new()
        };

        let mut part_resolved_mentions = Vec::with_capacity(request.parts.len());
        for part in &request.parts {
            if let ConversationSendPartContent::Text { text, mentions } = &part.content {
                let resolved = crate::conversation::mentions::validate_mentions(
                    text,
                    mentions,
                    &participants,
                )?;
                part_resolved_mentions.push(resolved);
            } else {
                part_resolved_mentions.push(Vec::new());
            }
        }

        let existing_batch_id: Option<String> = tx
            .query_row(
                "SELECT id FROM message_send_batches WHERE channel_account_id = ?1 AND client_batch_id = ?2",
                params![channel_account_id, request.client_batch_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

        if let Some(batch_id) = existing_batch_id {
            let mut stmt = tx
                .prepare(
                    "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                        ca.display_name, m.sender_identity_id, ci.display_name, m.direction,
                        m.provider_message_id, m.content_type, m.text_content, m.content_json,
                        COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                        m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                        m.provider_client_message_id, m.batch_index
                 FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
                 WHERE m.send_batch_id = ?1
                 ORDER BY m.batch_index ASC",
                )
                .map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?;

            let rows = stmt
                .query_map([&batch_id], map_conversation_message)
                .map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?;
            let mut messages = Vec::new();
            for r in rows {
                messages.push(r.map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?);
            }
            drop(stmt);
            tx.commit().map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?;

            return Ok(SendConversationBatchResult {
                batch_id,
                client_batch_id: request.client_batch_id.clone(),
                messages,
            });
        }

        if request.parts.is_empty() || request.parts.len() > 10 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Batch must contain between 1 and 10 parts",
            ));
        }

        let has_sticker = request
            .parts
            .iter()
            .any(|p| matches!(p.content, ConversationSendPartContent::Sticker { .. }));
        if has_sticker && request.parts.len() > 1 {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Sticker must be the only part in a message batch",
            ));
        }

        if let Some(quote_id) = request.reply_to_message_id {
            let quote_belongs: Option<i64> = tx
                .query_row(
                    "SELECT 1 FROM messages WHERE id = ?1 AND conversation_id = ?2",
                    params![quote_id, request.conversation_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?;

            if quote_belongs.is_none() {
                return Err(ConversationError::new(
                    ConversationErrorCode::QuoteNotReady,
                    "Quote reply target message not found in conversation",
                ));
            }
        }

        for part in &request.parts {
            if let ConversationSendPartContent::Attachment {
                staged_attachment_id,
            } = &part.content
            {
                let status: Option<String> = tx
                    .query_row(
                        "SELECT status FROM conversation_staged_attachments WHERE staged_attachment_id = ?1 AND conversation_id = ?2",
                        params![staged_attachment_id, request.conversation_id],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

                match status.as_deref() {
                    Some("staged") => {}
                    Some("claimed") | Some("discarded") | Some("expired") => {
                        return Err(ConversationError::new(
                            ConversationErrorCode::StageAlreadyClaimed,
                            format!(
                                "Attachment staging token {} is not active",
                                staged_attachment_id
                            ),
                        ));
                    }
                    None => {
                        return Err(ConversationError::new(
                            ConversationErrorCode::StageExpired,
                            format!(
                                "Attachment staging token {} not found",
                                staged_attachment_id
                            ),
                        ));
                    }
                    _ => {
                        return Err(ConversationError::new(
                            ConversationErrorCode::StageAlreadyClaimed,
                            format!(
                                "Attachment staging token {} is not active",
                                staged_attachment_id
                            ),
                        ));
                    }
                }
            }
        }

        let batch_id = format!(
            "batch-{:016x}{:016x}",
            rand::random::<u64>(),
            rand::random::<u64>()
        );
        tx.execute(
            "INSERT INTO message_send_batches
             (id, conversation_id, channel_account_id, client_batch_id, reply_to_message_id, part_count, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'queued', ?7, ?7)",
            params![
                batch_id,
                request.conversation_id,
                channel_account_id,
                request.client_batch_id,
                request.reply_to_message_id,
                request.parts.len() as i32,
                now,
            ],
        ).map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

        let mut created_message_ids = Vec::new();

        for (idx, part) in request.parts.iter().enumerate() {
            let local_req_id = format!(
                "batch:{}:part:{}",
                request.client_batch_id, part.client_part_id
            );
            let cli_msg_id = next_provider_client_message_id();
            let reply_to = if idx == 0 {
                request.reply_to_message_id
            } else {
                None
            };

            let (content_type, text_content, content_json) = match &part.content {
                ConversationSendPartContent::Text { text, .. } => (
                    "text",
                    Some(text.clone()),
                    serde_json::json!({ "text": text }).to_string(),
                ),
                ConversationSendPartContent::Forward { source_message_id } => (
                    "forward",
                    None,
                    serde_json::json!({ "sourceMessageId": source_message_id }).to_string(),
                ),
                ConversationSendPartContent::Sticker {
                    sticker_id,
                    cate_id,
                    sticker_type,
                } => (
                    "sticker",
                    None,
                    serde_json::json!({
                        "stickerId": sticker_id,
                        "cateId": cate_id,
                        "type": sticker_type,
                    })
                    .to_string(),
                ),
                ConversationSendPartContent::Attachment {
                    staged_attachment_id,
                } => (
                    "attachment",
                    None,
                    serde_json::json!({ "stagedAttachmentId": staged_attachment_id }).to_string(),
                ),
            };

            let message_id: i64 = tx.query_row(
                "INSERT INTO messages
                 (conversation_id, sender_identity_id, direction, provider_message_id, provider_client_message_id,
                  local_request_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp,
                  ingested_at, sent_at, edited_at, removed_at, status, error_code, error_message, reply_to_message_id,
                  send_batch_id, batch_index)
                 VALUES (?1, NULL, 'outbound', NULL, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7, NULL, NULL, NULL, 'queued', NULL, NULL, ?8, ?9, ?10)
                 RETURNING id",
                params![
                    request.conversation_id,
                    cli_msg_id,
                    local_req_id,
                    content_type,
                    text_content,
                    content_json,
                    now,
                    reply_to,
                    batch_id,
                    idx as i32,
                ],
                |row| row.get(0),
            ).map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

            created_message_ids.push(message_id);

            let resolved_mentions = &part_resolved_mentions[idx];
            if !resolved_mentions.is_empty() {
                if let ConversationSendPartContent::Text { text, .. } = &part.content {
                    let normalized_mentions: Vec<NormalizedMention> = resolved_mentions
                        .iter()
                        .map(|rm| {
                            let display_text =
                                crate::conversation::mentions::utf16_slice(text, rm.pos, rm.len)
                                    .unwrap_or_else(|| rm.display_name.clone().unwrap_or_default());
                            NormalizedMention {
                                identity_id: rm.target_identity_id,
                                external_id: if rm.is_all {
                                    Some("all".to_string())
                                } else {
                                    rm.target_external_id.clone()
                                },
                                display_text,
                                start_utf16: rm.pos,
                                length_utf16: rm.len,
                            }
                        })
                        .collect();

                    Self::save_message_mentions_tx(
                        &tx,
                        message_id,
                        "zalo",
                        &normalized_mentions,
                        now,
                    )
                    .map_err(|e| {
                        ConversationError::new(
                            ConversationErrorCode::ProviderFailure,
                            e.to_string(),
                        )
                    })?;
                }
            }

            if let ConversationSendPartContent::Attachment {
                staged_attachment_id,
            } = &part.content
            {
                let rec: (String, String, String, u64, String, Option<String>) = tx
                    .query_row(
                        "SELECT media_type, mime_type, file_name, file_size_bytes, local_path, thumbnail_path
                         FROM conversation_staged_attachments
                         WHERE staged_attachment_id = ?1",
                        params![staged_attachment_id],
                        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                    )
                    .map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

                tx.execute(
                    "INSERT INTO message_attachments
                     (message_id, provider_attachment_id, attachment_type, file_name, mime_type, size_bytes, local_path, thumbnail_url, metadata_json, created_at)
                     VALUES (?1, NULL, ?2, ?3, ?4, ?5, ?6, ?7, '{}', ?8)",
                    params![message_id, rec.0, rec.2, rec.1, rec.3, rec.4, rec.5, now],
                ).map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;

                tx.execute(
                    "UPDATE conversation_staged_attachments
                     SET status = 'claimed', claimed_at = ?1
                     WHERE staged_attachment_id = ?2",
                    params![now, staged_attachment_id],
                )
                .map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?;
            } else if let ConversationSendPartContent::Sticker {
                sticker_id,
                cate_id,
                sticker_type,
            } = &part.content
            {
                let meta = serde_json::json!({
                    "stickerId": sticker_id,
                    "cateId": cate_id,
                    "type": sticker_type,
                })
                .to_string();

                tx.execute(
                    "INSERT INTO message_attachments
                     (message_id, provider_attachment_id, attachment_type, file_name, mime_type, size_bytes, local_path, thumbnail_url, metadata_json, created_at)
                     VALUES (?1, NULL, 'sticker', NULL, NULL, NULL, NULL, NULL, ?2, ?3)",
                    params![message_id, meta, now],
                ).map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;
            }

            let job_id = format!("outbox:{}:{:016x}", message_id, rand::random::<u64>());
            tx.execute(
                "INSERT INTO message_outbox_jobs
                 (id, message_id, channel_account_id, status, attempt_count, next_attempt_at, last_error, created_at, updated_at, completed_at)
                 VALUES (?1, ?2, ?3, 'pending', 0, ?4, NULL, ?4, ?4, NULL)",
                params![job_id, message_id, channel_account_id, now],
            ).map_err(|e| ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string()))?;
        }

        let mut messages = Vec::new();
        for msg_id in created_message_ids {
            let msg = tx
                .query_row(
                    "SELECT m.id, m.conversation_id, cc.channel_account_id, ca.channel,
                        ca.display_name, m.sender_identity_id, ci.display_name, m.direction,
                        m.provider_message_id, m.content_type, m.text_content, m.content_json,
                        COALESCE(m.provider_timestamp, m.ingested_at) AS occurred_at,
                        m.status, m.removed_at, m.error_message, m.reply_to_message_id,
                        m.provider_client_message_id, m.batch_index
                 FROM messages m
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
                 WHERE m.id = ?1",
                    [msg_id],
                    map_conversation_message,
                )
                .map_err(|e| {
                    ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
                })?;

            messages.push(msg);
        }

        tx.commit().map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
        })?;

        Ok(SendConversationBatchResult {
            batch_id,
            client_batch_id: request.client_batch_id.clone(),
            messages,
        })
    }

    pub fn forward_conversation_message(
        &self,
        request: &crate::conversation::types::ForwardConversationMessageRequest,
    ) -> crate::conversation::error::ConversationResult<
        crate::conversation::types::ForwardConversationMessageResult,
    > {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = utc_now();
        let result =
            crate::conversation::forwarding::enqueue_forward_operation(&tx, request, &now)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn get_forward_operation(
        &self,
        client_operation_id: &str,
    ) -> crate::conversation::error::ConversationResult<
        crate::conversation::types::ForwardConversationMessageResult,
    > {
        let conn = open_db_connection(&self.db_path)?;
        crate::conversation::forwarding::load_forward_operation(&conn, client_operation_id)
    }

    pub fn update_forward_target_status(
        &self,
        client_operation_id: &str,
        target_conversation_id: i64,
        status: crate::conversation::types::ForwardTargetStatus,
        error_code: Option<&str>,
        error_message: Option<&str>,
        now: &str,
    ) -> crate::conversation::error::ConversationResult<
        crate::conversation::types::ForwardOperationStatus,
    > {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let res = crate::conversation::forwarding::update_forward_target_status(
            &tx,
            client_operation_id,
            target_conversation_id,
            status,
            error_code,
            error_message,
            now,
        )?;
        tx.commit()?;
        Ok(res)
    }

    pub fn retry_forward_target(
        &self,
        client_operation_id: &str,
        target_conversation_id: i64,
    ) -> crate::conversation::error::ConversationResult<
        crate::conversation::types::ConversationForwardTargetResult,
    > {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = utc_now();
        let res = crate::conversation::forwarding::retry_forward_target(
            &tx,
            client_operation_id,
            target_conversation_id,
            &now,
        )?;
        tx.commit()?;
        Ok(res)
    }

    pub fn get_source_provider_message_id(&self, source_message_id: i64) -> Result<Option<String>> {
        let conn = open_db_connection(&self.db_path)?;
        let p_id: Option<String> = conn
            .query_row(
                "SELECT provider_message_id FROM messages WHERE id = ?1",
                params![source_message_id],
                |r| r.get(0),
            )
            .optional()?;
        Ok(p_id)
    }

    pub fn delete_conversation(
        &self,
        request: &crate::conversation::types::DeleteConversationRequest,
    ) -> Result<()> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        if let Some(conv_id) = request.conversation_id {
            if conv_id > 0 {
                tx.execute(
                    "DELETE FROM messages WHERE conversation_id = ?1",
                    params![conv_id],
                )?;
                tx.execute(
                    "DELETE FROM conversation_participants WHERE conversation_id = ?1",
                    params![conv_id],
                )?;
                tx.execute(
                    "DELETE FROM channel_conversations WHERE id = ?1",
                    params![conv_id],
                )?;
            }
        }

        if let Some(identity_id) = request.identity_id {
            if identity_id > 0 {
                tx.execute(
                    "DELETE FROM messages WHERE sender_identity_id = ?1",
                    params![identity_id],
                )?;
                tx.execute(
                    "DELETE FROM conversation_participants WHERE identity_id = ?1",
                    params![identity_id],
                )?;
                tx.execute(
                    "DELETE FROM channel_identities WHERE id = ?1 AND lead_id IS NULL",
                    params![identity_id],
                )?;
            }
        }

        if let Some(lead_id) = request.lead_id {
            if lead_id > 0 {
                let conv_ids: Vec<i64> = {
                    let mut stmt = tx.prepare(
                        "SELECT DISTINCT cp.conversation_id FROM conversation_participants cp
                         JOIN channel_identities ci ON ci.id = cp.identity_id
                         WHERE ci.lead_id = ?1",
                    )?;
                    let rows = stmt.query_map(params![lead_id], |row| row.get(0))?;
                    rows.filter_map(Result::ok).collect()
                };

                for cid in conv_ids {
                    tx.execute(
                        "DELETE FROM messages WHERE conversation_id = ?1",
                        params![cid],
                    )?;
                    tx.execute(
                        "DELETE FROM conversation_participants WHERE conversation_id = ?1",
                        params![cid],
                    )?;
                    tx.execute(
                        "DELETE FROM channel_conversations WHERE id = ?1",
                        params![cid],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn start_direct_conversation(
        &self,
        request: &crate::conversation::types::StartDirectConversationRequest,
        now: &str,
    ) -> Result<ConversationMessage> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let phone_clean = request.phone_or_external_id.trim();
        if phone_clean.is_empty() {
            return Err(rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "phone number must not be empty",
                ),
            )));
        }
        let normalized_phone = crate::commands::normalize_phone_digits(phone_clean);

        // 1. Try to find a matching lead by phone or zalo_uid
        let matched_lead_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM leads
                 WHERE phone = ?1 OR phone = ?2 OR zalo_uid = ?1 OR zalo_uid = ?2
                 LIMIT 1",
                params![phone_clean, normalized_phone],
                |row| row.get(0),
            )
            .optional()
            .unwrap_or(None);

        let matched_lead_id: Option<i64> = match matched_lead_id {
            Some(lid) => Some(lid),
            None => {
                if let Ok(mut stmt) = tx.prepare(
                    "SELECT id, phone FROM leads WHERE phone IS NOT NULL AND TRIM(phone) != ''",
                ) {
                    let mut found_id = None;
                    if let Ok(mut rows) = stmt.query([]) {
                        while let Ok(Some(row)) = rows.next() {
                            let id: i64 = match row.get(0) {
                                Ok(i) => i,
                                Err(_) => break,
                            };
                            let raw_phone: String = match row.get(1) {
                                Ok(p) => p,
                                Err(_) => break,
                            };
                            if crate::commands::normalize_phone_digits(&raw_phone)
                                == normalized_phone
                            {
                                found_id = Some(id);
                                break;
                            }
                        }
                    }
                    found_id
                } else {
                    None
                }
            }
        };

        // 2. Resolve channel_identity
        let identity_id: i64 = match tx
            .query_row(
                "SELECT id FROM channel_identities
                 WHERE (identity_namespace = 'zalo_uid' AND external_id IN (?1, ?2))
                    OR (channel = 'zalo' AND lead_id IS NOT NULL AND lead_id = ?3)
                 ORDER BY (CASE WHEN lead_id IS NOT NULL THEN 0 ELSE 1 END), id ASC
                 LIMIT 1",
                params![phone_clean, normalized_phone, matched_lead_id],
                |row| row.get(0),
            )
            .optional()?
        {
            Some(id) => {
                if let Some(lead_id) = matched_lead_id {
                    tx.execute(
                        "UPDATE channel_identities SET lead_id = ?1, link_source = 'provider_match' WHERE id = ?2 AND lead_id IS NULL",
                        params![lead_id, id],
                    )?;
                }
                if let Some(ref name) = request.display_name {
                    if !name.trim().is_empty() {
                        tx.execute(
                            "UPDATE channel_identities SET display_name = ?1 WHERE id = ?2 AND (display_name IS NULL OR TRIM(display_name) = '')",
                            params![name.trim(), id],
                        )?;
                    }
                }
                id
            }
            None => tx.query_row(
                "INSERT INTO channel_identities (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
                 VALUES (?1, 'zalo', 'zalo_uid', ?2, ?3, NULL, ?4, ?5, ?5)
                 RETURNING id",
                params![
                    matched_lead_id,
                    phone_clean,
                    request.display_name.as_deref().map(str::trim),
                    if matched_lead_id.is_some() { "provider_match" } else { "unlinked" },
                    now,
                ],
                |row| row.get(0),
            )?,
        };

        // 3. Resolve channel_conversations
        let conversation_id: i64 = match tx
            .query_row(
                "SELECT cc.id FROM channel_conversations cc
                 LEFT JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 WHERE cc.channel_account_id = ?1 AND cc.kind = 'direct' AND (
                     cp.identity_id = ?2
                     OR cc.external_thread_id = ?3
                     OR cc.external_thread_id = ?4
                     OR (?5 IS NOT NULL AND cp.identity_id IN (SELECT id FROM channel_identities WHERE lead_id = ?5))
                 )
                 ORDER BY (CASE WHEN cp.identity_id = ?2 THEN 0 ELSE 1 END), cc.id ASC
                 LIMIT 1",
                params![
                    request.channel_account_id,
                    identity_id,
                    phone_clean,
                    normalized_phone,
                    matched_lead_id,
                ],
                |row| row.get(0),
            )
            .optional()?
        {
            Some(cid) => {
                tx.execute(
                    "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                     VALUES (?1, ?2, 'member', ?3)
                     ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                    params![cid, identity_id, now],
                )?;
                cid
            }
            None => {
                let cid: i64 = tx.query_row(
                    "INSERT INTO channel_conversations (channel_account_id, kind, external_thread_id, title, avatar_url, created_at, updated_at, last_message_at)
                     VALUES (?1, 'direct', ?2, ?3, NULL, ?4, ?4, ?4)
                     RETURNING id",
                    params![
                        request.channel_account_id,
                        phone_clean,
                        request.display_name.as_deref().unwrap_or(phone_clean),
                        now,
                    ],
                    |row| row.get(0),
                )?;
                tx.execute(
                    "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role, joined_at)
                     VALUES (?1, ?2, 'member', ?3)
                     ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                    params![cid, identity_id, now],
                )?;
                cid
            }
        };

        tx.commit()?;

        let send_req = crate::conversation::types::SendConversationMessageRequest {
            conversation_id,
            channel_account_id: request.channel_account_id,
            text: request.initial_message.clone(),
            reply_to_message_id: None,
        };

        let local_request_id = format!("msg-{}", rand::random::<u128>());
        let job_id = format!("outbox-{}", rand::random::<u128>());
        self.enqueue_outbound_message(&send_req, &local_request_id, &job_id, now)
    }

    pub fn resolve_direct_phone_recipient(
        &self,
        request: ResolveDirectPhoneRecipientRequest<'_>,
    ) -> Result<ResolvedDirectRecipient> {
        let ResolveDirectPhoneRecipientRequest {
            message_id,
            channel_account_id,
            phone,
            resolved_uid,
            display_name,
            avatar_url,
            now,
        } = request;
        let resolved_uid = resolved_uid.trim();
        if resolved_uid.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "resolved_uid must not be empty".to_string(),
            ));
        }

        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let normalized_phone = crate::commands::normalize_phone_digits(phone);

        let source_conversation_id: i64 = tx.query_row(
            "SELECT cc.id
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             WHERE m.id = ?1
               AND cc.channel_account_id = ?2
               AND cc.kind = 'direct'",
            params![message_id, channel_account_id],
            |row| row.get(0),
        )?;

        let source_identity_ids: Vec<i64> = {
            let mut stmt = tx.prepare(
                "SELECT identity_id
                 FROM conversation_participants
                 WHERE conversation_id = ?1",
            )?;
            let rows = stmt.query_map([source_conversation_id], |row| row.get(0))?;
            rows.collect::<Result<Vec<_>>>()?
        };

        let mut lead_id: Option<i64> = tx
            .query_row(
                "SELECT ci.lead_id
                 FROM conversation_participants cp
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE cp.conversation_id = ?1 AND ci.lead_id IS NOT NULL
                 ORDER BY ci.id ASC
                 LIMIT 1",
                [source_conversation_id],
                |row| row.get(0),
            )
            .optional()?;

        if lead_id.is_none() {
            lead_id = tx
                .query_row(
                    "SELECT id FROM leads
                     WHERE phone = ?1 OR phone = ?2
                     LIMIT 1",
                    params![phone.trim(), normalized_phone],
                    |row| row.get(0),
                )
                .optional()?;
        }

        if lead_id.is_none() {
            let mut stmt = tx.prepare(
                "SELECT id, phone FROM leads WHERE phone IS NOT NULL AND TRIM(phone) != ''",
            )?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let candidate_id: i64 = row.get(0)?;
                let candidate_phone: String = row.get(1)?;
                if crate::commands::normalize_phone_digits(&candidate_phone) == normalized_phone {
                    lead_id = Some(candidate_id);
                    break;
                }
            }
        }

        let identity_id: i64 = tx.query_row(
            "INSERT INTO channel_identities
             (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
             VALUES (?1, 'zalo', 'zalo_uid', ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(channel, identity_namespace, external_id) DO UPDATE SET
                 lead_id = COALESCE(channel_identities.lead_id, excluded.lead_id),
                 display_name = COALESCE(excluded.display_name, channel_identities.display_name),
                 avatar_url = COALESCE(excluded.avatar_url, channel_identities.avatar_url),
                 link_source = CASE
                     WHEN channel_identities.lead_id IS NULL AND excluded.lead_id IS NOT NULL
                     THEN 'provider_match'
                     ELSE channel_identities.link_source
                 END,
                 updated_at = excluded.updated_at
             RETURNING id",
            params![
                lead_id,
                resolved_uid,
                display_name.map(str::trim).filter(|value| !value.is_empty()),
                avatar_url.map(str::trim).filter(|value| !value.is_empty()),
                if lead_id.is_some() {
                    "provider_match"
                } else {
                    "unlinked"
                },
                now,
            ],
            |row| row.get(0),
        )?;

        if let Some(lead_id) = lead_id {
            tx.execute(
                "UPDATE leads SET zalo_uid = ?1 WHERE id = ?2",
                params![resolved_uid, lead_id],
            )?;
        }

        let existing_conversation_id: Option<i64> = tx
            .query_row(
                "SELECT cc.id
                 FROM channel_conversations cc
                 LEFT JOIN conversation_participants cp ON cp.conversation_id = cc.id
                 WHERE cc.channel_account_id = ?1
                   AND cc.kind = 'direct'
                   AND cc.id != ?2
                   AND (cc.external_thread_id = ?3 OR cp.identity_id = ?4)
                 ORDER BY CASE WHEN cc.external_thread_id = ?3 THEN 0 ELSE 1 END, cc.id ASC
                 LIMIT 1",
                params![
                    channel_account_id,
                    source_conversation_id,
                    resolved_uid,
                    identity_id,
                ],
                |row| row.get(0),
            )
            .optional()?;

        let conversation_id = if let Some(existing_conversation_id) = existing_conversation_id {
            tx.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', ?3)
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![existing_conversation_id, identity_id, now],
            )?;
            tx.execute(
                "UPDATE messages SET conversation_id = ?1 WHERE conversation_id = ?2",
                params![existing_conversation_id, source_conversation_id],
            )?;
            tx.execute(
                "UPDATE channel_conversations
                 SET title = COALESCE(?2, title),
                     avatar_url = COALESCE(?3, avatar_url),
                     last_message_at = COALESCE(
                         (SELECT last_message_at FROM channel_conversations WHERE id = ?4),
                         last_message_at
                     ),
                     updated_at = ?5
                 WHERE id = ?1",
                params![
                    existing_conversation_id,
                    display_name
                        .map(str::trim)
                        .filter(|value| !value.is_empty()),
                    avatar_url.map(str::trim).filter(|value| !value.is_empty()),
                    source_conversation_id,
                    now,
                ],
            )?;
            tx.execute(
                "DELETE FROM channel_conversations WHERE id = ?1",
                [source_conversation_id],
            )?;
            existing_conversation_id
        } else {
            tx.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at)
                 VALUES (?1, ?2, 'member', ?3)
                 ON CONFLICT(conversation_id, identity_id) DO NOTHING",
                params![source_conversation_id, identity_id, now],
            )?;
            tx.execute(
                "DELETE FROM conversation_participants
                 WHERE conversation_id = ?1 AND identity_id != ?2",
                params![source_conversation_id, identity_id],
            )?;
            tx.execute(
                "UPDATE channel_conversations
                 SET external_thread_id = ?2,
                     title = COALESCE(?3, title),
                     avatar_url = COALESCE(?4, avatar_url),
                     updated_at = ?5
                 WHERE id = ?1",
                params![
                    source_conversation_id,
                    resolved_uid,
                    display_name
                        .map(str::trim)
                        .filter(|value| !value.is_empty()),
                    avatar_url.map(str::trim).filter(|value| !value.is_empty()),
                    now,
                ],
            )?;
            source_conversation_id
        };

        for source_identity_id in source_identity_ids {
            if source_identity_id == identity_id {
                continue;
            }
            tx.execute(
                "DELETE FROM channel_identities
                 WHERE id = ?1
                   AND NOT EXISTS (
                       SELECT 1 FROM conversation_participants WHERE identity_id = ?1
                   )
                   AND NOT EXISTS (
                       SELECT 1 FROM messages WHERE sender_identity_id = ?1
                   )",
                [source_identity_id],
            )?;
        }

        tx.commit()?;
        Ok(ResolvedDirectRecipient {
            conversation_id,
            external_thread_id: resolved_uid.to_string(),
        })
    }

    pub fn claim_next_outbox_job(
        &self,
        channel_account_id: i64,
        now: &str,
    ) -> Result<Option<ClaimedOutboxJob>> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let row = tx
            .query_row(
                "SELECT oj.id, oj.message_id, m.conversation_id, oj.channel_account_id, ca.provider_account_key,
                        cc.external_thread_id, cc.kind, m.content_type, m.text_content, m.content_json,
                        m.provider_client_message_id, m.reply_to_message_id, m.send_batch_id, m.batch_index,
                        oj.attempt_count
                 FROM message_outbox_jobs oj
                 JOIN messages m ON m.id = oj.message_id
                 JOIN channel_conversations cc ON cc.id = m.conversation_id
                 JOIN channel_accounts ca ON ca.id = oj.channel_account_id
                 WHERE oj.channel_account_id = ?1
                   AND (ca.connection_status IS NULL OR ca.connection_status = 'connected' OR ca.connection_status = 'synced')
                   AND (
                     oj.status = 'pending'
                     OR (oj.status = 'retry_wait' AND (oj.next_attempt_at IS NULL OR oj.next_attempt_at <= ?2))
                   )
                 ORDER BY COALESCE((SELECT created_at FROM message_send_batches WHERE id = m.send_batch_id), m.ingested_at) ASC,
                          COALESCE(m.batch_index, 0) ASC,
                          oj.created_at ASC,
                          m.id ASC
                 LIMIT 1",
                params![channel_account_id, now],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<i64>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<u32>>(13)?,
                        row.get::<_, u32>(14)?,
                    ))
                },
            )
            .optional()?;

        let (
            job_id,
            message_id,
            conversation_id,
            account_id,
            provider_account_key,
            external_thread_id,
            kind_str,
            content_type,
            text_opt,
            content_json_raw,
            cli_msg_id_opt,
            reply_to_id,
            send_batch_id,
            batch_index,
            current_attempt,
        ) = match row {
            Some(data) => data,
            None => return Ok(None),
        };

        let new_attempt_count = current_attempt + 1;
        tx.execute(
            "UPDATE message_outbox_jobs
             SET status = 'running', attempt_count = ?2, updated_at = ?3
             WHERE id = ?1",
            params![job_id, new_attempt_count, now],
        )?;

        tx.execute(
            "UPDATE messages SET status = 'sending' WHERE id = ?1",
            params![message_id],
        )?;

        if let Some(ref batch_id) = send_batch_id {
            tx.execute(
                "UPDATE message_send_batches SET status = 'sending', updated_at = ?2 WHERE id = ?1 AND status = 'queued'",
                params![batch_id, now],
            )?;
        }

        let attachment: Option<ClaimedAttachment> = tx
            .query_row(
                "SELECT attachment_type, mime_type, file_name, size_bytes, local_path
                 FROM message_attachments
                 WHERE message_id = ?1
                 LIMIT 1",
                [message_id],
                |r| {
                    Ok(ClaimedAttachment {
                        media_type: r.get(0)?,
                        mime_type: r.get(1)?,
                        file_name: r.get(2)?,
                        size_bytes: r.get(3)?,
                        local_path: r.get(4)?,
                    })
                },
            )
            .optional()?;

        let quote: Option<crate::conversation::zalo::client::ProviderQuoteContext> = if let Some(
            reply_id,
        ) =
            reply_to_id
        {
            tx.query_row(
                "SELECT m.provider_message_id, m.provider_client_message_id, ci.external_id, m.text_content, m.content_type, m.provider_timestamp
                 FROM messages m
                 LEFT JOIN channel_identities ci ON ci.id = m.sender_identity_id
                 WHERE m.id = ?1",
                [reply_id],
                |r| {
                    let p_msg_id: Option<String> = r.get(0)?;
                    let p_cli_id: Option<String> = r.get(1)?;
                    let sender_ext: Option<String> = r.get(2)?;
                    let text: Option<String> = r.get(3)?;
                    let c_type: String = r.get(4)?;
                    let ts_str: Option<String> = r.get(5)?;

                    if let (Some(provider_message_id), Some(provider_client_message_id), Some(sender_external_id)) = (p_msg_id, p_cli_id, sender_ext) {
                        let msg_type = match c_type.as_str() {
                            "text" => 1,
                            "sticker" => 2,
                            "attachment" | "image" | "file" => 3,
                            _ => 1,
                        };
                        let provider_timestamp_ms = ts_str
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                            .map(|dt| dt.timestamp_millis() as u64)
                            .unwrap_or(0);
                        Ok(Some(crate::conversation::zalo::client::ProviderQuoteContext {
                            provider_message_id,
                            provider_client_message_id,
                            sender_external_id,
                            content: text.unwrap_or_default(),
                            message_type: msg_type,
                            property_ext: None,
                            provider_timestamp_ms,
                            ttl: 0,
                        }))
                    } else {
                        Ok(None)
                    }
                },
            )?
        } else {
            None
        };

        tx.commit()?;

        let is_group = kind_str == "group";
        let content_json =
            serde_json::from_str(&content_json_raw).unwrap_or(serde_json::Value::Null);
        let provider_client_message_id = cli_msg_id_opt.unwrap_or_default();

        Ok(Some(ClaimedOutboxJob {
            job_id,
            message_id,
            conversation_id,
            channel_account_id: account_id,
            provider_account_key,
            external_thread_id,
            is_group,
            content_type,
            text_content: text_opt,
            content_json,
            provider_client_message_id,
            attachment,
            quote,
            send_batch_id,
            batch_index,
            attempt_count: new_attempt_count,
        }))
    }

    pub fn complete_outbox_job(
        &self,
        job_id: &str,
        provider_message_id: &str,
        provider_client_message_id: &str,
        now: &str,
    ) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let (message_id, send_batch_id): (i64, Option<String>) = tx.query_row(
            "SELECT oj.message_id, m.send_batch_id
             FROM message_outbox_jobs oj
             JOIN messages m ON m.id = oj.message_id
             WHERE oj.id = ?1",
            [job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        tx.execute(
            "UPDATE message_outbox_jobs
             SET status = 'completed', completed_at = ?2, updated_at = ?2
             WHERE id = ?1",
            params![job_id, now],
        )?;

        tx.execute(
            "UPDATE messages
             SET status = 'sent', provider_message_id = ?2, provider_client_message_id = COALESCE(provider_client_message_id, ?3), sent_at = ?4
             WHERE id = ?1",
            params![message_id, provider_message_id, provider_client_message_id, now],
        )?;

        if let Some(ref batch_id) = send_batch_id {
            Self::recalculate_send_batch_status_tx(&tx, batch_id, now)?;
        }

        tx.execute(
            "UPDATE channel_conversations
             SET last_message_at = ?2, updated_at = ?2
             WHERE id = (SELECT conversation_id FROM messages WHERE id = ?1)",
            params![message_id, now],
        )?;

        tx.commit()?;
        Ok(message_id)
    }

    pub fn schedule_outbox_retry(
        &self,
        job_id: &str,
        retry_at: &str,
        code: crate::conversation::error::ConversationErrorCode,
        error: &str,
        now: &str,
    ) -> Result<()> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let (message_id, send_batch_id): (i64, Option<String>) = tx.query_row(
            "SELECT oj.message_id, m.send_batch_id
             FROM message_outbox_jobs oj
             JOIN messages m ON m.id = oj.message_id
             WHERE oj.id = ?1",
            [job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        tx.execute(
            "UPDATE message_outbox_jobs
             SET status = 'retry_wait', next_attempt_at = ?2, last_error = ?3, updated_at = ?4
             WHERE id = ?1",
            params![job_id, retry_at, error, now],
        )?;

        tx.execute(
            "UPDATE messages
             SET status = 'queued', error_code = ?2, error_message = ?3
             WHERE id = ?1",
            params![message_id, code.as_str(), error],
        )?;

        if let Some(ref batch_id) = send_batch_id {
            Self::recalculate_send_batch_status_tx(&tx, batch_id, now)?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn fail_outbox_job(
        &self,
        job_id: &str,
        code: crate::conversation::error::ConversationErrorCode,
        error: &str,
        now: &str,
    ) -> Result<()> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let (message_id, send_batch_id): (i64, Option<String>) = tx.query_row(
            "SELECT oj.message_id, m.send_batch_id
             FROM message_outbox_jobs oj
             JOIN messages m ON m.id = oj.message_id
             WHERE oj.id = ?1",
            [job_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        tx.execute(
            "UPDATE message_outbox_jobs
             SET status = 'failed', last_error = ?2, updated_at = ?3
             WHERE id = ?1",
            params![job_id, error, now],
        )?;

        tx.execute(
            "UPDATE messages
             SET status = 'failed', error_code = ?2, error_message = ?3
             WHERE id = ?1",
            params![message_id, code.as_str(), error],
        )?;

        if let Some(ref batch_id) = send_batch_id {
            Self::recalculate_send_batch_status_tx(&tx, batch_id, now)?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn mark_running_jobs_delivery_unknown(&self, now: &str) -> Result<u64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let error_msg = "Delivery state unknown after application restart; manual retry required.";

        let mut stmt = tx.prepare(
            "SELECT DISTINCT client_operation_id
             FROM message_forward_targets
             WHERE status IN ('sending', 'running')
                OR send_batch_id IN (
                    SELECT m.send_batch_id
                    FROM messages m
                    JOIN message_outbox_jobs j ON j.message_id = m.id
                    WHERE j.status = 'running' AND m.send_batch_id IS NOT NULL
                )",
        )?;
        let affected_ops: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        tx.execute(
            "UPDATE message_forward_targets
             SET status = 'delivery_unknown', error_code = 'delivery_unknown', error_message = ?1, updated_at = ?2
             WHERE status IN ('sending', 'running')
                OR send_batch_id IN (
                    SELECT m.send_batch_id
                    FROM messages m
                    JOIN message_outbox_jobs j ON j.message_id = m.id
                    WHERE j.status = 'running' AND m.send_batch_id IS NOT NULL
                )",
            params![error_msg, now],
        )?;

        tx.execute(
            "UPDATE messages
             SET status = 'failed', error_code = 'delivery_unknown', error_message = ?1
             WHERE id IN (SELECT message_id FROM message_outbox_jobs WHERE status = 'running')",
            params![error_msg],
        )?;

        let count = tx.execute(
            "UPDATE message_outbox_jobs
             SET status = 'failed', last_error = ?1, updated_at = ?2
             WHERE status = 'running'",
            params![error_msg, now],
        )?;

        for op_id in affected_ops {
            let _ = crate::conversation::forwarding::recalculate_forward_operation_status(
                &tx, &op_id, now,
            );
        }

        tx.commit()?;
        Ok(count as u64)
    }

    pub fn get_zalo_identities_needing_profile_sync(
        &self,
        account_id: i64,
        limit: usize,
    ) -> Result<Vec<(i64, String)>> {
        let conn = open_db_connection(&self.db_path)?;
        let now = utc_now();
        let ttl_threshold = Utc::now()
            .checked_sub_signed(chrono::Duration::days(7))
            .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
            .unwrap_or_else(utc_now);

        let mut stmt = conn.prepare(
            "SELECT DISTINCT ci.id, ci.external_id
             FROM channel_identities ci
             JOIN conversation_participants cp ON cp.identity_id = ci.id
             JOIN channel_conversations cc ON cc.id = cp.conversation_id
             WHERE cc.channel_account_id = ?1
               AND cc.kind = 'direct'
               AND ci.channel = 'zalo'
               AND ci.identity_namespace = 'zalo_uid'
               AND (
                 ci.profile_synced_at IS NULL
                 OR ci.profile_synced_at <= ?2
               )
               AND (ci.next_profile_retry_at IS NULL OR ci.next_profile_retry_at <= ?3)
             ORDER BY COALESCE(ci.profile_synced_at, '1970-01-01T00:00:00.000Z') ASC, ci.id ASC
             LIMIT ?4",
        )?;

        let rows = stmt
            .query_map(
                params![account_id, ttl_threshold, now, limit as i64],
                |row: &rusqlite::Row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
            )?
            .collect::<Result<Vec<_>>>()?;

        Ok(rows)
    }

    pub fn mark_identities_profile_synced(
        &self,
        requested_ids: &[i64],
        profiles: &[crate::conversation::zalo::client::NormalizedProfile],
    ) -> Result<usize> {
        if requested_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = utc_now();
        let mut updated = 0;

        // 1. Mark ALL requested UIDs as synced with timestamp to prevent 5s re-query loop
        for id in requested_ids {
            tx.execute(
                "UPDATE channel_identities
                 SET profile_synced_at = ?1,
                     profile_sync_error = NULL,
                     profile_sync_attempts = 0,
                     next_profile_retry_at = NULL,
                     updated_at = ?1
                 WHERE id = ?2",
                params![now, id],
            )?;
        }

        // 2. Update display_name and avatar_url for returned profiles if non-empty
        for profile in profiles {
            let res = tx.execute(
                "UPDATE channel_identities
                 SET display_name = CASE WHEN ?1 IS NOT NULL AND TRIM(?1) != '' THEN ?1 ELSE display_name END,
                     avatar_url = CASE WHEN ?2 IS NOT NULL AND TRIM(?2) != '' THEN ?2 ELSE avatar_url END,
                     updated_at = ?3
                 WHERE channel = 'zalo'
                   AND identity_namespace = 'zalo_uid'
                   AND external_id = ?4",
                params![profile.display_name, profile.avatar_url, now, profile.external_id],
            )?;
            updated += res;
        }

        tx.commit()?;
        Ok(updated)
    }

    pub fn mark_identity_profile_sync_failed(
        &self,
        identity_ids: &[i64],
        error_msg: &str,
    ) -> Result<usize> {
        if identity_ids.is_empty() {
            return Ok(0);
        }

        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = utc_now();
        let mut updated = 0;

        for id in identity_ids {
            let current_attempts: i64 = tx.query_row(
                "SELECT profile_sync_attempts FROM channel_identities WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;

            let new_attempts = current_attempts + 1;
            // Exponential backoff: 300s, 600s, 1200s, 2400s... capped at 86400s (24 hours)
            let backoff_secs =
                (300i64 * (1i64 << (new_attempts.saturating_sub(1).min(10)))).min(86400);
            let next_retry = Utc::now()
                .checked_add_signed(chrono::Duration::seconds(backoff_secs))
                .map(|t: chrono::DateTime<Utc>| {
                    t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                })
                .unwrap_or_else(utc_now);

            let res = tx.execute(
                "UPDATE channel_identities
                 SET profile_sync_error = ?1,
                     profile_sync_attempts = ?2,
                     next_profile_retry_at = ?3,
                     updated_at = ?4
                 WHERE id = ?5",
                params![error_msg, new_attempts, next_retry, now, id],
            )?;
            updated += res;
        }

        tx.commit()?;
        Ok(updated)
    }

    pub fn get_message_mentions_for_provider(
        &self,
        message_id: i64,
    ) -> Result<Vec<crate::conversation::zalo::client::ProviderMention>, ConversationError> {
        let conn = open_db_connection(&self.db_path).map_err(|e| {
            ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
        })?;
        let mut stmt = conn
            .prepare(
                "SELECT start_utf16, length_utf16, identity_id, external_id, display_text
                 FROM message_mentions
                 WHERE message_id = ?1
                 ORDER BY start_utf16 ASC",
            )
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?;

        let rows = stmt
            .query_map([message_id], |row| {
                let pos: i64 = row.get(0)?;
                let len: i64 = row.get(1)?;
                let target_identity_id: Option<i64> = row.get(2)?;
                let target_external_id: Option<String> = row.get(3)?;
                let display_text: Option<String> = row.get(4)?;

                let is_all = target_external_id.as_deref() == Some("all")
                    || (target_identity_id.is_none()
                        && target_external_id.is_none()
                        && display_text.as_deref() == Some("all"));

                let final_external_id = if is_all { None } else { target_external_id };

                Ok(crate::conversation::zalo::client::ProviderMention {
                    pos: pos as usize,
                    len: len as usize,
                    target_external_id: final_external_id,
                    is_all,
                })
            })
            .map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(|e| {
                ConversationError::new(ConversationErrorCode::ProviderFailure, e.to_string())
            })?);
        }
        Ok(result)
    }

    pub fn list_mention_candidates(
        &self,
        conversation_id: i64,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::conversation::types::ConversationMentionCandidate>> {
        let conn = open_db_connection(&self.db_path)?;

        let kind: Option<String> = conn
            .query_row(
                "SELECT kind FROM channel_conversations WHERE id = ?1",
                [conversation_id],
                |r| r.get(0),
            )
            .optional()?;

        let is_group = kind.as_deref() == Some("group");
        let query_clean = query.unwrap_or("").trim().to_lowercase();

        let mut candidates = Vec::new();

        if is_group
            && (query_clean.is_empty()
                || "all".contains(&query_clean)
                || "all".starts_with(&query_clean))
        {
            candidates.push(crate::conversation::types::ConversationMentionCandidate {
                identity_id: None,
                external_id: None,
                display_name: "all".to_string(),
                avatar_url: None,
                is_all: true,
            });
        }

        let mut stmt = conn.prepare(
            "SELECT ci.id, ci.external_id, ci.display_name, ci.avatar_url, ci.normalized_address, l.phone
             FROM conversation_participants cp
             JOIN channel_identities ci ON ci.id = cp.identity_id
             LEFT JOIN leads l ON l.id = ci.lead_id
             WHERE cp.conversation_id = ?1 AND cp.left_at IS NULL
             ORDER BY ci.display_name ASC",
        )?;

        let rows = stmt.query_map([conversation_id], |row| {
            let identity_id: i64 = row.get(0)?;
            let external_id: Option<String> = row.get(1)?;
            let display_name: Option<String> = row.get(2)?;
            let avatar_url: Option<String> = row.get(3)?;
            let normalized_address: Option<String> = row.get(4)?;
            let phone: Option<String> = row.get(5)?;
            Ok((
                identity_id,
                external_id,
                display_name,
                avatar_url,
                normalized_address,
                phone,
            ))
        })?;

        for row in rows {
            let (id, ext_id, name, avatar, norm_addr, phone) = row?;
            let name_str = name.unwrap_or_else(|| format!("User {}", id));
            let name_lower = name_str.to_lowercase();
            let phone_lower = phone.as_deref().unwrap_or("").to_lowercase();
            let addr_lower = norm_addr.as_deref().unwrap_or("").to_lowercase();

            let matches = query_clean.is_empty()
                || name_lower.contains(&query_clean)
                || (!phone_lower.is_empty() && phone_lower.contains(&query_clean))
                || (!addr_lower.is_empty() && addr_lower.contains(&query_clean));

            if matches {
                candidates.push(crate::conversation::types::ConversationMentionCandidate {
                    identity_id: Some(id),
                    external_id: ext_id,
                    display_name: name_str,
                    avatar_url: avatar,
                    is_all: false,
                });
            }
        }

        if let Some(lim) = limit {
            candidates.truncate(lim);
        }

        Ok(candidates)
    }
}

fn resolve_message_id(
    tx: &rusqlite::Transaction,
    provider_account_key: &str,
    external_thread_id: &str,
    is_group: bool,
    provider_message_id: &str,
    provider_client_message_id: &str,
) -> Result<Option<i64>> {
    let channel_account_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM channel_accounts WHERE channel = 'zalo' AND provider_account_key = ?1",
            [provider_account_key],
            |row| row.get(0),
        )
        .optional()?;

    let channel_account_id = match channel_account_id {
        Some(id) => id,
        None => return Ok(None),
    };

    let kind = if is_group { "group" } else { "direct" };
    let conversation_id: Option<i64> = tx
        .query_row(
            "SELECT id FROM channel_conversations WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
            params![channel_account_id, external_thread_id, kind],
            |row| row.get(0),
        )
        .optional()?;

    let conversation_id = match conversation_id {
        Some(id) => id,
        None => return Ok(None),
    };

    if !provider_message_id.is_empty() {
        let msg_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![conversation_id, provider_message_id],
                |row| row.get(0),
            )
            .optional()?;
        if msg_id.is_some() {
            return Ok(msg_id);
        }
    }

    if !provider_client_message_id.is_empty() {
        let msg_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_client_message_id = ?2",
                params![conversation_id, provider_client_message_id],
                |row| row.get(0),
            )
            .optional()?;
        if msg_id.is_some() {
            return Ok(msg_id);
        }
    }

    Ok(None)
}

fn status_rank(status: &str) -> i32 {
    match status {
        "queued" => 1,
        "sending" => 2,
        "sent" => 3,
        "delivered" => 4,
        "read" => 5,
        _ => 0,
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

    let reply_to_message_id: Option<i64> = row.get(16)?;
    let reply_target = reply_to_message_id.map(|id| ConversationReplyTarget {
        message_id: Some(id),
        sender_display_name: None,
        content_type: "text".into(),
        text_content: None,
        media_type: None,
        occurred_at: None,
    });

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
        provider_client_message_id: row.get(17)?,
        content_type: row.get(9)?,
        text_content: row.get(10)?,
        content,
        occurred_at: row.get(12)?,
        status,
        removed_at: row.get(14)?,
        error_message: row.get(15)?,
        reply_to_message_id,
        attachments: Vec::new(),
        reply_target,
        reactions: Vec::new(),
        batch_index: row.get(18)?,
        send_batch: None,
        mentions: Vec::new(),
        actions: ConversationMessageActions::default(),
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
