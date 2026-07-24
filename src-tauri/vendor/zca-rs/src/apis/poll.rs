use serde_json::Value;

use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{PollDetail, PollOptions};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreatePollOptions {
    pub question: String,
    pub options: Vec<String>,
    pub expired_time: Option<u64>,
    pub allow_multi_choices: Option<bool>,
    pub allow_add_new_option: Option<bool>,
    pub hide_vote_preview: Option<bool>,
    pub is_anonymous: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VotePollResponse {
    pub options: Vec<PollOptions>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddPollOptionsOption {
    pub voted: bool,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddPollOptionsPayload {
    pub poll_id: u64,
    pub options: Vec<AddPollOptionsOption>,
    pub voted_option_ids: Vec<u64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddPollOptionsResponse {
    pub options: Vec<PollOptions>,
}

impl Zalo {
    /// Create a poll in a group.
    pub async fn create_poll(
        &self,
        options: CreatePollOptions,
        group_id: &str,
    ) -> Result<PollDetail, ZaloError> {
        let params = serde_json::json!({
            "group_id": group_id,
            "question": options.question,
            "options": options.options,
            "expired_time": options.expired_time.unwrap_or(0),
            "pinAct": false,
            "allow_multi_choices": options.allow_multi_choices.unwrap_or(false),
            "allow_add_new_option": options.allow_add_new_option.unwrap_or(false),
            "is_hide_vote_preview": options.hide_vote_preview.unwrap_or(false),
            "is_anonymous": options.is_anonymous.unwrap_or(false),
            "poll_type": 0,
            "src": 1,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::POST,
            "group",
            "api/poll/create",
            params,
            true,
        )
        .await
    }

    /// Vote on a poll
    pub async fn vote_poll(
        &self,
        poll_id: u64,
        option_ids: &[u32],
    ) -> Result<VotePollResponse, ZaloError> {
        let params = serde_json::json!({
            "poll_id": poll_id,
            "option_ids": option_ids,
            "imei": self.ctx.imei,
        });

        self.request_api(reqwest::Method::GET, "group", "api/poll/vote", params, true)
            .await
    }

    /// Lock a poll, preventing further votes.
    pub async fn lock_poll(&self, poll_id: u64) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "poll_id": poll_id,
            "imei": self.ctx.imei,
        });

        self.request_api(reqwest::Method::POST, "group", "api/poll/end", params, true)
            .await
    }

    /// Add new options to a poll
    pub async fn add_poll_options(
        &self,
        payload: AddPollOptionsPayload,
    ) -> Result<AddPollOptionsResponse, ZaloError> {
        let options_str = serde_json::to_string(&payload.options)?;
        let params = serde_json::json!({
            "poll_id": payload.poll_id,
            "new_options": options_str,
            "voted_option_ids": payload.voted_option_ids,
        });

        self.request_api(
            reqwest::Method::GET,
            "group",
            "api/poll/option/add",
            params,
            true,
        )
        .await
    }

    /// Get poll detail
    pub async fn get_poll_detail(&self, poll_id: u64) -> Result<PollDetail, ZaloError> {
        let params = serde_json::json!({
            "poll_id": poll_id,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::POST,
            "group",
            "api/poll/detail",
            params,
            true,
        )
        .await
    }

    /// Share a poll
    pub async fn share_poll(&self, poll_id: u64) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "poll_id": poll_id,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::POST,
            "group",
            "api/poll/share",
            params,
            true,
        )
        .await
    }
}
