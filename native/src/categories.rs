//! Categories and the leads assigned to them.

use crate::error::CoreApiError;
use crate::runtime::{id32, on_blocking, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};

/// Lead categories and their membership. `Leads.categories` stays as it is; this module adds the
/// writes and the reverse lookups of the category manager.
#[module]
pub struct Categories;

#[module]
impl Categories {
    /// All categories, as `{ id, name }`.
    pub async fn list() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_categories(state)?)
        })
        .await
    }

    /// The categories lead `leadId` belongs to.
    pub async fn for_lead(lead_id: JsonValue) -> Result<JsonValue, JsError> {
        let lead = id32(lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_categories_for_lead(state, lead)?)
        })
        .await
    }

    /// Creates a category named `name` and resolves its id. A blank name rejects.
    pub async fn create(name: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_category(state, name)?)
        })
        .await
    }

    /// Deletes category `id`; its leads stay.
    pub async fn delete(id: JsonValue) -> Result<JsonValue, JsError> {
        let id = id32(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_category(state, id)?)
        })
        .await
    }

    /// Adds leads `leadIds` to category `categoryId` in one transaction; an empty list does nothing.
    pub async fn add_leads(
        lead_ids: Vec<JsonValue>,
        category_id: JsonValue,
    ) -> Result<JsonValue, JsError> {
        let leads = lead_ids
            .into_iter()
            .map(id32)
            .collect::<Result<Vec<_>, _>>()?;
        let category = id32(category_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::add_leads_to_category(state, leads, category)?)
        })
        .await
    }

    /// Removes leads `leadIds` from category `categoryId` in one transaction; an empty list does nothing.
    pub async fn remove_leads(
        lead_ids: Vec<JsonValue>,
        category_id: JsonValue,
    ) -> Result<JsonValue, JsError> {
        let leads = lead_ids
            .into_iter()
            .map(id32)
            .collect::<Result<Vec<_>, _>>()?;
        let category = id32(category_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::remove_leads_from_category(
                state, leads, category,
            )?)
        })
        .await
    }

    /// All leads of category `categoryId`.
    pub async fn leads(category_id: JsonValue) -> Result<JsonValue, JsError> {
        let category = id32(category_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_leads_by_category(state, category)?)
        })
        .await
    }
}
