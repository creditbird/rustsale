use crate::conversation::types::{Channel, ConversationMessage};
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
    AccountStatus,
    HistorySync,
    AttachmentUpdate,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessageDeltaEvent {
    pub sequence: u64,
    pub conversation_id: i64,
    pub lead_id: Option<i64>,
    pub channel: Channel,
    pub message: ConversationMessage,
    pub emitted_at_ms: i64,
}

pub struct ConversationEventPublisher {
    sender: broadcast::Sender<ConversationInvalidatedEvent>,
    delta_sender: broadcast::Sender<ConversationMessageDeltaEvent>,
}

impl ConversationEventPublisher {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(1));
        let (delta_sender, _) = broadcast::channel(capacity.max(1));
        Self {
            sender,
            delta_sender,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ConversationInvalidatedEvent> {
        self.sender.subscribe()
    }

    pub fn subscribe_delta(&self) -> broadcast::Receiver<ConversationMessageDeltaEvent> {
        self.delta_sender.subscribe()
    }

    pub fn publish(&self, event: ConversationInvalidatedEvent) {
        let _ = self.sender.send(event);
    }

    pub fn publish_delta(&self, event: ConversationMessageDeltaEvent) {
        let _ = self.delta_sender.send(event);
    }

    pub fn spawn_tauri_forwarder(
        self: std::sync::Arc<Self>,
        app_handle: tauri::AppHandle,
    ) -> tauri::async_runtime::JoinHandle<()> {
        tauri::async_runtime::spawn(async move {
            let mut invalidation_rx = self.subscribe();
            let mut delta_rx = self.subscribe_delta();
            loop {
                tokio::select! {
                    inval = invalidation_rx.recv() => {
                        match inval {
                            Ok(event) => {
                                if let Err(error) = app_handle.emit("conversation-invalidated", event) {
                                    tracing::warn!(target: "conversation", %error, "tauri emit conversation-invalidated failed");
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!(target: "conversation", skipped, "invalidation forwarder lagged");
                                let _ = app_handle.emit("conversation-invalidated", serde_json::json!({
                                    "reason": "ReconcileRequired",
                                    "skipped": skipped
                                }));
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                    delta = delta_rx.recv() => {
                        match delta {
                            Ok(event) => {
                                if let Err(error) = app_handle.emit("conversation-message-delta", event) {
                                    tracing::warn!(target: "conversation", %error, "tauri emit conversation-message-delta failed");
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                                tracing::warn!(target: "conversation", skipped, "delta forwarder lagged");
                                let _ = app_handle.emit("conversation-delta-lagged", serde_json::json!({
                                    "skipped": skipped
                                }));
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        })
    }
}
