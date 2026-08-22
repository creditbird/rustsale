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
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

static LAST_PROVIDER_CLIENT_ID: AtomicU64 = AtomicU64::new(0);

fn message_occurred_after_tombstone(message: &NormalizedMessage, deleted_at: &str) -> bool {
    let occurred_at = message
        .provider_timestamp
        .as_deref()
        .unwrap_or(&message.ingested_at);
    let Ok(occurred_at) = chrono::DateTime::parse_from_rfc3339(occurred_at) else {
        tracing::warn!(
            provider_timestamp = ?message.provider_timestamp,
            ingested_at = %message.ingested_at,
            "Suppressing tombstoned conversation message with invalid timestamp"
        );
        return false;
    };
    let Ok(deleted_at) = chrono::DateTime::parse_from_rfc3339(deleted_at) else {
        tracing::warn!(
            deleted_at,
            "Suppressing tombstoned conversation message with invalid deletion timestamp"
        );
        return false;
    };
    occurred_at > deleted_at
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZaloUidCandidate {
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMemberLeadImportItem {
    pub identity_id: Option<i64>,
    pub external_id: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMemberLeadImportRequest {
    pub conversation_id: i64,
    pub members: Vec<GroupMemberLeadImportItem>,
    pub category_ids: Vec<i32>,
    pub new_category_name: Option<String>,
    pub skip_existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupMemberLeadImportResult {
    pub imported_count: usize,
    pub updated_count: usize,
    pub skipped_count: usize,
    pub assigned_category_ids: Vec<i32>,
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
    pub replayed_reaction_count: usize,
}

#[derive(Debug)]
struct PendingMessageReaction {
    id: i64,
    actor_external_id: String,
    reaction: String,
    removed: bool,
    occurred_at: String,
}

#[derive(Debug, Clone)]
struct ExistingZaloIdentity {
    id: i64,
    identity_namespace: String,
    lead_id: Option<i64>,
    normalized_address: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
    profile_synced_at: Option<String>,
    profile_sync_error: Option<String>,
    profile_sync_attempts: i64,
    next_profile_retry_at: Option<String>,
    link_source: String,
    link_confidence: Option<f64>,
    verified_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone)]
struct ResolvedGroupMemberImport {
    identity_id: i64,
    external_id: String,
    display_name: String,
    linked_lead_id: Option<i64>,
    phone: Option<String>,
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

    pub fn attach_zalo_uids_to_conversation(
        &self,
        conversation_id: i64,
        candidates: &[ZaloUidCandidate],
    ) -> Result<Vec<i64>> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let (channel, kind): (String, String) = tx.query_row(
            "SELECT ca.channel, cc.kind
             FROM channel_conversations cc
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             WHERE cc.id = ?1",
            [conversation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if channel != "zalo" || kind != "group" {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "conversation {conversation_id} is not a Zalo group"
            )));
        }

        let mut deduplicated = BTreeMap::<String, ZaloUidCandidate>::new();
        for candidate in candidates {
            let external_id = validated_zalo_uid(&candidate.external_id)?;
            let entry =
                deduplicated
                    .entry(external_id.clone())
                    .or_insert_with(|| ZaloUidCandidate {
                        external_id,
                        display_name: None,
                        avatar_url: None,
                    });
            if non_empty(candidate.display_name.as_deref()).is_some() {
                entry.display_name = candidate.display_name.clone();
            }
            if non_empty(candidate.avatar_url.as_deref()).is_some() {
                entry.avatar_url = candidate.avatar_url.clone();
            }
        }

        let now = utc_now();
        let mut identity_ids = Vec::with_capacity(deduplicated.len());
        for candidate in deduplicated.values() {
            let identity_id = canonicalize_zalo_uid_tx(&tx, candidate, None, &now)?;
            tx.execute(
                "INSERT INTO conversation_participants
                 (conversation_id, identity_id, participant_role, joined_at, left_at)
                 VALUES (?1, ?2, 'member', ?3, NULL)
                 ON CONFLICT(conversation_id, identity_id) DO UPDATE SET
                     joined_at = COALESCE(conversation_participants.joined_at, excluded.joined_at),
                     left_at = NULL",
                params![conversation_id, identity_id, now],
            )?;
            identity_ids.push(identity_id);
        }

        tx.commit()?;
        Ok(identity_ids)
    }

    pub fn import_group_members_to_leads(
        &self,
        request: &GroupMemberLeadImportRequest,
    ) -> Result<GroupMemberLeadImportResult> {
        if request.members.is_empty() {
            return Err(invalid_group_member_import(
                "At least one group member is required",
            ));
        }

        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let (channel, kind, conversation_title): (String, String, Option<String>) = tx
            .query_row(
                "SELECT ca.channel, cc.kind, cc.title
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 WHERE cc.id = ?1",
                [request.conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or_else(|| {
                invalid_group_member_import(format!(
                    "Conversation {} was not found",
                    request.conversation_id
                ))
            })?;
        if channel != "zalo" || kind != "group" {
            return Err(invalid_group_member_import(format!(
                "Conversation {} is not a Zalo group",
                request.conversation_id
            )));
        }

        let mut resolved_members = BTreeMap::<i64, ResolvedGroupMemberImport>::new();
        for member in &request.members {
            let resolved = resolve_group_member_import_tx(&tx, request.conversation_id, member)?;
            match resolved_members.entry(resolved.identity_id) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(resolved);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let existing = entry.get_mut();
                    match (existing.phone.as_deref(), resolved.phone.as_deref()) {
                        (Some(current), Some(next)) if current != next => {
                            return Err(invalid_group_member_import(format!(
                                "Conflicting phone values for identity {}",
                                existing.identity_id
                            )));
                        }
                        (None, Some(_)) => existing.phone = resolved.phone,
                        _ => {}
                    }
                }
            }
        }

        let mut category_ids = request
            .category_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        for category_id in &category_ids {
            if *category_id <= 0 {
                return Err(invalid_group_member_import(format!(
                    "Invalid category ID {category_id}"
                )));
            }
            let exists = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM categories WHERE id = ?1)",
                [category_id],
                |row| row.get::<_, bool>(0),
            )?;
            if !exists {
                return Err(invalid_group_member_import(format!(
                    "Category {category_id} was not found"
                )));
            }
        }
        if let Some(category_name) = request
            .new_category_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
        {
            tx.execute(
                "INSERT INTO categories (name) VALUES (?1)
                 ON CONFLICT(name) DO NOTHING",
                [category_name],
            )?;
            let category_id = tx.query_row(
                "SELECT id FROM categories WHERE name = ?1",
                [category_name],
                |row| row.get::<_, i32>(0),
            )?;
            category_ids.insert(category_id);
        }

        let now = utc_now();
        let group_name = conversation_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty())
            .unwrap_or("Zalo Group");
        let mut imported_count = 0;
        let mut updated_count = 0;
        let mut skipped_count = 0;

        for member in resolved_members.values() {
            let exact_uid_lead_id = if member.linked_lead_id.is_none() {
                exact_lead_id_for_zalo_uid_tx(&tx, &member.external_id)?
            } else {
                None
            };
            let target_lead_id = match member.linked_lead_id.or(exact_uid_lead_id) {
                Some(lead_id) => {
                    let exists = tx.query_row(
                        "SELECT EXISTS(SELECT 1 FROM leads WHERE id = ?1)",
                        [lead_id],
                        |row| row.get::<_, bool>(0),
                    )?;
                    if !exists {
                        return Err(invalid_group_member_import(format!(
                            "Identity {} links to missing lead {lead_id}",
                            member.identity_id
                        )));
                    }
                    if request.skip_existing {
                        skipped_count += 1;
                    } else {
                        tx.execute(
                            "UPDATE leads
                             SET last_activity = ?1,
                                 phone = CASE
                                     WHEN TRIM(COALESCE(phone, '')) = ''
                                     THEN COALESCE(?2, phone)
                                     ELSE phone
                                 END,
                                 has_zalo = 1,
                                 zalo_uid = COALESCE(NULLIF(TRIM(zalo_uid), ''), ?3),
                                 zalo_status = 'VERIFIED_EXIST'
                             WHERE id = ?4",
                            params![now, member.phone, member.external_id, lead_id],
                        )?;
                        updated_count += 1;
                    }
                    lead_id
                }
                None => {
                    let about = format!(
                        "Imported from Zalo group conversation {}",
                        request.conversation_id
                    );
                    tx.execute(
                        "INSERT INTO leads
                         (name, company, status, value, last_activity, owner, email, phone, about,
                          address, has_zalo, zalo_reached, zalo_uid, zalo_status,
                          relationship_status)
                         VALUES (?1, ?2, 'New', 0, ?3, 'System', '', ?4, ?5, '', 1, 0, ?6,
                                 'VERIFIED_EXIST', 'UNKNOWN')",
                        params![
                            member.display_name,
                            group_name,
                            now,
                            member.phone.as_deref().unwrap_or(""),
                            about,
                            member.external_id,
                        ],
                    )?;
                    imported_count += 1;
                    tx.last_insert_rowid()
                }
            };

            let linked = tx.execute(
                "UPDATE channel_identities
                 SET lead_id = ?1,
                     link_source = 'group_import',
                     link_confidence = 1.0,
                     verified_at = ?2,
                     updated_at = ?2
                 WHERE id = ?3 AND channel = 'zalo' AND identity_namespace = 'zalo_uid'",
                params![target_lead_id, now, member.identity_id],
            )?;
            if linked != 1 {
                return Err(invalid_group_member_import(format!(
                    "Canonical identity {} changed during import",
                    member.identity_id
                )));
            }

            for category_id in &category_ids {
                tx.execute(
                    "INSERT INTO lead_categories (lead_id, category_id) VALUES (?1, ?2)
                     ON CONFLICT(lead_id, category_id) DO NOTHING",
                    params![target_lead_id, category_id],
                )?;
            }
        }

        tx.commit()?;
        Ok(GroupMemberLeadImportResult {
            imported_count,
            updated_count,
            skipped_count,
            assigned_category_ids: category_ids.into_iter().collect(),
        })
    }

    pub fn list_group_member_external_ids(&self, conversation_id: i64) -> Result<Vec<String>> {
        let conn = open_db_connection(&self.db_path)?;
        let mut statement = conn.prepare(
            "SELECT external_id
             FROM (
                 SELECT ci.external_id AS external_id
                 FROM conversation_participants cp
                 JOIN channel_identities ci ON ci.id = cp.identity_id
                 WHERE cp.conversation_id = ?1 AND cp.left_at IS NULL

                 UNION

                 SELECT ci.external_id
                 FROM messages m
                 JOIN channel_identities ci ON ci.id = m.sender_identity_id
                 WHERE m.conversation_id = ?1 AND m.sender_identity_id IS NOT NULL

                 UNION

                 SELECT COALESCE(ci.external_id, mm.external_id)
                 FROM message_mentions mm
                 JOIN messages m ON m.id = mm.message_id
                 LEFT JOIN channel_identities ci ON ci.id = mm.identity_id
                 WHERE m.conversation_id = ?1

                 UNION

                 SELECT mr.actor_external_id
                 FROM message_reactions mr
                 JOIN messages m ON m.id = mr.message_id
                 WHERE m.conversation_id = ?1 AND mr.removed_at IS NULL

                 UNION

                 SELECT ci.external_id
                 FROM messages m
                 JOIN channel_identities ci ON ci.id = m.reply_sender_identity_id
                 WHERE m.conversation_id = ?1 AND m.reply_sender_identity_id IS NOT NULL
             )
             WHERE external_id IS NOT NULL AND TRIM(external_id) <> ''
             ORDER BY external_id",
        )?;
        let rows = statement.query_map([conversation_id], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn apply_normalized_message(&self, message: &NormalizedMessage) -> Result<AppliedMessage> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = &message.ingested_at;

        let channel_account_id = ensure_channel_account(
            &tx,
            message.channel.as_str(),
            &message.provider_account_key,
            message.account_display_name.as_deref(),
            "{}",
            now,
        )?;

        let identity_id = if let Some(identity) = &message.participant {
            if message.channel == Channel::Zalo {
                Some(canonicalize_zalo_uid_tx(
                    &tx,
                    &ZaloUidCandidate {
                        external_id: identity.external_id.clone(),
                        display_name: identity.display_name.clone(),
                        avatar_url: identity.avatar_url.clone(),
                    },
                    identity.lead_id,
                    now,
                )?)
            } else {
                Some(tx.query_row(
                    "INSERT INTO channel_identities
                     (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
                     ON CONFLICT(channel, identity_namespace, external_id) WHERE channel_account_id IS NULL DO UPDATE SET
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
            }
        } else {
            None
        };

        let reply_sender_identity_id = if message.channel == Channel::Zalo {
            match message
                .reply
                .as_ref()
                .map(|reply| reply.sender_external_id.trim())
                .filter(|external_id| !external_id.is_empty())
            {
                Some(external_id) => Some(canonicalize_zalo_uid_tx(
                    &tx,
                    &ZaloUidCandidate {
                        external_id: external_id.to_string(),
                        display_name: None,
                        avatar_url: None,
                    },
                    None,
                    now,
                )?),
                None => None,
            }
        } else {
            None
        };

        let tombstone_deleted_at: Option<String> = tx
            .query_row(
                "SELECT deleted_at FROM deleted_conversations
                 WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
                params![
                    channel_account_id,
                    message.external_thread_id,
                    message.conversation_kind.as_str()
                ],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(tombstone_at) = tombstone_deleted_at {
            if !message_occurred_after_tombstone(message, &tombstone_at) {
                return Ok(AppliedMessage {
                    channel_account_id,
                    identity_id,
                    conversation_id: 0,
                    message_id: 0,
                    duplicate: true,
                    replayed_reaction_count: 0,
                });
            } else {
                tx.execute(
                    "DELETE FROM deleted_conversations
                     WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = ?3",
                    params![
                        channel_account_id,
                        message.external_thread_id,
                        message.conversation_kind.as_str()
                    ],
                )?;
            }
        }

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
                     provider_client_message_id = COALESCE(?3, provider_client_message_id),
                     provider_timestamp = COALESCE(provider_timestamp, ?4),
                     raw_payload_json = COALESCE(?5, raw_payload_json),
                     reply_sender_identity_id = COALESCE(reply_sender_identity_id, ?6),
                     status = CASE WHEN status IN ('sending', 'queued', 'failed') THEN 'sent' ELSE status END
                 WHERE id = ?1",
                params![
                    message_id,
                    message.provider_message_id,
                    message.provider_client_message_id,
                    message.provider_timestamp,
                    message.raw_payload.as_ref().map(serde_json::Value::to_string),
                    reply_sender_identity_id,
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
                         metadata_json = CASE
                             WHEN ?9 IS NULL THEN metadata_json
                             WHEN metadata_json IS NULL THEN ?9
                             WHEN json_valid(metadata_json) AND json_valid(?9) AND json_type(?9) = 'object' THEN (
                                 SELECT json_patch(
                                     json_patch(metadata_json, ?9),
                                     json_object(
                                         'provider_attachment_id_aliases', json(COALESCE(json_extract(metadata_json, '$.provider_attachment_id_aliases'), 'null')),
                                         'remote_url_aliases', json(COALESCE(json_extract(metadata_json, '$.remote_url_aliases'), 'null')),
                                         '_merged_attachment_rows', json(COALESCE(json_extract(metadata_json, '$._merged_attachment_rows'), 'null'))
                                     )
                                 )
                             )
                             ELSE metadata_json
                         END
                     WHERE message_id = ?1
                       AND (
                           (?2 IS NOT NULL AND provider_attachment_id = ?2)
                           OR (?7 IS NOT NULL AND remote_url = ?7)
                           OR (?2 IS NOT NULL AND json_valid(metadata_json) AND EXISTS (
                               SELECT 1 FROM json_each(metadata_json, '$.provider_attachment_id_aliases') WHERE value = ?2
                           ))
                           OR (?7 IS NOT NULL AND json_valid(metadata_json) AND EXISTS (
                               SELECT 1 FROM json_each(metadata_json, '$.remote_url_aliases') WHERE value = ?7
                           ))
                           OR (provider_attachment_id IS NULL AND remote_url IS NULL AND attachment_type = ?3)
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

            let replayed_reaction_count =
                replay_pending_reactions_tx(&tx, channel_account_id, message_id, message)?;

            tx.commit()?;
            return Ok(AppliedMessage {
                channel_account_id,
                identity_id,
                conversation_id,
                message_id,
                duplicate: true,
                replayed_reaction_count,
            });
        }

        tx.execute(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id,
              provider_client_message_id, local_request_id, content_type, text_content,
              content_json, raw_payload_json, provider_timestamp, ingested_at, status,
              reply_sender_identity_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                message.status.as_str(),
                reply_sender_identity_id,
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

        if message.direction == MessageDirection::Inbound {
            tx.execute(
                "UPDATE messages
                 SET status = 'read'
                 WHERE conversation_id = ?1
                   AND direction = 'outbound'
                   AND status IN ('sent', 'delivered')",
                params![conversation_id],
            )?;
        } else if message.direction == MessageDirection::Outbound {
            tx.execute(
                "UPDATE messages
                 SET status = 'read'
                 WHERE conversation_id = ?1
                   AND direction = 'inbound'
                   AND status != 'read'",
                params![conversation_id],
            )?;
        }

        Self::save_message_mentions_tx(
            &tx,
            message_id,
            message.channel.as_str(),
            &message.mentions,
            now,
        )?;

        let replayed_reaction_count =
            replay_pending_reactions_tx(&tx, channel_account_id, message_id, message)?;

        tx.commit()?;

        Ok(AppliedMessage {
            channel_account_id,
            identity_id,
            conversation_id,
            message_id,
            duplicate: false,
            replayed_reaction_count,
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
                "SELECT ca.channel, COUNT(m.id),
                        SUM(CASE WHEN m.direction = 'inbound' AND m.status != 'read'
                                      AND (m.ingested_at > COALESCE((SELECT MAX(m2.ingested_at) FROM messages m2 WHERE m2.conversation_id = cc.id AND m2.direction = 'outbound'), ''))
                                 THEN 1 ELSE 0 END)
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
                    let unread: i64 = row.get::<_, Option<i64>>(2)?.unwrap_or(0);
                    Ok(ChannelBadgeSummary {
                        channel,
                        message_count: count as u64,
                        unread_count: unread as u64,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            let unread_count: u64 = channels.iter().map(|c| c.unread_count).sum();

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
                unread_count,
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
                              AND m.sender_identity_id = ci.id
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
                "SELECT ca.channel, COUNT(m.id),
                        SUM(CASE WHEN m.direction = 'inbound' AND m.status != 'read'
                                      AND (m.ingested_at > COALESCE((SELECT MAX(m2.ingested_at) FROM messages m2 WHERE m2.conversation_id = cc.id AND m2.direction = 'outbound'), ''))
                                 THEN 1 ELSE 0 END)
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
                    let unread: i64 = row.get::<_, Option<i64>>(2)?.unwrap_or(0);
                    Ok(ChannelBadgeSummary {
                        channel,
                        message_count: count as u64,
                        unread_count: unread as u64,
                    })
                })?
                .collect::<Result<Vec<_>>>()?;

            let unread_count: u64 = channels.iter().map(|c| c.unread_count).sum();

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
                unread_count,
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

            let (msg_count, unread_cnt): (i64, i64) = conn.query_row(
                "SELECT COUNT(*), COALESCE(SUM(CASE WHEN direction = 'inbound' AND status != 'read'
                                                         AND (ingested_at > COALESCE((SELECT MAX(m2.ingested_at) FROM messages m2 WHERE m2.conversation_id = ?1 AND m2.direction = 'outbound'), ''))
                                                    THEN 1 ELSE 0 END), 0)
                 FROM messages WHERE conversation_id = ?1",
                [conv_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;

            let channels = vec![ChannelBadgeSummary {
                channel,
                message_count: msg_count as u64,
                unread_count: unread_cnt as u64,
            }];

            let participant_count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = ?1 AND left_at IS NULL",
                [conv_id],
                |row| row.get(0),
            )?;

            let mut part_stmt = conn.prepare(
                "SELECT cp.identity_id, ci.external_id, ci.display_name, ci.avatar_url, cp.participant_role
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
                        external_id: row.get(1)?,
                        display_name: row.get(2)?,
                        avatar_url: row.get(3)?,
                        participant_role: row.get(4)?,
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
                unread_count: unread_cnt as u64,
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

    pub fn mark_conversation_read(&self, key: &str) -> Result<u64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let rows_affected = if let Some(lead_id_str) = key.strip_prefix("lead:") {
            let lead_id: i64 = lead_id_str
                .parse()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            tx.execute(
                "UPDATE messages
                 SET status = 'read'
                 WHERE direction = 'inbound' AND status != 'read'
                   AND conversation_id IN (
                       SELECT cc.id
                       FROM channel_conversations cc
                       JOIN conversation_participants cp ON cp.conversation_id = cc.id
                       JOIN channel_identities ci ON ci.id = cp.identity_id
                       WHERE ci.lead_id = ?1
                   )",
                params![lead_id],
            )?
        } else if let Some(identity_id_str) = key.strip_prefix("identity:") {
            let identity_id: i64 = identity_id_str
                .parse()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            tx.execute(
                "UPDATE messages
                 SET status = 'read'
                 WHERE direction = 'inbound' AND status != 'read'
                   AND conversation_id IN (
                       SELECT cc.id
                       FROM channel_conversations cc
                       JOIN conversation_participants cp ON cp.conversation_id = cc.id
                       WHERE cp.identity_id = ?1
                   )",
                params![identity_id],
            )?
        } else if let Some(conv_id_str) = key.strip_prefix("conversation:") {
            let conv_id: i64 = conv_id_str
                .parse()
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            tx.execute(
                "UPDATE messages
                 SET status = 'read'
                 WHERE conversation_id = ?1 AND direction = 'inbound' AND status != 'read'",
                params![conv_id],
            )?
        } else {
            0
        };

        tx.commit()?;
        Ok(rows_affected as u64)
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
                    m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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
        let mut page = page_from_descending_rows(rows, clamped_limit);
        crate::conversation::projection::hydrate_messages(&conn, &mut page.items)?;
        Ok(page)
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
                    m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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
                    m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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

        let channel_account_id =
            find_channel_account_id_by_key(&tx, "zalo", &reaction.provider_account_key)?
                .ok_or(rusqlite::Error::QueryReturnedNoRows)?;

        let message_id = match resolve_message_id(
            &tx,
            &reaction.provider_account_key,
            &reaction.external_thread_id,
            reaction.is_group,
            &reaction.provider_message_id,
            &reaction.provider_client_message_id,
        )? {
            Some(id) => id,
            None => {
                stage_pending_reaction_tx(&tx, channel_account_id, reaction)?;
                tx.commit()?;
                return Ok(None);
            }
        };

        apply_reaction_to_message_tx(
            &tx,
            message_id,
            &reaction.actor_external_id,
            &reaction.reaction,
            reaction.removed,
            &reaction.occurred_at,
        )?;

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
        let channel_account_id = ensure_channel_account(
            &tx,
            "zalo",
            &snapshot.provider_account_key,
            None,
            &default_caps,
            now,
        )?;

        let tombstone_deleted_at: Option<String> = tx
            .query_row(
                "SELECT deleted_at FROM deleted_conversations
                 WHERE channel_account_id = ?1 AND external_thread_id = ?2 AND kind = 'group'",
                params![channel_account_id, snapshot.external_thread_id],
                |row| row.get(0),
            )
            .optional()?;

        if tombstone_deleted_at.is_some() {
            return Ok(0);
        }

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
            let identity_id = canonicalize_zalo_uid_tx(
                &tx,
                &ZaloUidCandidate {
                    external_id: participant.identity.external_id.clone(),
                    display_name: participant.identity.display_name.clone(),
                    avatar_url: participant.identity.avatar_url.clone(),
                },
                participant.identity.lead_id,
                now,
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

        let account_id_opt = find_channel_account_id_by_key(&conn, "zalo", provider_account_key)?;

        let id: i64 = if let Some(acc_id) = account_id_opt {
            conn.execute(
                "UPDATE channel_accounts
                 SET connection_status = ?2,
                     last_error = ?3,
                     last_connected_at = COALESCE(?4, last_connected_at),
                     updated_at = ?5
                 WHERE id = ?1",
                params![acc_id, status_str, error, last_connected, now],
            )?;
            acc_id
        } else {
            conn.query_row(
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
            )?
        };

        link_zalo_sessions_to_account(&conn, provider_account_key, id)?;

        Ok(id)
    }

    pub fn reconcile_zalo_channel_account(
        &self,
        imei: &str,
        zalo_uid: Option<&str>,
        display_name: Option<&str>,
        avatar_url: Option<&str>,
    ) -> Result<i64> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let now = utc_now();

        let target_key = zalo_uid.unwrap_or(imei);

        let existing_id = find_channel_account_id_by_key(&tx, "zalo", target_key)?
            .or(find_channel_account_id_by_key(&tx, "zalo", imei)?);

        let acc_id = if let Some(acc_id) = existing_id {
            tx.execute(
                "UPDATE channel_accounts
                 SET provider_account_key = ?2,
                     self_external_id = COALESCE(?3, self_external_id),
                     display_name = COALESCE(?4, display_name),
                     avatar_url = COALESCE(?5, avatar_url),
                     updated_at = ?6
                 WHERE id = ?1",
                params![acc_id, target_key, zalo_uid, display_name, avatar_url, now],
            )?;
            acc_id
        } else {
            let default_caps = serde_json::to_string(
                &crate::conversation::types::ConversationCapabilities::zalo_phase_one(),
            )
            .unwrap_or_else(|_| "{}".to_string());

            tx.query_row(
                "INSERT INTO channel_accounts
                 (channel, provider_account_key, self_external_id, display_name, avatar_url, connection_status, capabilities_json, created_at, updated_at)
                 VALUES ('zalo', ?1, ?2, ?3, ?4, 'disconnected', ?5, ?6, ?6)
                 ON CONFLICT(channel, provider_account_key) DO UPDATE SET
                     self_external_id = COALESCE(excluded.self_external_id, channel_accounts.self_external_id),
                     display_name = COALESCE(excluded.display_name, channel_accounts.display_name),
                     avatar_url = COALESCE(excluded.avatar_url, channel_accounts.avatar_url),
                     updated_at = excluded.updated_at
                 RETURNING id",
                params![target_key, zalo_uid, display_name, avatar_url, default_caps, now],
                |row| row.get(0),
            )?
        };

        tx.execute(
            "UPDATE zalo_sessions
             SET zalo_uid = COALESCE(?2, zalo_uid), channel_account_id = ?3
             WHERE imei = ?1 OR (?2 IS NOT NULL AND zalo_uid = ?2)",
            params![imei, zalo_uid, acc_id],
        )?;

        if let Some(uid) = zalo_uid.filter(|u| !u.trim().is_empty()) {
            tx.execute(
                "DELETE FROM zalo_sessions
                 WHERE TRIM(zalo_uid) = ?1 AND imei != ?2",
                params![uid.trim(), imei],
            )?;
        }
        tx.commit()?;

        Ok(acc_id)
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
                    m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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
                    m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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
                    let display_name: Option<String> = row.get(2)?;
                    let is_active_int: i32 = row.get(3)?;
                    Ok(crate::conversation::mentions::MentionParticipant {
                        identity_id,
                        external_id,
                        display_name: display_name.unwrap_or_default(),
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
                        m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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
                    url,
                } => (
                    "sticker",
                    None,
                    serde_json::json!({
                        "stickerId": sticker_id,
                        "cateId": cate_id,
                        "type": sticker_type,
                        "url": url,
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
                url,
            } = &part.content
            {
                let meta = serde_json::json!({
                    "stickerId": sticker_id,
                    "cateId": cate_id,
                    "type": sticker_type,
                    "url": url,
                })
                .to_string();

                tx.execute(
                    "INSERT INTO message_attachments
                     (message_id, provider_attachment_id, attachment_type, file_name, mime_type, size_bytes, local_path, remote_url, thumbnail_url, metadata_json, created_at)
                     VALUES (?1, NULL, 'sticker', NULL, NULL, NULL, NULL, ?2, NULL, ?3, ?4)",
                    params![message_id, url, meta, now],
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
                        m.provider_client_message_id, m.batch_index, ci.external_id AS sender_external_id
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

    fn collect_conversation_files(
        tx: &rusqlite::Transaction,
        conv_ids: &[i64],
        pending_files: &mut Vec<String>,
        now: &str,
    ) -> Result<()> {
        if conv_ids.is_empty() {
            return Ok(());
        }
        for &cid in conv_ids {
            if cid <= 0 {
                continue;
            }
            let mut stmt = tx.prepare(
                "SELECT ma.local_path FROM message_attachments ma
                 JOIN messages m ON m.id = ma.message_id
                 WHERE m.conversation_id = ?1 AND ma.local_path IS NOT NULL AND TRIM(ma.local_path) != ''",
            )?;
            let paths = stmt.query_map(params![cid], |row| row.get::<_, String>(0))?;
            for path_result in paths {
                let path = path_result?;
                tx.execute(
                    "INSERT OR IGNORE INTO pending_spool_cleanups (local_path, created_at) VALUES (?1, ?2)",
                    params![&path, now],
                )?;
                pending_files.push(path);
            }

            let mut stmt = tx.prepare(
                "SELECT local_path, thumbnail_path FROM conversation_staged_attachments
                 WHERE conversation_id = ?1",
            )?;
            let rows = stmt.query_map(params![cid], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            })?;
            for row_result in rows {
                let (local_path, thumbnail_path) = row_result?;
                for path in [local_path, thumbnail_path].into_iter().flatten() {
                    if path.trim().is_empty() {
                        continue;
                    }
                    tx.execute(
                        "INSERT OR IGNORE INTO pending_spool_cleanups (local_path, created_at) VALUES (?1, ?2)",
                        params![&path, now],
                    )?;
                    pending_files.push(path);
                }
            }
        }
        Ok(())
    }

    fn delete_single_conv_tx(
        tx: &rusqlite::Transaction,
        cid: i64,
        pending_files: &mut Vec<String>,
        now: &str,
    ) -> Result<()> {
        if cid <= 0 {
            return Ok(());
        }

        Self::collect_conversation_files(tx, &[cid], pending_files, now)?;

        tx.execute(
            "INSERT INTO deleted_conversations (channel_account_id, external_thread_id, kind, deleted_at)
             SELECT channel_account_id, external_thread_id, kind, ?2
             FROM channel_conversations WHERE id = ?1
             ON CONFLICT(channel_account_id, external_thread_id, kind) DO UPDATE SET deleted_at = excluded.deleted_at",
            params![cid, now],
        )?;

        tx.execute(
            "DELETE FROM message_attachments WHERE message_id IN (SELECT id FROM messages WHERE conversation_id = ?1)",
            params![cid],
        )?;
        tx.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            params![cid],
        )?;
        tx.execute(
            "DELETE FROM conversation_participants WHERE conversation_id = ?1",
            params![cid],
        )?;
        tx.execute(
            "DELETE FROM conversation_staged_attachments WHERE conversation_id = ?1",
            params![cid],
        )?;
        tx.execute(
            "DELETE FROM channel_conversations WHERE id = ?1",
            params![cid],
        )?;

        Ok(())
    }

    fn delete_conversation_tx(
        tx: &rusqlite::Transaction,
        request: &crate::conversation::types::DeleteConversationRequest,
        pending_files: &mut Vec<String>,
    ) -> Result<()> {
        request.validate()?;
        let now = chrono::Utc::now().to_rfc3339();

        // Priority 1: Specific conversation_id provided
        if let Some(conv_id) = request.conversation_id {
            if conv_id > 0 {
                Self::delete_single_conv_tx(tx, conv_id, pending_files, &now)?;
                return Ok(());
            }
        }

        // Priority 2: Unlinked contact identity_id provided (without specific conversation_id)
        if let Some(identity_id) = request.identity_id {
            if identity_id > 0 {
                let conv_ids: Vec<i64> = {
                    let mut stmt = tx.prepare(
                        "SELECT DISTINCT cc.id FROM channel_conversations cc
                         JOIN conversation_participants cp ON cp.conversation_id = cc.id
                         WHERE cp.identity_id = ?1 AND cc.kind IN ('direct', 'email_thread')",
                    )?;
                    let rows = stmt.query_map(params![identity_id], |row| row.get(0))?;
                    rows.filter_map(Result::ok).collect()
                };

                for cid in conv_ids {
                    Self::delete_single_conv_tx(tx, cid, pending_files, &now)?;
                }

                let remaining_memberships: i64 = tx.query_row(
                    "SELECT COUNT(*) FROM conversation_participants WHERE identity_id = ?1",
                    params![identity_id],
                    |row| row.get(0),
                )?;
                if remaining_memberships == 0 {
                    tx.execute(
                        "DELETE FROM channel_identities WHERE id = ?1 AND lead_id IS NULL",
                        params![identity_id],
                    )?;
                }

                return Ok(());
            }
        }

        // Priority 3: Lead lead_id provided (without specific conversation_id)
        if let Some(lead_id) = request.lead_id {
            if lead_id > 0 {
                let conv_ids: Vec<i64> = {
                    let mut stmt = tx.prepare(
                        "SELECT DISTINCT cc.id FROM channel_conversations cc
                         JOIN conversation_participants cp ON cp.conversation_id = cc.id
                         JOIN channel_identities ci ON ci.id = cp.identity_id
                         WHERE ci.lead_id = ?1 AND cc.kind IN ('direct', 'email_thread')",
                    )?;
                    let rows = stmt.query_map(params![lead_id], |row| row.get(0))?;
                    rows.filter_map(Result::ok).collect()
                };

                for cid in conv_ids {
                    Self::delete_single_conv_tx(tx, cid, pending_files, &now)?;
                }

                return Ok(());
            }
        }

        Ok(())
    }

    pub fn cleanup_spool_files(&self, file_paths: &[String], spool_root: Option<&std::path::Path>) {
        let conn = match open_db_connection(&self.db_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to open DB connection for spool cleanup check: {e}");
                return;
            }
        };

        let canonical_spool = match spool_root {
            Some(root) => match root.canonicalize() {
                Ok(root) => root,
                Err(error) => {
                    tracing::warn!(
                        spool_root = %root.display(),
                        %error,
                        "Skipping spool cleanup because the spool root cannot be canonicalized"
                    );
                    return;
                }
            },
            None => {
                tracing::warn!("Skipping spool cleanup because no spool root was provided");
                return;
            }
        };

        let active_canonical_set: std::collections::HashSet<std::path::PathBuf> = {
            let mut set = std::collections::HashSet::new();
            let mut stmt = match conn.prepare(
                "SELECT local_path FROM message_attachments WHERE local_path IS NOT NULL AND TRIM(local_path) != ''
                 UNION
                 SELECT local_path FROM conversation_staged_attachments WHERE local_path IS NOT NULL AND TRIM(local_path) != ''
                 UNION
                 SELECT thumbnail_path FROM conversation_staged_attachments WHERE thumbnail_path IS NOT NULL AND TRIM(thumbnail_path) != ''",
            ) {
                Ok(stmt) => stmt,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "Skipping spool cleanup because active attachment references could not be queried"
                    );
                    return;
                }
            };
            let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
                Ok(rows) => rows,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "Skipping spool cleanup because active attachment references could not be read"
                    );
                    return;
                }
            };
            for path_result in rows {
                let path = match path_result {
                    Ok(path) => path,
                    Err(error) => {
                        tracing::warn!(
                            %error,
                            "Skipping spool cleanup because an active attachment path was invalid"
                        );
                        return;
                    }
                };
                let path = std::path::Path::new(&path);
                let canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
                set.insert(canonical_path);
            }
            set
        };

        let mut targets: Vec<String> = file_paths.to_vec();

        let mut stmt = match conn.prepare("SELECT local_path FROM pending_spool_cleanups") {
            Ok(stmt) => stmt,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "Skipping spool cleanup because the durable cleanup queue could not be queried"
                );
                return;
            }
        };
        let rows = match stmt.query_map([], |row| row.get::<_, String>(0)) {
            Ok(rows) => rows,
            Err(error) => {
                tracing::warn!(
                    %error,
                    "Skipping spool cleanup because the durable cleanup queue could not be read"
                );
                return;
            }
        };
        for path_result in rows {
            let path = match path_result {
                Ok(path) => path,
                Err(error) => {
                    tracing::warn!(
                        %error,
                        "Skipping spool cleanup because a queued cleanup path was invalid"
                    );
                    return;
                }
            };
            if !targets.contains(&path) {
                targets.push(path);
            }
        }

        for raw_path in &targets {
            let path_str = raw_path.trim();
            if path_str.is_empty() {
                continue;
            }

            let path = std::path::Path::new(path_str);
            let canonical_file = match path.canonicalize() {
                Ok(path) => path,
                Err(error) => match std::fs::metadata(path) {
                    Err(metadata_error)
                        if metadata_error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        let _ = conn.execute(
                            "DELETE FROM pending_spool_cleanups WHERE local_path = ?1",
                            params![path_str],
                        );
                        continue;
                    }
                    _ => {
                        tracing::warn!(
                            path = path_str,
                            %error,
                            "Keeping queued spool cleanup because the target cannot be canonicalized"
                        );
                        continue;
                    }
                },
            };

            if !canonical_file.starts_with(&canonical_spool) {
                tracing::warn!(
                    "Skipping file cleanup outside spool root: {path_str} (canonical: {}, spool_root: {})",
                    canonical_file.display(),
                    canonical_spool.display()
                );
                let _ = conn.execute(
                    "DELETE FROM pending_spool_cleanups WHERE local_path = ?1",
                    params![path_str],
                );
                continue;
            }

            if active_canonical_set.contains(&canonical_file) {
                tracing::info!(
                    "Skipping file cleanup for shared file still referenced in DB: {path_str}"
                );
                let _ = conn.execute(
                    "DELETE FROM pending_spool_cleanups WHERE local_path = ?1",
                    params![path_str],
                );
                continue;
            }

            match std::fs::remove_file(&canonical_file) {
                Ok(_) => {
                    let _ = conn.execute(
                        "DELETE FROM pending_spool_cleanups WHERE local_path = ?1",
                        params![path_str],
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to remove spool file {path_str}: {e}");
                }
            }
        }
    }

    pub fn delete_conversation(
        &self,
        request: &crate::conversation::types::DeleteConversationRequest,
        spool_root: Option<&std::path::Path>,
    ) -> Result<()> {
        request.validate()?;
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let mut pending_files = Vec::new();
        Self::delete_conversation_tx(&tx, request, &mut pending_files)?;
        tx.commit()?;
        self.cleanup_spool_files(&pending_files, spool_root);
        Ok(())
    }

    pub fn delete_conversations_batch(
        &self,
        requests: &[crate::conversation::types::DeleteConversationRequest],
        spool_root: Option<&std::path::Path>,
    ) -> Result<()> {
        for req in requests {
            req.validate()?;
        }
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;
        let mut pending_files = Vec::new();
        for request in requests {
            Self::delete_conversation_tx(&tx, request, &mut pending_files)?;
        }
        tx.commit()?;
        self.cleanup_spool_files(&pending_files, spool_root);
        Ok(())
    }

    pub fn start_direct_conversation(
        &self,
        request: &crate::conversation::types::StartDirectConversationRequest,
        now: &str,
    ) -> Result<ConversationMessage> {
        let mut conn = open_db_connection(&self.db_path)?;
        let tx = conn.transaction()?;

        let mut phone_clean_buf = request.phone_or_external_id.trim().to_string();
        if let Ok(identity_id) = phone_clean_buf.parse::<i64>() {
            if let Ok(ext_id) = tx.query_row(
                "SELECT external_id FROM channel_identities WHERE id = ?1",
                [identity_id],
                |row| row.get::<_, String>(0),
            ) {
                if !ext_id.trim().is_empty() {
                    phone_clean_buf = ext_id.trim().to_string();
                }
            }
        } else if !phone_clean_buf.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(ext_id) = tx.query_row(
                "SELECT external_id FROM channel_identities WHERE lower(trim(display_name)) = lower(trim(?1)) AND external_id IS NOT NULL AND trim(external_id) <> '' LIMIT 1",
                [&phone_clean_buf],
                |row| row.get::<_, String>(0),
            ) {
                if !ext_id.trim().is_empty() {
                    phone_clean_buf = ext_id.trim().to_string();
                }
            }
        }
        let phone_clean = phone_clean_buf.as_str();
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

        tx.execute(
            "DELETE FROM deleted_conversations WHERE channel_account_id = ?1 AND (external_thread_id = ?2 OR external_thread_id = ?3 OR external_thread_id IN (SELECT external_thread_id FROM channel_conversations WHERE id = ?4))",
            params![request.channel_account_id, phone_clean, normalized_phone, conversation_id],
        )?;

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
             ON CONFLICT(channel, identity_namespace, external_id) WHERE channel_account_id IS NULL DO UPDATE SET
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

        tx.execute(
            "DELETE FROM deleted_conversations
             WHERE channel_account_id = ?1
               AND external_thread_id = ?2
               AND kind = 'direct'",
            params![channel_account_id, resolved_uid],
        )?;

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
             SET status = 'sent', provider_message_id = ?2, provider_client_message_id = CASE WHEN ?3 != '' THEN ?3 ELSE provider_client_message_id END, sent_at = ?4
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
        source_message_id: Option<i64>,
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

        if is_group {
            if let Some(source_message_id) = source_message_id {
                let reply_sender = conn
                    .query_row(
                        "SELECT ci.id, ci.external_id, ci.display_name, ci.avatar_url,
                                ci.normalized_address, l.phone
                         FROM messages m
                         JOIN channel_identities ci ON ci.id = m.reply_sender_identity_id
                         LEFT JOIN leads l ON l.id = ci.lead_id
                         WHERE m.id = ?1 AND m.conversation_id = ?2",
                        rusqlite::params![source_message_id, conversation_id],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, Option<String>>(3)?,
                                row.get::<_, Option<String>>(4)?,
                                row.get::<_, Option<String>>(5)?,
                            ))
                        },
                    )
                    .optional()?;

                if let Some((id, ext_id, name, avatar, norm_addr, phone)) = reply_sender {
                    let name_str = name.unwrap_or_else(|| format!("User {}", id));
                    let name_lower = name_str.to_lowercase();
                    let phone_lower = phone.as_deref().unwrap_or("").to_lowercase();
                    let addr_lower = norm_addr.as_deref().unwrap_or("").to_lowercase();
                    let matches = query_clean.is_empty()
                        || name_lower.contains(&query_clean)
                        || (!phone_lower.is_empty() && phone_lower.contains(&query_clean))
                        || (!addr_lower.is_empty() && addr_lower.contains(&query_clean));
                    let already_present = candidates
                        .iter()
                        .any(|candidate| candidate.identity_id == Some(id));

                    if matches && !already_present {
                        candidates.push(crate::conversation::types::ConversationMentionCandidate {
                            identity_id: Some(id),
                            external_id: ext_id,
                            display_name: name_str,
                            avatar_url: avatar,
                            is_all: false,
                        });
                    }
                }
            }
        }

        if let Some(lim) = limit {
            candidates.truncate(lim);
        }

        Ok(candidates)
    }

    pub fn pin_message(
        &self,
        conversation_id: i64,
        message_id: i64,
        pinned_by: Option<&str>,
    ) -> Result<crate::conversation::types::PinnedMessage> {
        let conn = open_db_connection(&self.db_path)?;
        let now = crate::conversation::types::utc_now();

        conn.execute(
            "INSERT OR REPLACE INTO conversation_pinned_messages (conversation_id, message_id, pinned_by, pinned_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![conversation_id, message_id, pinned_by, now],
        )?;

        let id = conn.last_insert_rowid();

        let (text_content, sender_name): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT m.text_content, ci.display_name
                 FROM messages m
                 LEFT JOIN channel_identities ci ON m.sender_identity_id = ci.id
                 WHERE m.id = ?1",
                rusqlite::params![message_id],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .unwrap_or((None, None));

        Ok(crate::conversation::types::PinnedMessage {
            id,
            conversation_id,
            message_id,
            pinned_by: pinned_by.map(String::from),
            pinned_at: now,
            message_text: text_content,
            sender_name,
        })
    }

    pub fn unpin_message(&self, conversation_id: i64, message_id: i64) -> Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        conn.execute(
            "DELETE FROM conversation_pinned_messages WHERE conversation_id = ?1 AND message_id = ?2",
            rusqlite::params![conversation_id, message_id],
        )?;
        Ok(())
    }

    pub fn list_pinned_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<crate::conversation::types::PinnedMessage>> {
        let conn = open_db_connection(&self.db_path)?;
        let mut stmt = conn.prepare(
            "SELECT cpm.id, cpm.conversation_id, cpm.message_id, cpm.pinned_by, cpm.pinned_at,
                    m.text_content, ci.display_name
             FROM conversation_pinned_messages cpm
             JOIN messages m ON cpm.message_id = m.id
             LEFT JOIN channel_identities ci ON m.sender_identity_id = ci.id
             WHERE cpm.conversation_id = ?1
             ORDER BY cpm.pinned_at DESC",
        )?;

        let rows = stmt.query_map(rusqlite::params![conversation_id], |row| {
            Ok(crate::conversation::types::PinnedMessage {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                message_id: row.get(2)?,
                pinned_by: row.get(3)?,
                pinned_at: row.get(4)?,
                message_text: row.get(5)?,
                sender_name: row.get(6)?,
            })
        })?;

        let mut pinned = Vec::new();
        for r in rows {
            pinned.push(r?);
        }
        Ok(pinned)
    }
}

fn invalid_group_member_import(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

fn resolve_group_member_import_tx(
    tx: &rusqlite::Transaction<'_>,
    conversation_id: i64,
    item: &GroupMemberLeadImportItem,
) -> Result<ResolvedGroupMemberImport> {
    type IdentityRow = (i64, String, Option<String>, Option<i64>);

    let requested_external_id = item
        .external_id
        .as_deref()
        .map(validated_zalo_uid)
        .transpose()?;
    let identity_by_id = match item.identity_id {
        Some(identity_id) => Some(
            tx.query_row(
                "SELECT id, external_id, display_name, lead_id
                 FROM channel_identities
                 WHERE id = ?1 AND channel = 'zalo' AND identity_namespace = 'zalo_uid'",
                [identity_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| {
                invalid_group_member_import(format!(
                    "Canonical Zalo identity {identity_id} was not found"
                ))
            })?,
        ),
        None => None,
    };
    let identity_by_external_id = match requested_external_id.as_deref() {
        Some(external_id) => Some(
            tx.query_row(
                "SELECT id, external_id, display_name, lead_id
                 FROM channel_identities
                 WHERE channel = 'zalo' AND identity_namespace = 'zalo_uid' AND external_id = ?1",
                [external_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| {
                invalid_group_member_import(format!(
                    "Canonical Zalo identity {external_id:?} was not found"
                ))
            })?,
        ),
        None => None,
    };

    let identity: IdentityRow = match (identity_by_id, identity_by_external_id) {
        (Some(by_id), Some(by_external_id)) if by_id.0 != by_external_id.0 => {
            return Err(invalid_group_member_import(format!(
                "identity_id {} conflicts with external_id {:?}",
                by_id.0, by_external_id.1
            )));
        }
        (Some(by_id), Some(_)) | (Some(by_id), None) => by_id,
        (None, Some(by_external_id)) => by_external_id,
        (None, None) => {
            return Err(invalid_group_member_import(
                "Each import item requires identity_id or external_id",
            ));
        }
    };

    let is_evidenced = tx.query_row(
        "SELECT EXISTS(
             SELECT 1
             FROM conversation_participants cp
             WHERE cp.conversation_id = ?1 AND cp.identity_id = ?2 AND cp.left_at IS NULL

             UNION ALL

             SELECT 1
             FROM messages m
             WHERE m.conversation_id = ?1 AND m.sender_identity_id = ?2

             UNION ALL

             SELECT 1
             FROM message_mentions mm
             JOIN messages m ON m.id = mm.message_id
             WHERE m.conversation_id = ?1
               AND (mm.identity_id = ?2 OR mm.external_id = ?3)

             UNION ALL

             SELECT 1
             FROM message_reactions mr
             JOIN messages m ON m.id = mr.message_id
             WHERE m.conversation_id = ?1
               AND mr.actor_external_id = ?3
               AND mr.removed_at IS NULL

             UNION ALL

             SELECT 1
             FROM messages m
             WHERE m.conversation_id = ?1 AND m.reply_sender_identity_id = ?2
         )",
        params![conversation_id, identity.0, identity.1],
        |row| row.get::<_, bool>(0),
    )?;
    if !is_evidenced {
        return Err(invalid_group_member_import(format!(
            "Identity {} is not evidenced by conversation {conversation_id}",
            identity.0
        )));
    }

    let phone = item
        .phone
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if phone.as_ref().is_some_and(|value| value.len() > 64) {
        return Err(invalid_group_member_import(format!(
            "Phone value for identity {} is too long",
            identity.0
        )));
    }

    Ok(ResolvedGroupMemberImport {
        identity_id: identity.0,
        external_id: identity.1,
        display_name: non_empty(identity.2.as_deref()).unwrap_or_else(|| "Zalo Member".to_string()),
        linked_lead_id: identity.3,
        phone,
    })
}

fn exact_lead_id_for_zalo_uid_tx(
    tx: &rusqlite::Transaction<'_>,
    external_id: &str,
) -> Result<Option<i64>> {
    let mut statement = tx.prepare(
        "SELECT id
         FROM leads
         WHERE TRIM(COALESCE(zalo_uid, '')) = ?1
         ORDER BY id
         LIMIT 2",
    )?;
    let ids = statement
        .query_map([external_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>>>()?;
    if ids.len() > 1 {
        return Err(invalid_group_member_import(format!(
            "Multiple leads use exact Zalo UID {external_id:?}"
        )));
    }
    Ok(ids.into_iter().next())
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn validated_zalo_uid(value: &str) -> Result<String> {
    let normalized = value.trim();
    let lowercase = normalized.to_ascii_lowercase();
    if normalized.is_empty() || matches!(lowercase.as_str(), "0" | "-1" | "null" | "undefined") {
        return Err(rusqlite::Error::InvalidParameterName(format!(
            "invalid Zalo UID {value:?}"
        )));
    }
    Ok(normalized.to_string())
}

fn canonicalize_zalo_uid_tx(
    tx: &rusqlite::Transaction<'_>,
    candidate: &ZaloUidCandidate,
    preferred_lead_id: Option<i64>,
    now: &str,
) -> Result<i64> {
    let external_id = validated_zalo_uid(&candidate.external_id)?;
    let existing = {
        let mut statement = tx.prepare(
            "SELECT id, identity_namespace, lead_id, normalized_address, display_name, avatar_url,
                    profile_synced_at, profile_sync_error, profile_sync_attempts,
                    next_profile_retry_at, link_source, link_confidence, verified_at, created_at
             FROM channel_identities
             WHERE channel = 'zalo' AND external_id = ?1
             ORDER BY
                 CASE identity_namespace WHEN 'zalo_uid' THEN 0 ELSE 1 END,
                 CASE WHEN lead_id IS NOT NULL THEN 0 ELSE 1 END,
                 COALESCE(link_confidence, 0) DESC,
                 id",
        )?;
        let rows = statement.query_map([&external_id], |row| {
            Ok(ExistingZaloIdentity {
                id: row.get(0)?,
                identity_namespace: row.get(1)?,
                lead_id: row.get(2)?,
                normalized_address: row.get(3)?,
                display_name: row.get(4)?,
                avatar_url: row.get(5)?,
                profile_synced_at: row.get(6)?,
                profile_sync_error: row.get(7)?,
                profile_sync_attempts: row.get(8)?,
                next_profile_retry_at: row.get(9)?,
                link_source: row.get(10)?,
                link_confidence: row.get(11)?,
                verified_at: row.get(12)?,
                created_at: row.get(13)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()?
    };

    let canonical = existing
        .iter()
        .find(|identity| identity.identity_namespace == "zalo_uid");
    let linked = if canonical.is_some_and(|identity| identity.lead_id.is_some()) {
        canonical
    } else {
        existing.iter().find(|identity| identity.lead_id.is_some())
    };
    let lead_id = canonical
        .and_then(|identity| identity.lead_id)
        .or_else(|| linked.and_then(|identity| identity.lead_id))
        .or(preferred_lead_id);
    let display_name = non_empty(candidate.display_name.as_deref())
        .or_else(|| canonical.and_then(|identity| non_empty(identity.display_name.as_deref())))
        .or_else(|| {
            existing
                .iter()
                .find_map(|identity| non_empty(identity.display_name.as_deref()))
        });
    let avatar_url = non_empty(candidate.avatar_url.as_deref())
        .or_else(|| canonical.and_then(|identity| non_empty(identity.avatar_url.as_deref())))
        .or_else(|| {
            existing
                .iter()
                .find_map(|identity| non_empty(identity.avatar_url.as_deref()))
        });
    let normalized_address = canonical
        .and_then(|identity| identity.normalized_address.clone())
        .or_else(|| {
            existing
                .iter()
                .find_map(|identity| identity.normalized_address.clone())
        });
    let profile_synced_at = existing
        .iter()
        .filter_map(|identity| identity.profile_synced_at.clone())
        .max();
    let profile_sync_error = canonical
        .and_then(|identity| identity.profile_sync_error.clone())
        .or_else(|| {
            existing
                .iter()
                .find_map(|identity| identity.profile_sync_error.clone())
        });
    let profile_sync_attempts = existing
        .iter()
        .map(|identity| identity.profile_sync_attempts)
        .max()
        .unwrap_or(0);
    let next_profile_retry_at = existing
        .iter()
        .filter_map(|identity| identity.next_profile_retry_at.clone())
        .max();
    let link_source = if lead_id.is_some() {
        linked
            .map(|identity| identity.link_source.clone())
            .or_else(|| canonical.map(|identity| identity.link_source.clone()))
            .unwrap_or_else(|| "provider_match".to_string())
    } else {
        "unlinked".to_string()
    };
    let link_confidence = if lead_id.is_some() {
        linked
            .and_then(|identity| identity.link_confidence)
            .or_else(|| canonical.and_then(|identity| identity.link_confidence))
            .or(Some(0.8))
    } else {
        Some(0.0)
    };
    let verified_at = if lead_id.is_some() {
        linked
            .and_then(|identity| identity.verified_at.clone())
            .or_else(|| canonical.and_then(|identity| identity.verified_at.clone()))
    } else {
        None
    };
    let created_at = existing
        .iter()
        .map(|identity| identity.created_at.clone())
        .min()
        .unwrap_or_else(|| now.to_string());

    let canonical_id = if let Some(canonical) = canonical {
        tx.execute(
            "UPDATE channel_identities
             SET lead_id = ?2,
                 normalized_address = COALESCE(?3, normalized_address),
                 display_name = COALESCE(?4, display_name),
                 avatar_url = COALESCE(?5, avatar_url),
                 profile_synced_at = COALESCE(?6, profile_synced_at),
                 profile_sync_error = ?7,
                 profile_sync_attempts = MAX(profile_sync_attempts, ?8),
                 next_profile_retry_at = COALESCE(?9, next_profile_retry_at),
                 link_source = ?10,
                 link_confidence = ?11,
                 verified_at = ?12,
                 created_at = MIN(created_at, ?13),
                 updated_at = ?14
             WHERE id = ?1",
            params![
                canonical.id,
                lead_id,
                normalized_address,
                display_name,
                avatar_url,
                profile_synced_at,
                profile_sync_error,
                profile_sync_attempts,
                next_profile_retry_at,
                link_source,
                link_confidence,
                verified_at,
                created_at,
                now,
            ],
        )?;
        canonical.id
    } else {
        tx.query_row(
            "INSERT INTO channel_identities
             (lead_id, channel, identity_namespace, external_id, normalized_address,
              display_name, avatar_url, profile_synced_at, profile_sync_error,
              profile_sync_attempts, next_profile_retry_at, link_source, link_confidence,
              verified_at, created_at, updated_at)
             VALUES (?1, 'zalo', 'zalo_uid', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             RETURNING id",
            params![
                lead_id,
                external_id,
                normalized_address,
                display_name,
                avatar_url,
                profile_synced_at,
                profile_sync_error,
                profile_sync_attempts,
                next_profile_retry_at,
                link_source,
                link_confidence,
                verified_at,
                created_at,
                now,
            ],
            |row| row.get(0),
        )?
    };

    for duplicate in existing
        .iter()
        .filter(|identity| identity.id != canonical_id)
    {
        tx.execute(
            "INSERT INTO conversation_participants
             (conversation_id, identity_id, participant_role, joined_at, left_at)
             SELECT conversation_id, ?1, participant_role, joined_at, left_at
             FROM conversation_participants
             WHERE identity_id = ?2
             ON CONFLICT(conversation_id, identity_id) DO UPDATE SET
                 participant_role = CASE
                     WHEN conversation_participants.participant_role = 'admin'
                          OR excluded.participant_role = 'admin' THEN 'admin'
                     WHEN conversation_participants.participant_role = 'deputy'
                          OR excluded.participant_role = 'deputy' THEN 'deputy'
                     ELSE 'member'
                 END,
                 joined_at = CASE
                     WHEN conversation_participants.joined_at IS NULL THEN excluded.joined_at
                     WHEN excluded.joined_at IS NULL THEN conversation_participants.joined_at
                     ELSE MIN(conversation_participants.joined_at, excluded.joined_at)
                 END,
                 left_at = CASE
                     WHEN conversation_participants.left_at IS NULL OR excluded.left_at IS NULL
                     THEN NULL
                     ELSE MAX(conversation_participants.left_at, excluded.left_at)
                 END",
            params![canonical_id, duplicate.id],
        )?;
        tx.execute(
            "DELETE FROM conversation_participants WHERE identity_id = ?1",
            [duplicate.id],
        )?;
        tx.execute(
            "UPDATE messages SET sender_identity_id = ?1 WHERE sender_identity_id = ?2",
            params![canonical_id, duplicate.id],
        )?;
        tx.execute(
            "UPDATE messages SET reply_sender_identity_id = ?1 WHERE reply_sender_identity_id = ?2",
            params![canonical_id, duplicate.id],
        )?;
        tx.execute(
            "UPDATE message_mentions SET identity_id = ?1 WHERE identity_id = ?2",
            params![canonical_id, duplicate.id],
        )?;
        tx.execute(
            "DELETE FROM channel_identities WHERE id = ?1",
            [duplicate.id],
        )?;
    }

    Ok(canonical_id)
}

fn find_channel_account_id_by_key(
    conn: &Connection,
    channel: &str,
    provider_account_key: &str,
) -> Result<Option<i64>> {
    if channel != "zalo" {
        return conn
            .query_row(
                "SELECT id FROM channel_accounts WHERE channel = ?1 AND provider_account_key = ?2",
                params![channel, provider_account_key],
                |row| row.get(0),
            )
            .optional();
    }

    conn.query_row(
        "SELECT ca.id
         FROM channel_accounts ca
         WHERE ca.channel = 'zalo'
           AND (
                ca.provider_account_key = ?1
                OR ca.self_external_id = ?1
                OR ca.id IN (
                    SELECT channel_account_id FROM zalo_sessions
                    WHERE channel_account_id IS NOT NULL AND (imei = ?1 OR zalo_uid = ?1)
                )
                OR ca.provider_account_key IN (
                    SELECT zalo_uid FROM zalo_sessions
                    WHERE zalo_uid IS NOT NULL AND zalo_uid != '' AND (imei = ?1 OR zalo_uid = ?1)
                )
                OR ca.self_external_id IN (
                    SELECT zalo_uid FROM zalo_sessions
                    WHERE zalo_uid IS NOT NULL AND zalo_uid != '' AND (imei = ?1 OR zalo_uid = ?1)
                )
           )
         ORDER BY CASE
                    WHEN ca.provider_account_key = ?1 THEN 0
                    WHEN ca.self_external_id = ?1 THEN 1
                    ELSE 2
                  END,
                  ca.id
         LIMIT 1",
        [provider_account_key],
        |row| row.get(0),
    )
    .optional()
}

fn link_zalo_sessions_to_account(
    conn: &Connection,
    provider_account_key: &str,
    channel_account_id: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE zalo_sessions
         SET channel_account_id = ?2,
             zalo_uid = COALESCE(
                 zalo_uid,
                 (SELECT self_external_id FROM channel_accounts WHERE id = ?2)
             )
         WHERE imei = ?1 OR zalo_uid = ?1 OR channel_account_id = ?2",
        params![provider_account_key, channel_account_id],
    )?;
    Ok(())
}

fn ensure_channel_account(
    conn: &Connection,
    channel: &str,
    provider_account_key: &str,
    display_name: Option<&str>,
    capabilities_json: &str,
    now: &str,
) -> Result<i64> {
    if let Some(account_id) = find_channel_account_id_by_key(conn, channel, provider_account_key)? {
        conn.execute(
            "UPDATE channel_accounts
             SET display_name = COALESCE(?2, display_name),
                 capabilities_json = CASE
                     WHEN capabilities_json = '{}' AND ?3 != '{}' THEN ?3
                     ELSE capabilities_json
                 END,
                 updated_at = ?4
             WHERE id = ?1",
            params![account_id, display_name, capabilities_json, now],
        )?;
        if channel == "zalo" {
            link_zalo_sessions_to_account(conn, provider_account_key, account_id)?;
        }
        return Ok(account_id);
    }

    conn.query_row(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'disconnected', ?4, ?5, ?5)
         ON CONFLICT(channel, provider_account_key) DO UPDATE SET
             display_name = COALESCE(excluded.display_name, channel_accounts.display_name),
             capabilities_json = CASE
                 WHEN channel_accounts.capabilities_json = '{}' AND excluded.capabilities_json != '{}'
                 THEN excluded.capabilities_json ELSE channel_accounts.capabilities_json END,
             updated_at = excluded.updated_at
         RETURNING id",
        params![
            channel,
            provider_account_key,
            display_name,
            capabilities_json,
            now
        ],
        |row| row.get(0),
    )
}

fn resolve_message_id(
    tx: &rusqlite::Transaction,
    provider_account_key: &str,
    external_thread_id: &str,
    is_group: bool,
    provider_message_id: &str,
    provider_client_message_id: &str,
) -> Result<Option<i64>> {
    let channel_account_id = find_channel_account_id_by_key(tx, "zalo", provider_account_key)?;

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

    if !provider_message_id.is_empty() && provider_message_id != "0" {
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

fn stage_pending_reaction_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_account_id: i64,
    reaction: &NormalizedReaction,
) -> Result<()> {
    let provider_message_id = reaction.provider_message_id.trim();
    let provider_client_message_id = reaction.provider_client_message_id.trim();
    if provider_message_id.is_empty() && provider_client_message_id.is_empty() {
        return Err(rusqlite::Error::InvalidQuery);
    }

    let now = utc_now();
    tx.execute(
        "INSERT INTO pending_message_reactions
            (channel_account_id, external_thread_id, conversation_kind, provider_event_id,
             provider_message_id, provider_client_message_id, actor_external_id, reaction,
             removed, occurred_at, raw_payload_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
         ON CONFLICT(
             channel_account_id, external_thread_id, conversation_kind, provider_event_id,
             provider_message_id, provider_client_message_id, actor_external_id, reaction, removed
         ) DO UPDATE SET
             occurred_at = MAX(pending_message_reactions.occurred_at, excluded.occurred_at),
             raw_payload_json = CASE
                 WHEN excluded.occurred_at >= pending_message_reactions.occurred_at
                 THEN excluded.raw_payload_json
                 ELSE pending_message_reactions.raw_payload_json
             END,
             updated_at = excluded.updated_at",
        params![
            channel_account_id,
            reaction.external_thread_id,
            if reaction.is_group { "group" } else { "direct" },
            reaction.provider_event_id.as_deref().unwrap_or("").trim(),
            provider_message_id,
            provider_client_message_id,
            reaction.actor_external_id,
            reaction.reaction.trim(),
            if reaction.removed { 1 } else { 0 },
            reaction.occurred_at,
            reaction.raw_payload.to_string(),
            now,
        ],
    )?;
    Ok(())
}

fn replay_pending_reactions_tx(
    tx: &rusqlite::Transaction<'_>,
    channel_account_id: i64,
    message_id: i64,
    message: &NormalizedMessage,
) -> Result<usize> {
    let provider_message_id = message
        .provider_message_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    let provider_client_message_id = message
        .provider_client_message_id
        .as_deref()
        .map(str::trim)
        .unwrap_or("");
    if provider_message_id.is_empty() && provider_client_message_id.is_empty() {
        return Ok(0);
    }

    let pending = {
        let mut statement = tx.prepare(
            "SELECT id, actor_external_id, reaction, removed, occurred_at
             FROM pending_message_reactions
             WHERE channel_account_id = ?1
               AND external_thread_id = ?2
               AND conversation_kind = ?3
               AND (
                   (?4 != '' AND provider_message_id = ?4)
                   OR (?5 != '' AND provider_client_message_id = ?5)
               )
             ORDER BY occurred_at, id",
        )?;
        let rows = statement.query_map(
            params![
                channel_account_id,
                message.external_thread_id,
                message.conversation_kind.as_str(),
                provider_message_id,
                provider_client_message_id,
            ],
            |row| {
                Ok(PendingMessageReaction {
                    id: row.get(0)?,
                    actor_external_id: row.get(1)?,
                    reaction: row.get(2)?,
                    removed: row.get::<_, i64>(3)? != 0,
                    occurred_at: row.get(4)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>>>()?
    };

    for pending_reaction in &pending {
        apply_reaction_to_message_tx(
            tx,
            message_id,
            &pending_reaction.actor_external_id,
            &pending_reaction.reaction,
            pending_reaction.removed,
            &pending_reaction.occurred_at,
        )?;
    }

    for pending_reaction in &pending {
        tx.execute(
            "DELETE FROM pending_message_reactions WHERE id = ?1",
            [pending_reaction.id],
        )?;
    }

    Ok(pending.len())
}

fn apply_reaction_to_message_tx(
    tx: &rusqlite::Transaction<'_>,
    message_id: i64,
    actor_external_id: &str,
    reaction: &str,
    removed: bool,
    occurred_at: &str,
) -> Result<()> {
    let canonical_code = canonical_reaction_code(reaction);
    let stored_reaction = canonical_code.unwrap_or(reaction.trim());

    if removed {
        mark_reaction_removed(
            tx,
            message_id,
            actor_external_id,
            canonical_code,
            occurred_at,
        )?;
    } else {
        tx.execute(
            "INSERT INTO message_reactions (message_id, actor_external_id, reaction, count, reacted_at, removed_at)
             VALUES (?1, ?2, ?3, 1, ?4, NULL)
             ON CONFLICT(message_id, actor_external_id, reaction) DO UPDATE SET
                 count = MAX(message_reactions.count, 1),
                 reacted_at = COALESCE(message_reactions.reacted_at, excluded.reacted_at),
                 removed_at = NULL",
            params![message_id, actor_external_id, stored_reaction, occurred_at],
        )?;
    }

    let active_flag = if removed { 0 } else { 1 };
    let now = utc_now();
    if let Some(reaction_code) = canonical_code {
        tx.execute(
            "UPDATE reaction_mutations
             SET status = 'succeeded', updated_at = ?1
             WHERE message_id = ?2 AND reaction_code = ?3 AND active = ?4 AND status IN ('running', 'delivery_unknown')",
            params![now, message_id, reaction_code, active_flag],
        )?;
    } else if removed {
        tx.execute(
            "UPDATE reaction_mutations
             SET status = 'succeeded', updated_at = ?1
             WHERE message_id = ?2 AND active = 0 AND status IN ('running', 'delivery_unknown')",
            params![now, message_id],
        )?;
    }

    Ok(())
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

    let account_display_name: Option<String> = row.get(4)?;
    let sender_display_name_raw: Option<String> = row.get(6)?;
    let sender_display_name = match (
        &direction,
        sender_display_name_raw.as_deref(),
        account_display_name.as_deref(),
    ) {
        (MessageDirection::Inbound, Some(s), Some(a))
            if s.trim().eq_ignore_ascii_case(a.trim()) =>
        {
            None
        }
        (_, s, _) => s.map(str::to_string),
    };

    let sender_external_id: Option<String> = row.get(19).ok();

    Ok(ConversationMessage {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        channel_account_id: row.get(2)?,
        channel,
        account_display_name,
        sender_identity_id: row.get(5)?,
        sender_external_id,
        sender_display_name,
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
