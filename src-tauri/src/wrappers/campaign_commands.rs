#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
#[allow(unused_imports)]
use rustsale_core::campaign::commands::*;
use rustsale_core::commands::DbState;
use rustsale_core::host::{AppHandle, State};
use rustsale_core::models::{Campaign, CampaignStep};
use serde::{Deserialize, Serialize};

#[tauri::command]
pub fn start_unified_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::start_unified_campaign(campaign_id, st_state)
}

#[tauri::command]
pub fn pause_unified_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::pause_unified_campaign(campaign_id, st_state)
}

#[tauri::command]
pub fn enroll_leads_into_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
    lead_ids: Vec<i32>,
) -> Result<usize, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::enroll_leads_into_campaign(campaign_id, lead_ids, st_state)
}

#[tauri::command]
pub fn get_unified_campaign_analytics(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<CampaignAnalytics, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::get_unified_campaign_analytics(campaign_id, st_state)
}

#[tauri::command]
pub fn get_all_unified_campaigns(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<Campaign>, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::get_all_unified_campaigns(st_state)
}

#[tauri::command]
pub fn save_unified_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign: Campaign,
    steps: Vec<CampaignStep>,
    confirm_remove_pending: Option<bool>,
) -> Result<i64, String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::save_unified_campaign(
        campaign,
        steps,
        confirm_remove_pending,
        st_state,
    )
}

#[tauri::command]
pub fn delete_unified_campaign(
    core: tauri::State<'_, CoreHandle>,
    campaign_id: i64,
) -> Result<(), String> {
    let app = core.handle();
    let st_state = app.state::<DbState>().map_err(|e| e.to_string())?;
    rustsale_core::campaign::commands::delete_unified_campaign(campaign_id, st_state)
}
