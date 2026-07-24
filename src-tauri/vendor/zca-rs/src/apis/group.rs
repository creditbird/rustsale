use reqwest::multipart::{Form, Part};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::apis::upload::get_image_dimensions_from_bytes;
use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{
    AttachmentSource, BoardType, GroupInfo, GroupMessage, GroupSetting, GroupTopic, TGroupMessage,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGroupChatHistoryResponse {
    pub last_action_id: String,
    pub last_action_id_other: String,
    pub more: i64,
    pub group_msgs: Vec<GroupMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoardItem {
    pub board_type: BoardType,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetListBoardResponse {
    pub items: Vec<BoardItem>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateGroupOptions {
    pub name: Option<String>,
    pub members: Vec<String>,
    pub avatar_source: Option<AttachmentSource>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateGroupResponse {
    pub group_type: i32,
    #[serde(rename = "sucessMembers")]
    pub success_members: Vec<String>,
    pub group_id: String,
    pub error_members: Vec<String>,
    pub error_data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddUserToGroupResponse {
    pub error_members: Vec<String>,
    pub error_data: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveUserFromGroupResponse {
    pub error_members: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaveGroupResponse {
    pub member_error: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfoPendingApprove {
    pub time: u64,
    pub uids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfoWithVer {
    #[serde(flatten)]
    pub info: GroupInfo,
    pub mem_ver_list: Option<Vec<String>>,
    pub pending_approve: Option<GroupInfoPendingApprove>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfoResponse {
    pub removeds_group: Vec<String>,
    pub unchangeds_group: Vec<String>,
    pub grid_info_map: HashMap<String, GroupInfoWithVer>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAllGroupsResponse {
    pub version: String,
    pub grid_ver_map: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupMemberProfile {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "zaloName")]
    pub zalo_name: String,
    pub avatar: String,
    #[serde(rename = "accountStatus")]
    pub account_status: i32,
    pub r#type: i32,
    #[serde(rename = "lastUpdateTime")]
    pub last_update_time: u64,
    #[serde(rename = "globalId")]
    pub global_id: String,
    pub id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetGroupMembersInfoResponse {
    pub profiles: HashMap<String, GroupMemberProfile>,
    pub unchangeds_profile: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChangeGroupOwnerResponse {
    pub time: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupBlockedMemberItem {
    pub id: String,
    #[serde(rename = "dName")]
    pub d_name: String,
    #[serde(rename = "zaloName")]
    pub zalo_name: String,
    pub avatar: String,
    pub avatar_25: String,
    #[serde(rename = "accountStatus")]
    pub account_status: i32,
    pub r#type: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetGroupBlockedMemberResponse {
    pub blocked_members: Vec<GroupBlockedMemberItem>,
    pub has_more: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLinkCurrentMem {
    pub id: String,
    #[serde(rename = "dName")]
    pub d_name: String,
    #[serde(rename = "zaloName")]
    pub zalo_name: String,
    pub avatar: String,
    pub avatar_25: String,
    pub account_status: i32,
    pub r#type: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGroupLinkInfoResponse {
    pub group_id: String,
    pub name: String,
    pub desc: String,
    pub r#type: i32,
    pub creator_id: String,
    pub avt: String,
    pub full_avt: String,
    pub admin_ids: Vec<String>,
    pub current_mems: Vec<GroupLinkCurrentMem>,
    pub admins: Option<Value>,
    pub has_more_member: i32,
    pub sub_type: i32,
    pub total_member: u32,
    pub setting: GroupSetting,
    pub global_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetGroupLinkDetailResponse {
    pub link: Option<String>,
    pub expiration_date: Option<u64>,
    pub enabled: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInviteBoxItem {
    pub group_info: GroupInfo,
    pub inviter_info: GroupLinkCurrentMem,
    pub gr_creator_info: GroupLinkCurrentMem,
    pub expired_ts: String,
    pub r#type: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGroupInviteBoxListResponse {
    pub invitations: Vec<GroupInviteBoxItem>,
    pub total: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetGroupInviteBoxInfoResponse {
    pub invitations: Option<Value>, // Contains invitation list or similar structure
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingGroupMemberItem {
    pub id: String,
    #[serde(rename = "dName")]
    pub d_name: String,
    #[serde(rename = "zaloName")]
    pub zalo_name: String,
    pub avatar: String,
    pub avatar_25: String,
    pub account_status: i32,
    pub r#type: i32,
    pub is_direct_invite: i32,
    pub request_time: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetPendingGroupMembersResponse {
    pub pending_members: Vec<PendingGroupMemberItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CreateNoteOptions {
    pub title: String,
    pub pin_act: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditNoteOptions {
    pub topic_id: String,
    pub title: String,
    pub pin_act: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGroupSettingsOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_name: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign_admin_msg: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_topic_only: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_msg_history: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub join_appr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_create_post: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_create_poll: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_send_msg: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lock_view_member: Option<bool>,
}

impl Zalo {
    pub async fn create_group(
        &self,
        options: CreateGroupOptions,
    ) -> Result<CreateGroupResponse, ZaloError> {
        if options.members.is_empty() {
            return Err(ZaloError::ApiError {
                message: "Group must have at least one member".to_string(),
                code: None,
            });
        }

        let now_ms = chrono::Utc::now().timestamp_millis() as u64;
        let mut params = serde_json::json!({
            "clientId": now_ms,
            "gname": now_ms.to_string(),
            "gdesc": serde_json::Value::Null,
            "members": options.members,
            "membersTypes": options.members.iter().map(|_| -1).collect::<Vec<i32>>(),
            "nameChanged": 0,
            "createLink": 1,
            "clientLang": self.ctx.language,
            "imei": self.ctx.imei,
            "zsource": 601,
        });

        if let Some(ref name) = options.name {
            if !name.is_empty() {
                if let Some(obj) = params.as_object_mut() {
                    obj.insert("gname".to_string(), serde_json::json!(name));
                    obj.insert("nameChanged".to_string(), serde_json::json!(1));
                }
            }
        }

        let res: CreateGroupResponse = self
            .request_api(Method::POST, "group", "api/group/create/v2", params, true)
            .await?;

        if let Some(ref avt) = options.avatar_source {
            let _ = self.change_group_avatar(avt.clone(), &res.group_id).await;
        }

        Ok(res)
    }

    pub async fn add_user_to_group(
        &self,
        member_id: Vec<String>,
        group_id: &str,
    ) -> Result<AddUserToGroupResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_id,
            "memberTypes": member_id.iter().map(|_| -1).collect::<Vec<i32>>(),
            "imei": self.ctx.imei,
            "clientLang": self.ctx.language,
        });

        self.request_api_form("group", "api/group/invite/v2", params)
            .await
    }

    pub async fn remove_user_from_group(
        &self,
        member_id: Vec<String>,
        group_id: &str,
    ) -> Result<RemoveUserFromGroupResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_id,
            "imei": self.ctx.imei,
        });

        self.request_api_form("group", "api/group/kickout", params)
            .await
    }

    pub async fn leave_group(
        &self,
        group_id: &str,
        silent: bool,
    ) -> Result<LeaveGroupResponse, ZaloError> {
        let params = serde_json::json!({
            "grids": [group_id],
            "imei": self.ctx.imei,
            "silent": if silent { 1 } else { 0 },
            "language": self.ctx.language,
        });

        self.request_api_form("group", "api/group/leave", params)
            .await
    }

    pub async fn get_group_info(
        &self,
        group_ids: Vec<String>,
    ) -> Result<GroupInfoResponse, ZaloError> {
        let mut grid_ver_map = HashMap::new();
        for gid in group_ids {
            grid_ver_map.insert(gid, 0);
        }

        let params = serde_json::json!({
            "gridVerMap": serde_json::to_string(&grid_ver_map)?,
        });

        self.request_api_form("group", "api/group/getmg-v2", params)
            .await
    }

    pub async fn get_all_groups(&self) -> Result<GetAllGroupsResponse, ZaloError> {
        let params = serde_json::json!({});
        self.request_api(
            Method::GET,
            "group_poll",
            "api/group/getlg/v4",
            params,
            true,
        )
        .await
    }

    pub async fn get_group_members_info(
        &self,
        member_ids: Vec<String>,
    ) -> Result<GetGroupMembersInfoResponse, ZaloError> {
        let friend_pversion_map: Vec<String> = member_ids
            .into_iter()
            .map(|id| {
                if id.ends_with("_0") {
                    id
                } else {
                    format!("{}_0", id)
                }
            })
            .collect();

        let params = serde_json::json!({
            "friend_pversion_map": friend_pversion_map,
        });

        self.request_api(
            Method::GET,
            "profile",
            "api/social/group/members",
            params,
            true,
        )
        .await
    }

    pub async fn add_group_deputy(
        &self,
        member_ids: Vec<String>,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_ids,
            "imei": self.ctx.imei,
        });

        self.request_api(Method::GET, "group", "api/group/admins/add", params, true)
            .await
    }

    pub async fn remove_group_deputy(
        &self,
        member_ids: Vec<String>,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_ids,
            "imei": self.ctx.imei,
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/admins/remove",
            params,
            true,
        )
        .await
    }

    pub async fn change_group_name(&self, name: &str, group_id: &str) -> Result<Value, ZaloError> {
        let final_name = if name.is_empty() {
            chrono::Utc::now().timestamp_millis().to_string()
        } else {
            name.to_string()
        };

        let params = serde_json::json!({
            "grid": group_id,
            "gname": final_name,
            "imei": self.ctx.imei,
        });

        self.request_api_form("group", "api/group/updateinfo", params)
            .await
    }

    pub async fn change_group_avatar(
        &self,
        avatar_source: AttachmentSource,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let (data, filename, metadata) = match avatar_source {
            AttachmentSource::Url(_) => {
                return Err(ZaloError::PayloadFormat(
                    "URL avatar source not supported".to_string(),
                ));
            }
            AttachmentSource::Buffer {
                data,
                filename,
                metadata,
            } => (data, filename, metadata),
        };

        let now_str = chrono::Local::now().format("%Y%m%d%H%M%S").to_string();
        let client_id = format!("g{}{}", group_id, now_str);

        let mut width = metadata.width.unwrap_or(0);
        let mut height = metadata.height.unwrap_or(0);
        if width == 0 || height == 0 {
            if let Some((w, h)) = get_image_dimensions_from_bytes(&data) {
                width = w;
                height = h;
            } else {
                width = 1080;
                height = 1080;
            }
        }

        let params = serde_json::json!({
            "grid": group_id,
            "avatarSize": 120,
            "clientId": client_id,
            "imei": self.ctx.imei,
            "originWidth": width,
            "originHeight": height,
        });

        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let stringified = serde_json::to_string(&params)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())?;

        let service_url = self.get_service_url("file", "api/group/upavatar")?;
        let url = self
            .ctx
            .make_url(&service_url, &[("params", &encrypted_params)], true)?;

        let part = Part::bytes(data)
            .file_name(filename)
            .mime_str("image/jpeg")?;
        let form = Form::new().part("fileContent", part);

        let response = self
            .ctx
            .reqwest_client
            .post(&url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Origin", "https://chat.zalo.me")
            .header("Referer", "https://chat.zalo.me/")
            .multipart(form)
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

        let encrypted_data = response_json
            .get("data")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ZaloError::PayloadFormat("Missing data field in encrypted response".to_string())
            })?;
        let decrypted_str = crate::crypto::aes::decode_aes(&secret_key, encrypted_data)?;
        let decrypted_json: Value = serde_json::from_str(&decrypted_str)?;

        let inner_error_code = decrypted_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if inner_error_code != 0 {
            let msg = decrypted_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Inner API error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(inner_error_code as i32),
            });
        }

        Ok(decrypted_json.get("data").cloned().unwrap_or(Value::Null))
    }

    pub async fn change_group_owner(
        &self,
        member_id: &str,
        group_id: &str,
    ) -> Result<ChangeGroupOwnerResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "newAdminId": member_id,
            "imei": self.ctx.imei,
            "language": self.ctx.language,
        });

        self.request_api(Method::GET, "group", "api/group/change-owner", params, true)
            .await
    }

    pub async fn disperse_group(&self, group_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "imei": self.ctx.imei,
        });

        self.request_api_form("group", "api/group/disperse", params)
            .await
    }

    pub async fn get_group_blocked_member(
        &self,
        page: Option<u32>,
        count: Option<u32>,
        group_id: &str,
    ) -> Result<GetGroupBlockedMemberResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "page": page.unwrap_or(1),
            "count": count.unwrap_or(50),
            "imei": self.ctx.imei,
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/blockedmems/list",
            params,
            true,
        )
        .await
    }

    pub async fn add_group_blocked_member(
        &self,
        member_ids: Vec<String>,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_ids,
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/blockedmems/add",
            params,
            true,
        )
        .await
    }

    pub async fn remove_group_blocked_member(
        &self,
        member_ids: Vec<String>,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_ids,
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/blockedmems/remove",
            params,
            true,
        )
        .await
    }

    pub async fn enable_group_link(&self, group_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "imei": self.ctx.imei,
        });

        self.request_api(Method::GET, "group", "api/group/link/new", params, true)
            .await
    }

    pub async fn disable_group_link(&self, group_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
        });

        self.request_api(Method::GET, "group", "api/group/link/disable", params, true)
            .await
    }

    pub async fn get_group_link_info(
        &self,
        link: &str,
        member_page: Option<u32>,
    ) -> Result<GetGroupLinkInfoResponse, ZaloError> {
        let params = serde_json::json!({
            "link": link,
            "avatar_size": 120,
            "member_avatar_size": 120,
            "mpage": member_page.unwrap_or(1),
        });

        self.request_api(Method::GET, "group", "api/group/link/ginfo", params, true)
            .await
    }

    pub async fn get_group_link_detail(
        &self,
        group_id: &str,
    ) -> Result<GetGroupLinkDetailResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "imei": self.ctx.imei,
        });

        self.request_api(Method::GET, "group", "api/group/link/detail", params, true)
            .await
    }

    pub async fn join_group_link(&self, link: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "link": link,
            "clientLang": self.ctx.language,
        });

        self.request_api(Method::GET, "group", "api/group/link/join", params, true)
            .await
    }

    pub async fn get_group_invite_box_list(
        &self,
        mpage: Option<u32>,
        page: Option<u32>,
        inv_per_page: Option<u32>,
        mcount: Option<u32>,
    ) -> Result<GetGroupInviteBoxListResponse, ZaloError> {
        let params = serde_json::json!({
            "mpage": mpage.unwrap_or(1),
            "page": page.unwrap_or(0),
            "invPerPage": inv_per_page.unwrap_or(12),
            "mcount": mcount.unwrap_or(10),
            "lastGroupId": serde_json::Value::Null,
            "avatar_size": 120,
            "member_avatar_size": 120,
        });

        self.request_api(Method::GET, "group", "api/group/inv-box/list", params, true)
            .await
    }

    pub async fn get_group_invite_box_info(
        &self,
        group_id: &str,
        mcount: Option<u32>,
        mpage: Option<u32>,
    ) -> Result<GetGroupInviteBoxInfoResponse, ZaloError> {
        let params = serde_json::json!({
            "grId": group_id,
            "mcount": mcount.unwrap_or(10),
            "mpage": mpage.unwrap_or(1),
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/inv-box/inv-info",
            params,
            true,
        )
        .await
    }

    pub async fn join_group_invite_box(&self, group_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "lang": self.ctx.language,
        });

        self.request_api(Method::GET, "group", "api/group/inv-box/join", params, true)
            .await
    }

    pub async fn delete_group_invite_box(
        &self,
        group_ids: Vec<String>,
        block_future_invite: bool,
    ) -> Result<Value, ZaloError> {
        let invites: Vec<serde_json::Value> = group_ids
            .iter()
            .map(|grid| serde_json::json!({ "grid": grid }))
            .collect();

        let params = serde_json::json!({
            "invitations": serde_json::to_string(&invites)?,
            "block": if block_future_invite { 1 } else { 0 },
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/inv-box/mdel-inv",
            params,
            true,
        )
        .await
    }

    pub async fn get_pending_group_members(
        &self,
        group_id: &str,
    ) -> Result<GetPendingGroupMembersResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "imei": self.ctx.imei,
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/pending-mems/list",
            params,
            true,
        )
        .await
    }

    pub async fn review_pending_member_request(
        &self,
        member_ids: Vec<String>,
        approve: bool,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "members": member_ids,
            "isApprove": if approve { 1 } else { 0 },
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/pending-mems/review",
            params,
            true,
        )
        .await
    }

    pub async fn create_note(
        &self,
        options: CreateNoteOptions,
        group_id: &str,
    ) -> Result<GroupTopic, ZaloError> {
        let note_params = serde_json::json!({
            "title": options.title,
        });

        let params = serde_json::json!({
            "grid": group_id,
            "type": 0,
            "color": -16777216,
            "emoji": "",
            "startTime": -1,
            "duration": -1,
            "params": serde_json::to_string(&note_params)?,
            "repeat": 0,
            "src": 1,
            "imei": self.ctx.imei,
            "pinAct": if options.pin_act { 1 } else { 0 },
        });

        self.request_api_form("group_board", "api/board/topic/createv2", params)
            .await
    }

    pub async fn edit_note(
        &self,
        options: EditNoteOptions,
        group_id: &str,
    ) -> Result<GroupTopic, ZaloError> {
        let note_params = serde_json::json!({
            "title": options.title,
        });

        let params = serde_json::json!({
            "grid": group_id,
            "type": 0,
            "color": -16777216,
            "emoji": "",
            "startTime": -1,
            "duration": -1,
            "params": serde_json::to_string(&note_params)?,
            "topicId": options.topic_id,
            "repeat": 0,
            "imei": self.ctx.imei,
            "pinAct": if options.pin_act { 1 } else { 2 },
        });

        self.request_api_form("group_board", "api/board/topic/updatev2", params)
            .await
    }

    pub async fn update_group_settings(
        &self,
        options: UpdateGroupSettingsOptions,
        group_id: &str,
    ) -> Result<Value, ZaloError> {
        let mut params = serde_json::json!({
            "bannFeature": 0,
            "dirtyMedia": 0,
            "banDuration": 0,
            "blocked_members": Vec::<String>::new(),
            "grid": group_id,
            "imei": self.ctx.imei,
        });

        if let Some(ref mut obj) = params.as_object_mut() {
            if let Some(v) = options.block_name {
                obj.insert(
                    "blockName".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.sign_admin_msg {
                obj.insert(
                    "signAdminMsg".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.set_topic_only {
                obj.insert(
                    "setTopicOnly".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.enable_msg_history {
                obj.insert(
                    "enableMsgHistory".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.join_appr {
                obj.insert(
                    "joinAppr".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.lock_create_post {
                obj.insert(
                    "lockCreatePost".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.lock_create_poll {
                obj.insert(
                    "lockCreatePoll".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.lock_send_msg {
                obj.insert(
                    "lockSendMsg".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
            if let Some(v) = options.lock_view_member {
                obj.insert(
                    "lockViewMember".to_string(),
                    serde_json::json!(if v { 1 } else { 0 }),
                );
            }
        }

        self.request_api(
            Method::GET,
            "group",
            "api/group/setting/update",
            params,
            true,
        )
        .await
    }

    pub async fn upgrade_group_to_community(&self, group_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grId": group_id,
            "language": self.ctx.language,
        });

        self.request_api(
            Method::GET,
            "group",
            "api/group/upgrade/community",
            params,
            true,
        )
        .await
    }

    pub async fn invite_user_to_groups(
        &self,
        user_id: &str,
        group_ids: Vec<String>,
    ) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "grids": group_ids,
            "member": user_id,
            "memberType": -1,
            "srcInteraction": 2,
            "clientLang": self.ctx.language,
        });

        self.request_api(Method::GET, "group", "api/group/invite/multi", params, true)
            .await
    }

    /// Get group chat history.
    pub async fn get_group_chat_history(
        &self,
        group_id: &str,
        count: Option<i64>,
    ) -> Result<GetGroupChatHistoryResponse, ZaloError> {
        let params = serde_json::json!({
            "grid": group_id,
            "count": count.unwrap_or(50),
        });

        let raw: Value = self
            .request_api(Method::GET, "group", "api/group/history", params, true)
            .await?;

        // Zalo sometimes returns the payload as a JSON-encoded string.
        let mut data: Value = match raw {
            Value::String(s) => serde_json::from_str(&s)?,
            other => other,
        };

        let uid = {
            let uid_guard = self.ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
            uid_guard.clone().unwrap_or_default()
        };

        let mut group_msgs = Vec::new();
        if let Some(arr) = data.get_mut("groupMsgs").and_then(|v| v.as_array_mut()) {
            for msg_val in arr.drain(..) {
                let raw_msg: TGroupMessage = serde_json::from_value(msg_val)?;
                group_msgs.push(GroupMessage::new(&uid, raw_msg));
            }
        }

        Ok(GetGroupChatHistoryResponse {
            last_action_id: data
                .get("lastActionId")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            last_action_id_other: data
                .get("lastActionIdOther")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            more: data.get("more").and_then(|v| v.as_i64()).unwrap_or(0),
            group_msgs,
        })
    }

    /// Get list of board items (notes, pinned messages, polls) in a group.
    pub async fn get_list_board(
        &self,
        group_id: &str,
        page: Option<i64>,
        count: Option<i64>,
    ) -> Result<GetListBoardResponse, ZaloError> {
        let params = serde_json::json!({
            "group_id": group_id,
            "board_type": 0,
            "page": page.unwrap_or(1),
            "count": count.unwrap_or(20),
            "last_id": 0,
            "last_type": 0,
            "imei": self.ctx.imei,
        });

        let mut result: GetListBoardResponse = self
            .request_api(Method::GET, "group_board", "api/board/list", params, true)
            .await?;

        // For non-poll items, `data.params` is a JSON-encoded string; decode it.
        for item in &mut result.items {
            if item.board_type != BoardType::Poll {
                if let Some(params_str) = item
                    .data
                    .get("params")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                {
                    if let Ok(parsed) = serde_json::from_str::<Value>(&params_str) {
                        if let Some(obj) = item.data.as_object_mut() {
                            obj.insert("params".to_string(), parsed);
                        }
                    }
                }
            }
        }

        Ok(result)
    }
}
