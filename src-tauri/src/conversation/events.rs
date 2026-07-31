use crate::conversation::types::Channel;
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationInvalidationReason {
    Message,
    Receipt,
    Reaction,
    Metadata,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInvalidatedEvent {
    pub conversation_id: i64,
    pub lead_id: Option<i64>,
    pub channel: Channel,
    pub reason: ConversationInvalidationReason,
    pub message_id: Option<i64>,
}

pub struct ConversationEventPublisher {
    sender: broadcast::Sender<ConversationInvalidatedEvent>,
}

impl ConversationEventPublisher {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConversationInvalidatedEvent> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: ConversationInvalidatedEvent) {
        let _ = self.sender.send(event);
    }

    pub fn spawn_tauri_forwarder(
        self: std::sync::Arc<Self>,
        app_handle: tauri::AppHandle,
    ) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
            let mut receiver = self.subscribe();
            loop {
                match receiver.recv().await {
                    Ok(event) => {
                        if let Err(error) = app_handle.emit("conversation-invalidated", event) {
                            tracing::warn!(target: "conversation", %error, "tauri emit failed");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(target: "conversation", skipped, "event forwarder lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        })
    }
}
