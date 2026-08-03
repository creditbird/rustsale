use crate::conversation::error::{ConversationError, ConversationErrorCode, ConversationResult};
use crate::conversation::repository::next_provider_client_message_id;
use crate::conversation::types::{
    ConversationCapabilities, ConversationForwardTargetResult, ForwardConversationMessageRequest,
    ForwardConversationMessageResult, ForwardOperationStatus, ForwardTargetStatus,
};
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;

pub fn enqueue_forward_operation(
    tx: &rusqlite::Transaction<'_>,
    request: &ForwardConversationMessageRequest,
    now: &str,
) -> ConversationResult<ForwardConversationMessageResult> {
    // 1. Target count validation
    if request.target_conversation_ids.is_empty() || request.target_conversation_ids.len() > 10 {
        return Err(ConversationError::new(
            ConversationErrorCode::ForwardTargetLimitExceeded,
            format!(
                "Target count must be between 1 and 10, got {}",
                request.target_conversation_ids.len()
            ),
        ));
    }

    // 2. Target uniqueness validation
    let mut unique_targets = HashSet::new();
    for &target_id in &request.target_conversation_ids {
        if !unique_targets.insert(target_id) {
            return Err(ConversationError::new(
                ConversationErrorCode::ForwardTargetLimitExceeded,
                format!("Duplicate target conversation ID {}", target_id),
            ));
        }
    }

    // 3. Idempotency Check
    let existing_op: Option<(i64, String)> = tx
        .query_row(
            "SELECT source_message_id, status FROM message_forward_operations WHERE client_operation_id = ?1",
            params![request.client_operation_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    if let Some((src_msg_id, _op_status_str)) = existing_op {
        if src_msg_id != request.source_message_id {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!(
                    "Client operation ID {} reused with different source message ID (existing {}, requested {})",
                    request.client_operation_id, src_msg_id, request.source_message_id
                ),
            ));
        }

        let mut stmt = tx.prepare(
            "SELECT target_conversation_id, send_batch_id, status, error_message
             FROM message_forward_targets
             WHERE client_operation_id = ?1
             ORDER BY id ASC",
        )?;
        let target_rows = stmt.query_map(params![request.client_operation_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;

        let mut target_results = Vec::new();
        let mut target_statuses = Vec::new();
        let mut existing_target_ids = Vec::new();

        for tr in target_rows {
            let (t_id, batch_id, t_status_str, err_msg) = tr?;
            existing_target_ids.push(t_id);
            let fwd_msg_id: Option<i64> = if let Some(ref b_id) = batch_id {
                tx.query_row(
                    "SELECT id FROM messages WHERE send_batch_id = ?1 AND conversation_id = ?2",
                    params![b_id, t_id],
                    |row| row.get(0),
                )
                .optional()?
            } else {
                None
            };

            target_statuses.push(t_status_str.clone());
            let target_status = ForwardTargetStatus::from_db_str(&t_status_str);

            target_results.push(ConversationForwardTargetResult {
                conversation_id: t_id,
                status: target_status,
                forwarded_message_id: fwd_msg_id,
                error_message: err_msg,
            });
        }

        if existing_target_ids != request.target_conversation_ids {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!(
                    "Client operation ID {} reused with different target conversation IDs",
                    request.client_operation_id
                ),
            ));
        }

        let op_status_str = compute_aggregate_operation_status(&target_statuses);
        let op_status = ForwardOperationStatus::from_db_str(op_status_str);

        return Ok(ForwardConversationMessageResult {
            client_operation_id: request.client_operation_id.clone(),
            source_message_id: src_msg_id,
            status: op_status,
            targets: target_results,
        });
    }

    // 4. Source message validation
    struct SourceMsgRow {
        _src_conv_id: i64,
        p_msg_id: Option<String>,
        p_cli_id: Option<String>,
        source_account_id: i64,
        capabilities_json: String,
        content_type: String,
    }

    let src_row: Option<SourceMsgRow> = tx
        .query_row(
            "SELECT m.conversation_id, m.provider_message_id, m.provider_client_message_id,
                    cc.channel_account_id, ca.capabilities_json, m.content_type
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             WHERE m.id = ?1",
            params![request.source_message_id],
            |row| {
                Ok(SourceMsgRow {
                    _src_conv_id: row.get(0)?,
                    p_msg_id: row.get(1)?,
                    p_cli_id: row.get(2)?,
                    source_account_id: row.get(3)?,
                    capabilities_json: row.get(4)?,
                    content_type: row.get(5)?,
                })
            },
        )
        .optional()?;

    let SourceMsgRow {
        _src_conv_id,
        p_msg_id,
        p_cli_id,
        source_account_id,
        capabilities_json,
        content_type,
    } = match src_row {
        Some(row) => row,
        None => {
            return Err(ConversationError::new(
                ConversationErrorCode::ForwardSourceMissingProviderIds,
                format!("Source message {} not found", request.source_message_id),
            ))
        }
    };

    let forwardable_types = [
        "text", "image", "video", "audio", "file", "sticker", "link", "forward",
    ];
    if !forwardable_types.contains(&content_type.as_str()) {
        return Err(ConversationError::new(
            ConversationErrorCode::ForwardNotSupported,
            format!("Content type '{}' cannot be forwarded", content_type),
        ));
    }

    if p_msg_id.as_deref().unwrap_or("").trim().is_empty()
        || p_cli_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Err(ConversationError::new(
            ConversationErrorCode::ForwardSourceMissingProviderIds,
            format!(
                "Source message {} is missing provider IDs",
                request.source_message_id
            ),
        ));
    }

    let caps = if capabilities_json.trim().is_empty() || capabilities_json.trim() == "{}" {
        ConversationCapabilities::zalo_phase_one()
    } else {
        ConversationCapabilities::from_json(&capabilities_json)
    };
    if !caps.forward.enabled || caps.forward.max_targets_per_forward == 0 {
        return Err(ConversationError::new(
            ConversationErrorCode::ForwardNotSupported,
            "Forwarding is not supported on this channel account",
        ));
    }

    // 5. Target conversations validation
    for &target_id in &request.target_conversation_ids {
        let target_acc_id: Option<i64> = tx
            .query_row(
                "SELECT channel_account_id FROM channel_conversations WHERE id = ?1 AND status = 'active'",
                params![target_id],
                |row| row.get(0),
            )
            .optional()?;

        match target_acc_id {
            Some(acc_id) => {
                if acc_id != source_account_id {
                    return Err(ConversationError::new(
                        ConversationErrorCode::ForwardTargetAccountMismatch,
                        format!(
                            "Target conversation {} belongs to channel account {}, but source belongs to account {}",
                            target_id, acc_id, source_account_id
                        ),
                    ));
                }
            }
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Target conversation {} not found", target_id),
                ));
            }
        }
    }

    // 6. Insert forward operation row
    tx.execute(
        "INSERT INTO message_forward_operations
         (client_operation_id, source_message_id, status, created_at, updated_at)
         VALUES (?1, ?2, 'queued', ?3, ?3)",
        params![request.client_operation_id, request.source_message_id, now],
    )?;

    let mut target_results = Vec::new();

    for &target_id in &request.target_conversation_ids {
        let batch_id = format!("{}:{}", request.client_operation_id, target_id);

        // Insert message_send_batches
        tx.execute(
            "INSERT INTO message_send_batches
             (id, conversation_id, channel_account_id, client_batch_id, reply_to_message_id, part_count, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, NULL, 1, 'queued', ?5, ?5)",
            params![
                batch_id,
                target_id,
                source_account_id,
                batch_id,
                now,
            ],
        )?;

        // Insert message_forward_targets
        tx.execute(
            "INSERT INTO message_forward_targets
             (client_operation_id, target_conversation_id, send_batch_id, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'queued', ?4, ?4)",
            params![request.client_operation_id, target_id, batch_id, now],
        )?;

        // Insert target message in messages table
        let cli_msg_id = next_provider_client_message_id();
        let local_req_id = format!("batch:{}:part:fwd", batch_id);
        let content_json = serde_json::json!({
            "sourceMessageId": request.source_message_id,
            "clientOperationId": request.client_operation_id,
        })
        .to_string();

        let target_msg_id: i64 = tx.query_row(
            "INSERT INTO messages
             (conversation_id, sender_identity_id, direction, provider_message_id, provider_client_message_id,
              local_request_id, content_type, text_content, content_json, raw_payload_json, provider_timestamp,
              ingested_at, sent_at, edited_at, removed_at, status, error_code, error_message, reply_to_message_id,
              send_batch_id, batch_index)
             VALUES (?1, NULL, 'outbound', NULL, ?2, ?3, 'forward', ?4, ?5, NULL, NULL, ?6, NULL, NULL, NULL, 'queued', NULL, NULL, NULL, ?7, 0)
             RETURNING id",
            params![
                target_id,
                cli_msg_id,
                local_req_id,
                "Đã chuyển tiếp một tin nhắn",
                content_json,
                now,
                batch_id,
            ],
            |row| row.get(0),
        )?;

        // Insert outbox job
        let job_id = format!("outbox:{}:{}", batch_id, target_msg_id);
        tx.execute(
            "INSERT INTO message_outbox_jobs
             (id, message_id, channel_account_id, status, attempt_count, next_attempt_at, last_error, created_at, updated_at, completed_at)
             VALUES (?1, ?2, ?3, 'pending', 0, ?4, NULL, ?4, ?4, NULL)",
            params![job_id, target_msg_id, source_account_id, now],
        )?;

        target_results.push(ConversationForwardTargetResult {
            conversation_id: target_id,
            status: ForwardTargetStatus::Queued,
            forwarded_message_id: Some(target_msg_id),
            error_message: None,
        });
    }

    let op_status = recalculate_forward_operation_status(tx, &request.client_operation_id, now)?;

    Ok(ForwardConversationMessageResult {
        client_operation_id: request.client_operation_id.clone(),
        source_message_id: request.source_message_id,
        status: op_status,
        targets: target_results,
    })
}

pub fn compute_aggregate_operation_status(statuses: &[impl AsRef<str>]) -> &'static str {
    if statuses.is_empty() {
        return "queued";
    }
    let total = statuses.len();
    let count_sent = statuses.iter().filter(|s| s.as_ref() == "sent").count();
    let count_failed = statuses.iter().filter(|s| s.as_ref() == "failed").count();
    let count_unknown = statuses
        .iter()
        .filter(|s| s.as_ref() == "delivery_unknown")
        .count();
    let count_sending = statuses.iter().filter(|s| s.as_ref() == "sending").count();
    let count_queued = statuses.iter().filter(|s| s.as_ref() == "queued").count();

    if count_sent == total {
        "sent"
    } else if count_failed == total {
        "failed"
    } else if count_unknown == total {
        "delivery_unknown"
    } else if count_queued == total {
        "queued"
    } else if count_sending == total
        || (count_sending + count_queued == total
            && count_sent == 0
            && count_failed == 0
            && count_unknown == 0)
    {
        "sending"
    } else {
        "partial"
    }
}

pub fn recalculate_forward_operation_status(
    tx: &rusqlite::Transaction<'_>,
    client_operation_id: &str,
    now: &str,
) -> ConversationResult<ForwardOperationStatus> {
    let mut stmt =
        tx.prepare("SELECT status FROM message_forward_targets WHERE client_operation_id = ?1")?;
    let target_rows =
        stmt.query_map(params![client_operation_id], |row| row.get::<_, String>(0))?;
    let mut statuses = Vec::new();
    for r in target_rows {
        statuses.push(r?);
    }

    let op_status_str = compute_aggregate_operation_status(&statuses);

    tx.execute(
        "UPDATE message_forward_operations
         SET status = ?2, updated_at = ?3
         WHERE client_operation_id = ?1",
        params![client_operation_id, op_status_str, now],
    )?;

    Ok(ForwardOperationStatus::from_db_str(op_status_str))
}

pub fn update_forward_target_status(
    tx: &rusqlite::Transaction<'_>,
    client_operation_id: &str,
    target_conversation_id: i64,
    status: ForwardTargetStatus,
    error_code: Option<&str>,
    error_message: Option<&str>,
    now: &str,
) -> ConversationResult<ForwardOperationStatus> {
    tx.execute(
        "UPDATE message_forward_targets
         SET status = ?3, error_code = ?4, error_message = ?5, updated_at = ?6
         WHERE client_operation_id = ?1 AND target_conversation_id = ?2",
        params![
            client_operation_id,
            target_conversation_id,
            status.as_str(),
            error_code,
            error_message,
            now,
        ],
    )?;

    recalculate_forward_operation_status(tx, client_operation_id, now)
}

pub fn retry_forward_target(
    tx: &rusqlite::Transaction<'_>,
    client_operation_id: &str,
    target_conversation_id: i64,
    now: &str,
) -> ConversationResult<ConversationForwardTargetResult> {
    let row: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT status, send_batch_id, error_message
             FROM message_forward_targets
             WHERE client_operation_id = ?1 AND target_conversation_id = ?2",
            params![client_operation_id, target_conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    let (current_status, batch_id, _err_msg) = match row {
        Some(r) => r,
        None => {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                format!(
                    "Target conversation {} for operation {} not found",
                    target_conversation_id, client_operation_id
                ),
            ));
        }
    };

    if current_status != "failed" {
        return Err(ConversationError::new(
            ConversationErrorCode::InvalidRequest,
            format!(
                "Cannot retry target in status '{}'; manual retry is only allowed for status 'failed'",
                current_status
            ),
        ));
    }

    let batch_id = batch_id.ok_or_else(|| {
        ConversationError::new(
            ConversationErrorCode::InvalidRequest,
            "Target batch ID missing",
        )
    })?;

    // 1. Update message_forward_targets status -> 'queued'
    tx.execute(
        "UPDATE message_forward_targets
         SET status = 'queued', error_code = NULL, error_message = NULL, updated_at = ?3
         WHERE client_operation_id = ?1 AND target_conversation_id = ?2",
        params![client_operation_id, target_conversation_id, now],
    )?;

    // 2. Find target message ID
    let target_msg_id: i64 = tx.query_row(
        "SELECT id FROM messages WHERE send_batch_id = ?1 AND conversation_id = ?2",
        params![batch_id, target_conversation_id],
        |r| r.get(0),
    )?;

    // 3. Update message status -> 'queued'
    tx.execute(
        "UPDATE messages
         SET status = 'queued', error_code = NULL, error_message = NULL
         WHERE id = ?1",
        params![target_msg_id],
    )?;

    // 4. Reset outbox job -> 'pending'
    tx.execute(
        "UPDATE message_outbox_jobs
         SET status = 'pending', attempt_count = 0, next_attempt_at = ?2, last_error = NULL, updated_at = ?2, completed_at = NULL
         WHERE message_id = ?1",
        params![target_msg_id, now],
    )?;

    // 5. Update send_batch status -> 'queued'
    tx.execute(
        "UPDATE message_send_batches
         SET status = 'queued', error_code = NULL, error_message = NULL, updated_at = ?2
         WHERE id = ?1",
        params![batch_id, now],
    )?;

    // 6. Recalculate operation status
    recalculate_forward_operation_status(tx, client_operation_id, now)?;

    Ok(ConversationForwardTargetResult {
        conversation_id: target_conversation_id,
        status: ForwardTargetStatus::Queued,
        forwarded_message_id: Some(target_msg_id),
        error_message: None,
    })
}
