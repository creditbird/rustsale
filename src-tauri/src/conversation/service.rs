use super::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
use super::repository::{
    ConversationRepository, GroupMemberLeadImportRequest, GroupMemberLeadImportResult,
    ResolveDirectPhoneRecipientRequest,
};
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
        source_message_id: Option<i64>,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> ConversationResult<Vec<super::types::ConversationMentionCandidate>> {
        self.repository
            .list_mention_candidates(conversation_id, source_message_id, query, limit)
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
            .provider_client_message_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .or_else(|| {
                msg.content
                    .get("cliMsgId")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
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

    pub fn import_group_members_to_leads(
        &self,
        request: &GroupMemberLeadImportRequest,
    ) -> Result<GroupMemberLeadImportResult> {
        let result = self.repository.import_group_members_to_leads(request)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id: request.conversation_id,
            lead_id: None,
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Link,
            message_id: None,
        });
        Ok(result)
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
        self.repository
            .delete_conversation(request, Some(self.stager.root()))?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id: request.conversation_id.unwrap_or(0),
            lead_id: request.lead_id,
            channel: Channel::Zalo,
            reason: ConversationInvalidationReason::Link,
            message_id: None,
        });
        Ok(())
    }

    pub fn delete_conversations_batch(
        &self,
        requests: &[super::types::DeleteConversationRequest],
    ) -> Result<()> {
        self.repository
            .delete_conversations_batch(requests, Some(self.stager.root()))?;
        for request in requests {
            self.events.publish(ConversationInvalidatedEvent {
                conversation_id: request.conversation_id.unwrap_or(0),
                lead_id: request.lead_id,
                channel: Channel::Zalo,
                reason: ConversationInvalidationReason::Link,
                message_id: None,
            });
        }
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
                    } else if applied.replayed_reaction_count > 0 {
                        self.publish_message_invalidation(
                            applied.message_id,
                            ConversationInvalidationReason::Reaction,
                        )?;
                    }
                }
                ZaloMutation::Reaction(reaction) => {
                    use crate::conversation::reactions::canonical_reaction_code;
                    let reaction_str = reaction.reaction.as_str();
                    let canonical = canonical_reaction_code(reaction_str).unwrap_or(reaction_str);
                    let account_id = self
                        .repository
                        .get_channel_account_id(provider_account_key)
                        .ok()
                        .flatten();

                    let is_echo = if !reaction.removed {
                        account_id.is_some_and(|account_id| {
                            let cache =
                                crate::conversation::echo_cache::ReactionEchoCache::global();
                            let provider_message_match = cache.consume_if_expected(
                                account_id,
                                &reaction.provider_message_id,
                                canonical,
                                &reaction.actor_external_id,
                            );
                            let provider_client_message_match =
                                !reaction.provider_client_message_id.is_empty()
                                    && cache.consume_if_expected(
                                        account_id,
                                        &reaction.provider_client_message_id,
                                        canonical,
                                        &reaction.actor_external_id,
                                    );
                            provider_message_match || provider_client_message_match
                        })
                    } else {
                        false
                    };

                    if !is_echo {
                        if let Some(message_id) = self.repository.apply_reaction(&reaction)? {
                            self.publish_message_invalidation(
                                message_id,
                                ConversationInvalidationReason::Reaction,
                            )?;
                        }
                    }
                    if let Some(account_id) = account_id {
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
            message_tag: None,
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
        self.repository
            .cleanup_spool_files(&[], Some(self.stager.root()));
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
    pub async fn process_next_messenger_outbox_job(
        &self,
        channel_account_id: i64,
        client: &dyn crate::conversation::facebook::client::FacebookGraphClient,
        page_id: &str,
        page_token: &str,
    ) -> Result<bool> {
        let now = crate::conversation::types::utc_now();
        let job = match self
            .repository
            .claim_next_outbox_job(channel_account_id, &now)?
        {
            Some(job) => job,
            None => return Ok(false),
        };

        let recipient_psid = {
            let conn_res = rusqlite::Connection::open(self.repository.db_path());
            let psid_opt = conn_res.ok().and_then(|conn| {
                conn.query_row(
                    "SELECT ci.external_id
                     FROM conversation_participants cp
                     JOIN channel_identities ci ON ci.id = cp.identity_id
                     WHERE cp.conversation_id = ?1 AND ci.channel = 'messenger' AND ci.external_id != ?2
                     ORDER BY cp.joined_at ASC
                     LIMIT 1",
                    rusqlite::params![job.conversation_id, page_id],
                    |r| r.get::<_, String>(0),
                )
                .ok()
            });

            match psid_opt {
                Some(p) if !p.trim().is_empty() => p,
                _ => {
                    if job.external_thread_id.starts_with("t_") {
                        job.external_thread_id[2..].to_string()
                    } else {
                        job.external_thread_id.clone()
                    }
                }
            }
        };

        let message_tag = job
            .content_json
            .get("messageTag")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty());

        let text = job.text_content.as_deref().unwrap_or("");

        match client
            .send_messenger_message(page_id, page_token, &recipient_psid, text, message_tag)
            .await
        {
            Ok(resp) => {
                self.repository
                    .complete_outbox_job(&job.job_id, &resp.message_id, "", &now)?;
                self.publish_message_invalidation(
                    job.message_id,
                    ConversationInvalidationReason::Message,
                )?;
                Ok(true)
            }
            Err(err) => {
                let err_msg = err.to_string();
                let is_auth_failure = if let Some(meta_err) =
                    err.downcast_ref::<crate::conversation::facebook::error::MetaGraphError>()
                {
                    meta_err.is_auth_failure()
                } else {
                    let msg = err.to_string();
                    msg.contains("code 190")
                        || msg.contains("code 102")
                        || msg.contains("status 401")
                        || msg.contains("Error validating access token")
                        || msg.contains("Session has expired")
                        || msg.contains("The access token could not be decrypted")
                };

                let error_code = if is_auth_failure {
                    crate::conversation::error::ConversationErrorCode::ProviderAuth
                } else {
                    crate::conversation::error::ConversationErrorCode::ProviderFailure
                };

                self.repository
                    .fail_outbox_job(&job.job_id, error_code, &err_msg, &now)?;
                self.publish_message_invalidation(
                    job.message_id,
                    ConversationInvalidationReason::Message,
                )?;

                if is_auth_failure {
                    if let Ok(mut conn) = rusqlite::Connection::open(self.repository.db_path()) {
                        if let Ok(tx) = conn.transaction() {
                            let _ = tx.execute(
                                "UPDATE channel_accounts SET connection_status = 'invalid', last_error = ?1, updated_at = datetime('now') WHERE id = ?2",
                                rusqlite::params![err_msg, channel_account_id],
                            );
                            let _ = tx.execute(
                                "UPDATE facebook_page_tokens SET token_status = 'expired', updated_at = datetime('now') WHERE channel_account_id = ?1",
                                rusqlite::params![channel_account_id],
                            );
                            let _ = tx.commit();
                        }
                    }
                    self.events.publish(ConversationInvalidatedEvent {
                        conversation_id: job.conversation_id,
                        lead_id: None,
                        channel: Channel::Messenger,
                        reason: ConversationInvalidationReason::AccountStatus,
                        message_id: None,
                    });
                    // Circuit breaker: halt draining remaining jobs for this account
                    return Ok(false);
                }

                Ok(true)
            }
        }
    }

    pub async fn process_next_whatsapp_outbox_job(
        &self,
        channel_account_id: i64,
        client: &dyn crate::conversation::whatsapp::client::WhatsAppClient,
    ) -> Result<bool> {
        let now = crate::conversation::types::utc_now();
        let job = match self
            .repository
            .claim_next_outbox_job(channel_account_id, &now)?
        {
            Some(job) => job,
            None => return Ok(false),
        };

        // 1. Resolve recipient JID
        let recipient_jid = if job.external_thread_id.contains('@') {
            job.external_thread_id.clone()
        } else {
            crate::conversation::whatsapp::phone::phone_to_direct_jid(&job.external_thread_id)
        };

        // 2. Resolve quote reference if replying
        let quote_opt = if let Some(ref q) = job.quote {
            let stanza_id = if !q.provider_message_id.is_empty() {
                q.provider_message_id.clone()
            } else {
                q.provider_client_message_id.clone()
            };
            let participant = if !q.sender_external_id.is_empty() {
                if q.sender_external_id.contains('@') {
                    q.sender_external_id.clone()
                } else {
                    crate::conversation::whatsapp::phone::phone_to_direct_jid(&q.sender_external_id)
                }
            } else {
                recipient_jid.clone()
            };
            Some(
                crate::conversation::whatsapp::normalizer::RawWhatsAppQuote {
                    stanza_id,
                    participant_jid: participant,
                    text: Some(q.content.clone()),
                    media_type: None,
                    timestamp: Some(q.provider_timestamp_ms.to_string()),
                },
            )
        } else {
            let conn_res = rusqlite::Connection::open(self.repository.db_path());
            let parent_info = conn_res.ok().and_then(|conn| {
                conn.query_row(
                    "SELECT parent.provider_message_id, parent.provider_client_message_id, parent.text_content, parent.content_type, COALESCE(parent.sent_at, parent.ingested_at)
                     FROM messages m
                     JOIN messages parent ON parent.id = m.reply_to_message_id
                     WHERE m.id = ?1",
                    rusqlite::params![job.message_id],
                    |r| {
                        Ok((
                            r.get::<_, Option<String>>(0)?,
                            r.get::<_, Option<String>>(1)?,
                            r.get::<_, Option<String>>(2)?,
                            r.get::<_, String>(3)?,
                            r.get::<_, Option<String>>(4)?,
                        ))
                    },
                ).ok()
            });

            parent_info.map(|(p_msg_id, p_cli_id, text, content_type, occurred_at)| {
                let stanza_id = p_msg_id
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| p_cli_id.filter(|s| !s.trim().is_empty()))
                    .unwrap_or_default();
                crate::conversation::whatsapp::normalizer::RawWhatsAppQuote {
                    stanza_id,
                    participant_jid: recipient_jid.clone(),
                    text,
                    media_type: if content_type != "text" {
                        Some(content_type)
                    } else {
                        None
                    },
                    timestamp: occurred_at,
                }
            })
        };

        // 3. Dispatch based on content_type
        let dispatch_result = match job.content_type.as_str() {
            "text" | "plain_text" => {
                let text = job.text_content.as_deref().unwrap_or("");
                client
                    .send_text_message(&recipient_jid, text, quote_opt.as_ref())
                    .await
            }
            "attachment" | "media" | "image" | "video" | "audio" | "file" | "document"
            | "sticker" => {
                let media_url = job
                    .content_json
                    .get("url")
                    .or_else(|| job.content_json.get("file_url"))
                    .or_else(|| job.content_json.get("path"))
                    .or_else(|| job.content_json.get("local_path"))
                    .and_then(|v| v.as_str())
                    .or_else(|| {
                        job.attachment
                            .as_ref()
                            .and_then(|a| a.local_path.as_deref())
                    })
                    .unwrap_or("");
                let media_type = job
                    .attachment
                    .as_ref()
                    .map(|a| a.media_type.as_str())
                    .unwrap_or(match job.content_type.as_str() {
                        "attachment" => "document",
                        other => other,
                    });
                let caption = job
                    .text_content
                    .as_deref()
                    .or_else(|| job.content_json.get("caption").and_then(|v| v.as_str()));
                client
                    .send_media_message(
                        &recipient_jid,
                        media_type,
                        media_url,
                        caption,
                        quote_opt.as_ref(),
                    )
                    .await
            }
            "reaction" => {
                let emoji_raw = job
                    .content_json
                    .get("emoji")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let emoji = crate::conversation::types::normalize_reaction_emoji(emoji_raw);
                let target_msg_id = job
                    .content_json
                    .get("target_message_id")
                    .or_else(|| job.content_json.get("targetMessageId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&job.external_thread_id);
                let target_sender = job
                    .content_json
                    .get("target_sender_jid")
                    .or_else(|| job.content_json.get("targetSenderJid"))
                    .and_then(|v| v.as_str());
                let target_is_from_me = job
                    .content_json
                    .get("target_is_from_me")
                    .or_else(|| job.content_json.get("targetIsFromMe"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                match client
                    .send_reaction(&recipient_jid, target_msg_id, target_sender, emoji, target_is_from_me)
                    .await
                {
                    Ok(()) => Ok(
                        crate::conversation::whatsapp::client::WhatsAppSendResponse {
                            provider_message_id: format!("react-{}", job.job_id),
                            timestamp: Some(now.clone()),
                            media_metadata: None,
                        },
                    ),
                    Err(e) => Err(e),
                }
            }
            "forward" => {
                let fwd_text = job
                    .content_json
                    .get("text")
                    .and_then(|v| v.as_str())
                    .or(job.text_content.as_deref())
                    .unwrap_or("");
                let is_media = job
                    .content_json
                    .get("mediaType")
                    .or_else(|| job.content_json.get("media_type"))
                    .is_some();
                if is_media {
                    let media_type = job
                        .content_json
                        .get("mediaType")
                        .or_else(|| job.content_json.get("media_type"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("image");
                    let media_url = job
                        .content_json
                        .get("url")
                        .or_else(|| job.content_json.get("file_url"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let caption = job
                        .content_json
                        .get("caption")
                        .and_then(|v| v.as_str())
                        .or(job.text_content.as_deref());
                    client
                        .send_media_message(&recipient_jid, media_type, media_url, caption, None)
                        .await
                } else {
                    client
                        .send_text_message(&recipient_jid, fwd_text, None)
                        .await
                }
            }
            _ => {
                if let Some(ref text) = job.text_content {
                    client
                        .send_text_message(&recipient_jid, text, quote_opt.as_ref())
                        .await
                } else {
                    Err(anyhow::anyhow!(
                        "Unsupported content type for WhatsApp: {}",
                        job.content_type
                    ))
                }
            }
        };

        match dispatch_result {
            Ok(send_res) => {
                if let Some(ref meta) = send_res.media_metadata {
                    if let Ok(conn) = crate::db::open_db_connection(self.repository.db_path()) {
                        let meta_json = serde_json::json!({
                            "direct_path": meta.direct_path,
                            "media_key": meta.media_key,
                            "file_sha256": meta.file_sha256,
                            "file_enc_sha256": meta.file_enc_sha256,
                            "file_length": meta.file_length,
                        })
                        .to_string();

                        let _ = conn.execute(
                            "UPDATE message_attachments
                             SET remote_url = COALESCE(remote_url, ?1),
                                 metadata_json = ?2
                             WHERE message_id = ?3",
                            rusqlite::params![meta.url, meta_json, job.message_id],
                        );
                    }
                }

                self.repository.complete_outbox_job(
                    &job.job_id,
                    &send_res.provider_message_id,
                    &job.provider_client_message_id,
                    &now,
                )?;
                self.publish_message_invalidation(
                    job.message_id,
                    ConversationInvalidationReason::Message,
                )?;
                Ok(true)
            }
            Err(err) => {
                let err_msg = err.to_string();
                let error_kind =
                    crate::conversation::whatsapp::client::categorize_whatsapp_error(err.as_ref());

                match error_kind {
                    crate::conversation::whatsapp::client::WhatsAppErrorKind::AuthFailure => {
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::ProviderAuth,
                            &err_msg,
                            &now,
                        )?;
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;

                        if let Ok(mut conn) = rusqlite::Connection::open(self.repository.db_path())
                        {
                            if let Ok(tx) = conn.transaction() {
                                let _ = tx.execute(
                                    "UPDATE channel_accounts SET connection_status = 'invalid', last_error = ?1, updated_at = datetime('now') WHERE id = ?2",
                                    rusqlite::params![err_msg, channel_account_id],
                                );
                                let _ = tx.execute(
                                    "UPDATE whatsapp_sessions SET is_valid = 0, last_error = ?1, updated_at = datetime('now') WHERE channel_account_id = ?2",
                                    rusqlite::params![err_msg, channel_account_id],
                                );
                                let _ = tx.commit();
                            }
                        }

                        self.events.publish(ConversationInvalidatedEvent {
                            conversation_id: job.conversation_id,
                            lead_id: None,
                            channel: Channel::WhatsApp,
                            reason: ConversationInvalidationReason::AccountStatus,
                            message_id: None,
                        });

                        // Halt queue drain on auth failure
                        Ok(false)
                    }
                    crate::conversation::whatsapp::client::WhatsAppErrorKind::InvalidRecipient => {
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

                        // Add to suppression list if valid phone extracted
                        if let Some(phone) =
                            crate::conversation::whatsapp::phone::jid_to_phone(&recipient_jid)
                        {
                            if let Ok(conn) = rusqlite::Connection::open(self.repository.db_path())
                            {
                                let _ = conn.execute(
                                    "INSERT OR IGNORE INTO contact_suppression_list (channel, phone_or_email, reason, source, created_at)
                                     VALUES ('whatsapp', ?1, 'invalid_number', 'dispatch_error', ?2)",
                                    rusqlite::params![phone, now],
                                );
                            }
                        }

                        Ok(true)
                    }
                    crate::conversation::whatsapp::client::WhatsAppErrorKind::AmbiguousTimeout => {
                        self.repository.fail_outbox_job(
                            &job.job_id,
                            crate::conversation::error::ConversationErrorCode::DeliveryUnknown,
                            &err_msg,
                            &now,
                        )?;
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                        Ok(true)
                    }
                    crate::conversation::whatsapp::client::WhatsAppErrorKind::RateLimited
                    | crate::conversation::whatsapp::client::WhatsAppErrorKind::NetworkTransient => {
                        if job.attempt_count < 3 {
                            let backoff_secs = 5 * (2_i64.pow(job.attempt_count));
                            let retry_dt =
                                chrono::Utc::now() + chrono::Duration::seconds(backoff_secs);
                            let retry_at = retry_dt.to_rfc3339();

                            self.repository.schedule_outbox_retry(
                                &job.job_id,
                                &retry_at,
                                crate::conversation::error::ConversationErrorCode::ProviderFailure,
                                &err_msg,
                                &now,
                            )?;
                        } else {
                            self.repository.fail_outbox_job(
                                &job.job_id,
                                crate::conversation::error::ConversationErrorCode::ProviderFailure,
                                &format!("Max retries exceeded: {}", err_msg),
                                &now,
                            )?;
                        }
                        self.publish_message_invalidation(
                            job.message_id,
                            ConversationInvalidationReason::Message,
                        )?;
                        Ok(true)
                    }
                    crate::conversation::whatsapp::client::WhatsAppErrorKind::Permanent => {
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
                        Ok(true)
                    }
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
        use super::reactions::{PartialReactionFailure, ReactionMutationRepository};
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

        let self_external_id = if let Some(self_external_id) =
            self_external_id_opt.filter(|value| !value.trim().is_empty())
        {
            self_external_id
        } else if let Some(self_external_id) = client
            .own_external_id()
            .filter(|value| !value.trim().is_empty())
        {
            let capabilities_json = serde_json::to_string(&capabilities).map_err(|error| {
                ConversationError::new(
                    ConversationErrorCode::ProviderFailure,
                    format!("Failed to serialize account capabilities: {error}"),
                )
            })?;
            self.repository.update_account_identity_and_capabilities(
                msg.channel_account_id,
                Some(&self_external_id),
                &capabilities_json,
                &crate::conversation::types::utc_now(),
            )?;
            self_external_id
        } else {
            return Err(ConversationError::new(
                ConversationErrorCode::InvalidRequest,
                "Zalo account is missing its own external user ID",
            ));
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
                reaction: request.reaction.clone(),
                requested_active: request.active,
                status,
                reactions: groups,
                error_code: mutation.error_code,
                error_message: mutation.error_message,
            });
        }

        let now = crate::conversation::types::utc_now();
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

        let req_reaction_str = request.reaction.as_str();
        use crate::conversation::reactions::canonical_reaction_code;
        let reaction_code_str =
            canonical_reaction_code(req_reaction_str).unwrap_or(req_reaction_str);
        let echo_cache = crate::conversation::echo_cache::ReactionEchoCache::global();
        let requested_count = request.count.unwrap_or(1).max(1);
        let provider_client_id_base =
            (chrono::Utc::now().timestamp_millis().max(0) as u64).saturating_mul(1_000);
        let mut successful_count = 0;

        for attempt in 0..requested_count {
            echo_cache.mark_expected(
                msg.channel_account_id,
                &provider_message_id,
                reaction_code_str,
                &self_external_id,
            );
            if !provider_client_message_id.is_empty()
                && provider_client_message_id != provider_message_id
            {
                echo_cache.mark_expected(
                    msg.channel_account_id,
                    &provider_client_message_id,
                    reaction_code_str,
                    &self_external_id,
                );
            }

            let provider_req = crate::conversation::zalo::client::ProviderReactionRequest {
                provider_message_id: provider_message_id.clone(),
                provider_client_message_id: provider_client_message_id.clone(),
                external_thread_id: external_thread_id.clone(),
                thread_type,
                reaction_code: if request.active {
                    Some(request.reaction.clone())
                } else {
                    None
                },
                client_mutation_id: Some(format!(
                    "mut-{}-{}",
                    provider_client_id_base.saturating_add(u64::from(attempt)),
                    mutation.client_mutation_id
                )),
            };

            match client.set_reaction(provider_req).await {
                Ok(()) => successful_count += 1,
                Err(err) => {
                    echo_cache.cancel_one_expected(
                        msg.channel_account_id,
                        &provider_message_id,
                        reaction_code_str,
                        &self_external_id,
                    );
                    if !provider_client_message_id.is_empty()
                        && provider_client_message_id != provider_message_id
                    {
                        echo_cache.cancel_one_expected(
                            msg.channel_account_id,
                            &provider_client_message_id,
                            reaction_code_str,
                            &self_external_id,
                        );
                    }

                    use crate::conversation::zalo::client::ProviderSendErrorKind;
                    let (is_unknown, mutation_status) = match err.kind {
                        ProviderSendErrorKind::Retryable | ProviderSendErrorKind::RateLimited => {
                            (true, ReactionMutationStatus::DeliveryUnknown)
                        }
                        ProviderSendErrorKind::Authentication
                        | ProviderSendErrorKind::Permanent => {
                            (false, ReactionMutationStatus::Failed)
                        }
                    };
                    let error_code = format!("{:?}", err.kind);
                    let current_groups = if successful_count > 0 {
                        reaction_mutations.fail_after_partial_success(
                            &mutation.client_mutation_id,
                            &self_external_id,
                            PartialReactionFailure {
                                occurred_at: &now,
                                successful_count,
                                code: &error_code,
                                message: &err.message,
                                unknown: is_unknown,
                            },
                        )?
                    } else {
                        reaction_mutations.fail(
                            &mutation.client_mutation_id,
                            &error_code,
                            &err.message,
                            is_unknown,
                        )?;
                        let read_conn = open_db_connection(self.repository.db_path())?;
                        load_message_reaction_groups(&read_conn, request.message_id)?
                    };

                    if successful_count > 0 {
                        self.publish_message_invalidation(
                            request.message_id,
                            super::events::ConversationInvalidationReason::Reaction,
                        )?;
                    }

                    if is_unknown {
                        let _ = client.request_old_reactions(thread_type, None);
                    }

                    return Ok(ConversationReactionMutationResult {
                        client_mutation_id: mutation.client_mutation_id,
                        message_id: request.message_id,
                        reaction: request.reaction.clone(),
                        requested_active: request.active,
                        status: mutation_status,
                        reactions: current_groups,
                        error_code: Some(error_code),
                        error_message: Some(err.message),
                    });
                }
            }
        }

        let groups =
            reaction_mutations.succeed(&mutation.client_mutation_id, &self_external_id, &now)?;
        self.publish_message_invalidation(
            request.message_id,
            super::events::ConversationInvalidationReason::Reaction,
        )?;
        Ok(ConversationReactionMutationResult {
            client_mutation_id: mutation.client_mutation_id,
            message_id: request.message_id,
            reaction: request.reaction.clone(),
            requested_active: request.active,
            status: ReactionMutationStatus::Succeeded,
            reactions: groups,
            error_code: None,
            error_message: None,
        })
    }

    pub async fn set_whatsapp_conversation_reaction(
        &self,
        request: &super::types::SetConversationReactionRequest,
        client: &dyn crate::conversation::whatsapp::client::WhatsAppClient,
    ) -> ConversationResult<super::types::ConversationReactionMutationResult> {
        use super::error::{ConversationError, ConversationErrorCode};
        use super::projection::load_message_reaction_groups;
        use super::reactions::ReactionMutationRepository;
        use super::types::{
            ConversationCapabilities, ConversationReactionCode,
            ConversationReactionMutationResult, ReactionMutationStatus,
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
            .unwrap_or_else(ConversationCapabilities::whatsapp);

        let is_custom_or_supported = capabilities.reactions.supported.contains(&request.reaction)
            || matches!(request.reaction, ConversationReactionCode::Custom(_));
        if !capabilities.supports_reactions
            || !capabilities.reactions.write
            || (!capabilities.reactions.supported.is_empty() && !is_custom_or_supported)
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
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "me".to_string());

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
                reaction: request.reaction.clone(),
                requested_active: request.active,
                status,
                reactions: groups,
                error_code: mutation.error_code,
                error_message: mutation.error_message,
            });
        }

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

        let emoji_str = if request.active {
            request.reaction.to_emoji_glyph()
        } else {
            ""
        };

        let sender_jid = if is_group {
            msg.sender_identity_id.and_then(|id| {
                let conn = open_db_connection(self.repository.db_path()).ok()?;
                conn.query_row(
                    "SELECT external_id FROM channel_identities WHERE id = ?1",
                    rusqlite::params![id],
                    |r| r.get::<_, String>(0),
                )
                .ok()
            })
        } else {
            None
        };

        let is_from_me = msg.direction == crate::conversation::types::MessageDirection::Outbound;

        let send_res = client
            .send_reaction(
                &external_thread_id,
                &provider_message_id,
                sender_jid.as_deref(),
                emoji_str,
                is_from_me,
            )
            .await;

        match send_res {
            Ok(_) => {
                let now = crate::conversation::types::utc_now();
                let groups = reaction_mutations.succeed_single_active(
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
                    reaction: request.reaction.clone(),
                    requested_active: request.active,
                    status: ReactionMutationStatus::Succeeded,
                    reactions: groups,
                    error_code: None,
                    error_message: None,
                })
            }
            Err(e) => {
                let err_msg = e.to_string();
                reaction_mutations.fail(
                    &mutation.client_mutation_id,
                    "provider_failure",
                    &err_msg,
                    false,
                )?;
                let read_conn = open_db_connection(self.repository.db_path())?;
                let groups = load_message_reaction_groups(&read_conn, request.message_id)?;
                Ok(ConversationReactionMutationResult {
                    client_mutation_id: mutation.client_mutation_id,
                    message_id: request.message_id,
                    reaction: request.reaction.clone(),
                    requested_active: request.active,
                    status: ReactionMutationStatus::Failed,
                    reactions: groups,
                    error_code: Some("provider_failure".to_string()),
                    error_message: Some(err_msg),
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
