//! Google Maps scrape jobs.

use crate::error::CoreApiError;
use crate::runtime::{args, body, limit, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::google_maps_jobs::commands as jobs;
use rustsale_core::google_maps_jobs::commands::GoogleMapsJobState;
use rustsale_core::paging::places::PlaceQuery;

/// Google Maps scrape jobs: create, control and read their places. The job runs in the core;
/// follow it with the `scrape-job-event` events of `Host.onEvent`.
#[module]
pub struct GoogleMaps;

#[module]
impl GoogleMaps {
    /// Whether the scraper's browser is installed and ready.
    pub async fn runtime_status() -> Result<JsonValue, CoreApiError> {
        on_core(|h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::get_google_maps_runtime_status(state).await?)
        })
        .await
    }

    /// All jobs, newest first.
    pub async fn list_jobs() -> Result<JsonValue, CoreApiError> {
        on_core(|h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::list_google_maps_scrape_jobs(state).await?)
        })
        .await
    }

    /// One job by id.
    pub async fn get_job(job_id: String) -> Result<JsonValue, CoreApiError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::get_google_maps_scrape_job(state, job_id).await?)
        })
        .await
    }

    /// Creates and queues a job from `{ mode, keyword, location?, bbox?, stepKm?, districts?,
    /// headless }` and resolves it.
    pub async fn create_job(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request = body(request)?;
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::create_google_maps_scrape_job(state, request).await?)
        })
        .await
    }

    /// Pauses a running job and resolves it.
    pub async fn pause_job(job_id: String) -> Result<JsonValue, CoreApiError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::pause_google_maps_scrape_job(state, job_id).await?)
        })
        .await
    }

    /// Resumes a paused job and resolves it.
    pub async fn resume_job(job_id: String) -> Result<JsonValue, CoreApiError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::resume_google_maps_scrape_job(state, job_id).await?)
        })
        .await
    }

    /// Cancels a job and resolves it.
    pub async fn cancel_job(job_id: String) -> Result<JsonValue, CoreApiError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::cancel_google_maps_scrape_job(state, job_id).await?)
        })
        .await
    }

    /// Deletes a finished job and its places; resolves whether it existed.
    pub async fn delete_job(job_id: String) -> Result<JsonValue, CoreApiError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::delete_google_maps_scrape_job(state, job_id).await?)
        })
        .await
    }

    /// One page of the places of a job, filtered and sorted in the core. `query` is `{ search?,
    /// importStatus?, hasPhone?, sort?, descending? }`. Resolves `{ items, total, nextCursor }`.
    pub async fn results_page(
        job_id: String,
        query: Option<JsonValue>,
        cursor: Option<String>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let query: PlaceQuery = args(query)?;
        let limit = limit(limit_);
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(
                jobs::list_google_maps_results_page(state, job_id, query, cursor, limit).await?,
            )
        })
        .await
    }
}
