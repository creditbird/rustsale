use reqwest::Method;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::HashMap;

use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{AvatarSize, User, UserBasic, ZBusinessPackage};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetFriendBoardListResponse {
    pub data: Vec<String>,
    pub version: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasItem {
    pub user_id: String,
    pub alias: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAliasListResponse {
    pub items: Vec<AliasItem>,
    pub update_time: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendOnlineStatus {
    pub user_id: String,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFriendOnlinesResponse {
    pub predefine: Vec<String>,
    pub owner_status: String,
    pub onlines: Vec<FriendOnlineStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FriendRecommendationsType {
    RecommendedFriend = 1,
    ReceivedFriendRequest = 2,
}

impl Serialize for FriendRecommendationsType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u8(*self as u8)
    }
}

impl<'de> Deserialize<'de> for FriendRecommendationsType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let val = u8::deserialize(deserializer)?;
        match val {
            1 => Ok(FriendRecommendationsType::RecommendedFriend),
            2 => Ok(FriendRecommendationsType::ReceivedFriendRequest),
            _ => Ok(FriendRecommendationsType::RecommendedFriend),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRecommendationsCollapseMsgListConfig {
    pub collapse_id: i64,
    pub collapse_x_item: i64,
    pub collapse_y_item: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRecommendationsRecommInfo {
    pub suggest_way: i32,
    pub source: i32,
    pub message: String,
    pub custom_text: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRecommendationsDataInfo {
    pub user_id: String,
    pub zalo_name: String,
    pub display_name: String,
    pub avatar: String,
    pub phone_number: String,
    pub status: String,
    pub gender: crate::models::Gender,
    pub dob: u32,
    pub r#type: i32,
    pub recomm_type: FriendRecommendationsType,
    pub recomm_src: i32,
    pub recomm_time: u64,
    pub recomm_info: FriendRecommendationsRecommInfo,
    pub biz_pkg: ZBusinessPackage,
    pub is_seen_friend_req: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRecommendationsRecommItem {
    pub recomm_item_type: i32,
    pub data_info: FriendRecommendationsDataInfo,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFriendRecommendationsResponse {
    pub expired_duration: u64,
    pub collapse_msg_list_config: FriendRecommendationsCollapseMsgListConfig,
    pub recomm_items: Vec<FriendRecommendationsRecommItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetFriendRequestStatusResponse {
    pub add_friend_privacy: i32,
    pub is_seen_friend_req: bool,
    pub is_friend: i32,
    pub is_requested: i32,
    pub is_requesting: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FriendRequestInfo {
    pub message: String,
    pub src: i32,
    pub time: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentFriendRequestInfo {
    pub user_id: String,
    pub zalo_name: String,
    pub display_name: String,
    pub avatar: String,
    pub global_id: String,
    pub biz_pkg: ZBusinessPackage,
    pub f_req_info: FriendRequestInfo,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetRelatedFriendGroupResponse {
    pub group_relateds: HashMap<String, Vec<String>>,
}

impl Zalo {
    /// Accept a friend request from a User.
    pub async fn accept_friend_request(&self, friend_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
            "language": self.ctx.language,
        });
        self.request_api_form("friend", "/api/friend/accept", params)
            .await
    }

    /// Reject a friend request from a User.
    pub async fn reject_friend_request(&self, friend_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
        });
        self.request_api_form("friend", "/api/friend/reject", params)
            .await
    }

    /// Send a friend request to a user.
    pub async fn send_friend_request(&self, msg: &str, user_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "toid": user_id,
            "msg": msg,
            "reqsrc": 30,
            "imei": self.ctx.imei,
            "language": self.ctx.language,
            "srcParams": serde_json::json!({ "uidTo": user_id }).to_string(),
        });
        self.request_api_form("friend", "/api/friend/sendreq", params)
            .await
    }

    /// Undo send a friend request to a user.
    pub async fn undo_friend_request(&self, friend_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
        });
        self.request_api_form("friend", "/api/friend/undo", params)
            .await
    }

    /// Remove a friend.
    pub async fn remove_friend(&self, friend_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
            "imei": self.ctx.imei,
        });
        self.request_api_form("friend", "/api/friend/remove", params)
            .await
    }

    /// Change a friend's alias.
    pub async fn change_friend_alias(
        &self,
        alias: &str,
        friend_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "friendId": friend_id,
            "alias": alias,
            "imei": self.ctx.imei,
        });
        self.request_api(Method::GET, "alias", "/api/alias/update", params, true)
            .await
    }

    /// Remove a friend's alias.
    pub async fn remove_friend_alias(&self, friend_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "friendId": friend_id,
        });
        self.request_api(Method::GET, "alias", "/api/alias/remove", params, true)
            .await
    }

    /// Get alias list.
    pub async fn get_alias_list(
        &self,
        count: Option<u32>,
        page: Option<u32>,
    ) -> Result<GetAliasListResponse, ZaloError> {
        let params = serde_json::json!({
            "page": page.unwrap_or(1),
            "count": count.unwrap_or(100),
            "imei": self.ctx.imei,
        });
        self.request_api(Method::GET, "alias", "/api/alias/list", params, true)
            .await
    }

    /// Get all friends.
    pub async fn get_all_friends(
        &self,
        count: Option<u32>,
        page: Option<u32>,
        avatar_size: Option<AvatarSize>,
    ) -> Result<Vec<User>, ZaloError> {
        let params = serde_json::json!({
            "incInvalid": 1,
            "page": page.unwrap_or(1),
            "count": count.unwrap_or(20000),
            "avatar_size": avatar_size.unwrap_or(AvatarSize::Small),
            "actiontime": 0,
            "imei": self.ctx.imei,
        });
        self.request_api(
            Method::GET,
            "profile",
            "/api/social/friend/getfriends",
            params,
            true,
        )
        .await
    }

    /// Get close friends.
    pub async fn get_close_friends(&self) -> Result<Vec<User>, ZaloError> {
        self.request_api(
            Method::GET,
            "profile",
            "/api/social/friend/getclosedfriends",
            serde_json::json!({}),
            true,
        )
        .await
    }

    /// Get online status of friends.
    pub async fn get_friend_onlines(&self) -> Result<GetFriendOnlinesResponse, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        let mut res: GetFriendOnlinesResponse = self
            .request_api(
                Method::GET,
                "profile",
                "/api/social/friend/onlines",
                params,
                true,
            )
            .await?;

        for online in &mut res.onlines {
            if let Ok(parsed) = serde_json::from_str::<Value>(&online.status) {
                if let Some(status_str) = parsed.get("status").and_then(|s| s.as_str()) {
                    online.status = status_str.to_string();
                }
            }
        }

        Ok(res)
    }

    /// Get friend recommendations / received friend requests.
    pub async fn get_friend_recommendations(
        &self,
    ) -> Result<GetFriendRecommendationsResponse, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        self.request_api(
            Method::GET,
            "friend",
            "/api/friend/recommendsv2/list",
            params,
            true,
        )
        .await
    }

    /// Get friend request status.
    pub async fn get_friend_request_status(
        &self,
        friend_id: &str,
    ) -> Result<GetFriendRequestStatusResponse, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
            "imei": self.ctx.imei,
        });
        self.request_api(Method::GET, "friend", "/api/friend/reqstatus", params, true)
            .await
    }

    /// Get sent friend requests.
    pub async fn get_sent_friend_request(
        &self,
    ) -> Result<HashMap<String, SentFriendRequestInfo>, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        self.request_api(
            Method::GET,
            "friend",
            "/api/friend/requested/list",
            params,
            true,
        )
        .await
    }

    /// Find user by phone number. Returns `None` if error code is 216 (User not found).
    pub async fn find_user(
        &self,
        phone_number: &str,
        avatar_size: Option<AvatarSize>,
    ) -> Result<Option<UserBasic>, ZaloError> {
        let mut phone = phone_number.to_string();
        if phone.starts_with('0') && self.ctx.language == "vi" {
            phone = format!("84{}", &phone[1..]);
        }

        let params = serde_json::json!({
            "phone": phone,
            "avatar_size": avatar_size.unwrap_or(AvatarSize::Large),
            "language": self.ctx.language,
            "imei": self.ctx.imei,
            "reqSrc": 40,
        });

        match self
            .request_api::<_, UserBasic>(
                Method::GET,
                "friend",
                "/api/friend/profile/get",
                params,
                true,
            )
            .await
        {
            Ok(user) => Ok(Some(user)),
            Err(ZaloError::ApiError {
                code: Some(216), ..
            }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Find user by username.
    pub async fn find_user_by_username(
        &self,
        username: &str,
        avatar_size: Option<AvatarSize>,
    ) -> Result<UserBasic, ZaloError> {
        let params = serde_json::json!({
            "user_name": username,
            "avatar_size": avatar_size.unwrap_or(AvatarSize::Large),
        });
        self.request_api(
            Method::GET,
            "friend",
            "/api/friend/search/by-user-name",
            params,
            true,
        )
        .await
    }

    /// Get multiple users by phone numbers.
    pub async fn get_multi_users_by_phones(
        &self,
        phone_numbers: &[String],
        avatar_size: Option<AvatarSize>,
    ) -> Result<HashMap<String, UserBasic>, ZaloError> {
        let mapped_phones: Vec<String> = phone_numbers
            .iter()
            .map(|phone| {
                if phone.starts_with('0') && self.ctx.language == "vi" {
                    format!("84{}", &phone[1..])
                } else {
                    phone.clone()
                }
            })
            .collect();

        let params = serde_json::json!({
            "phones": mapped_phones,
            "avatar_size": avatar_size.unwrap_or(AvatarSize::Large),
            "language": self.ctx.language,
        });

        self.request_api(
            Method::GET,
            "friend",
            "/api/friend/profile/multiget",
            params,
            true,
        )
        .await
    }

    /// Get related friend group.
    pub async fn get_related_friend_group(
        &self,
        friend_ids: &[String],
    ) -> Result<GetRelatedFriendGroupResponse, ZaloError> {
        let params = serde_json::json!({
            "friend_ids": serde_json::to_string(friend_ids)?,
            "imei": self.ctx.imei,
        });
        self.request_api_form("friend", "/api/friend/group/related", params)
            .await
    }

    /// Block a user.
    pub async fn block_user(&self, user_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": user_id,
            "imei": self.ctx.imei,
        });
        self.request_api_form("friend", "/api/friend/block", params)
            .await
    }

    /// Unblock a user.
    pub async fn unblock_user(&self, user_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "fid": user_id,
            "imei": self.ctx.imei,
        });
        self.request_api_form("friend", "/api/friend/unblock", params)
            .await
    }

    /// Get the friend board list for a conversation.
    pub async fn get_friend_board_list(
        &self,
        conversation_id: &str,
    ) -> Result<GetFriendBoardListResponse, ZaloError> {
        let params = serde_json::json!({
            "conversationId": conversation_id,
            "version": 0,
            "imei": self.ctx.imei,
        });
        self.request_api(
            Method::GET,
            "friend_board",
            "api/friendboard/list",
            params,
            true,
        )
        .await
    }
}
