//! Deals of the pipeline: the list, stage moves and create/delete.

use crate::error::CoreApiError;
use crate::runtime::{body, on_blocking, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use serde::Deserialize;

/// The fields of a new deal (`Deals.create`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewDeal {
    lead_id: i32,
    title: String,
    #[serde(default)]
    value: f64,
    stage: String,
    #[serde(default)]
    currency: Option<String>,
}

/// Deals attached to leads, as the pipeline and dashboard read them. Records keep the core's field
/// names (snake_case), as the v2 commands returned them.
#[module]
pub struct Deals;

#[module]
impl Deals {
    /// All deals, newest first, with their lead, stage, value and currency.
    pub async fn list() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_deals(state)?)
        })
        .await
    }

    /// Moves deal `id` to `stage`; the core accepts any stage string.
    pub async fn update_stage(id: i32, stage: String) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_deal_stage(id, stage, state)?)
        })
        .await
    }

    /// Creates a deal from `{ leadId, title, value?, stage, currency? }` and resolves the new deal.
    /// `currency` defaults to VND; any other value than VND or USD rejects.
    pub async fn create(deal: JsonValue) -> Result<JsonValue, CoreApiError> {
        let deal: NewDeal = body(deal)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_deal(
                deal.lead_id,
                deal.title,
                deal.value,
                deal.stage,
                deal.currency,
                state,
            )?)
        })
        .await
    }

    /// Deletes deal `id`; a missing id is not an error.
    pub async fn delete(id: i32) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_deal(id, state)?)
        })
        .await
    }
}
