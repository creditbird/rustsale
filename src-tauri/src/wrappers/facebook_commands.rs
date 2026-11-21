#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
use rustsale_core::commands::DbState;
#[allow(unused_imports)]
use rustsale_core::facebook::commands::*;
use rustsale_core::host::{AppHandle, State};
use serde::{Deserialize, Serialize};

#[tauri::command]
pub fn schedule_facebook_post(
    core: tauri::State<'_, CoreHandle>,
    request: CreateFacebookPostRequest,
) -> Result<i64, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::schedule_facebook_post(st_state, request)
}

#[tauri::command]
pub fn publish_facebook_post_now(
    core: tauri::State<'_, CoreHandle>,
    post_id: i64,
) -> Result<bool, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::publish_facebook_post_now(st_state, post_id)
}

#[tauri::command]
pub fn list_facebook_scheduled_posts(
    core: tauri::State<'_, CoreHandle>,
    page_id: Option<String>,
) -> Result<Vec<FacebookScheduledPostDto>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::list_facebook_scheduled_posts(st_state, page_id)
}

#[tauri::command]
pub fn cancel_facebook_scheduled_post(
    core: tauri::State<'_, CoreHandle>,
    post_id: i64,
) -> Result<bool, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::cancel_facebook_scheduled_post(st_state, post_id)
}

#[tauri::command]
pub fn create_comment_automation(
    core: tauri::State<'_, CoreHandle>,
    req: CreateCommentAutomationRequest,
) -> Result<i64, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::create_comment_automation(st_state, req)
}

#[tauri::command]
pub fn list_comment_automations(
    core: tauri::State<'_, CoreHandle>,
    page_id: Option<String>,
) -> Result<Vec<CommentAutomationDto>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::list_comment_automations(st_state, page_id)
}

#[tauri::command]
pub fn update_comment_automation(
    core: tauri::State<'_, CoreHandle>,
    req: UpdateCommentAutomationRequest,
) -> Result<bool, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::update_comment_automation(st_state, req)
}

#[tauri::command]
pub fn delete_comment_automation(
    core: tauri::State<'_, CoreHandle>,
    id: i64,
) -> Result<bool, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::delete_comment_automation(st_state, id)
}

#[tauri::command]
pub fn list_comment_logs(
    core: tauri::State<'_, CoreHandle>,
    page_id: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<CommentLogDto>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::list_comment_logs(st_state, page_id, limit)
}

#[tauri::command]
pub fn get_facebook_page_health(
    core: tauri::State<'_, CoreHandle>,
    page_id: String,
) -> Result<PageHealthDto, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::facebook::commands::get_facebook_page_health(st_state, page_id)
}
