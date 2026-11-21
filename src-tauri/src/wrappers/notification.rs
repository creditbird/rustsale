#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
use rustsale_core::conversation::types::Channel;
use rustsale_core::host::{AppHandle, State};
#[allow(unused_imports)]
use rustsale_core::notification::*;
use std::collections::HashMap;
use std::sync::{Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant};

#[tauri::command]
pub fn get_notification_settings(
    core: tauri::State<'_, CoreHandle>,
) -> Result<NotificationSettings, String> {
    let app = core.handle();
    let st_state = app
        .state::<std::sync::Arc<NotificationManager>>()
        .map_err(|e| e.to_string())?;
    rustsale_core::notification::get_notification_settings(st_state)
}

#[tauri::command]
pub fn update_notification_settings(
    core: tauri::State<'_, CoreHandle>,
    settings: NotificationSettings,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<std::sync::Arc<NotificationManager>>()
        .map_err(|e| e.to_string())?;
    rustsale_core::notification::update_notification_settings(app.clone(), settings, st_state)
}

#[tauri::command]
pub fn set_active_conversation_focus(
    core: tauri::State<'_, CoreHandle>,
    active_conversation_id: Option<i64>,
    is_focused: bool,
    seq: Option<u64>,
) -> Result<FocusSyncResult, String> {
    let app = core.handle();
    let st_state = app
        .state::<std::sync::Arc<NotificationManager>>()
        .map_err(|e| e.to_string())?;
    rustsale_core::notification::set_active_conversation_focus(
        active_conversation_id,
        is_focused,
        seq,
        st_state,
    )
}

#[tauri::command]
pub async fn test_desktop_notification(core: tauri::State<'_, CoreHandle>) -> Result<(), String> {
    let app = core.handle();
    let st_state = app
        .state::<std::sync::Arc<NotificationManager>>()
        .map_err(|e| e.to_string())?;
    rustsale_core::notification::test_desktop_notification(app.clone(), st_state).await
}
