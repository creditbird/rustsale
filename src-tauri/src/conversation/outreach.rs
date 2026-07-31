use crate::conversation::repository::AppliedMessage;
use rusqlite::{params, OptionalExtension, Result, Transaction};
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZaloProviderReceipt {
    pub primary_message_id: Option<String>,
    pub attachment_message_ids: Vec<String>,
}

impl ZaloProviderReceipt {
    pub fn from_response(response: zca_rs::models::SendMessageResponse) -> Self {
        let text_message_id = response.message.map(|message| message.msg_id.to_string());
        let attachment_message_ids = response
            .attachment
            .into_iter()
            .map(|message| message.msg_id.to_string())
            .collect::<Vec<_>>();
        let primary_message_id =
            text_message_id.or_else(|| attachment_message_ids.first().cloned());
        Self {
            primary_message_id,
            attachment_message_ids,
        }
    }

    pub fn mock(local_request_id: &str) -> Self {
        Self {
            primary_message_id: Some(format!("mock-{local_request_id}")),
            attachment_message_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OutreachConversationRecord {
    pub lead_id: i64,
    pub provider_account_key: String,
    pub account_display_name: Option<String>,
    pub external_user_id: String,
    pub user_display_name: Option<String>,
    pub user_avatar_url: Option<String>,
    pub template_id: i64,
    pub source: String,
    pub text: String,
    pub local_request_id: String,
    pub provider_receipt: ZaloProviderReceipt,
    pub sent_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedOutreachMessage {
    pub applied: AppliedMessage,
    pub lead_id: i64,
}

pub fn record_outreach_success_in_transaction(
    tx: &Transaction<'_>,
    record: &OutreachConversationRecord,
) -> Result<AppliedOutreachMessage> {
    let now = &record.sent_at;

    // 1. Upsert channel_accounts (channel = 'zalo', provider_account_key)
    let channel_account_id: i64 = tx.query_row(
        "INSERT INTO channel_accounts
         (channel, provider_account_key, display_name, connection_status, capabilities_json, created_at, updated_at)
         VALUES ('zalo', ?1, ?2, 'disconnected', '{}', ?3, ?3)
         ON CONFLICT(channel, provider_account_key) DO UPDATE SET
             display_name = COALESCE(excluded.display_name, channel_accounts.display_name),
             updated_at = excluded.updated_at
         RETURNING id",
        params![
            record.provider_account_key,
            record.account_display_name,
            now
        ],
        |row| row.get(0),
    )?;

    // 2. Upsert channel_identities (namespace = 'zalo_uid', external_id = external_user_id, lead_id)
    let identity_id: i64 = tx.query_row(
        "INSERT INTO channel_identities
         (lead_id, channel, identity_namespace, external_id, display_name, avatar_url, link_source, created_at, updated_at)
         VALUES (?1, 'zalo', 'zalo_uid', ?2, ?3, ?4, 'outreach', ?5, ?5)
         ON CONFLICT(channel, identity_namespace, external_id) DO UPDATE SET
             lead_id = COALESCE(channel_identities.lead_id, excluded.lead_id),
             display_name = COALESCE(excluded.display_name, channel_identities.display_name),
             avatar_url = COALESCE(excluded.avatar_url, channel_identities.avatar_url),
             updated_at = excluded.updated_at
         RETURNING id",
        params![
            record.lead_id,
            record.external_user_id,
            record.user_display_name,
            record.user_avatar_url,
            now
        ],
        |row| row.get(0),
    )?;

    // 3. Upsert channel_conversations (kind = 'direct')
    let conversation_id: i64 = tx.query_row(
        "INSERT INTO channel_conversations
         (channel_account_id, external_thread_id, kind, title, status, last_message_at, last_synced_at, created_at, updated_at)
         VALUES (?1, ?2, 'direct', ?3, 'active', ?4, ?4, ?4, ?4)
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
            record.external_user_id,
            record.user_display_name,
            now
        ],
        |row| row.get(0),
    )?;

    // 4. Upsert participant (role = 'member')
    tx.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role)
         VALUES (?1, ?2, 'member')
         ON CONFLICT(conversation_id, identity_id) DO NOTHING",
        params![conversation_id, identity_id],
    )?;

    // 5. Find existing message by (conversation_id, provider_message_id) or local_request_id
    let existing_message_id =
        if let Some(provider_message_id) = &record.provider_receipt.primary_message_id {
            tx.query_row(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND provider_message_id = ?2",
                params![conversation_id, provider_message_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        } else {
            tx.query_row(
                "SELECT id FROM messages WHERE local_request_id = ?1",
                params![&record.local_request_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        };

    if let Some(message_id) = existing_message_id {
        return Ok(AppliedOutreachMessage {
            applied: AppliedMessage {
                channel_account_id,
                identity_id: Some(identity_id),
                conversation_id,
                message_id,
                duplicate: true,
            },
            lead_id: record.lead_id,
        });
    }

    // 6. Insert one outbound message
    let content_type = if record.provider_receipt.attachment_message_ids.is_empty() {
        "text"
    } else {
        "template"
    };

    let content_json = json!({
        "text": record.text,
        "outreach": {
            "templateId": record.template_id,
            "source": record.source,
            "providerAttachmentMessageIds": record.provider_receipt.attachment_message_ids,
        }
    });

    let message_id: i64 = tx.query_row(
        "INSERT INTO messages
         (conversation_id, sender_identity_id, direction, provider_message_id,
          provider_client_message_id, local_request_id, content_type, text_content,
          content_json, raw_payload_json, provider_timestamp, ingested_at, sent_at, status)
         VALUES (?1, NULL, 'outbound', ?2, NULL, ?3, ?4, ?5, ?6, NULL, ?7, ?7, ?7, 'sent')
         RETURNING id",
        params![
            conversation_id,
            record.provider_receipt.primary_message_id,
            record.local_request_id,
            content_type,
            record.text,
            content_json.to_string(),
            now,
        ],
        |row| row.get(0),
    )?;

    // 7. Return AppliedOutreachMessage
    Ok(AppliedOutreachMessage {
        applied: AppliedMessage {
            channel_account_id,
            identity_id: Some(identity_id),
            conversation_id,
            message_id,
            duplicate: false,
        },
        lead_id: record.lead_id,
    })
}
