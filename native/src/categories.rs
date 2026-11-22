//! Categories and the leads assigned to them.

use crate::error::CoreApiError;
use crate::runtime::{on_blocking, state_of};
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
    pub async fn for_lead(lead_id: i32) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_categories_for_lead(state, lead_id)?)
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
    pub async fn delete(id: i32) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_category(state, id)?)
        })
        .await
    }

    /// Adds leads `leadIds` to category `categoryId` in one transaction; an empty list does nothing.
    pub async fn add_leads(lead_ids: Vec<i32>, category_id: i32) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::add_leads_to_category(
                state,
                lead_ids,
                category_id,
            )?)
        })
        .await
    }

    /// Removes leads `leadIds` from category `categoryId` in one transaction; an empty list does nothing.
    pub async fn remove_leads(lead_ids: Vec<i32>, category_id: i32) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::remove_leads_from_category(
                state,
                lead_ids,
                category_id,
            )?)
        })
        .await
    }

    /// All leads of category `categoryId`.
    pub async fn leads(category_id: i32) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_leads_by_category(state, category_id)?)
        })
        .await
    }
}
