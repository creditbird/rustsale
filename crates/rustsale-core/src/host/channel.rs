//! `Channel<T>`: a one-way stream of typed messages from a command back to the
//! caller (progress reporting). Shells wrap their own transport in it.

use super::error::PortError;
use std::sync::Arc;

type SendFn<T> = dyn Fn(T) -> Result<(), PortError> + Send + Sync;

/// Cloneable sender handed to long-running commands.
pub struct Channel<T> {
    send: Arc<SendFn<T>>,
}

impl<T> Clone for Channel<T> {
    fn clone(&self) -> Self {
        Self {
            send: self.send.clone(),
        }
    }
}

impl<T> Channel<T> {
    pub fn new(send: impl Fn(T) -> Result<(), PortError> + Send + Sync + 'static) -> Self {
        Self {
            send: Arc::new(send),
        }
    }

    /// A channel that discards everything.
    pub fn noop() -> Self {
        Self::new(|_| Ok(()))
    }

    pub fn send(&self, message: T) -> Result<(), PortError> {
        (self.send)(message)
    }
}
