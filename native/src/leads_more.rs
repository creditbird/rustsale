//! The remaining lead commands: outreach paging, the full list, the activity log and lead edits.

use crate::error::CoreApiError;
use crate::runtime::{
    args, body, checked_id32, checked_optional_id32, id32, limit, on_blocking, optional_id32,
    state_of,
};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use serde::Deserialize;

/// The outreach filters, as the Zalo and Email screens send them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct OutreachFilters {
    category_id: Option<i32>,
    segmentation: String,
    filter_template_id: Option<i32>,
    channel: String,
    search: Option<String>,
    filter_verified_zalo: bool,
    filter_unreached: bool,
    filter_mobile_only: bool,
}

impl OutreachFilters {
    /// Checks the ids serde accepted, before the core pages over rows.
    fn checked(mut self) -> Result<Self, CoreApiError> {
        self.category_id = checked_optional_id32(self.category_id)?;
        self.filter_template_id = checked_optional_id32(self.filter_template_id)?;
        Ok(self)
    }
}

impl Default for OutreachFilters {
    fn default() -> Self {
        Self {
            category_id: None,
            segmentation: String::new(),
            filter_template_id: None,
            channel: "zalo".to_owned(),
            search: None,
            filter_verified_zalo: false,
            filter_unreached: false,
            filter_mobile_only: false,
        }
    }
}

/// The fields of a new activity (`LeadsMore.addActivity`).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewActivity {
    lead_id: i32,
    activity_type: String,
    content: String,
    #[serde(default)]
    performed_by: String,
}

/// The full field set of a lead edit (`LeadsMore.update`): the core replaces every field, so the
/// caller sends the whole lead as `Leads.details` returned it, plus the changes.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeadEdit {
    id: i32,
    name: String,
    #[serde(default)]
    company: String,
    #[serde(default)]
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

/// Lead commands outside the paged list: outreach filters, the plain list, activities and edits.
#[module]
pub struct LeadsMore;

#[module]
impl LeadsMore {
    /// One outreach page of leads matching `filters` (all optional: `categoryId`, `segmentation`,
    /// `filterTemplateId`, `channel`, `search`, `filterVerifiedZalo`, `filterUnreached`,
    /// `filterMobileOnly`). Resolves `{ items, total, next_cursor }`; `cursor` is the last
    /// `next_cursor`, `limit` 0 or absent means the core default.
    pub async fn outreach_page(
        filters: Option<JsonValue>,
        cursor: Option<f64>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let filters = OutreachFilters::checked(args(filters)?)?;
        let limit = limit(limit_);
        let cursor = optional_id32(cursor)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_outreach_leads_page(
                state,
                filters.category_id,
                filters.segmentation,
                filters.filter_template_id,
                filters.channel,
                filters.search,
                filters.filter_verified_zalo,
                filters.filter_unreached,
                filters.filter_mobile_only,
                cursor,
                limit,
            )?)
        })
        .await
    }

    /// Every lead id matching `filters` (same shape as `outreachPage`), without `excludedLeadIds`.
    pub async fn outreach_ids(
        filters: Option<JsonValue>,
        excluded_lead_ids: Option<Vec<f64>>,
    ) -> Result<JsonValue, JsError> {
        let filters = OutreachFilters::checked(args(filters)?)?;
        let excluded = excluded_lead_ids
            .unwrap_or_default()
            .into_iter()
            .map(id32)
            .collect::<Result<Vec<_>, _>>()?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::resolve_outreach_lead_ids(
                state,
                filters.category_id,
                filters.segmentation,
                filters.filter_template_id,
                filters.channel,
                filters.search,
                filters.filter_verified_zalo,
                filters.filter_unreached,
                filters.filter_mobile_only,
                excluded,
            )?)
        })
        .await
    }

    /// All leads, converted ones included; the caller splits them.
    pub async fn list() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_leads(state)?)
        })
        .await
    }

    /// The full activity log of lead `leadId`, newest first.
    pub async fn activities(lead_id: f64) -> Result<JsonValue, JsError> {
        let lead = id32(lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_activities(lead, state)?)
        })
        .await
    }

    /// The last `limit` activities of all leads, newest first; absent means 10.
    pub async fn recent_activities(limit_: Option<f64>) -> Result<JsonValue, JsError> {
        let limit = limit_
            .filter(|n| n.is_finite() && *n > 0.0)
            .map(|n| n.min(f64::from(i32::MAX)) as i32);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_recent_activities(limit, state)?)
        })
        .await
    }

    /// Logs an activity on a lead from `{ leadId, activityType, content, performedBy? }`.
    pub async fn add_activity(activity: JsonValue) -> Result<JsonValue, JsError> {
        let mut activity: NewActivity = body(activity)?;
        activity.lead_id = checked_id32(activity.lead_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::add_activity(
                activity.lead_id,
                activity.activity_type,
                activity.content,
                activity.performed_by,
                state,
            )?)
        })
        .await
    }

    /// Replaces lead `lead` from `{ id, name, company?, status?, value?, owner?, email?, phone?,
    /// about?, address?, currency? }`.
    pub async fn update(lead: JsonValue) -> Result<JsonValue, JsError> {
        let mut lead: LeadEdit = body(lead)?;
        lead.id = checked_id32(lead.id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_lead(
                lead.id,
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

    /// Deletes lead `id` with its activities and deals; a missing id is not an error.
    pub async fn delete(id: f64) -> Result<JsonValue, JsError> {
        let id = id32(id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_lead(id, state)?)
        })
        .await
    }
}
