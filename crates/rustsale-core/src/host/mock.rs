//! In-memory host for tests: records emitted events, lets tests register
//! listeners and managed state without any UI framework.

use super::error::PortError;
use super::handle::{AppHandle, State};
use super::ports::{DesktopNotifier, EventSink, Ports};
use serde_json::Value;
use std::sync::{Arc, Mutex, PoisonError};

type Listener = Box<dyn Fn(&Value) + Send + Sync>;

/// An event captured by [`MockEvents`].
#[derive(Debug, Clone, PartialEq)]
pub struct EmittedEvent {
    pub event: String,
    pub payload: Value,
}

/// `EventSink` that records every event and notifies listeners.
#[derive(Default)]
pub struct MockEvents {
    emitted: Mutex<Vec<EmittedEvent>>,
    listeners: Mutex<Vec<(String, Listener)>>,
}

impl MockEvents {
    pub fn emitted(&self) -> Vec<EmittedEvent> {
        self.emitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn count(&self, event: &str) -> usize {
        self.emitted().iter().filter(|e| e.event == event).count()
    }

    pub fn listen(&self, event: &str, callback: impl Fn(&Value) + Send + Sync + 'static) {
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((event.to_string(), Box::new(callback)));
    }
}

impl EventSink for MockEvents {
    fn emit(&self, event: &str, payload: Value) -> Result<(), PortError> {
        {
            let listeners = self
                .listeners
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            for (name, callback) in listeners.iter() {
                if name == event {
                    callback(&payload);
                }
            }
        }
        self.emitted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(EmittedEvent {
                event: event.to_string(),
                payload,
            });
        Ok(())
    }
}

/// `DesktopNotifier` that records notifications and always succeeds.
#[derive(Default)]
pub struct MockNotifier {
    shown: Mutex<Vec<(String, String)>>,
}

impl MockNotifier {
    pub fn shown(&self) -> Vec<(String, String)> {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl DesktopNotifier for MockNotifier {
    fn show(&self, title: &str, body: &str) -> Result<(), PortError> {
        self.shown
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((title.to_string(), body.to_string()));
        Ok(())
    }
}

/// Test application: an [`AppHandle`] over recording ports.
pub struct MockApp {
    handle: AppHandle,
    pub events: Arc<MockEvents>,
    pub notifier: Arc<MockNotifier>,
}

impl Default for MockApp {
    fn default() -> Self {
        Self::new()
    }
}

impl MockApp {
    pub fn new() -> Self {
        let events = Arc::new(MockEvents::default());
        let notifier = Arc::new(MockNotifier::default());
        let mut ports = Ports::noop();
        ports.events = events.clone();
        ports.notifier = notifier.clone();
        Self {
            handle: AppHandle::new(ports),
            events,
            notifier,
        }
    }

    /// Register a state object (builder style).
    pub fn manage<T: Send + Sync + 'static>(self, state: T) -> Self {
        self.handle.manage(state);
        self
    }

    pub fn handle(&self) -> &AppHandle {
        &self.handle
    }

    /// Registered state. Panics when missing: test helper only.
    pub fn state<T: Send + Sync + 'static>(&self) -> State<'_, T> {
        match self.handle.state::<T>() {
            Ok(state) => state,
            Err(error) => panic!("MockApp: {error}"),
        }
    }

    pub fn listen(&self, event: &str, callback: impl Fn(&Value) + Send + Sync + 'static) {
        self.events.listen(event, callback);
    }
}
