#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
#[allow(unused_imports)]
use rustsale_core::google_maps_jobs::commands::*;
use rustsale_core::google_maps_jobs::repository::ScrapeJobRepository;
use rustsale_core::google_maps_jobs::runner::ScrapeJobRunner;
use rustsale_core::google_maps_jobs::types::{
    CreateScrapeJobRequest, ScrapeJob, ScrapeJobStatus, ScrapeMode, ScrapeStage, ScrapeUnit,
    ScrapeUnitStatus, TransitionCause,
};
use rustsale_core::host::{AppHandle, State};
use rustsale_core::scrapers::browser::{BrowserManager, RuntimeStatus};
use rustsale_core::scrapers::grid::{generate_initial_cells, BoundingBox, GridLimits};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

#[tauri::command]
pub async fn get_google_maps_runtime_status(
    core: tauri::State<'_, CoreHandle>,
) -> Result<RuntimeStatus, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::get_google_maps_runtime_status(st_state).await
}

#[tauri::command]
pub async fn prepare_google_maps_runtime(
    core: tauri::State<'_, CoreHandle>,
) -> Result<RuntimeStatus, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::prepare_google_maps_runtime(st_state).await
}

#[tauri::command]
pub async fn create_google_maps_scrape_job(
    core: tauri::State<'_, CoreHandle>,
    request: CreateScrapeJobRequest,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::create_google_maps_scrape_job(st_state, request)
        .await
}

#[tauri::command]
pub async fn pause_google_maps_scrape_job(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::pause_google_maps_scrape_job(st_state, job_id).await
}

#[tauri::command]
pub async fn resume_google_maps_scrape_job(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::resume_google_maps_scrape_job(st_state, job_id).await
}

#[tauri::command]
pub async fn cancel_google_maps_scrape_job(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::cancel_google_maps_scrape_job(st_state, job_id).await
}

#[tauri::command]
pub async fn retry_google_maps_failed_units(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::retry_google_maps_failed_units(st_state, job_id)
        .await
}

#[tauri::command]
pub async fn retry_google_maps_email_enrichment(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::retry_google_maps_email_enrichment(st_state, job_id)
        .await
}

#[tauri::command]
pub async fn get_google_maps_scrape_job(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<ScrapeJob, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::get_google_maps_scrape_job(st_state, job_id).await
}

#[tauri::command]
pub async fn list_google_maps_scrape_jobs(
    core: tauri::State<'_, CoreHandle>,
) -> Result<Vec<ScrapeJob>, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::list_google_maps_scrape_jobs(st_state).await
}

#[tauri::command]
pub async fn delete_google_maps_scrape_job(
    core: tauri::State<'_, CoreHandle>,
    job_id: String,
) -> Result<bool, String> {
    let app = core.handle();
    let st_state = app
        .state::<GoogleMapsJobState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::google_maps_jobs::commands::delete_google_maps_scrape_job(st_state, job_id).await
}
