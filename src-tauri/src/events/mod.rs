use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Core domain events across RustSale v2 for omnichannel outreach,
/// conversation lifecycle, lead journey progression, and AI agent actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AppEvent {
    InboundMessageReceived {
        platform: String,
        sender: String,
        conversation_id: i64,
        content: String,
    },
    OutboundMessageDispatched {
        platform: String,
        recipient: String,
        message_id: Option<i64>,
        status: String,
    },
    LeadStageChanged {
        lead_id: i32,
        from_stage: String,
        to_stage: String,
    },
    CampaignStepDue {
        execution_id: i64,
        campaign_id: i64,
        step_id: i64,
        lead_id: i32,
    },
    AIProposalGenerated {
        proposal_id: i64,
        lead_id: Option<i32>,
        platform: String,
    },
    AIProposalApproved {
        proposal_id: i64,
        reviewer: String,
    },
}

/// Internal EventBus providing asynchronous multi-producer multi-consumer
/// broadcast messaging across engine components and background workers.
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<AppEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(16));
        Self { sender }
    }

    pub fn publish(&self, event: AppEvent) -> Result<usize, broadcast::error::SendError<AppEvent>> {
        self.sender.send(event)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.sender.subscribe()
    }

    pub fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(512)
    }
}

pub type SharedEventBus = Arc<EventBus>;

pub fn create_shared_event_bus(capacity: usize) -> SharedEventBus {
    Arc::new(EventBus::new(capacity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_and_subscribe() {
        let bus = EventBus::new(32);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        let event = AppEvent::InboundMessageReceived {
            platform: "zalo".to_string(),
            sender: "0987654321".to_string(),
            conversation_id: 101,
            content: "Hello from Zalo customer".to_string(),
        };

        bus.publish(event.clone()).expect("publish should succeed");

        let received1 = rx1.recv().await.expect("rx1 should receive event");
        let received2 = rx2.recv().await.expect("rx2 should receive event");

        assert_eq!(received1, event);
        assert_eq!(received2, event);
    }

    #[tokio::test]
    async fn test_event_bus_all_event_variants() {
        let bus = EventBus::new(32);
        let mut rx = bus.subscribe();

        let events = vec![
            AppEvent::InboundMessageReceived {
                platform: "facebook".to_string(),
                sender: "fb_user_123".to_string(),
                conversation_id: 202,
                content: "Interested in product".to_string(),
            },
            AppEvent::OutboundMessageDispatched {
                platform: "zalo".to_string(),
                recipient: "0987654321".to_string(),
                message_id: Some(5001),
                status: "DELIVERED".to_string(),
            },
            AppEvent::LeadStageChanged {
                lead_id: 42,
                from_stage: "DISCOVERY".to_string(),
                to_stage: "QUALIFIED".to_string(),
            },
            AppEvent::CampaignStepDue {
                execution_id: 88,
                campaign_id: 1,
                step_id: 2,
                lead_id: 42,
            },
            AppEvent::AIProposalGenerated {
                proposal_id: 77,
                lead_id: Some(42),
                platform: "zalo".to_string(),
            },
            AppEvent::AIProposalApproved {
                proposal_id: 77,
                reviewer: "Alex Rivera".to_string(),
            },
        ];

        for ev in &events {
            bus.publish(ev.clone()).expect("publish event");
        }

        for expected in &events {
            let received = rx.recv().await.expect("receive event");
            assert_eq!(&received, expected);
        }
    }
}
