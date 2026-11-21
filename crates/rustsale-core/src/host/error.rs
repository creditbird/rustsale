//! Typed errors for the host abstraction layer.

use thiserror::Error;

/// Failure reported by a [`crate::host::ports`] implementation.
#[derive(Debug, Error)]
pub enum PortError {
    /// The shell that embeds the core does not provide this capability.
    #[error("host capability not available: {0}")]
    Unsupported(&'static str),
    /// The capability exists but the call failed.
    #[error("{0}")]
    Failed(String),
}

impl PortError {
    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed(message.into())
    }
}

/// Failure of an [`crate::host::AppHandle`] operation.
#[derive(Debug, Error)]
pub enum HostError {
    /// No state of this type was registered with `AppHandle::manage`.
    #[error("state not managed: {0}")]
    StateMissing(&'static str),
    /// The event payload could not be turned into JSON.
    #[error("event payload serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
    /// The event sink rejected the event.
    #[error("event delivery failed: {0}")]
    Emit(#[from] PortError),
}
