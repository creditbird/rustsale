use super::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
use super::repository::{ConversationRepository, ResolveDirectPhoneRecipientRequest};
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

use super::error::ConversationResult;
use super::quick_templates::QuickTemplateRepository;
use super::staging::ConversationAttachmentStager;
use super::types::{
    ConversationMessage, ConversationQuickTemplate, CreateConversationQuickTemplateRequest,
    DeleteConversationQuickTemplateRequest, ForwardConversationMessageRequest,
    ForwardConversationMessageResult, ListConversationQuickTemplatesRequest,
    MarkConversationQuickTemplateUsedRequest, MessageDirection, MessageStatus,
    UpdateConversationQuickTemplateRequest,
};

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
    pub quick_templates: QuickTemplateRepository,
}

impl ConversationService {
    pub fn new(
        repository: Arc<ConversationRepository>,
        events: Arc<ConversationEventPublisher>,
        spool_root: std::path::PathBuf,
    ) -> Self {
        let stager = ConversationAttachmentStager::new(spool_root, repository.clone());
        let quick_templates = QuickTemplateRepository::new(repository.db_path());
        Self {
            repository,
            events,
            stager,
            quick_templates,
        }
    }

    pub fn list_quick_templates(
        &self,
        request: ListConversationQuickTemplatesRequest,
    ) -> ConversationResult<Vec<ConversationQuickTemplate>> {
        if let Some(query) = request
            .search_query
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            let templates = self.quick_templates.search(query, 500)?;
            if let Some(category) = request.category.as_deref().filter(|c| !c.trim().is_empty()) {
                let cat_trimmed = category.trim();
                Ok(templates
                    .into_iter()
                    .filter(|t| {
                        t.category
                            .as_deref()
                            .map(|c| c.eq_ignore_ascii_case(cat_trimmed))
                            .unwrap_or(false)
                    })
                    .collect())
            } else {
                Ok(templates)
            }
        } else {
            let templates = self.quick_templates.list(request.include_inactive)?;
            if let Some(category) = request.category.as_deref().filter(|c| !c.trim().is_empty()) {
                let cat_trimmed = category.trim();
                Ok(templates
                    .into_iter()
                    .filter(|t| {
                        t.category
                            .as_deref()
                            .map(|c| c.eq_ignore_ascii_case(cat_trimmed))
                            .unwrap_or(false)
                    })
                    .collect())
            } else {
                Ok(templates)
            }
        }
    }

    pub fn create_quick_template(
        &self,
        request: CreateConversationQuickTemplateRequest,
    ) -> ConversationResult<ConversationQuickTemplate> {
        self.quick_templates.create(request)
    }

    pub fn update_quick_template(
        &self,
        request: UpdateConversationQuickTemplateRequest,
    ) -> ConversationResult<ConversationQuickTemplate> {
        self.quick_templates.update(request)
    }

    pub fn delete_quick_template(
        &self,
        request: DeleteConversationQuickTemplateRequest,
    ) -> ConversationResult<()> {
        self.quick_templates.delete(request.id)
    }

    pub fn mark_quick_template_used(
        &self,
        request: MarkConversationQuickTemplateUsedRequest,
    ) -> ConversationResult<()> {
        self.quick_templates.mark_used(request.id).map(|_| ())
    }

    pub fn forward_conversation_message(
        &self,
        request: ForwardConversationMessageRequest,
    ) -> ConversationResult<ForwardConversationMessageResult> {
        self.repository.forward_conversation_message(&request)
    }

    pub fn get_forward_operation(
        &self,
        client_operation_id: &str,
    ) -> ConversationResult<ForwardConversationMessageResult> {
        self.repository.get_forward_operation(client_operation_id)
    }

    pub fn retry_forward_target(
        &self,
        client_operation_id: &str,
        target_conversation_id: i64,
    ) -> ConversationResult<super::types::ConversationForwardTargetResult> {
        self.repository
            .retry_forward_target(client_operation_id, target_conversation_id)
    }

    pub fn list_mention_candidates(
        &self,
        conversation_id: i64,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> ConversationResult<Vec<super::types::ConversationMentionCandidate>> {
        self.repository
            .list_mention_candidates(conversation_id, query, limit)
            .map_err(|e| e.into())
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

    pub fn mark_conversation_read(&self, key: &str) -> Result<u64> {
        Ok(self.repository.mark_conversation_read(key)?)
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
            let _ =
                self.publish_message_invalidation(msg.id, ConversationInvalidationReason::Message);
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
                    mentions: Vec::new(),
                },
            }],
        };
        let res = self
            .enqueue_batch(&batch_req)
            .map_err(|e| anyhow::anyhow!(e.message))?;
        Ok(res.messages.into_iter().next().unwrap())
    }

    pub fn recover_startup(&self) -> Result<u64> {
        let conn = crate::db::open_db_connection(self.repository.db_path())?;
        crate::conversation::migration::migrate(&conn)?;
        let now = crate::conversation::types::utc_now();
        let reaction_repo = crate::conversation::reactions::ReactionMutationRepository::new(
            self.repository.db_path(),
        );
        let _ = reaction_repo.mark_running_delivery_unknown(&now);
        let count = self.repository.mark_running_jobs_delivery_unknown(&now)?;
        Ok(count)
    }

    pub fn recover_outbox_startup(&self) -> Result<u64> {
        self.recover_startup()
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
                            ResolveDirectPhoneRecipientRequest {
                                message_id: job.message_id,
                                channel_account_id: job.channel_account_id,
                                phone: &phone,
                                resolved_uid: &profile.external_id,
                                display_name: profile.display_name.as_deref(),
                                avatar_url: profile.avatar_url.as_deref(),
                                now: &now,
                            },
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

        if job.content_type.as_str() == "forward" {
            let source_message_id = job
                .content_json
                .get("sourceMessageId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let client_operation_id = job
                .content_json
                .get("clientOperationId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let source = self.repository.get_message_by_id(source_message_id)?;
            let source_p_msg_id = job
                .content_json
                .get("sourceProviderMessageId")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .or_else(|| {
                    source
                        .as_ref()
                        .and_then(|message| message.provider_message_id.clone())
                });
            let source_message = job
                .content_json
                .get("sourceMessage")
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(str::to_string)
                .or_else(|| {
                    source
                        .as_ref()
                        .and_then(|message| message.text_content.clone())
                });
            let source_timestamp_ms = job
                .content_json
                .get("sourceTimestampMs")
                .and_then(|value| value.as_u64())
                .or_else(|| {
                    source.as_ref().and_then(|message| {
                        chrono::DateTime::parse_from_rfc3339(&message.occurred_at)
                            .ok()
                            .map(|timestamp| timestamp.timestamp_millis())
                            .filter(|timestamp| *timestamp > 0)
                            .map(|timestamp| timestamp as u64)
                    })
                });

            let (source_p_msg_id, source_message, source_timestamp_ms) = match (
                source_p_msg_id.filter(|value| !value.trim().is_empty()),
                source_message.filter(|value| !value.trim().is_empty()),
                source_timestamp_ms.filter(|value| *value > 0),
            ) {
                (Some(provider_message_id), Some(message), Some(timestamp_ms)) => {
                    (provider_message_id, message, timestamp_ms)
                }
                _ => {
                    let err_msg = format!(
                        "Source message {} is missing native Forward metadata",
                        source_message_id
                    );
                    self.repository.fail_outbox_job(
                        &job.job_id,
                        crate::conversation::error::ConversationErrorCode::ProviderFailure,
                        &err_msg,
                        &now,
                    )?;
                    let _ = self.repository.update_forward_target_status(
                        &client_operation_id,
                        job.conversation_id,
                        crate::conversation::types::ForwardTargetStatus::Failed,
                        Some("provider_failure"),
                        Some(&err_msg),
                        &now,
                    );
                    self.publish_message_invalidation(
                        job.message_id,
                        ConversationInvalidationReason::Message,
                    )?;
                    return Ok(true);
                }
            };

            let _ = self.repository.update_forward_target_status(
                &client_operation_id,
                job.conversation_id,
                crate::conversation::types::ForwardTargetStatus::Sending,
                None,
                None,
                &now,
            );

            let provider_req = crate::conversation::zalo::client::ProviderForwardRequest {
                external_thread_id: target_thread_id,
                thread_type,
                provider_client_message_id: job.provider_client_message_id.clone(),
                source_provider_message_id: source_p_msg_id,
                source_message,
                source_timestamp_ms,
            };

            let retry_policy = crate::conversation::retry::RetryPolicy::default();

            match client.forward_message(provider_req).await {
                Ok(res) => {
                    let message_id = self.repository.complete_outbox_job(
                        &job.job_id,
                        &res.provider_message_id,
                        &res.provider_client_message_id,
                        &now,
                    )?;

                    let _ = self.repository.update_forward_target_status(
                        &client_operation_id,
                        job.conversation_id,
                        crate::conversation::types::ForwardTargetStatus::Sent,
                        None,
                        None,
                        &now,
                    );

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
                            let _ = self.repository.update_forward_target_status(
                                &client_operation_id,
                                job.conversation_id,
                                crate::conversation::types::ForwardTargetStatus::Failed,
                                Some("provider_auth"),
                                Some(&err_msg),
                                &now,
                            );
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
                            let _ = self.repository.update_forward_target_status(
                                &client_operation_id,
                                job.conversation_id,
                                crate::conversation::types::ForwardTargetStatus::Failed,
                                Some("provider_failure"),
                                Some(&err_msg),
                                &now,
                            );
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
                                let _ = self.repository.update_forward_target_status(
                                    &client_operation_id,
                                    job.conversation_id,
                                    crate::conversation::types::ForwardTargetStatus::Failed,
                                    Some("provider_failure"),
                                    Some(&err_msg),
                                    &now,
                                );
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
        } else {
            let content_result: Result<crate::conversation::zalo::client::ProviderOutboundContent> =
                match job.content_type.as_str() {
                    "text" => {
                        let mentions = self
                            .repository
                            .get_message_mentions_for_provider(job.message_id)
                            .map_err(|e| {
                                anyhow::anyhow!("Failed to load mentions: {}", e.message)
                            })?;
                        Ok(
                            crate::conversation::zalo::client::ProviderOutboundContent::Text {
                                text: job.text_content.clone().unwrap_or_default(),
                                mentions,
                            },
                        )
                    }
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

    pub async fn set_conversation_reaction(
        &self,
        request: &super::types::SetConversationReactionRequest,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
    ) -> ConversationResult<super::types::ConversationReactionMutationResult> {
        use super::error::{ConversationError, ConversationErrorCode};
        use super::projection::load_message_reaction_groups;
        use super::reactions::ReactionMutationRepository;
        use super::types::{
            ConversationCapabilities, ConversationReactionMutationResult, ReactionMutationStatus,
        };
        use crate::db::open_db_connection;
        use rusqlite::OptionalExtension;

        let msg = match self.repository.get_message_by_id(request.message_id)? {
            Some(m) => m,
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::InvalidRequest,
                    format!("Message {} not found", request.message_id),
                ));
            }
        };

        let (_provider_account_key, external_thread_id, is_group) = self
            .repository
            .lookup_message_thread_info(request.message_id)?;

        let conn = open_db_connection(self.repository.db_path())?;
        let (capabilities_json, self_external_id_opt): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT capabilities_json, self_external_id FROM channel_accounts WHERE id = ?1",
                rusqlite::params![msg.channel_account_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .unwrap_or((None, None));

        let capabilities = capabilities_json
            .as_deref()
            .filter(|s| !s.trim().is_empty() && s.trim() != "{}")
            .map(ConversationCapabilities::from_json)
            .unwrap_or_else(ConversationCapabilities::zalo_phase_one);

        if !capabilities.supports_reactions
            || !capabilities.reactions.write
            || !capabilities.reactions.supported.contains(&request.reaction)
        {
            return Err(ConversationError::new(
                ConversationErrorCode::ReactionNotSupported,
                format!(
                    "Reactions not supported for channel {}",
                    msg.channel.as_str()
                ),
            ));
        }

        let self_external_id = self_external_id_opt
            .or_else(|| client.own_external_id())
            .unwrap_or_else(|| "self".to_string());

        let provider_message_id = match msg
            .provider_message_id
            .clone()
            .filter(|s| !s.trim().is_empty())
        {
            Some(id) => id,
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::ReactionNotSupported,
                    format!(
                        "Message {} is missing provider message ID",
                        request.message_id
                    ),
                ));
            }
        };

        let provider_client_message_id = match msg
            .provider_client_message_id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| {
                msg.content
                    .get("cliMsgId")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(String::from)
            }) {
            Some(id) => id,
            None => {
                return Err(ConversationError::new(
                    ConversationErrorCode::ReactionNotSupported,
                    format!(
                        "Message {} is missing provider client message ID",
                        request.message_id
                    ),
                ));
            }
        };

        let thread_type = if is_group {
            zca_rs::models::ThreadType::Group
        } else {
            zca_rs::models::ThreadType::User
        };

        let reaction_mutations = ReactionMutationRepository::new(self.repository.db_path());
        let mutation = reaction_mutations.begin(request, msg.channel_account_id)?;

        if mutation.is_existing {
            let read_conn = open_db_connection(self.repository.db_path())?;
            let groups = load_message_reaction_groups(&read_conn, request.message_id)?;
            let status = match mutation.status.as_str() {
                "running" => ReactionMutationStatus::Running,
                "succeeded" => ReactionMutationStatus::Succeeded,
                "failed" => ReactionMutationStatus::Failed,
                _ => ReactionMutationStatus::DeliveryUnknown,
            };
            return Ok(ConversationReactionMutationResult {
                client_mutation_id: mutation.client_mutation_id,
                message_id: request.message_id,
                reaction: request.reaction,
                requested_active: request.active,
                status,
                reactions: groups,
                error_code: mutation.error_code,
                error_message: mutation.error_message,
            });
        }

        let provider_req = crate::conversation::zalo::client::ProviderReactionRequest {
            provider_message_id,
            provider_client_message_id,
            external_thread_id,
            thread_type,
            reaction_code: if request.active {
                Some(request.reaction)
            } else {
                None
            },
            client_mutation_id: Some(mutation.client_mutation_id.clone()),
        };

        let now = crate::conversation::types::utc_now();

        match client.set_reaction(provider_req).await {
            Ok(()) => {
                let groups = reaction_mutations.succeed(
                    &mutation.client_mutation_id,
                    &self_external_id,
                    &now,
                )?;
                self.publish_message_invalidation(
                    request.message_id,
                    super::events::ConversationInvalidationReason::Reaction,
                )?;
                Ok(ConversationReactionMutationResult {
                    client_mutation_id: mutation.client_mutation_id,
                    message_id: request.message_id,
                    reaction: request.reaction,
                    requested_active: request.active,
                    status: ReactionMutationStatus::Succeeded,
                    reactions: groups,
                    error_code: None,
                    error_message: None,
                })
            }
            Err(err) => {
                use crate::conversation::zalo::client::ProviderSendErrorKind;
                let (is_unknown, mutation_status) = match err.kind {
                    ProviderSendErrorKind::Retryable | ProviderSendErrorKind::RateLimited => {
                        (true, ReactionMutationStatus::DeliveryUnknown)
                    }
                    ProviderSendErrorKind::Authentication | ProviderSendErrorKind::Permanent => {
                        (false, ReactionMutationStatus::Failed)
                    }
                };

                let error_code = format!("{:?}", err.kind);
                reaction_mutations.fail(
                    &mutation.client_mutation_id,
                    &error_code,
                    &err.message,
                    is_unknown,
                )?;

                if is_unknown {
                    let _ = client.request_old_reactions(thread_type, None);
                }

                let read_conn = open_db_connection(self.repository.db_path())?;
                let current_groups = load_message_reaction_groups(&read_conn, request.message_id)?;

                Ok(ConversationReactionMutationResult {
                    client_mutation_id: mutation.client_mutation_id,
                    message_id: request.message_id,
                    reaction: request.reaction,
                    requested_active: request.active,
                    status: mutation_status,
                    reactions: current_groups,
                    error_code: Some(error_code),
                    error_message: Some(err.message),
                })
            }
        }
    }

    pub fn pin_message(
        &self,
        conversation_id: i64,
        message_id: i64,
        pinned_by: Option<&str>,
    ) -> Result<crate::conversation::types::PinnedMessage> {
        Ok(self
            .repository
            .pin_message(conversation_id, message_id, pinned_by)?)
    }

    pub fn unpin_message(&self, conversation_id: i64, message_id: i64) -> Result<()> {
        Ok(self.repository.unpin_message(conversation_id, message_id)?)
    }

    pub fn list_pinned_messages(
        &self,
        conversation_id: i64,
    ) -> Result<Vec<crate::conversation::types::PinnedMessage>> {
        Ok(self.repository.list_pinned_messages(conversation_id)?)
    }
}
