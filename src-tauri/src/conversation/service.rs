use super::events::{
    ConversationEventPublisher, ConversationInvalidatedEvent, ConversationInvalidationReason,
};
use super::repository::ConversationRepository;
use super::types::{Channel, InboxItem, TimelineCursor, TimelinePage};
use anyhow::Result;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConversationService {
    repository: Arc<ConversationRepository>,
    events: Arc<ConversationEventPublisher>,
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
}
