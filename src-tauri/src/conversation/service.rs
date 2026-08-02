use super::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
use super::repository::ConversationRepository;
use super::types::{Channel, ConversationRuntimeStatus, InboxItem, TimelineCursor, TimelinePage};
use crate::conversation::outreach::AppliedOutreachMessage;
use crate::conversation::zalo::normalizer::{NormalizedGroupSnapshot, ZaloMutation};
use crate::conversation::zalo::{
    DIRECT_MESSAGE_WATERMARK_SCOPE, DIRECT_REACTION_WATERMARK_SCOPE, GROUP_MESSAGE_WATERMARK_SCOPE,
    GROUP_REACTION_WATERMARK_SCOPE,
};
use anyhow::Result;
use std::sync::Arc;

fn normalized_vn_mobile_phone(value: &str) -> Option<String> {
    let normalized = crate::commands::normalize_phone_digits(value);
    if normalized.len() != 10 {
        return None;
    }

    matches!(&normalized[0..2], "03" | "05" | "07" | "08" | "09").then_some(normalized)
}

use super::staging::ConversationAttachmentStager;
use super::types::{ConversationMessage, MessageDirection, MessageStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedRecall {
    pub message_id: i64,
    pub conversation_id: i64,
    pub channel: Channel,
    pub provider_account_key: String,
    pub external_thread_id: String,
    pub is_group: bool,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
}

#[derive(Clone)]
pub struct ConversationService {
    pub repository: Arc<ConversationRepository>,
    pub events: Arc<ConversationEventPublisher>,
    pub stager: ConversationAttachmentStager,
}

impl ConversationService {
    pub fn new(
        repository: Arc<ConversationRepository>,
        events: Arc<ConversationEventPublisher>,
        spool_root: std::path::PathBuf,
    ) -> Self {
        let stager = ConversationAttachmentStager::new(spool_root, repository.clone());
        Self {
            repository,
            events,
            stager,
        }
    }

    pub fn prepare_recall(&self, message_id: i64) -> Result<PreparedRecall> {
        let msg = self
            .repository
            .get_message_by_id(message_id)?
            .ok_or_else(|| anyhow::anyhow!("message {message_id} not found"))?;

        if msg.direction != MessageDirection::Outbound {
            anyhow::bail!("cannot recall inbound or system message");
        }
        if msg.status == MessageStatus::Removed {
            anyhow::bail!("message already recalled");
        }
        if msg.status == MessageStatus::Queued || msg.status == MessageStatus::Failed {
            anyhow::bail!("cannot recall queued or failed message");
        }

        let p_msg_id = msg
            .provider_message_id
            .ok_or_else(|| anyhow::anyhow!("missing provider_message_id"))?;

        let (provider_account_key, external_thread_id, is_group) =
            self.repository.lookup_message_thread_info(message_id)?;

        let cli_msg_id = msg
            .content
            .get("cliMsgId")
            .and_then(|v| v.as_str())
            .unwrap_or(&p_msg_id)
            .to_string();

        Ok(PreparedRecall {
            message_id,
            conversation_id: msg.conversation_id,
            channel: msg.channel,
            provider_account_key,
            external_thread_id,
            is_group,
            provider_message_id: p_msg_id,
            provider_client_message_id: cli_msg_id,
        })
    }

    pub fn complete_recall(&self, message_id: i64, now: &str) -> Result<ConversationMessage> {
        self.repository.mark_message_recalled(message_id, now)?;
        self.publish_message_invalidation(message_id, ConversationInvalidationReason::Message)?;
        let msg = self
            .repository
            .get_message_by_id(message_id)?
            .ok_or_else(|| anyhow::anyhow!("message {message_id} not found"))?;
        Ok(msg)
    }

    pub fn list_inbox(&self) -> Result<Vec<InboxItem>> {
        Ok(self.repository.list_inbox()?)
    }

    pub fn get_contact_timeline(
        &self,
        lead_id: i64,
        channels: &[Channel],
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        Ok(self
            .repository
            .get_contact_timeline(lead_id, channels, before, limit)?)
    }

    pub fn get_conversation_timeline(
        &self,
        conversation_id: i64,
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        Ok(self
            .repository
            .get_conversation_timeline(conversation_id, before, limit)?)
    }

    pub fn get_identity_timeline(
        &self,
        identity_id: i64,
        before: Option<&TimelineCursor>,
        limit: u32,
    ) -> Result<TimelinePage> {
        Ok(self
            .repository
            .get_identity_timeline(identity_id, before, limit)?)
    }

    pub fn link_identity(&self, identity_id: i64, lead_id: i64) -> Result<()> {
        let conversation_id = self.repository.link_identity(identity_id, lead_id)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id,
            lead_id: Some(lead_id),
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Link,
            message_id: None,
        });
        Ok(())
    }

    pub fn unlink_identity(&self, identity_id: i64) -> Result<()> {
        let conversation_id = self.repository.unlink_identity(identity_id)?;
        if conversation_id > 0 {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id,
                lead_id: None,
                channel: Channel::Zalo,
                reason: ConversationInvalidationReason::Link,
                message_id: None,
            });
        }
        Ok(())
    }

    pub fn unlink_lead(&self, lead_id: i64) -> Result<()> {
        let conversation_id = self.repository.unlink_lead(lead_id)?;
        if conversation_id > 0 {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id,
                lead_id: Some(lead_id),
                channel: Channel::Zalo,
                reason: ConversationInvalidationReason::Link,
                message_id: None,
            });
        }
        Ok(())
    }

    pub fn relink_lead(&self, source_lead_id: i64, target_lead_id: i64) -> Result<()> {
        let conversation_id = self
            .repository
            .relink_lead(source_lead_id, target_lead_id)?;
        if conversation_id > 0 {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id,
                lead_id: Some(source_lead_id),
                channel: Channel::Zalo,
                reason: ConversationInvalidationReason::Link,
                message_id: None,
            });
            if target_lead_id != source_lead_id {
                self.events.publish(ConversationInvalidatedEvent {
                    conversation_id,
                    lead_id: Some(target_lead_id),
                    channel: Channel::Zalo,
                    reason: ConversationInvalidationReason::Link,
                    message_id: None,
                });
            }
        }
        Ok(())
    }

    pub fn delete_conversation(
        &self,
        request: &super::types::DeleteConversationRequest,
    ) -> Result<()> {
        self.repository.delete_conversation(request)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id: request.conversation_id.unwrap_or(0),
            lead_id: request.lead_id,
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Link,
            message_id: None,
        });
        Ok(())
    }

    pub fn start_direct_conversation(
        &self,
        request: &super::types::StartDirectConversationRequest,
    ) -> Result<super::types::ConversationMessage> {
        let now = crate::conversation::types::utc_now();
        let message = self.repository.start_direct_conversation(request, &now)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id: message.conversation_id,
            lead_id: None,
            channel: message.channel,
            reason: ConversationInvalidationReason::Message,
            message_id: Some(message.id),
        });
        Ok(message)
    }

    pub fn ingest_zalo_event(
        &self,
        provider_account_key: &str,
        account_display_name: Option<&str>,
        event: zca_rs::listener::ListenerEvent,
    ) -> Result<()> {
        for mutation in crate::conversation::zalo::normalizer::normalize_listener_event(
            provider_account_key,
            account_display_name,
            event,
        )? {
            match mutation {
                ZaloMutation::Message(message) => {
                    let is_group = matches!(
                        message.conversation_kind,
                        crate::conversation::types::ConversationKind::Group
                    );
                    let scope = if is_group {
                        GROUP_MESSAGE_WATERMARK_SCOPE
                    } else {
                        DIRECT_MESSAGE_WATERMARK_SCOPE
                    };
                    let applied = self.repository.apply_normalized_message(&message)?;
                    let event_at = message
                        .provider_timestamp
                        .as_deref()
                        .unwrap_or(&message.ingested_at);
                    self.repository
                        .update_checkpoint(
                            applied.channel_account_id,
                            scope,
                            message.provider_message_id.as_deref(),
                            Some(event_at),
                            None,
                        )
                        .map_err(|error| {
                            anyhow::anyhow!("Failed to update message checkpoint: {error}")
                        })?;
                    if !applied.duplicate {
                        let lead_id = applied.identity_id.and_then(|identity_id| {
                            self.repository
                                .lead_id_for_identity(identity_id)
                                .ok()
                                .flatten()
                        });
                        self.events.publish(ConversationInvalidatedEvent {
                            conversation_id: applied.conversation_id,
                            lead_id,
                            channel: Channel::Zalo,
                            reason: ConversationInvalidationReason::Message,
                            message_id: Some(applied.message_id),
                        });
                    }
                }
                ZaloMutation::Reaction(reaction) => {
                    if let Some(message_id) = self.repository.apply_reaction(&reaction)? {
                        self.publish_message_invalidation(
                            message_id,
                            ConversationInvalidationReason::Reaction,
                        )?;
                    }
                    if let Ok(Some(account_id)) =
                        self.repository.get_channel_account_id(provider_account_key)
                    {
                        let scope = if reaction.is_group {
                            GROUP_REACTION_WATERMARK_SCOPE
                        } else {
                            DIRECT_REACTION_WATERMARK_SCOPE
                        };
                        self.repository
                            .update_checkpoint(
                                account_id,
                                scope,
                                reaction.provider_event_id.as_deref(),
                                Some(&reaction.occurred_at),
                                None,
                            )
                            .map_err(|error| {
                                anyhow::anyhow!("Failed to update reaction checkpoint: {error}")
                            })?;
                    }
                }
                ZaloMutation::Undo(undo) => {
                    if let Some(message_id) = self.repository.apply_undo(&undo)? {
                        self.publish_message_invalidation(
                            message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                    }
                }
                ZaloMutation::Receipt(receipt) => {
                    if let Some(message_id) = self.repository.apply_receipt(&receipt)? {
                        self.publish_message_invalidation(
                            message_id,
                            ConversationInvalidationReason::Receipt,
                        )?;
                    }
                }
                ZaloMutation::GroupSnapshot(snapshot) => {
                    let conversation_id = self.repository.apply_group_snapshot(&snapshot)?;
                    self.events.publish(ConversationInvalidatedEvent {
                        conversation_id,
                        lead_id: None,
                        channel: Channel::Zalo,
                        reason: ConversationInvalidationReason::Metadata,
                        message_id: None,
                    });
                }
                ZaloMutation::ReconcileRequired { scope } => {
                    if let Ok(Some(account_id)) =
                        self.repository.get_channel_account_id(provider_account_key)
                    {
                        let scopes: &[&str] = if scope == "all" {
                            &[
                                DIRECT_MESSAGE_WATERMARK_SCOPE,
                                GROUP_MESSAGE_WATERMARK_SCOPE,
                                DIRECT_REACTION_WATERMARK_SCOPE,
                                GROUP_REACTION_WATERMARK_SCOPE,
                            ]
                        } else {
                            &[scope]
                        };
                        for target_scope in scopes {
                            if let Err(e) = self.repository.update_checkpoint(
                                account_id,
                                target_scope,
                                None,
                                None,
                                Some("Reconcile required"),
                            ) {
                                tracing::warn!("Failed to set reconcile checkpoint: {}", e);
                            }
                        }
                    }
                    self.events.publish(ConversationInvalidatedEvent {
                        conversation_id: 0,
                        lead_id: None,
                        channel: Channel::Zalo,
                        reason: ConversationInvalidationReason::Metadata,
                        message_id: None,
                    });
                }
            }
        }
        Ok(())
    }

    fn publish_message_invalidation(
        &self,
        message_id: i64,
        reason: ConversationInvalidationReason,
    ) -> Result<()> {
        if let Some((conversation_id, lead_id, channel)) = self
            .repository
            .lookup_message_invalidation_info(message_id)?
        {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id,
                lead_id,
                channel,
                reason,
                message_id: Some(message_id),
            });
        }
        Ok(())
    }

    pub fn apply_group_snapshot(&self, snapshot: &NormalizedGroupSnapshot) -> Result<i64> {
        let conversation_id = self.repository.apply_group_snapshot(snapshot)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id,
            lead_id: None,
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Metadata,
            message_id: None,
        });
        Ok(conversation_id)
    }

    pub fn publish_outreach_success(&self, applied: &AppliedOutreachMessage) {
        if applied.applied.duplicate {
            return;
        }
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id: applied.applied.conversation_id,
            lead_id: Some(applied.lead_id),
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Message,
            message_id: Some(applied.applied.message_id),
        });
    }

    pub fn runtime_status(&self) -> Result<ConversationRuntimeStatus> {
        Ok(self.repository.get_runtime_status()?)
    }

    pub fn enqueue_batch(
        &self,
        request: &super::types::SendConversationBatchRequest,
    ) -> Result<super::types::SendConversationBatchResult, super::error::ConversationError> {
        let now = crate::conversation::types::utc_now();
        let res = self.repository.enqueue_outbound_batch(request, &now)?;
        for msg in &res.messages {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id: msg.conversation_id,
                lead_id: None,
                channel: msg.channel,
                reason: ConversationInvalidationReason::Message,
                message_id: Some(msg.id),
            });
        }
        Ok(res)
    }

    pub fn enqueue_message(
        &self,
        request: &super::types::SendConversationMessageRequest,
    ) -> Result<super::types::ConversationMessage> {
        if request.text.trim().is_empty() {
            anyhow::bail!("message text must not be empty");
        }
        let batch_req = super::types::SendConversationBatchRequest {
            conversation_id: request.conversation_id,
            client_batch_id: format!("msg-{}", rand::random::<u128>()),
            reply_to_message_id: request.reply_to_message_id,
            parts: vec![super::types::ConversationSendPartRequest {
                client_part_id: "part-0".to_string(),
                content: super::types::ConversationSendPartContent::Text {
                    text: request.text.clone(),
                },
            }],
        };
        let res = self
            .enqueue_batch(&batch_req)
            .map_err(|e| anyhow::anyhow!(e.message))?;
        Ok(res.messages.into_iter().next().unwrap())
    }

    pub fn recover_outbox_startup(&self) -> Result<u64> {
        let now = crate::conversation::types::utc_now();
        let count = self.repository.mark_running_jobs_delivery_unknown(&now)?;
        Ok(count)
    }

    pub async fn sync_identity_profiles_for_account(
        &self,
        account_id: i64,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
    ) -> Result<usize> {
        let items = self
            .repository
            .get_zalo_identities_needing_profile_sync(account_id, 50)?;
        if items.is_empty() {
            return Ok(0);
        }

        let ids: Vec<i64> = items.iter().map(|(id, _)| *id).collect();
        let uids: Vec<String> = items.iter().map(|(_, uid)| uid.clone()).collect();

        match client.fetch_user_profiles(&uids).await {
            Ok(profiles) => {
                let updated = self
                    .repository
                    .mark_identities_profile_synced(&ids, &profiles)?;
                if updated > 0 {
                    self.events.publish(ConversationInvalidatedEvent {
                        conversation_id: 0,
                        lead_id: None,
                        channel: Channel::Zalo,
                        reason: ConversationInvalidationReason::Metadata,
                        message_id: None,
                    });
                }
                Ok(updated)
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.repository
                    .mark_identity_profile_sync_failed(&ids, &error_msg)?;
                Err(e)
            }
        }
    }

    pub async fn process_next_outbox_job(
        &self,
        channel_account_id: i64,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
    ) -> Result<bool> {
        let now = crate::conversation::types::utc_now();
        let job = match self
            .repository
            .claim_next_outbox_job(channel_account_id, &now)?
        {
            Some(job) => job,
            None => return Ok(false),
        };

        let thread_type = if job.is_group {
            zca_rs::models::ThreadType::Group
        } else {
            zca_rs::models::ThreadType::User
        };

        let target_thread_id = if !job.is_group {
            if let Some(phone) = normalized_vn_mobile_phone(&job.external_thread_id) {
                match client.find_user_by_phone(&phone).await {
                    Ok(Some(profile)) if !profile.external_id.trim().is_empty() => {
                        let resolved = self.repository.resolve_direct_phone_recipient(
                            job.message_id,
                            job.channel_account_id,
                            &phone,
                            &profile.external_id,
                            profile.display_name.as_deref(),
                            profile.avatar_url.as_deref(),
                            &now,
                        )?;
                        resolved.external_thread_id
                    }
                    Ok(Some(_)) => {
                        let err_msg =
                            format!("Zalo phone lookup returned an empty user ID for {phone}");
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::ProviderFailure,
                            &err_msg,
                            &now,
                        )?;
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                        return Ok(true);
                    }
                    Ok(None) => {
                        let err_msg = format!("Phone number {phone} was not found on Zalo");
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::ProviderFailure,
                            &err_msg,
                            &now,
                        )?;
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                        return Ok(true);
                    }
                    Err(err) => {
                        let err_msg = err.to_string();
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::ProviderFailure,
                            &err_msg,
                            &now,
                        )?;
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                        return Ok(true);
                    }
                }
            } else {
                job.external_thread_id.clone()
            }
        } else {
            job.external_thread_id.clone()
        };

        let content_result: Result<crate::conversation::zalo::client::ProviderOutboundContent> =
            match job.content_type.as_str() {
                "text" => Ok(
                    crate::conversation::zalo::client::ProviderOutboundContent::Text {
                        text: job.text_content.clone().unwrap_or_default(),
                    },
                ),
                "sticker" => {
                    let sticker_id = job
                        .content_json
                        .get("stickerId")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let cate_id = job
                        .content_json
                        .get("cateId")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let sticker_type = job
                        .content_json
                        .get("type")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    if sticker_id == 0 {
                        Err(anyhow::anyhow!("Invalid sticker payload missing stickerId"))
                    } else {
                        Ok(
                            crate::conversation::zalo::client::ProviderOutboundContent::Sticker {
                                sticker_id,
                                cate_id,
                                sticker_type,
                            },
                        )
                    }
                }
                "attachment" | "image" | "file" | "audio" | "video" => {
                    if let Some(att) = &job.attachment {
                        if let Some(path) = &att.local_path {
                            match std::fs::read(path) {
                            Ok(data) => Ok(crate::conversation::zalo::client::ProviderOutboundContent::Attachment {
                                media_type: att.media_type.clone(),
                                mime_type: att.mime_type.clone(),
                                file_name: att.file_name.clone().unwrap_or_else(|| "attachment".into()),
                                size_bytes: data.len() as u64,
                                data,
                            }),
                            Err(e) => Err(anyhow::anyhow!("Failed to read attachment file at {}: {}", path, e)),
                        }
                        } else {
                            Err(anyhow::anyhow!("Attachment job missing local_path"))
                        }
                    } else {
                        Err(anyhow::anyhow!("Attachment job missing attachment record"))
                    }
                }
                other => Err(anyhow::anyhow!("Unsupported content_type {}", other)),
            };

        let content = match content_result {
            Ok(c) => c,
            Err(e) => {
                let err_msg = e.to_string();
                self.repository.fail_outbox_job(
                    &job.job_id,
                    crate::conversation::error::ConversationErrorCode::InvalidRequest,
                    &err_msg,
                    &now,
                )?;
                self.publish_message_invalidation(
                    job.message_id,
                    ConversationInvalidationReason::Message,
                )?;
                return Ok(true);
            }
        };

        let provider_req = crate::conversation::zalo::client::ProviderSendRequest {
            external_thread_id: target_thread_id,
            thread_type,
            provider_client_message_id: job.provider_client_message_id.clone(),
            content,
            quote: job.quote.clone(),
        };

        let retry_policy = crate::conversation::retry::RetryPolicy::default();

        match client.send_part(provider_req).await {
            Ok(res) => {
                let message_id = self.repository.complete_outbox_job(
                    &job.job_id,
                    &res.provider_message_id,
                    &res.provider_client_message_id,
                    &now,
                )?;
                self.publish_message_invalidation(
                    message_id,
                    ConversationInvalidationReason::Message,
                )?;
                Ok(true)
            }
            Err(err) => {
                let err_msg = err.message.clone();
                match err.kind {
                    crate::conversation::zalo::client::ProviderSendErrorKind::Authentication => {
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::ProviderAuth,
                            &err_msg,
                            &now,
                        )?;
                        let _ = self.repository.set_account_status(
                            &job.provider_account_key,
                            crate::conversation::types::AccountConnectionStatus::Invalid,
                            Some(&err_msg),
                        );
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                    }
                    crate::conversation::zalo::client::ProviderSendErrorKind::Permanent => {
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::ProviderFailure,
                            &err_msg,
                            &now,
                        )?;
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                    }
                    crate::conversation::zalo::client::ProviderSendErrorKind::RateLimited
                    | crate::conversation::zalo::client::ProviderSendErrorKind::Retryable => {
                        if retry_policy.should_stop(job.attempt_count) {
                            self.repository.fail_outbox_job(
                                &job.job_id,
                                crate::conversation::error::ConversationErrorCode::ProviderFailure,
                                &err_msg,
                                &now,
                            )?;
                        } else {
                            let delay =
                                retry_policy.delay(job.attempt_count, rand::random::<f64>());
                            let next_at = (chrono::Utc::now()
                                + chrono::Duration::from_std(delay)
                                    .unwrap_or(chrono::Duration::seconds(5)))
                            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                            self.repository.schedule_outbox_retry(
                                &job.job_id,
                                &next_at,
                                crate::conversation::error::ConversationErrorCode::ProviderFailure,
                                &err_msg,
                                &now,
                            )?;
                        }
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                    }
                }
                Ok(true)
            }
        }
    }
}
