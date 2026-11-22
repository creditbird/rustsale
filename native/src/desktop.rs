//! Desktop notifications: the focused-conversation guard and the test notification.

use crate::error::CoreApiError;
use crate::runtime::{on_blocking, on_core, optional_id64, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::notification::{self, NotificationManager};
use std::sync::Arc;

/// A JS number as an optional focus sequence.
fn sequence(value: Option<f64>) -> Option<u64> {
    value
        .filter(|n| n.is_finite() && *n >= 0.0 && *n <= u64::MAX as f64)
        .map(|n| n as u64)
}

/// Desktop notification behaviour that belongs to the window, not the CRM data.
#[module]
pub struct Desktop;

#[module]
impl Desktop {
    /// Tells the notifier which conversation is open and focused so its messages are not
    /// notified twice. `seq` orders updates: an update older than the backend sequence is
    /// rejected. Resolves `{ accepted, backend_seq }`.
    pub async fn set_active_conversation_focus(
        active_conversation_id: Option<f64>,
        is_focused: bool,
        seq: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let conversation_id = optional_id64(active_conversation_id)?;
        let seq = sequence(seq);
        on_blocking(move |h| {
            let state = state_of::<Arc<NotificationManager>>(&h)?;
            Ok::<_, CoreApiError>(notification::set_active_conversation_focus(
                conversation_id,
                is_focused,
                seq,
                state,
            )?)
        })
        .await
    }

    /// Shows a test desktop notification and emits `desktop-notification-sent` with
    /// `{ test: true, sound }`.
    pub async fn test_notification() -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let app = h.clone();
            let state = state_of::<Arc<NotificationManager>>(&h)?;
            Ok::<_, CoreApiError>(notification::test_desktop_notification(app, state).await?)
        })
        .await
    }
}
