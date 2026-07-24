//! Reminder Submodule for Zalo API
//!
//! Provides APIs to create, edit, remove, and list reminders in both user and group chats.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{Reminder, ReminderGroup, ReminderRepeatMode, ThreadType};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReminderOptions {
    pub title: String,
    pub emoji: Option<String>,
    pub start_time: Option<u64>,
    pub repeat: Option<ReminderRepeatMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditReminderOptions {
    pub title: String,
    pub topic_id: String,
    pub emoji: Option<String>,
    pub start_time: Option<u64>,
    pub repeat: Option<ReminderRepeatMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListReminderOptions {
    pub page: Option<u32>,
    pub count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetReminderResponsesResponse {
    pub reject_member: Vec<String>,
    pub accept_member: Vec<String>,
}

impl Zalo {
    /// Create a reminder in a thread (User or Group)
    pub async fn create_reminder(
        &self,
        options: CreateReminderOptions,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Reminder, ZaloError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        match thread_type {
            ThreadType::User => {
                let object_data = serde_json::json!({
                    "toUid": thread_id,
                    "type": 0,
                    "color": -16245706,
                    "emoji": options.emoji.as_deref().unwrap_or("⏰"),
                    "startTime": options.start_time.unwrap_or(now_ms),
                    "duration": -1,
                    "params": { "title": options.title },
                    "needPin": false,
                    "repeat": options.repeat.unwrap_or(ReminderRepeatMode::None),
                    "creatorUid": self.ctx.uid.lock().unwrap_or_else(|e| e.into_inner()).as_deref().unwrap_or(""),
                    "src": 1,
                });

                let params = serde_json::json!({
                    "objectData": serde_json::to_string(&object_data)?,
                    "imei": self.ctx.imei,
                });

                self.request_api_form("group_board", "api/board/oneone/create", params)
                    .await
            }
            ThreadType::Group => {
                let params = serde_json::json!({
                    "grid": thread_id,
                    "type": 0,
                    "color": -16245706,
                    "emoji": options.emoji.as_deref().unwrap_or("⏰"),
                    "startTime": options.start_time.unwrap_or(now_ms),
                    "duration": -1,
                    "params": serde_json::to_string(&serde_json::json!({ "title": options.title }))?,
                    "repeat": options.repeat.unwrap_or(ReminderRepeatMode::None),
                    "src": 1,
                    "imei": self.ctx.imei,
                    "pinAct": 0,
                });

                self.request_api_form("group_board", "api/board/topic/createv2", params)
                    .await
            }
        }
    }

    /// Edit an existing reminder
    pub async fn edit_reminder(
        &self,
        options: EditReminderOptions,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Reminder, ZaloError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        match thread_type {
            ThreadType::User => {
                let object_data = serde_json::json!({
                    "toUid": thread_id,
                    "type": 0,
                    "color": -16777216,
                    "emoji": options.emoji.as_deref().unwrap_or(""),
                    "startTime": options.start_time.unwrap_or(now_ms),
                    "duration": -1,
                    "params": { "title": options.title },
                    "needPin": false,
                    "reminderId": options.topic_id,
                    "repeat": options.repeat.unwrap_or(ReminderRepeatMode::None),
                });

                let params = serde_json::json!({
                    "objectData": serde_json::to_string(&object_data)?,
                });

                self.request_api_form("group_board", "api/board/oneone/update", params)
                    .await
            }
            ThreadType::Group => {
                let params = serde_json::json!({
                    "grid": thread_id,
                    "type": 0,
                    "color": -16777216,
                    "emoji": options.emoji.as_deref().unwrap_or(""),
                    "startTime": options.start_time.unwrap_or(now_ms),
                    "duration": -1,
                    "params": serde_json::to_string(&serde_json::json!({ "title": options.title }))?,
                    "topicId": options.topic_id,
                    "repeat": options.repeat.unwrap_or(ReminderRepeatMode::None),
                    "imei": self.ctx.imei,
                    "pinAct": 2,
                });

                self.request_api_form("group_board", "api/board/topic/updatev2", params)
                    .await
            }
        }
    }

    /// Remove a reminder from a user/group thread
    pub async fn remove_reminder(
        &self,
        reminder_id: &str,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let params = match thread_type {
            ThreadType::User => {
                serde_json::json!({
                    "uid": thread_id,
                    "reminderId": reminder_id,
                })
            }
            ThreadType::Group => {
                serde_json::json!({
                    "grid": thread_id,
                    "topicId": reminder_id,
                    "imei": self.ctx.imei,
                })
            }
        };

        let path = match thread_type {
            ThreadType::User => "api/board/oneone/remove",
            ThreadType::Group => "api/board/topic/remove",
        };

        self.request_api_form("group_board", path, params).await
    }

    /// Retrieve the list of reminders for a thread
    pub async fn get_list_reminder(
        &self,
        options: ListReminderOptions,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Vec<Reminder>, ZaloError> {
        let object_data = match thread_type {
            ThreadType::User => {
                serde_json::json!({
                    "uid": thread_id,
                    "board_type": 1,
                    "page": options.page.unwrap_or(1),
                    "count": options.count.unwrap_or(20),
                    "last_id": 0,
                    "last_type": 0,
                })
            }
            ThreadType::Group => {
                serde_json::json!({
                    "group_id": thread_id,
                    "board_type": 1,
                    "page": options.page.unwrap_or(1),
                    "count": options.count.unwrap_or(20),
                    "last_id": 0,
                    "last_type": 0,
                })
            }
        };

        let mut params = serde_json::json!({
            "objectData": serde_json::to_string(&object_data)?,
        });

        if thread_type == ThreadType::Group {
            if let Some(obj) = params.as_object_mut() {
                obj.insert("imei".to_string(), Value::String(self.ctx.imei.clone()));
            }
        }

        let path = match thread_type {
            ThreadType::User => "api/board/oneone/list",
            ThreadType::Group => "api/board/listReminder",
        };

        let res_val: Value = self
            .request_api(reqwest::Method::GET, "group_board", path, params, true)
            .await?;

        let res_str = res_val.as_str().ok_or_else(|| {
            ZaloError::PayloadFormat(
                "Expected a stringified JSON array in response data".to_string(),
            )
        })?;

        let list: Vec<Reminder> = serde_json::from_str(res_str)?;
        Ok(list)
    }

    /// Get reminder details from a group thread
    pub async fn get_reminder(&self, reminder_id: &str) -> Result<ReminderGroup, ZaloError> {
        let params = serde_json::json!({
            "eventId": reminder_id,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::GET,
            "group_board",
            "api/board/topic/getReminder",
            params,
            true,
        )
        .await
    }

    /// Retrieve the list of responses for a reminder in a group thread
    pub async fn get_reminder_responses(
        &self,
        reminder_id: &str,
    ) -> Result<GetReminderResponsesResponse, ZaloError> {
        let params = serde_json::json!({
            "eventId": reminder_id,
        });

        self.request_api(
            reqwest::Method::GET,
            "group_board",
            "api/board/topic/listResponseEvent",
            params,
            true,
        )
        .await
    }
}
