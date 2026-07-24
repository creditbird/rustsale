use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::apis::Zalo;
use crate::context::{Context, ZaloCookie};
use crate::error::ZaloError;
use crate::models::{
    AttachmentSource, AutoReplyItem, AutoReplyScope, LabelData, QuickMessage, ThreadType,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateSettingsType {
    #[serde(rename = "view_birthday")]
    ViewBirthday,
    #[serde(rename = "show_online_status")]
    ShowOnlineStatus,
    #[serde(rename = "display_seen_status")]
    DisplaySeenStatus,
    #[serde(rename = "receive_message")]
    ReceiveMessage,
    #[serde(rename = "accept_stranger_call")]
    AcceptCall,
    #[serde(rename = "add_friend_via_phone")]
    AddFriendViaPhone,
    #[serde(rename = "add_friend_via_qr")]
    AddFriendViaQR,
    #[serde(rename = "add_friend_via_group")]
    AddFriendViaGroup,
    #[serde(rename = "add_friend_via_contact")]
    AddFriendViaContact,
    #[serde(rename = "display_on_recommend_friend")]
    DisplayOnRecommendFriend,
    #[serde(rename = "archivedChatStatus")]
    ArchivedChat,
    #[serde(rename = "quickMessageStatus")]
    QuickMessage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuteDuration {
    OneHour,
    FourHours,
    Forever,
    Until8AM,
    Custom(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MuteAction {
    Mute = 1,
    Unmute = 3,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MuteEntriesInfo {
    pub id: String,
    pub duration: i32,
    pub start_time: u64,
    pub system_time: u64,
    pub current_time: u64,
    pub mute_mode: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetMuteResponse {
    pub chat_entries: Vec<MuteEntriesInfo>,
    pub group_chat_entries: Vec<MuteEntriesInfo>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetLabelsResponse {
    pub label_data: Vec<LabelData>,
    pub version: u32,
    pub last_update_time: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLabelsResponse {
    pub label_data: Vec<LabelData>,
    pub version: u32,
    pub last_update_time: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoDeleteChatConvers {
    pub dest_id: String,
    pub is_group: bool,
    pub ttl: u64,
    pub created_at: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetAutoDeleteChatResponse {
    pub convers: Vec<AutoDeleteChatConvers>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTTL {
    NoDelete,
    OneDay,
    SevenDays,
    FourteenDays,
    Custom(u64),
}

impl ChatTTL {
    pub fn to_millis(&self) -> u64 {
        match self {
            ChatTTL::NoDelete => 0,
            ChatTTL::OneDay => 86400000,
            ChatTTL::SevenDays => 604800000,
            ChatTTL::FourteenDays => 1209600000,
            ChatTTL::Custom(ms) => *ms,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetAutoReplyListResponse {
    pub item: Vec<AutoReplyItem>,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetQuickMessageListResponse {
    pub cursor: u64,
    pub version: u64,
    pub items: Vec<QuickMessage>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HiddenConversationThread {
    pub is_group: u8,
    pub thread_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetHiddenConversationsResponse {
    pub pin: String,
    pub threads: Vec<HiddenConversationThread>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetPinConversationsResponse {
    pub conversations: Vec<String>,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArchivedChatListResponse {
    pub need_resync: bool,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeleteAutoReplyResponse {
    pub item: u64,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveQuickMessageResponse {
    pub item_ids: Vec<u64>,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateActiveStatusResponse {
    pub status: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateAutoReplyResponse {
    pub item: AutoReplyItem,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateAutoReplyResponse {
    pub item: AutoReplyItem,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddQuickMessageResponse {
    pub item: QuickMessage,
    pub version: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateQuickMessageResponse {
    pub item: QuickMessage,
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UpdateLangAvailableLanguages {
    VI,
    EN,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArchivedChatTarget {
    pub id: String,
    #[serde(rename = "type")]
    pub thread_type: ThreadType,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeleteChatLastMessage {
    pub owner_id: String,
    pub cli_msg_id: String,
    pub global_msg_id: String,
}

fn encrypt_pin(pin: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(pin.as_bytes());
    format!("{:x}", hasher.finalize())
}

impl Zalo {
    pub async fn get_qr(&self, user_id: &str) -> Result<HashMap<String, String>, ZaloError> {
        let params = serde_json::json!({
            "fids": vec![user_id],
        });
        self.request_api_form("friend", "api/friend/mget-qr", params)
            .await
    }

    pub async fn parse_link(&self, link: &str) -> Result<Value, ZaloError> {
        let data = serde_json::json!({
            "link": link,
            "version": 1,
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "file",
            "api/message/parselink",
            data,
            true,
        )
        .await
    }

    pub async fn keep_alive(&self) -> Result<Value, ZaloError> {
        let service_url = self.get_service_url("chat", "keepalive")?;
        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let params_json = serde_json::json!({
            "imei": self.ctx.imei,
        });
        let stringified = serde_json::to_string(&params_json)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())?;

        let url = self
            .ctx
            .make_url(&service_url, &[("params", &encrypted_params)], true)?;

        let response = self
            .ctx
            .reqwest_client
            .get(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Origin", "https://chat.zalo.me")
            .header("Referer", "https://chat.zalo.me/")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ZaloError::ApiError {
                message: format!("HTTP error: {}", response.status()),
                code: Some(response.status().as_u16() as i32),
            });
        }

        let response_json: Value = response.json().await?;
        let error_code = response_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if error_code != 0 {
            let msg = response_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("API error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        let data = response_json.get("data").cloned().unwrap_or(Value::Null);
        Ok(data)
    }

    pub async fn last_online(&self, uid: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "uid": uid,
            "conv_type": 1,
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/profile/lastOnline",
            params,
            true,
        )
        .await
    }

    pub async fn get_settings(&self) -> Result<Value, ZaloError> {
        let params = serde_json::json!({});
        self.request_api(
            reqwest::Method::GET,
            "https://wpa.chat.zalo.me/api/setting/me",
            "",
            params,
            true,
        )
        .await
    }

    pub async fn update_settings(
        &self,
        setting_type: UpdateSettingsType,
        value: u32,
    ) -> Result<Value, ZaloError> {
        let mut params = HashMap::new();
        let key = serde_json::to_value(setting_type)?
            .as_str()
            .unwrap_or("")
            .to_string();
        params.insert(key, value);
        self.request_api(
            reqwest::Method::GET,
            "https://wpa.chat.zalo.me/api/setting/update",
            "",
            params,
            true,
        )
        .await
    }

    pub async fn update_active_status(
        &self,
        active: bool,
    ) -> Result<UpdateActiveStatusResponse, ZaloError> {
        let params = serde_json::json!({
            "status": if active { 1 } else { 0 },
            "imei": self.ctx.imei,
        });
        let path = if active {
            "api/social/profile/ping"
        } else {
            "api/social/profile/deactive"
        };
        self.request_api(reqwest::Method::GET, "profile", path, params, true)
            .await
    }

    pub async fn block_view_feed(
        &self,
        is_block_feed: bool,
        user_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": user_id,
            "isBlockFeed": if is_block_feed { 1 } else { 0 },
            "imei": self.ctx.imei,
        });
        self.request_api_form("friend", "api/friend/feed/block", params)
            .await
    }

    pub async fn get_biz_account(&self, friend_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
        });
        self.request_api_form("profile", "api/social/friend/get-bizacc", params)
            .await
    }

    pub async fn get_hidden_conversations(
        &self,
    ) -> Result<GetHiddenConversationsResponse, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "conversation",
            "api/hiddenconvers/get-all",
            params,
            true,
        )
        .await
    }

    pub async fn set_hidden_conversations(
        &self,
        hidden: bool,
        thread_ids: &[&str],
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        if thread_ids.is_empty() {
            return Err(ZaloError::PayloadFormat(
                "thread_ids must not be empty".to_string(),
            ));
        }
        let is_group = if thread_type == ThreadType::Group {
            1
        } else {
            0
        };
        let thread_objs: Vec<Value> = thread_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "thread_id": id,
                    "is_group": is_group,
                })
            })
            .collect();
        let threads_str = serde_json::to_string(&thread_objs)?;

        let mut params = serde_json::Map::new();
        if hidden {
            params.insert("add_threads".to_string(), Value::String(threads_str));
            params.insert("del_threads".to_string(), Value::String("[]".to_string()));
        } else {
            params.insert("del_threads".to_string(), Value::String(threads_str));
            params.insert("add_threads".to_string(), Value::String("[]".to_string()));
        }
        params.insert("imei".to_string(), Value::String(self.ctx.imei.clone()));

        self.request_api_form(
            "conversation",
            "api/hiddenconvers/add-remove",
            Value::Object(params),
        )
        .await
    }

    pub async fn update_hidden_convers_pin(&self, pin: &str) -> Result<Value, ZaloError> {
        if pin.len() != 4 || !pin.chars().all(|c| c.is_ascii_digit()) {
            return Err(ZaloError::PayloadFormat(
                "Pin must be a 4-digit number".to_string(),
            ));
        }
        let encrypted_pin = encrypt_pin(pin);
        let params = serde_json::json!({
            "new_pin": encrypted_pin,
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "conversation",
            "api/hiddenconvers/update-pin",
            params,
            true,
        )
        .await
    }

    pub async fn reset_hidden_convers_pin(&self) -> Result<Value, ZaloError> {
        let params = serde_json::json!({});
        self.request_api(
            reqwest::Method::GET,
            "conversation",
            "api/hiddenconvers/reset",
            params,
            true,
        )
        .await
    }

    pub async fn get_pin_conversations(&self) -> Result<GetPinConversationsResponse, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "conversation",
            "api/pinconvers/list",
            params,
            true,
        )
        .await
    }

    pub async fn set_pinned_conversations(
        &self,
        pinned: bool,
        thread_ids: &[&str],
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let prefix = if thread_type == ThreadType::Group {
            "g"
        } else {
            "u"
        };
        let conversations: Vec<String> = thread_ids
            .iter()
            .map(|id| format!("{}{}", prefix, id))
            .collect();
        let params = serde_json::json!({
            "actionType": if pinned { 1 } else { 2 },
            "conversations": conversations,
        });
        self.request_api_form("conversation", "api/pinconvers/updatev2", params)
            .await
    }

    pub async fn get_unread_mark(&self) -> Result<Value, ZaloError> {
        let params = serde_json::json!({});
        let res: Value = self
            .request_api(
                reqwest::Method::GET,
                "conversation",
                "api/conv/getUnreadMark",
                params,
                true,
            )
            .await?;
        if let Some(s) = res.as_str() {
            let parsed = serde_json::from_str(s)?;
            Ok(parsed)
        } else {
            Ok(res)
        }
    }

    pub async fn add_unread_mark(
        &self,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let is_group = thread_type == ThreadType::Group;

        let mut inner_param = serde_json::Map::new();
        let unread_item = serde_json::json!({
            "id": thread_id,
            "cliMsgId": timestamp.to_string(),
            "fromUid": "0",
            "ts": timestamp,
        });

        if is_group {
            inner_param.insert("convsGroup".to_string(), serde_json::json!([unread_item]));
            inner_param.insert("convsUser".to_string(), serde_json::json!([]));
        } else {
            inner_param.insert("convsUser".to_string(), serde_json::json!([unread_item]));
            inner_param.insert("convsGroup".to_string(), serde_json::json!([]));
        }
        inner_param.insert("imei".to_string(), serde_json::json!(self.ctx.imei));

        let param_str = serde_json::to_string(&inner_param)?;
        let request_params = serde_json::json!({
            "param": param_str,
        });

        let res: Value = self
            .request_api_form("conversation", "api/conv/addUnreadMark", request_params)
            .await?;
        if let Some(s) = res.as_str() {
            let parsed = serde_json::from_str(s)?;
            Ok(parsed)
        } else {
            Ok(res)
        }
    }

    pub async fn remove_unread_mark(
        &self,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let is_group = thread_type == ThreadType::Group;

        let mut inner_param = serde_json::Map::new();
        let unread_data = serde_json::json!({
            "id": thread_id,
            "ts": timestamp,
        });

        if is_group {
            inner_param.insert("convsGroup".to_string(), serde_json::json!([thread_id]));
            inner_param.insert("convsUser".to_string(), serde_json::json!([]));
            inner_param.insert(
                "convsGroupData".to_string(),
                serde_json::json!([unread_data]),
            );
            inner_param.insert("convsUserData".to_string(), serde_json::json!([]));
        } else {
            inner_param.insert("convsUser".to_string(), serde_json::json!([thread_id]));
            inner_param.insert("convsGroup".to_string(), serde_json::json!([]));
            inner_param.insert(
                "convsUserData".to_string(),
                serde_json::json!([unread_data]),
            );
            inner_param.insert("convsGroupData".to_string(), serde_json::json!([]));
        }

        let param_str = serde_json::to_string(&inner_param)?;
        let request_params = serde_json::json!({
            "param": param_str,
        });

        let res: Value = self
            .request_api_form("conversation", "api/conv/removeUnreadMark", request_params)
            .await?;
        if let Some(s) = res.as_str() {
            let parsed = serde_json::from_str(s)?;
            Ok(parsed)
        } else {
            Ok(res)
        }
    }

    pub async fn get_labels(&self) -> Result<GetLabelsResponse, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });

        let res: Value = self
            .request_api(
                reqwest::Method::GET,
                "label",
                "api/convlabel/get",
                params,
                true,
            )
            .await?;

        let label_data_str = res
            .get("labelData")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ZaloError::PayloadFormat("Missing labelData field in response".to_string())
            })?;

        let label_data: Vec<LabelData> = serde_json::from_str(label_data_str)?;
        let version = res.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let last_update_time = res
            .get("lastUpdateTime")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(GetLabelsResponse {
            label_data,
            version,
            last_update_time,
        })
    }

    pub async fn update_labels(
        &self,
        label_data: Vec<LabelData>,
        version: u32,
    ) -> Result<UpdateLabelsResponse, ZaloError> {
        let label_data_str = serde_json::to_string(&label_data)?;
        let params = serde_json::json!({
            "labelData": label_data_str,
            "version": version,
            "imei": self.ctx.imei,
        });

        let res: Value = self
            .request_api_form("label", "api/convlabel/update", params)
            .await?;

        let label_data_str = res
            .get("labelData")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ZaloError::PayloadFormat("Missing labelData field in response".to_string())
            })?;

        let label_data: Vec<LabelData> = serde_json::from_str(label_data_str)?;
        let version = res.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let last_update_time = res
            .get("lastUpdateTime")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(UpdateLabelsResponse {
            label_data,
            version,
            last_update_time,
        })
    }

    pub async fn get_mute(&self) -> Result<GetMuteResponse, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/profile/getmute",
            params,
            true,
        )
        .await
    }

    pub async fn set_mute(
        &self,
        thread_id: &str,
        thread_type: ThreadType,
        duration: MuteDuration,
        action: MuteAction,
    ) -> Result<Value, ZaloError> {
        let mute_duration_secs = match action {
            MuteAction::Unmute => -1,
            MuteAction::Mute => match duration {
                MuteDuration::Forever => -1,
                MuteDuration::OneHour => 3600,
                MuteDuration::FourHours => 14400,
                MuteDuration::Custom(secs) => secs,
                MuteDuration::Until8AM => {
                    use chrono::{Duration as ChronoDuration, Local, Timelike};
                    let now = Local::now();
                    let mut next_8am = now
                        .with_hour(8)
                        .and_then(|t| t.with_minute(0))
                        .and_then(|t| t.with_second(0))
                        .and_then(|t| t.with_nanosecond(0))
                        .unwrap_or(now);

                    if now.hour() >= 8 {
                        next_8am = next_8am
                            .checked_add_signed(ChronoDuration::days(1))
                            .unwrap_or(next_8am);
                    }

                    let diff = next_8am.signed_duration_since(now).num_seconds();
                    if diff > 0 {
                        diff as i32
                    } else {
                        0
                    }
                }
            },
        };

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let params = serde_json::json!({
            "toid": thread_id,
            "duration": mute_duration_secs,
            "action": action as i32,
            "startTime": timestamp,
            "muteType": if thread_type == ThreadType::User { 1 } else { 2 },
            "imei": self.ctx.imei,
        });

        self.request_api_form("profile", "api/social/profile/setmute", params)
            .await
    }

    pub async fn get_archived_chat_list(&self) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "version": 1,
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "label",
            "api/archivedchat/list",
            params,
            true,
        )
        .await
    }

    pub async fn update_archived_chat_list(
        &self,
        is_archived: bool,
        targets: Vec<ArchivedChatTarget>,
    ) -> Result<UpdateArchivedChatListResponse, ZaloError> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let params = serde_json::json!({
            "actionType": if is_archived { 0 } else { 1 },
            "ids": targets,
            "imei": self.ctx.imei,
            "version": timestamp,
        });

        self.request_api_form("label", "api/archivedchat/update", params)
            .await
    }

    pub async fn get_auto_delete_chat(&self) -> Result<GetAutoDeleteChatResponse, ZaloError> {
        let params = serde_json::json!({});
        self.request_api(
            reqwest::Method::GET,
            "conversation",
            "api/conv/autodelete/getConvers",
            params,
            true,
        )
        .await
    }

    pub async fn update_auto_delete_chat(
        &self,
        ttl: ChatTTL,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "threadId": thread_id,
            "isGroup": if thread_type == ThreadType::Group { 1 } else { 0 },
            "ttl": ttl.to_millis(),
            "clientLang": self.ctx.language,
        });

        self.request_api_form("conversation", "api/conv/autodelete/updateConvers", params)
            .await
    }

    pub async fn get_auto_reply_list(&self) -> Result<GetAutoReplyListResponse, ZaloError> {
        let params = serde_json::json!({
            "version": 0,
            "cliLang": self.ctx.language,
        });
        self.request_api(
            reqwest::Method::GET,
            "auto_reply",
            "api/autoreply/list",
            params,
            true,
        )
        .await
    }

    pub async fn create_auto_reply(
        &self,
        content: &str,
        is_enable: bool,
        start_time: u64,
        end_time: u64,
        scope: AutoReplyScope,
        uids: Vec<String>,
    ) -> Result<CreateAutoReplyResponse, ZaloError> {
        let is_specific_or_except = matches!(
            scope,
            AutoReplyScope::SpecificFriends | AutoReplyScope::FriendsExcept
        );
        let result_uids = if is_specific_or_except {
            uids
        } else {
            Vec::new()
        };

        let params = serde_json::json!({
            "cliLang": self.ctx.language,
            "enable": is_enable,
            "content": content,
            "startTime": start_time,
            "endTime": end_time,
            "recurrence": vec!["RRULE:FREQ=DAILY;"],
            "scope": scope as u8,
            "uids": result_uids,
        });

        self.request_api_form("auto_reply", "api/autoreply/create", params)
            .await
    }

    #[expect(clippy::too_many_arguments)]
    pub async fn update_auto_reply(
        &self,
        id: u64,
        content: &str,
        is_enable: bool,
        start_time: u64,
        end_time: u64,
        scope: AutoReplyScope,
        uids: Vec<String>,
    ) -> Result<UpdateAutoReplyResponse, ZaloError> {
        let is_specific_or_except = matches!(
            scope,
            AutoReplyScope::SpecificFriends | AutoReplyScope::FriendsExcept
        );
        let result_uids = if is_specific_or_except {
            uids
        } else {
            Vec::new()
        };

        let params = serde_json::json!({
            "cliLang": self.ctx.language,
            "id": id,
            "enable": is_enable,
            "content": content,
            "startTime": start_time,
            "endTime": end_time,
            "recurrence": vec!["RRULE:FREQ=DAILY;"],
            "scope": scope as u8,
            "uids": result_uids,
        });

        self.request_api_form("auto_reply", "api/autoreply/update", params)
            .await
    }

    pub async fn delete_auto_reply(&self, id: u64) -> Result<DeleteAutoReplyResponse, ZaloError> {
        let params = serde_json::json!({
            "cliLang": self.ctx.language,
            "id": id,
        });
        self.request_api_form("auto_reply", "api/autoreply/delete", params)
            .await
    }

    pub async fn get_quick_message_list(&self) -> Result<GetQuickMessageListResponse, ZaloError> {
        let params = serde_json::json!({
            "version": 0,
            "lang": 0,
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "quick_message",
            "api/quickmessage/list",
            params,
            true,
        )
        .await
    }

    pub async fn add_quick_message(
        &self,
        keyword: &str,
        title: &str,
        media: Option<AttachmentSource>,
    ) -> Result<AddQuickMessageResponse, ZaloError> {
        let mut params = serde_json::json!({
            "keyword": keyword,
            "message": {
                "title": title,
                "params": "",
            },
            "type": if media.is_some() { 1 } else { 0 },
            "imei": self.ctx.imei,
        });

        if let Some(source) = media {
            let upload_media = self.upload_product_photo(&source).await?;
            let photo_id = upload_media.photo_id.clone().unwrap_or_default();
            let thumb_url = upload_media.thumb_url.clone().unwrap_or_default();
            let normal_url = upload_media
                .normal_url
                .clone()
                .or(upload_media.hd_url.clone())
                .unwrap_or_default();
            let hd_url = upload_media
                .hd_url
                .clone()
                .or(upload_media.normal_url.clone())
                .unwrap_or_default();

            if let Some(obj) = params.as_object_mut() {
                obj.insert(
                    "media".to_string(),
                    serde_json::json!({
                        "items": [
                            {
                                "type": 0,
                                "photoId": photo_id,
                                "title": "",
                                "width": "",
                                "height": "",
                                "previewThumb": thumb_url,
                                "rawUrl": normal_url,
                                "thumbUrl": thumb_url,
                                "normalUrl": normal_url,
                                "hdUrl": hd_url,
                            }
                        ]
                    }),
                );
            }
        }

        self.request_api(
            reqwest::Method::GET,
            "quick_message",
            "api/quickmessage/create",
            params,
            true,
        )
        .await
    }

    pub async fn update_quick_message(
        &self,
        item_id: u64,
        keyword: &str,
        title: &str,
        media: Option<AttachmentSource>,
    ) -> Result<UpdateQuickMessageResponse, ZaloError> {
        let mut params = serde_json::json!({
            "itemId": item_id,
            "keyword": keyword,
            "message": {
                "title": title,
                "params": "",
            },
            "type": if media.is_some() { 1 } else { 0 },
        });

        if let Some(source) = media {
            let upload_media = self.upload_product_photo(&source).await?;
            let photo_id = upload_media.photo_id.clone().unwrap_or_default();
            let thumb_url = upload_media.thumb_url.clone().unwrap_or_default();
            let normal_url = upload_media
                .normal_url
                .clone()
                .or(upload_media.hd_url.clone())
                .unwrap_or_default();
            let hd_url = upload_media
                .hd_url
                .clone()
                .or(upload_media.normal_url.clone())
                .unwrap_or_default();

            if let Some(obj) = params.as_object_mut() {
                obj.insert(
                    "media".to_string(),
                    serde_json::json!({
                        "items": [
                            {
                                "type": 0,
                                "photoId": photo_id,
                                "title": "",
                                "width": "",
                                "height": "",
                                "previewThumb": thumb_url,
                                "rawUrl": normal_url,
                                "thumbUrl": thumb_url,
                                "normalUrl": normal_url,
                                "hdUrl": hd_url,
                            }
                        ]
                    }),
                );
            }
        }

        self.request_api(
            reqwest::Method::GET,
            "quick_message",
            "api/quickmessage/update",
            params,
            true,
        )
        .await
    }

    pub async fn remove_quick_message(
        &self,
        item_ids: &[u64],
    ) -> Result<RemoveQuickMessageResponse, ZaloError> {
        let params = serde_json::json!({
            "itemIds": item_ids,
        });
        self.request_api(
            reqwest::Method::GET,
            "quick_message",
            "api/quickmessage/delete",
            params,
            true,
        )
        .await
    }

    pub async fn update_lang(
        &self,
        language: UpdateLangAvailableLanguages,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "language": match language {
                UpdateLangAvailableLanguages::VI => "VI",
                UpdateLangAvailableLanguages::EN => "EN",
            },
        });
        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/profile/updatelang",
            params,
            true,
        )
        .await
    }

    pub async fn delete_chat(
        &self,
        last_message: DeleteChatLastMessage,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let timestamp_string = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        let service = if thread_type == ThreadType::Group {
            "group"
        } else {
            "chat"
        };
        let path = if thread_type == ThreadType::Group {
            "api/group/deleteconver"
        } else {
            "api/message/deleteconver"
        };

        let params = if thread_type == ThreadType::User {
            serde_json::json!({
                "toid": thread_id,
                "cliMsgId": timestamp_string,
                "conver": {
                    "ownerId": last_message.owner_id,
                    "cliMsgId": last_message.cli_msg_id,
                    "globalMsgId": last_message.global_msg_id,
                },
                "onlyMe": 1,
                "imei": self.ctx.imei,
            })
        } else {
            serde_json::json!({
                "grid": thread_id,
                "cliMsgId": timestamp_string,
                "conver": {
                    "ownerId": last_message.owner_id,
                    "cliMsgId": last_message.cli_msg_id,
                    "globalMsgId": last_message.global_msg_id,
                },
                "onlyMe": 1,
                "imei": self.ctx.imei,
            })
        };

        self.request_api_form(service, path, params).await
    }

    pub fn get_cookie(&self) -> Vec<ZaloCookie> {
        self.ctx.cookie_jar.to_vec()
    }

    pub fn get_context(&self) -> Arc<Context> {
        Arc::clone(&self.ctx)
    }

    pub fn get_own_id(&self) -> Option<String> {
        self.ctx
            .uid
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Send an abuse report for a user or group thread.
    ///
    /// `content` is only sent when `reason` is [`ReportReason::Other`].
    pub async fn send_report(
        &self,
        reason: ReportReason,
        content: Option<&str>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Value, ZaloError> {
        let params = if thread_type == ThreadType::Group {
            serde_json::json!({
                "uidTo": thread_id,
                "type": 14,
                "reason": reason as i32,
                "content": if reason == ReportReason::Other { content.unwrap_or("") } else { "" },
                "imei": self.ctx.imei,
            })
        } else {
            let mut map = serde_json::Map::new();
            map.insert("idTo".to_string(), Value::String(thread_id.to_string()));
            map.insert(
                "objId".to_string(),
                Value::String("person.profile".to_string()),
            );
            map.insert(
                "reason".to_string(),
                Value::String((reason as i32).to_string()),
            );
            if reason == ReportReason::Other {
                map.insert(
                    "content".to_string(),
                    Value::String(content.unwrap_or("").to_string()),
                );
            }
            Value::Object(map)
        };

        let path = if thread_type == ThreadType::Group {
            "api/social/profile/reportabuse"
        } else {
            "api/report/abuse-v2"
        };

        self.request_api_form("profile", path, params).await
    }
}

/// Reason codes for [`Zalo::send_report`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportReason {
    Other = 0,
    Sensitive = 1,
    Annoy = 2,
    Fraud = 3,
}
