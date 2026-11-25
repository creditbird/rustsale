//! Currency, notification and tray settings.

use crate::error::CoreApiError;
use crate::runtime::{body, on_blocking, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use rustsale_core::notification::{self, NotificationManager};
use rustsale_core::tray::{self, TrayState};
use std::sync::Arc;

/// App settings that live in the core. Their records keep the core's field names (snake_case), as the
/// v2 commands returned them.
#[module]
pub struct Settings;

#[module]
impl Settings {
    /// The base currency and the exchange rate: `{ base_currency, exchange_rate }`.
    pub async fn currency() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_currency_settings(state)?)
        })
        .await
    }

    /// Saves the base currency (`VND` or `USD`) and the exchange rate; resolves the saved settings.
    pub async fn update_currency(
        base_currency: String,
        exchange_rate: f64,
    ) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_currency_settings(
                base_currency,
                exchange_rate,
                state,
            )?)
        })
        .await
    }

    /// The desktop notification settings: `{ enabled, sound_enabled, privacy_mode, mute_groups }`.
    pub async fn notifications() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<Arc<NotificationManager>>(&h)?;
            Ok::<_, CoreApiError>(notification::get_notification_settings(state)?)
        })
        .await
    }

    /// Saves the notification settings, given in full as `notifications()` returns them.
    pub async fn update_notifications(settings: JsonValue) -> Result<JsonValue, JsError> {
        let settings = body(settings)?;
        on_blocking(move |h| {
            let state = state_of::<Arc<NotificationManager>>(&h)?;
            Ok::<_, CoreApiError>(notification::update_notification_settings(
                h.clone(),
                settings,
                state,
            )?)
        })
        .await
    }

    /// The tray settings: `{ close_to_tray, dnd_enabled }`.
    pub async fn tray() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<TrayState>(&h)?;
            Ok::<_, CoreApiError>(tray::get_tray_state(state)?)
        })
        .await
    }

    /// Whether closing the window hides it to the tray instead of quitting.
    pub async fn set_close_to_tray(enabled: bool) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<TrayState>(&h)?;
            Ok::<_, CoreApiError>(tray::set_close_to_tray(h.clone(), enabled, state)?)
        })
        .await
    }

    /// Switches do-not-disturb on or off.
    pub async fn set_do_not_disturb(enabled: bool) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<TrayState>(&h)?;
            Ok::<_, CoreApiError>(tray::set_tray_dnd(h.clone(), enabled, state)?)
        })
        .await
    }

    /// The interface language, as `{ language }` with `vi` or `en`; a missing or unknown value
    /// reads as `vi`.
    pub async fn language() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            let language = commands::get_language_setting(state)?;
            Ok::<_, CoreApiError>(serde_json::json!({ "language": language }))
        })
        .await
    }

    /// Saves the interface language (`vi` or `en`) and resolves `{ language }` with the saved value.
    pub async fn set_language(language: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            let saved = commands::update_language_setting(language, state)?;
            Ok::<_, CoreApiError>(serde_json::json!({ "language": saved }))
        })
        .await
    }

    /// Whether RustSale starts when the user signs in, as `{ enabled, supported }`. On systems this
    /// build does not cover (anything but Linux) `supported` is false and nothing is read.
    pub fn autostart() -> Result<JsonValue, JsError> {
        if !cfg!(target_os = "linux") {
            return Ok(serde_json::json!({ "enabled": false, "supported": false }));
        }
        let paths = crate::autostart::paths()?;
        Ok(serde_json::json!({
            "enabled": crate::autostart::is_enabled_at(&paths.config_home),
            "supported": true,
        }))
    }

    /// Turns starting at sign-in on or off (Linux: the XDG autostart entry `rustsale.desktop`) and
    /// resolves `{ enabled }`. Rejects with `unsupported` on systems this build does not cover.
    pub fn set_autostart(enabled: bool) -> Result<JsonValue, JsError> {
        if !cfg!(target_os = "linux") {
            return Err(CoreApiError::unsupported(
                "starting at sign-in is implemented on Linux only",
            )
            .into());
        }
        let paths = crate::autostart::paths()?;
        crate::autostart::set_enabled_at(&paths.config_home, &paths.exe, enabled)?;
        Ok(serde_json::json!({ "enabled": enabled }))
    }
}
