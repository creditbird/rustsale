use super::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
use super::repository::ConversationRepository;
use super::types::{Channel, ConversationRuntimeStatus, InboxItem, TimelineCursor, TimelinePage};
use crate::conversation::outreach::AppliedOutreachMessage;
use crate::conversation::zalo::normalizer::{NormalizedGroupSnapshot, ZaloMutation};
use anyhow::Result;
use std::sync::Arc;

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
                    let applied = self.repository.apply_normalized_message(&message)?;
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
                ZaloMutation::ReconcileRequired { .. } => {}
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

        match client
            .send_text(&job.external_thread_id, thread_type, &job.text)
            .await
        {
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

                if is_auth_err {
                    self.repository
                        .fail_outbox_job(&job.job_id, None, &err_msg, &now)?;
                    self.publish_message_invalidation(
                        job.message_id,
                        ConversationInvalidationReason::Message,
                    )?;
                    let _ = self.repository.set_account_status(
                        &job.provider_account_key,
                        crate::conversation::types::AccountConnectionStatus::Invalid,
                        Some(&err_msg),
                    );
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
