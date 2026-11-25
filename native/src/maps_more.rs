//! The rest of the Google Maps surface: grid planning, preset provinces, the administrative
//! catalog and the job retries.

use crate::error::CoreApiError;
use crate::runtime::{id64, limit, on_blocking, on_core, optional_id64, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::administrative_catalog::commands::{
    self as catalog, AdministrativeCatalogState,
};
use rustsale_core::commands;
use rustsale_core::google_maps_jobs::commands::{self as jobs, GoogleMapsJobState};

/// Google Maps planning and catalog commands that were left out of the job module.
#[module]
pub struct GoogleMapsMore;

#[module]
impl GoogleMapsMore {
    /// Plans a scrape grid over `bbox` (`[latMin, latMax, lngMin, lngMax]`) with `stepKm` and
    /// resolves `{ bbox, area_km2, width_km, height_km, requested_step_km, effective_step_km,
    /// estimated_cells, recommended_step_km, exceeds_limit, estimated_chunks }`.
    pub async fn plan_grid(bbox: Vec<f64>, step_km: f64) -> Result<JsonValue, JsError> {
        let bbox: [f64; 4] = bbox
            .try_into()
            .map_err(|_| CoreApiError::invalid("bbox must hold four numbers"))?;
        on_blocking(move |_h| Ok::<_, CoreApiError>(commands::plan_grid_crawl(bbox, step_km)?))
            .await
    }

    /// The hard-coded bounding boxes of the 63 Vietnamese provinces and cities, keyed by name.
    pub async fn preset_provinces() -> Result<JsonValue, JsError> {
        on_blocking(move |_h| Ok::<_, CoreApiError>(commands::get_preset_provinces()?)).await
    }

    /// The countries of the bundled GeoNames catalog.
    pub async fn countries() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(catalog::list_administrative_countries(state))
        })
        .await
    }

    /// The catalog state of `countryCode`: loaded or not, record count, bbox and last error.
    pub async fn country_status(country_code: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(
                catalog::get_administrative_country_status(state, country_code).await?,
            )
        })
        .await
    }

    /// Downloads (when absent or `forceRefresh`) and loads the administrative data of
    /// `countryCode` from GeoNames and resolves its new status.
    pub async fn load_country(
        country_code: String,
        force_refresh: Option<bool>,
    ) -> Result<JsonValue, JsError> {
        let force = force_refresh.unwrap_or(false);
        on_core(move |h| async move {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(
                catalog::load_administrative_country(state, country_code, force).await?,
            )
        })
        .await
    }

    /// Child areas of `countryCode` under `parentId` (top level when absent), at most `limit`.
    pub async fn areas(
        country_code: String,
        parent_id: Option<f64>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let parent = optional_id64(parent_id)?;
        let size = limit(limit_) as usize;
        on_core(move |h| async move {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(
                catalog::list_administrative_areas(state, country_code, parent, size).await?,
            )
        })
        .await
    }

    /// Areas of `countryCode` whose name matches `query`, at most `limit`.
    pub async fn search_areas(
        country_code: String,
        query: String,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let size = limit(limit_) as usize;
        on_core(move |h| async move {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(
                catalog::search_administrative_areas(state, country_code, query, size).await?,
            )
        })
        .await
    }

    /// The geographic bounds of geoname `geonameId` from the catalog cache.
    pub async fn resolve_bounds(geoname_id: f64) -> Result<JsonValue, JsError> {
        let geoname = id64(geoname_id)?;
        on_core(move |h| async move {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(catalog::resolve_administrative_bounds(state, geoname).await?)
        })
        .await
    }

    /// Geocodes `query` (optionally inside `countryCode`) to a bbox, or `null` when the catalog
    /// cannot place it. It never calls Google.
    pub async fn geocode_bbox(
        query: String,
        country_code: Option<String>,
    ) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<AdministrativeCatalogState>(&h)?;
            Ok::<_, CoreApiError>(catalog::geocode_bounding_box(state, query, country_code).await?)
        })
        .await
    }

    /// Installs or prepares the scraper's browser runtime and resolves its status. This can
    /// download the runtime and take minutes.
    pub async fn prepare_runtime() -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::prepare_google_maps_runtime(state).await?)
        })
        .await
    }

    /// Re-queues the failed units of job `jobId` and resolves the updated job.
    pub async fn retry_failed_units(job_id: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::retry_google_maps_failed_units(state, job_id).await?)
        })
        .await
    }

    /// Re-runs the email enrichment step of job `jobId` and resolves the updated job.
    pub async fn retry_email_enrichment(job_id: String) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<GoogleMapsJobState>(&h)?;
            Ok::<_, CoreApiError>(jobs::retry_google_maps_email_enrichment(state, job_id).await?)
        })
        .await
    }
}
