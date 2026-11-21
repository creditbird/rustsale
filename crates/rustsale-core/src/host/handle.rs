//! `AppHandle` and `State`: the shell-agnostic replacement for the parts of
//! Tauri's `AppHandle` / `State` the core relies on (typed state registry and
//! event emission).

use super::error::HostError;
use super::ports::Ports;
use serde::Serialize;
use std::any::{type_name, Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::{Arc, PoisonError, RwLock};

type AnyState = Arc<dyn Any + Send + Sync>;

struct Shared {
    ports: Ports,
    states: RwLock<HashMap<TypeId, AnyState>>,
}

/// Cheap-to-clone handle to the running core: owns the port bundle and a
/// registry of managed state objects (one per type).
#[derive(Clone)]
pub struct AppHandle {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for AppHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppHandle").finish_non_exhaustive()
    }
}

impl AppHandle {
    pub fn new(ports: Ports) -> Self {
        Self {
            shared: Arc::new(Shared {
                ports,
                states: RwLock::new(HashMap::new()),
            }),
        }
    }

    /// Ports provided by the shell.
    pub fn ports(&self) -> &Ports {
        &self.shared.ports
    }

    /// Register a state object. Returns `false` (and drops `state`) when a
    /// state of the same type is already registered.
    pub fn manage<T: Send + Sync + 'static>(&self, state: T) -> bool {
        let mut states = self
            .shared
            .states
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let key = TypeId::of::<T>();
        if states.contains_key(&key) {
            return false;
        }
        states.insert(key, Arc::new(state));
        true
    }

    /// The registered state of type `T`, if any.
    pub fn try_state<T: Send + Sync + 'static>(&self) -> Option<State<'_, T>> {
        let states = self
            .shared
            .states
            .read()
            .unwrap_or_else(PoisonError::into_inner);
        let any = states.get(&TypeId::of::<T>())?.clone();
        drop(states);
        any.downcast::<T>().ok().map(State::from_arc)
    }

    /// The registered state of type `T`, or [`HostError::StateMissing`].
    pub fn state<T: Send + Sync + 'static>(&self) -> Result<State<'_, T>, HostError> {
        self.try_state::<T>()
            .ok_or(HostError::StateMissing(type_name::<T>()))
    }

    /// Serialize `payload` and push it to the UI under `event`.
    pub fn emit<S: Serialize>(&self, event: &str, payload: S) -> Result<(), HostError> {
        let value = serde_json::to_value(payload)?;
        self.shared.ports.events.emit(event, value)?;
        Ok(())
    }
}

/// Shared reference to a managed state object. Derefs to `T`.
pub struct State<'r, T: Send + Sync + 'static> {
    inner: Arc<T>,
    _borrow: PhantomData<&'r ()>,
}

impl<T: Send + Sync + 'static> State<'_, T> {
    pub fn from_arc(inner: Arc<T>) -> Self {
        Self {
            inner,
            _borrow: PhantomData,
        }
    }

    /// Wrap an owned value (handy for tests and for callers outside a handle).
    pub fn new(value: T) -> Self {
        Self::from_arc(Arc::new(value))
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T: Send + Sync + 'static> Clone for State<'_, T> {
    fn clone(&self) -> Self {
        Self::from_arc(self.inner.clone())
    }
}

impl<T: Send + Sync + 'static> Deref for State<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}
