//! Outreach templates, Zalo and email outreach, WhatsApp campaigns and the unified
//! multi-channel campaign center.

use crate::error::CoreApiError;
use crate::runtime::{body, on_blocking, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::campaign::commands as unified;
use rustsale_core::commands::{self, DbState};
use rustsale_core::conversation::commands::{self as conversation, ConversationState};
use rustsale_core::conversation::whatsapp::campaign::{
    CampaignRecipientInput, CreateCampaignRequest,
};
use rustsale_core::models::{Campaign, CampaignStep};

/// A JS number as a database id; anything not finite becomes 0, which the core reports as missing.
fn id(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

/// A JS number as a lead id, refusing values an `i32` cannot hold.
fn lead_ids(values: Vec<f64>) -> Result<Vec<i32>, CoreApiError> {
    values
        .into_iter()
        .map(|value| {
            if value.is_finite() && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX) {
                Ok(value as i32)
            } else {
                Err(CoreApiError::invalid("lead id is out of range"))
            }
        })
        .collect()
}

/// A JS number as an `i32` row id, refusing values an `i32` cannot hold instead of wrapping them
/// onto another row.
fn id32(value: f64) -> Result<i32, CoreApiError> {
    if value.is_finite() && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX) {
        Ok(value as i32)
    } else {
        Err(CoreApiError::invalid("id is out of range"))
    }
}

/// An optional JS number as a `u32`.
fn optional_u32(value: Option<f64>) -> Option<u32> {
    value
        .filter(|n| n.is_finite() && *n >= 0.0 && *n <= f64::from(u32::MAX))
        .map(|n| n as u32)
}

/// An optional JS number as a `u64`.
fn optional_u64(value: Option<f64>) -> Option<u64> {
    value
        .filter(|n| n.is_finite() && *n >= 0.0 && *n <= u64::MAX as f64)
        .map(|n| n as u64)
}

/// Outreach templates, Zalo and email outreach jobs, WhatsApp campaigns and unified campaigns.
/// Long jobs answer when they start and report through `Host.onEvent` (`outreach-progress`,
/// `email-outreach-progress`, `campaign-progress`).
#[module]
pub struct Campaigns;

#[module]
impl Campaigns {
    /// All Zalo outreach templates, as `{ id, name, content, image_url, created_at }`.
    pub async fn outreach_templates() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_outreach_templates(state)?)
        })
        .await
    }

    /// Creates a Zalo outreach template and resolves its id. `imageUrl` may be absent.
    pub async fn create_outreach_template(
        name: String,
        content: String,
        image_url: Option<String>,
    ) -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::create_outreach_template(
                name, content, image_url, state,
            )?)
        })
        .await
    }

    /// Replaces the name, content and image of outreach template `id`.
    pub async fn update_outreach_template(
        template_id: f64,
        name: String,
        content: String,
        image_url: Option<String>,
    ) -> Result<JsonValue, CoreApiError> {
        let template = id32(template_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::update_outreach_template(
                template, name, content, image_url, state,
            )?)
        })
        .await
    }

    /// Deletes outreach template `id`.
    pub async fn delete_outreach_template(template_id: f64) -> Result<JsonValue, CoreApiError> {
        let template = id32(template_id)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_outreach_template(template, state)?)
        })
        .await
    }

    /// Sends Zalo outreach from account `imei` with template `templateId` to `leadIds`; progress
    /// arrives as `outreach-progress`, `outreach-log` and `outreach-complete` events.
    pub async fn send_zalo_outreach(
        imei: String,
        lead_ids_: Vec<f64>,
        template_id: f64,
    ) -> Result<JsonValue, CoreApiError> {
        let leads = lead_ids(lead_ids_)?;
        let template = id32(template_id)?;
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(commands::send_zalo_outreach(h, imei, leads, template).await?)
        })
        .await
    }

    /// Starts a batched Zalo outreach from `imeis` with template `templateId` to `leadIds`,
    /// optionally throttled by a burst limit and inter-message delay; progress arrives as events.
    pub async fn start_zalo_outreach(
        imeis: Vec<String>,
        template_id: f64,
        lead_ids_: Vec<f64>,
        enable_burst_limit: Option<bool>,
        burst_max_messages: Option<f64>,
        burst_delay_seconds: Option<f64>,
        inter_message_delay_seconds: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let leads = lead_ids(lead_ids_)?;
        let template = id32(template_id)?;
        let burst_max = optional_u32(burst_max_messages);
        let burst_delay = optional_u64(burst_delay_seconds);
        let inter_delay = optional_u64(inter_message_delay_seconds);
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(
                commands::start_zalo_outreach(
                    h,
                    imeis,
                    template,
                    leads,
                    enable_burst_limit,
                    burst_max,
                    burst_delay,
                    inter_delay,
                )
                .await?,
            )
        })
        .await
    }

    /// Sends email outreach through SMTP account `smtpId` with template `templateId` to `leadIds`;
    /// the job runs in the background and reports `email-outreach-progress`/`-log`/`-complete`.
    pub async fn send_email_outreach(
        smtp_id: f64,
        template_id: f64,
        lead_ids_: Vec<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let leads = lead_ids(lead_ids_)?;
        let smtp = id32(smtp_id)?;
        let template = id32(template_id)?;
        on_core(move |h| async move {
            Ok::<_, CoreApiError>(commands::send_email_outreach(h, smtp, template, leads).await?)
        })
        .await
    }

    /// Creates a WhatsApp campaign from `request` (the core's snake_case fields: `name`,
    /// `assigned_account_ids`, and `template_id`, `rate_limit_min_delay_secs`,
    /// `rate_limit_max_delay_secs`, `burst_limit_per_batch`, `burst_pause_secs`,
    /// `quiet_hours_enabled`, `quiet_hours_start`, `quiet_hours_end`, `timezone` as `null` or a
    /// value) and resolves its id.
    pub async fn create_whatsapp_campaign(request: JsonValue) -> Result<JsonValue, CoreApiError> {
        let request: CreateCampaignRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::create_whatsapp_campaign(state, request)?)
        })
        .await
    }

    /// Replaces the recipients of WhatsApp campaign `campaignId`; `recipients` are
    /// `{ lead_id, recipient_phone_raw, template, lead_vars }` and `template` is the default text.
    pub async fn populate_whatsapp_campaign_recipients(
        campaign_id: f64,
        recipients: JsonValue,
        template: Option<String>,
    ) -> Result<JsonValue, CoreApiError> {
        let recipients: Vec<CampaignRecipientInput> = body(recipients)?;
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::populate_whatsapp_campaign_recipients(
                state, campaign, recipients, template,
            )?)
        })
        .await
    }

    /// Starts the worker of WhatsApp campaign `campaignId`.
    pub async fn start_whatsapp_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::start_whatsapp_campaign(state, campaign).await?)
        })
        .await
    }

    /// Pauses WhatsApp campaign `campaignId`.
    pub async fn pause_whatsapp_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::pause_whatsapp_campaign(state, campaign)?)
        })
        .await
    }

    /// Resumes a paused WhatsApp campaign `campaignId`.
    pub async fn resume_whatsapp_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::resume_whatsapp_campaign(state, campaign)?)
        })
        .await
    }

    /// Cancels WhatsApp campaign `campaignId`.
    pub async fn cancel_whatsapp_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::cancel_whatsapp_campaign(state, campaign)?)
        })
        .await
    }

    /// One WhatsApp campaign with its counters.
    pub async fn whatsapp_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::get_whatsapp_campaign(state, campaign)?)
        })
        .await
    }

    /// All WhatsApp campaigns, newest first.
    pub async fn whatsapp_campaigns() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::list_whatsapp_campaigns(state)?)
        })
        .await
    }

    /// Recipients of WhatsApp campaign `campaignId`, with send status and the last error.
    pub async fn whatsapp_campaign_recipients(
        campaign_id: f64,
        limit_: Option<f64>,
        offset: Option<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        let limit = optional_u32(limit_);
        let offset = optional_u32(offset);
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::get_whatsapp_campaign_recipients(
                state, campaign, limit, offset,
            )?)
        })
        .await
    }

    /// Sets unified campaign `campaignId` active. It only changes the status; the scheduler sends.
    pub async fn start_unified_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::start_unified_campaign(campaign, state)?)
        })
        .await
    }

    /// Pauses unified campaign `campaignId`.
    pub async fn pause_unified_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::pause_unified_campaign(campaign, state)?)
        })
        .await
    }

    /// Enrolls `leadIds` at the first step of unified campaign `campaignId`; resolves how many
    /// were enrolled.
    pub async fn enroll_leads_into_campaign(
        campaign_id: f64,
        lead_ids_: Vec<f64>,
    ) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        let leads = lead_ids(lead_ids_)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::enroll_leads_into_campaign(campaign, leads, state)?)
        })
        .await
    }

    /// Per-step analytics of unified campaign `campaignId`, with response and conversion rates.
    pub async fn unified_campaign_analytics(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::get_unified_campaign_analytics(campaign, state)?)
        })
        .await
    }

    /// All unified campaigns with their metadata.
    pub async fn unified_campaigns() -> Result<JsonValue, CoreApiError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::get_all_unified_campaigns(state)?)
        })
        .await
    }

    /// Saves a unified campaign and its steps in one transaction and resolves the campaign id.
    /// `campaign` uses the core record's snake_case fields (`id?`, `name`, `description?`,
    /// `channel_type`, `status`, `audience_filter_json?`, `schedule_config_json?`, `created_at`,
    /// `updated_at`); `steps` uses `{ id?, campaign_id, step_order, channel, delay_minutes,
    /// condition_type, template_content, ai_prompt_override?, created_at, template_id? }`.
    pub async fn save_unified_campaign(
        campaign: JsonValue,
        steps: JsonValue,
    ) -> Result<JsonValue, CoreApiError> {
        let campaign: Campaign = body(campaign)?;
        let steps: Vec<CampaignStep> = body(steps)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::save_unified_campaign(campaign, steps, state)?)
        })
        .await
    }

    /// Deletes unified campaign `campaignId` and its steps.
    pub async fn delete_unified_campaign(campaign_id: f64) -> Result<JsonValue, CoreApiError> {
        let campaign = id(campaign_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(unified::delete_unified_campaign(campaign, state)?)
        })
        .await
    }
}
