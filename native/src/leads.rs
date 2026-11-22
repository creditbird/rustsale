//! Leads, categories and the lead activity log.

use crate::error::CoreApiError;
use crate::runtime::{args, body, limit, on_blocking, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use rustsale_core::paging::activities::get_activities_page;
use rustsale_core::paging::leads::{LeadQuery, get_leads_by_category_page, get_leads_page};
use serde::Deserialize;

/// The fields of a new lead (`Leads.add`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewLead {
    name: String,
    #[serde(default)]
    company: String,
    #[serde(default = "new_status")]
    status: String,
    #[serde(default)]
    value: f64,
    #[serde(default)]
    owner: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    phone: String,
    about: Option<String>,
    address: Option<String>,
    currency: Option<String>,
}

fn new_status() -> String {
    "New".to_owned()
}

/// Leads, their categories and activity log. Lists are cursor-paged: pass the `nextCursor` of the
/// last page back as `cursor`; `limit` 0 or absent means 50, the most is 200.
#[module]
pub struct Leads;

#[module]
impl Leads {
    /// One page of leads, filtered and sorted in the core. `query` is `{ search?, status?,
    /// categoryId?, sort?: "id"|"name"|"company"|"status"|"value"|"owner"|"lastActivity",
    /// descending? }`. Resolves `{ items, total, nextCursor }`.
    pub async fn page(
        query: Option<JsonValue>,
        cursor: Option<String>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let query: LeadQuery = args(query)?;
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(get_leads_page(state, query, cursor, limit)?)
        })
        .await
    }

    /// Like `page`, for the leads of category `categoryId`.
    pub async fn by_category_page(
        category_id: i32,
        query: Option<JsonValue>,
        cursor: Option<String>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let query: LeadQuery = args(query)?;
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(get_leads_by_category_page(
                state,
                category_id,
                query,
                cursor,
                limit,
            )?)
        })
        .await
    }

    /// One lead by id. Rejects with `not_found` when there is none.
    pub async fn details(id: i32) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_lead_details(id, state)?)
        })
        .await
    }

    /// Creates a lead from `{ name, company?, status?, value?, owner?, email?, phone?, about?,
    /// address?, currency? }` and resolves its id. `currency` defaults to VND.
    pub async fn add(lead: JsonValue) -> Result<JsonValue, CoreApiError> {
        let lead: NewLead = body(lead)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::add_lead(
                lead.name,
                lead.company,
                lead.status,
                lead.value,
                lead.owner,
                lead.email,
                lead.phone,
                lead.about,
                lead.address,
                lead.currency,
                state,
            )?)
        })
        .await
    }

    /// Sets the pipeline status of lead `id`.
    pub async fn update_status(id: i32, status: String) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_lead_status(id, status, state)?)
        })
        .await
    }

    /// One page of the activity log, newest first: of lead `leadId`, or of all leads when absent.
    /// Resolves `{ items, total, nextCursor }`.
    pub async fn activities_page(
        lead_id: Option<i32>,
        cursor: Option<String>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let limit = limit(limit_);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(get_activities_page(state, lead_id, cursor, limit)?)
        })
        .await
    }

    /// All categories, as `{ id, name }`.
    pub async fn categories() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_categories(state)?)
        })
        .await
    }
}
