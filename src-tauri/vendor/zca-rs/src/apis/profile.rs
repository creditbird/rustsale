use reqwest::multipart::{Form, Part};
use serde_json::Value;
use std::collections::HashMap;

use crate::apis::upload::get_image_dimensions_from_bytes;
use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{AttachmentSource, AvatarSize, BusinessCategory, Gender, User};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateProfilePayload {
    pub profile: UpdateProfileProfile,
    pub biz: Option<UpdateProfileBiz>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateProfileProfile {
    pub name: String,
    pub dob: String, // format "YYYY-MM-DD"
    pub gender: Gender,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateProfileBiz {
    pub cate: Option<BusinessCategory>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub website: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ZaloAccountProfile {
    pub name: Option<String>,
    pub phone: Option<String>,
    pub avatar: Option<String>,
}

impl ZaloAccountProfile {
    pub fn from_value(val: &Value) -> Self {
        let mut targets = vec![val];
        if let Some(d) = val.get("data") {
            targets.push(d);
        }
        if let Some(u) = val.get("userInfo") {
            targets.push(u);
        }
        if let Some(u) = val.get("data").and_then(|d| d.get("userInfo")) {
            targets.push(u);
        }
        if let Some(p) = val.get("profile") {
            targets.push(p);
        }

        let mut name = None;
        let mut phone = None;
        let mut avatar = None;

        for t in targets {
            if name.is_none() {
                name = t.get("zName")
                    .or_else(|| t.get("dpName"))
                    .or_else(|| t.get("zaloName"))
                    .or_else(|| t.get("displayName"))
                    .or_else(|| t.get("name"))
                    .or_else(|| t.get("display_name"))
                    .or_else(|| t.get("username"))
                    .or_else(|| t.get("user_name"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }

            if phone.is_none() {
                phone = t.get("phone")
                    .or_else(|| t.get("phoneNumber"))
                    .or_else(|| t.get("phone_number"))
                    .or_else(|| t.get("mobile"))
                    .or_else(|| t.get("sdT"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }

            if avatar.is_none() {
                avatar = t.get("avatar_240")
                    .or_else(|| t.get("avatar_120"))
                    .or_else(|| t.get("avatar"))
                    .or_else(|| t.get("avatarUrl"))
                    .or_else(|| t.get("avatar_url"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());
            }
        }

        Self { name, phone, avatar }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAvatarResponse {
    pub del_photo_ids: Vec<String>,
    pub err_map: HashMap<String, DeleteAvatarErrorDetail>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeleteAvatarErrorDetail {
    pub err: i32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AvatarUrlProfileInfo {
    pub avatar: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAvatarListResponse {
    pub album_id: String,
    pub next_photo_id: String,
    pub has_more: i32,
    pub photos: Vec<AvatarPhotoItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvatarPhotoItem {
    pub photo_id: String,
    pub thumbnail: String,
    pub url: String,
    pub bk_url: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GetFullAvatarResponse {
    pub bk_full_avatar: String,
    pub full_avatar: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserInfoResponse {
    pub unchanged_profiles: HashMap<String, Value>,
    pub phonebook_version: i32,
    pub changed_profiles: HashMap<String, User>,
}

impl Zalo {
    /// Change account setting information
    ///
    /// If your account is a Business Account, include the biz.cate field; otherwise the category will be removed.
    /// You may leave the other biz fields empty if you don’t want to change them.
    pub async fn update_profile(&self, payload: UpdateProfilePayload) -> Result<Value, ZaloError> {
        let profile_str = serde_json::to_string(&serde_json::json!({
            "name": payload.profile.name,
            "dob": payload.profile.dob,
            "gender": payload.profile.gender,
        }))?;

        let biz_str = serde_json::to_string(&serde_json::json!({
            "desc": payload.biz.as_ref().and_then(|b| b.description.as_deref()),
            "cate": payload.biz.as_ref().and_then(|b| b.cate),
            "addr": payload.biz.as_ref().and_then(|b| b.address.as_deref()),
            "website": payload.biz.as_ref().and_then(|b| b.website.as_deref()),
            "email": payload.biz.as_ref().and_then(|b| b.email.as_deref()),
        }))?;

        let params = serde_json::json!({
            "profile": profile_str,
            "biz": biz_str,
            "language": self.ctx.language,
        });

        self.request_api_form("profile", "api/social/profile/update", params)
            .await
    }

    /// Update profile bio
    pub async fn update_profile_bio(&self, status: &str) -> Result<Value, ZaloError> {
        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let params = serde_json::json!({
            "status": status,
        });
        let stringified = serde_json::to_string(&params)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())?;

        self.request_api_form_with_query(
            "profile",
            "api/social/profile/status",
            &[("params", &encrypted_params)],
            params,
        )
        .await
    }

    /// Change account avatar
    pub async fn change_account_avatar(
        &self,
        avatar_source: AttachmentSource,
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

        let file_size = data.len() as u64;

        let uid = {
            let uid_guard = self.ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
            uid_guard.clone().unwrap_or_default()
        };
        let time_str = chrono::Local::now().format("%H:%M %d/%m/%Y").to_string();
        let client_id = format!("{}{}", uid, time_str);

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
            "avatarSize": 120,
            "clientId": client_id,
            "language": self.ctx.language,
            "metaData": serde_json::to_string(&serde_json::json!({
                "origin": {
                    "width": width,
                    "height": height,
                },
                "processed": {
                    "width": width,
                    "height": height,
                    "size": file_size,
                }
            }))?,
        });

        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let stringified = serde_json::to_string(&params)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, stringified.as_bytes())?;

        let service_url = self.get_service_url("file", "api/profile/upavatar")?;
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

    /// Delete avatar from avatar list
    pub async fn delete_avatar(
        &self,
        photo_id: &[String],
    ) -> Result<DeleteAvatarResponse, ZaloError> {
        let del_photos: Vec<Value> = photo_id
            .iter()
            .map(|id| serde_json::json!({ "photoId": id }))
            .collect();

        let params = serde_json::json!({
            "delPhotos": serde_json::to_string(&del_photos)?,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/del-avatars",
            params,
            true,
        )
        .await
    }

    /// Get avatar url profile
    pub async fn get_avatar_url_profile(
        &self,
        friend_ids: &[String],
        avatar_size: Option<AvatarSize>,
    ) -> Result<HashMap<String, AvatarUrlProfileInfo>, ZaloError> {
        let size = avatar_size.unwrap_or(AvatarSize::Large);
        let params = serde_json::json!({
            "friend_ids": friend_ids,
            "avatar_size": size,
            "srcReq": -1,
        });

        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/profile/avatar-url",
            params,
            true,
        )
        .await
    }

    /// Fetch own account info
    pub async fn fetch_account_info(&self) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "imei": self.ctx.imei,
        });
        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/profile/me-v2",
            params,
            true,
        )
        .await
    }

    /// Fetch own account profile with automatic fallback parsing for name, phone, and avatar
    pub async fn fetch_account_profile(&self) -> Result<ZaloAccountProfile, ZaloError> {
        let val = match self.fetch_account_info().await {
            Ok(v) => v,
            Err(_) => {
                let li_guard = self.ctx.login_info.lock().unwrap_or_else(|e| e.into_inner());
                li_guard.clone()
            }
        };
        Ok(ZaloAccountProfile::from_value(&val))
    }

    /// Get avatar list
    pub async fn get_avatar_list(
        &self,
        count: Option<u32>,
        page: Option<u32>,
    ) -> Result<GetAvatarListResponse, ZaloError> {
        let params = serde_json::json!({
            "page": page.unwrap_or(1),
            "albumId": "0",
            "count": count.unwrap_or(50),
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/avatar-list",
            params,
            true,
        )
        .await
    }

    /// Get full avatar of a user
    pub async fn get_full_avatar(
        &self,
        friend_id: &str,
    ) -> Result<GetFullAvatarResponse, ZaloError> {
        let params = serde_json::json!({
            "fid": friend_id,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/profile/avatar",
            params,
            true,
        )
        .await
    }

    /// Get user info using user ID(s)
    pub async fn get_user_info(
        &self,
        user_ids: &[String],
        avatar_size: Option<AvatarSize>,
    ) -> Result<UserInfoResponse, ZaloError> {
        if user_ids.is_empty() {
            return Err(ZaloError::PayloadFormat("Missing user id".to_string()));
        }

        let mapped_user_ids: Vec<String> = user_ids
            .iter()
            .map(|id| {
                if id.contains('_') {
                    id.clone()
                } else {
                    format!("{}_0", id)
                }
            })
            .collect();

        let phonebook_ver = {
            let extra_ver = self.ctx.extra_ver.lock().unwrap_or_else(|e| e.into_inner());
            extra_ver
                .get("phonebook")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32
        };

        let size = avatar_size.unwrap_or(AvatarSize::Small);

        let params = serde_json::json!({
            "phonebook_version": phonebook_ver,
            "friend_pversion_map": mapped_user_ids,
            "avatar_size": size,
            "language": self.ctx.language,
            "show_online_status": 1,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::POST,
            "profile",
            "api/social/friend/getprofiles/v2",
            params,
            true,
        )
        .await
    }

    /// Reuse an existing avatar (by photo id from `get_avatar_list`)
    pub async fn reuse_avatar(&self, photo_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "photoId": photo_id,
            "isPostSocial": 0,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::GET,
            "profile",
            "api/social/reuse-avatar",
            params,
            true,
        )
        .await
    }
}
