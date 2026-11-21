use crate::host::State;
use std::sync::Arc;

use super::{
    service::AdministrativeCatalogService,
    types::{
        AdministrativeAreaSummary, AdministrativeBounds, AdministrativeCountry,
        AdministrativeCountryStatus,
    },
};

pub struct AdministrativeCatalogState {
    pub service: Arc<AdministrativeCatalogService>,
}

pub fn list_administrative_countries(
    state: State<'_, AdministrativeCatalogState>,
) -> Vec<AdministrativeCountry> {
    state.service.countries()
}

pub async fn get_administrative_country_status(
    state: State<'_, AdministrativeCatalogState>,
    country_code: String,
) -> Result<AdministrativeCountryStatus, String> {
    state
        .service
        .status(&country_code)
        .await
        .map_err(|error| error.to_string())
}

pub async fn load_administrative_country(
    state: State<'_, AdministrativeCatalogState>,
    country_code: String,
    force_refresh: bool,
) -> Result<AdministrativeCountryStatus, String> {
    state
        .service
        .load_country(&country_code, force_refresh)
        .await
        .map_err(|error| error.to_string())
}

pub async fn list_administrative_areas(
    state: State<'_, AdministrativeCatalogState>,
    country_code: String,
    parent_id: Option<i64>,
    limit: usize,
) -> Result<Vec<AdministrativeAreaSummary>, String> {
    state
        .service
        .list_children(&country_code, parent_id, limit)
        .await
        .map_err(|error| error.to_string())
}

pub async fn search_administrative_areas(
    state: State<'_, AdministrativeCatalogState>,
    country_code: String,
    query: String,
    limit: usize,
) -> Result<Vec<AdministrativeAreaSummary>, String> {
    state
        .service
        .search(&country_code, &query, limit)
        .await
        .map_err(|error| error.to_string())
}

pub async fn resolve_administrative_bounds(
    state: State<'_, AdministrativeCatalogState>,
    geoname_id: i64,
) -> Result<AdministrativeBounds, String> {
    state
        .service
        .resolve_bounds(geoname_id)
        .await
        .map_err(|error| error.to_string())
}

pub async fn geocode_bounding_box(
    state: State<'_, AdministrativeCatalogState>,
    query: String,
    country_code: Option<String>,
) -> Result<Option<[f64; 4]>, String> {
    state
        .service
        .geocode_query(&query, country_code.as_deref())
        .await
        .map_err(|error| error.to_string())
}
