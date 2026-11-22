//! Facebook Pages: accounts, scheduled posts, comment automations, page health and Messenger sync.

use crate::error::CoreApiError;
use crate::runtime::{body, on_blocking, on_core, state_of};
use nsh_core::JsonValue;
use nsh_core::prelude::*;
use rustsale_core::commands::{self, DbState};
use rustsale_core::conversation::commands as conversation;
use rustsale_core::conversation::commands::ConversationState;
use rustsale_core::facebook::commands as fb;
use rustsale_core::facebook::commands::{
    CreateCommentAutomationRequest, CreateFacebookPostRequest, UpdateCommentAutomationRequest,
};

/// A JS number as a database id; anything not finite becomes 0.
fn id(value: f64) -> i64 {
    if value.is_finite() { value as i64 } else { 0 }
}

/// An optional JS number as a result limit.
fn count(value: Option<f64>) -> Option<i64> {
    value
        .filter(|n| n.is_finite() && *n >= 0.0 && *n <= i64::MAX as f64)
        .map(|n| n as i64)
}

/// Facebook Pages and Messenger: connected accounts, post scheduling, comment automations,
/// token health and the Graph sync. Graph calls answer with `unavailable` when Meta cannot be
/// reached; nothing is retried here.
#[module]
pub struct Facebook;

#[module]
impl Facebook {
    /// Connected Facebook Pages with their token status, scopes and runtime status.
    pub async fn accounts() -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::get_managed_facebook_accounts(state)?)
        })
        .await
    }

    /// Deletes the Facebook Page token and account of `pageId`.
    pub async fn delete_account(page_id: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::delete_facebook_account(state, page_id)?)
        })
        .await
    }

    /// Verifies the token with Meta and saves (or updates) the Page. `request` is
    /// `{ pageId, accessToken, pageName?, avatarUrl?, grantedScopes? }`; an empty Page id or token
    /// rejects before any network call.
    pub async fn save_account(request: JsonValue) -> Result<JsonValue, JsError> {
        let request = body(request)?;
        on_core(move |h| async move {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(commands::save_managed_facebook_account(state, request).await?)
        })
        .await
    }

    /// Lists the Pages a Facebook user token grants. An empty token rejects before the Graph call.
    pub async fn discover_pages(
        token: String,
        app_id: Option<String>,
    ) -> Result<JsonValue, JsError> {
        on_core(move |_h| async move {
            Ok::<_, CoreApiError>(commands::fetch_facebook_pages_from_token(token, app_id).await?)
        })
        .await
    }

    /// Schedules a Page post from `request` (the core record's snake_case fields:
    /// `draft_group_id`, `channel_account_id`, `page_id`, `post_type`, `message`, `link_url`,
    /// `media_paths`, `scheduled_publish_time`) and resolves its id.
    pub async fn schedule_post(request: JsonValue) -> Result<JsonValue, JsError> {
        let request: CreateFacebookPostRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::schedule_facebook_post(state, request)?)
        })
        .await
    }

    /// Queues post `postId` for immediate publishing; resolves whether it changed.
    pub async fn publish_post_now(post_id: f64) -> Result<JsonValue, JsError> {
        let post = id(post_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::publish_facebook_post_now(state, post)?)
        })
        .await
    }

    /// Scheduled posts, newest first, at most 100; filtered to `pageId` when given.
    pub async fn scheduled_posts(page_id: Option<String>) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::list_facebook_scheduled_posts(state, page_id)?)
        })
        .await
    }

    /// Cancels draft or queued post `postId`; resolves whether it existed.
    pub async fn cancel_scheduled_post(post_id: f64) -> Result<JsonValue, JsError> {
        let post = id(post_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::cancel_facebook_scheduled_post(state, post)?)
        })
        .await
    }

    /// Creates a comment automation rule from `request` (`{ channelAccountId, pageId, ruleName,
    /// postIds?, keywords?, matchMode?, autoLike?, publicReplyTemplate?, privateReplyTemplate?,
    /// isEnabled? }`) and resolves its id.
    pub async fn create_comment_automation(request: JsonValue) -> Result<JsonValue, JsError> {
        let request: CreateCommentAutomationRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::create_comment_automation(state, request)?)
        })
        .await
    }

    /// Comment automation rules, optionally filtered to `pageId`.
    pub async fn comment_automations(page_id: Option<String>) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::list_comment_automations(state, page_id)?)
        })
        .await
    }

    /// Replaces the fields of a comment automation rule from `request` (`{ id, ruleName, postIds?,
    /// keywords?, matchMode?, autoLike?, publicReplyTemplate?, privateReplyTemplate?, isEnabled? }`).
    pub async fn update_comment_automation(request: JsonValue) -> Result<JsonValue, JsError> {
        let request: UpdateCommentAutomationRequest = body(request)?;
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::update_comment_automation(state, request)?)
        })
        .await
    }

    /// Deletes comment automation rule `id`; resolves whether it existed.
    pub async fn delete_comment_automation(rule_id: f64) -> Result<JsonValue, JsError> {
        let rule = id(rule_id);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::delete_comment_automation(state, rule)?)
        })
        .await
    }

    /// Comment reply logs, newest first, optionally filtered to `pageId`; `limit` absent means the
    /// core's default.
    pub async fn comment_logs(
        page_id: Option<String>,
        limit_: Option<f64>,
    ) -> Result<JsonValue, JsError> {
        let limit = count(limit_);
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::list_comment_logs(state, page_id, limit)?)
        })
        .await
    }

    /// The token and Page health of `pageId`: status, granted scopes, active posts and rules.
    pub async fn page_health(page_id: String) -> Result<JsonValue, JsError> {
        on_blocking(move |h| {
            let state = state_of::<DbState>(&h)?;
            Ok::<_, CoreApiError>(fb::get_facebook_page_health(state, page_id)?)
        })
        .await
    }

    /// Pulls Messenger conversations from the Graph API for `pageId`, or every Page when absent;
    /// resolves how many conversations were touched.
    pub async fn sync_conversations(page_id: Option<String>) -> Result<JsonValue, JsError> {
        on_core(move |h| async move {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::sync_facebook_conversations(state, page_id).await?)
        })
        .await
    }

    /// Ingests Messenger webhook `events` for `pageId` into SQLite; events for a Page that is not
    /// connected are ignored and resolve 0.
    pub async fn ingest_webhook_events(
        page_id: String,
        events: JsonValue,
    ) -> Result<JsonValue, JsError> {
        let events: Vec<JsonValue> = body(events)?;
        on_blocking(move |h| {
            let state = state_of::<ConversationState>(&h)?;
            Ok::<_, CoreApiError>(conversation::ingest_facebook_webhook_events(
                state, page_id, events,
            )?)
        })
        .await
    }
}
