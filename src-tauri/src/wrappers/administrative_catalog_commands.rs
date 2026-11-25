#![allow(
    unused_imports,
    unused_variables,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
use crate::CoreHandle;
#[allow(unused_imports)]
use rustsale_core::administrative_catalog::commands::*;
use rustsale_core::administrative_catalog::{
    service::AdministrativeCatalogService,
    types::{
        AdministrativeAreaSummary, AdministrativeBounds, AdministrativeCountry,
        AdministrativeCountryStatus,
    },
};
use rustsale_core::host::{AppHandle, State};
use std::sync::Arc;

#[tauri::command]
pub fn list_administrative_countries(
    core: tauri::State<'_, CoreHandle>,
) -> Vec<AdministrativeCountry> {
    let app = core.handle();
    let st_state = match app.state::<AdministrativeCatalogState>() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(target: "tauri_adapter", "state unavailable: {e}");
            return Default::default();
        }
    };
    rustsale_core::administrative_catalog::commands::list_administrative_countries(st_state)
}

#[tauri::command]
pub async fn get_administrative_country_status(
    core: tauri::State<'_, CoreHandle>,
    country_code: String,
) -> Result<AdministrativeCountryStatus, String> {
    let app = core.handle();
    let st_state = app
        .state::<AdministrativeCatalogState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::administrative_catalog::commands::get_administrative_country_status(
        st_state,
        country_code,
    )
    .await
}

#[tauri::command]
pub async fn load_administrative_country(
    core: tauri::State<'_, CoreHandle>,
    country_code: String,
    force_refresh: bool,
) -> Result<AdministrativeCountryStatus, String> {
    let app = core.handle();
    let st_state = app
        .state::<AdministrativeCatalogState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::administrative_catalog::commands::load_administrative_country(
        st_state,
        country_code,
        force_refresh,
    )
    .await
}

#[tauri::command]
pub async fn list_administrative_areas(
    core: tauri::State<'_, CoreHandle>,
    country_code: String,
    parent_id: Option<i64>,
    limit: usize,
) -> Result<Vec<AdministrativeAreaSummary>, String> {
    let app = core.handle();
    let st_state = app
        .state::<AdministrativeCatalogState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::administrative_catalog::commands::list_administrative_areas(
        st_state,
        country_code,
        parent_id,
        limit,
    )
    .await
}

#[tauri::command]
pub async fn search_administrative_areas(
    core: tauri::State<'_, CoreHandle>,
    country_code: String,
    query: String,
    limit: usize,
) -> Result<Vec<AdministrativeAreaSummary>, String> {
    let app = core.handle();
    let st_state = app
        .state::<AdministrativeCatalogState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::administrative_catalog::commands::search_administrative_areas(
        st_state,
        country_code,
        query,
        limit,
    )
    .await
}

#[tauri::command]
pub async fn resolve_administrative_bounds(
    core: tauri::State<'_, CoreHandle>,
    geoname_id: i64,
) -> Result<AdministrativeBounds, String> {
    let app = core.handle();
    let st_state = app
        .state::<AdministrativeCatalogState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::administrative_catalog::commands::resolve_administrative_bounds(
        st_state, geoname_id,
    )
    .await
}

#[tauri::command]
pub async fn geocode_bounding_box(
    core: tauri::State<'_, CoreHandle>,
    query: String,
    country_code: Option<String>,
) -> Result<Option<[f64; 4]>, String> {
    let app = core.handle();
    let st_state = app
        .state::<AdministrativeCatalogState>()
        .map_err(|e| e.to_string())?;
    rustsale_core::administrative_catalog::commands::geocode_bounding_box(
        st_state,
        query,
        country_code,
    )
    .await
}
