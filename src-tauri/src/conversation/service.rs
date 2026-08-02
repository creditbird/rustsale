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

    matches!(&normalized[0..2], "03" | "05" | "07" | "08" | "09")
        .then_some(normalized)
}

#[derive(Clone)]
pub struct ConversationService {
    pub repository: Arc<ConversationRepository>,
    pub events: Arc<ConversationEventPublisher>,
}

impl ConversationService {
    pub fn new(
        repository: Arc<ConversationRepository>,
        events: Arc<ConversationEventPublisher>,
    ) -> Self {
        Self { repository, events }
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

    pub fn enqueue_message(
        &self,
        request: &super::types::SendConversationMessageRequest,
    ) -> Result<super::types::ConversationMessage> {
        if request.text.trim().is_empty() {
            anyhow::bail!("message text must not be empty");
        }
        let local_request_id = format!("msg-{}", rand::random::<u128>());
        let job_id = format!("outbox-{}", rand::random::<u128>());
        let now = crate::conversation::types::utc_now();
        let message =
            self.repository
                .enqueue_outbound_message(request, &local_request_id, &job_id, &now)?;
        self.events.publish(ConversationInvalidatedEvent {
            conversation_id: message.conversation_id,
            lead_id: None,
            channel: message.channel,
            reason: ConversationInvalidationReason::Message,
            message_id: Some(message.id),
        });
        Ok(message)
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

        let mut permanent_recipient_error = false;
        let send_result = if !job.is_group {
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
                        client
                            .send_text(&resolved.external_thread_id, thread_type, &job.text)
                            .await
                    }
                    Ok(Some(_)) => {
                        permanent_recipient_error = true;
                        Err(anyhow::anyhow!(
                            "Zalo phone lookup returned an empty user ID for {phone}"
                        ))
                    }
                    Ok(None) => {
                        permanent_recipient_error = true;
                        Err(anyhow::anyhow!(
                            "Phone number {phone} was not found on Zalo"
                        ))
                    }
                    Err(error) => Err(error),
                }
            } else {
                client
                    .send_text(&job.external_thread_id, thread_type, &job.text)
                    .await
            }
        } else {
            client
                .send_text(&job.external_thread_id, thread_type, &job.text)
                .await
        };

        match send_result {
            Ok(provider_message_id) => {
                let message_id =
                    self.repository
                        .complete_outbox_job(&job.job_id, &provider_message_id, &now)?;
                self.publish_message_invalidation(
                    message_id,
                    ConversationInvalidationReason::Message,
                )?;
                Ok(true)
            }
            Err(err) => {
                let err_msg = err.to_string();
                let is_auth_err = err_msg.to_lowercase().contains("session")
                    || err_msg.to_lowercase().contains("login")
                    || err_msg.to_lowercase().contains("auth")
                    || err_msg.to_lowercase().contains("cookie");

                const MAX_ATTEMPTS: u32 = 5;
                let should_give_up = permanent_recipient_error
                    || is_auth_err
                    || job.attempt_count >= MAX_ATTEMPTS;

                if should_give_up {
                    self.repository
                        .fail_outbox_job(&job.job_id, None, &err_msg, &now)?;
                    self.publish_message_invalidation(
                        job.message_id,
                        ConversationInvalidationReason::Message,
                    )?;
                    if is_auth_err {
                        let _ = self.repository.set_account_status(
                            &job.provider_account_key,
                            crate::conversation::types::AccountConnectionStatus::Invalid,
                            Some(&err_msg),
                        );
                    }
                } else {
                    let retry_time = (chrono::Utc::now() + chrono::Duration::seconds(5))
                        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
                    self.repository.fail_outbox_job(
                        &job.job_id,
                        Some(&retry_time),
                        &err_msg,
                        &now,
                    )?;
                    self.publish_message_invalidation(
                        job.message_id,
                        ConversationInvalidationReason::Message,
                    )?;
                }
                Ok(true)
            }
        }
    }
}
