use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;

use crate::conversation::repository::NormalizedIdentity;
use crate::conversation::zalo::normalizer::{NormalizedGroupParticipant, NormalizedGroupSnapshot};

pub fn normalize_group_info(
    provider_account_key: &str,
    info: zca_rs::models::GroupInfo,
) -> NormalizedGroupSnapshot {
    let title = info.name;
    let avatar_url = if !info.full_avt.is_empty() {
        Some(info.full_avt)
    } else if !info.avt.is_empty() {
        Some(info.avt)
    } else {
        None
    };

    let participants = info
        .current_mems
        .into_iter()
        .map(|mem| {
            let role = if info.admin_ids.contains(&mem.id) {
                "admin".to_string()
            } else {
                "member".to_string()
            };
            let display_name = if !mem.d_name.is_empty() {
                Some(mem.d_name)
            } else if !mem.zalo_name.is_empty() {
                Some(mem.zalo_name)
            } else {
                None
            };
            let avatar = if !mem.avatar.is_empty() {
                Some(mem.avatar)
            } else if !mem.avatar_25.is_empty() {
                Some(mem.avatar_25)
            } else {
                None
            };

            NormalizedGroupParticipant {
                identity: NormalizedIdentity {
                    identity_namespace: "zalo_uid".to_string(),
                    external_id: mem.id,
                    display_name,
                    avatar_url: avatar,
                    lead_id: None,
                },
                participant_role: role,
            }
        })
        .collect();

    let membership_complete = info.has_more_member == 0;
    let occurred_at = crate::conversation::types::utc_now();

    NormalizedGroupSnapshot {
        provider_account_key: provider_account_key.to_string(),
        external_thread_id: info.group_id,
        title,
        avatar_url,
        participants,
        membership_complete,
        occurred_at,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedProfile {
    pub external_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderQuoteContext {
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub sender_external_id: String,
    pub content: String,
    pub message_type: u32,
    pub property_ext: Option<String>,
    pub provider_timestamp_ms: u64,
    pub ttl: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderOutboundContent {
    Text {
        text: String,
    },
    Attachment {
        media_type: String,
        mime_type: String,
        file_name: String,
        size_bytes: u64,
        data: Vec<u8>,
    },
    Sticker {
        sticker_id: u32,
        cate_id: u32,
        sticker_type: u32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProviderSendRequest {
    pub external_thread_id: String,
    pub thread_type: zca_rs::models::ThreadType,
    pub provider_client_message_id: String,
    pub content: ProviderOutboundContent,
    pub quote: Option<ProviderQuoteContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendResult {
    pub provider_message_id: String,
    pub provider_client_message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZaloStickerReference {
    pub sticker_id: u32,
    pub cate_id: u32,
    pub sticker_type: u32,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRecallRequest {
    pub external_thread_id: String,
    pub thread_type: zca_rs::models::ThreadType,
    pub provider_message_id: String,
    pub provider_client_message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSendErrorKind {
    Authentication,
    RateLimited,
    Retryable,
    Permanent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSendError {
    pub kind: ProviderSendErrorKind,
    pub message: String,
}

impl std::fmt::Display for ProviderSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ProviderSendError {}

impl From<zca_rs::error::ZaloError> for ProviderSendError {
    fn from(err: zca_rs::error::ZaloError) -> Self {
        use zca_rs::error::ZaloError;
        let message = err.to_string();
        let kind = match &err {
            ZaloError::Network(_) | ZaloError::WebSocket(_) => ProviderSendErrorKind::Retryable,
            ZaloError::ApiError { code, .. }
                if code.is_some_and(|c| c == 429)
                    || message.to_lowercase().contains("rate limit") =>
            {
                ProviderSendErrorKind::RateLimited
            }
            ZaloError::ApiError { code, .. } if code.is_some_and(|c| c >= 500) => {
                ProviderSendErrorKind::Retryable
            }
            ZaloError::ApiError { .. } | ZaloError::LoginQRAborted | ZaloError::LoginQRDeclined => {
                ProviderSendErrorKind::Authentication
            }
            ZaloError::PayloadFormat(_)
            | ZaloError::Io(_)
            | ZaloError::Json(_)
            | ZaloError::Crypto(_)
            | ZaloError::UrlParse(_)
            | ZaloError::Decompression(_) => ProviderSendErrorKind::Permanent,
            _ => ProviderSendErrorKind::Permanent,
        };
        ProviderSendError { kind, message }
    }
}

#[async_trait]
pub trait ZaloClient: Send + Sync {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent>;
    fn start(&self) -> anyhow::Result<()> {
        Ok(())
    }
    fn request_old_messages(
        &self,
        thread_type: zca_rs::models::ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()>;
    fn request_old_reactions(
        &self,
        thread_type: zca_rs::models::ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>>;
    async fn get_group_snapshot(
        &self,
        group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>>;
    async fn send_text(
        &self,
        thread_id: &str,
        thread_type: zca_rs::models::ThreadType,
        text: &str,
    ) -> anyhow::Result<String>;
    async fn send_part(
        &self,
        _request: ProviderSendRequest,
    ) -> Result<ProviderSendResult, ProviderSendError> {
        Err(ProviderSendError {
            kind: ProviderSendErrorKind::Permanent,
            message: "send_part not implemented".into(),
        })
    }
    async fn search_stickers(
        &self,
        _keyword: &str,
        _limit: u32,
    ) -> Result<Vec<ZaloStickerReference>, ProviderSendError> {
        Ok(Vec::new())
    }
    async fn recall(&self, _request: ProviderRecallRequest) -> Result<(), ProviderSendError> {
        Ok(())
    }
    fn own_external_id(&self) -> Option<String> {
        None
    }
    async fn find_user_by_phone(&self, _phone: &str) -> anyhow::Result<Option<NormalizedProfile>> {
        Ok(None)
    }
    async fn fetch_user_profiles(
        &self,
        _user_ids: &[String],
    ) -> anyhow::Result<Vec<NormalizedProfile>> {
        Ok(Vec::new())
    }
    fn stop(&self);
}

#[async_trait]
pub trait ZaloClientFactory: Send + Sync {
    async fn connect(
        &self,
        session: crate::models::ZaloSession,
    ) -> anyhow::Result<Arc<dyn ZaloClient>>;
}

pub struct RealZaloClient {
    inner: Arc<zca_rs::Zalo>,
    provider_account_key: String,
}

impl RealZaloClient {
    pub fn new(inner: Arc<zca_rs::Zalo>, provider_account_key: String) -> Self {
        Self {
            inner,
            provider_account_key,
        }
    }

    pub fn provider_account_key(&self) -> &str {
        &self.provider_account_key
    }
}

#[async_trait]
impl ZaloClient for RealZaloClient {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        self.inner.subscribe()
    }

    fn start(&self) -> anyhow::Result<()> {
        self.inner
            .listener
            .start()
            .map_err(|error| anyhow::anyhow!("Failed to start Zalo listener: {error}"))
    }

    fn request_old_messages(
        &self,
        thread_type: zca_rs::models::ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.inner
            .listener
            .request_old_messages(thread_type, cursor)
            .map_err(|e| anyhow::anyhow!("Failed to request old messages: {e}"))
    }

    fn request_old_reactions(
        &self,
        thread_type: zca_rs::models::ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.inner
            .listener
            .request_old_reactions(thread_type, cursor)
            .map_err(|e| anyhow::anyhow!("Failed to request old reactions: {e}"))
    }

    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>> {
        let all_groups = self
            .inner
            .get_all_groups()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch all groups: {e}"))?;

        let group_ids: Vec<String> = all_groups.grid_ver_map.into_keys().collect();
        let mut snapshots = Vec::with_capacity(group_ids.len());

        for chunk in group_ids.chunks(50) {
            let info_response = self
                .inner
                .get_group_info(chunk.to_vec())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to fetch group info batch: {e}"))?;

            for (_gid, info_with_ver) in info_response.grid_info_map {
                snapshots.push(normalize_group_info(
                    &self.provider_account_key,
                    info_with_ver.info,
                ));
            }
        }

        Ok(snapshots)
    }

    async fn get_group_snapshot(
        &self,
        group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>> {
        let mut info_response = self
            .inner
            .get_group_info(vec![group_id.to_string()])
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch group info for {group_id}: {e}"))?;

        if let Some(info_with_ver) = info_response.grid_info_map.remove(group_id) {
            Ok(Some(normalize_group_info(
                &self.provider_account_key,
                info_with_ver.info,
            )))
        } else if let Some(info_with_ver) = info_response.grid_info_map.into_values().next() {
            Ok(Some(normalize_group_info(
                &self.provider_account_key,
                info_with_ver.info,
            )))
        } else {
            Ok(None)
        }
    }

    async fn send_text(
        &self,
        thread_id: &str,
        thread_type: zca_rs::models::ThreadType,
        text: &str,
    ) -> anyhow::Result<String> {
        let content = zca_rs::models::MessageContent {
            msg: text.to_string(),
            styles: None,
            urgency: None,
            quote: None,
            mentions: None,
            attachments: None,
            ttl: None,
        };
        let response = self
            .inner
            .send_message(content, thread_id, thread_type)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send text message: {e}"))?;

        if let Some(msg_res) = response.message {
            if msg_res.msg_id != 0 {
                return Ok(msg_res.msg_id.to_string());
            }
        }

        Err(anyhow::anyhow!("Zalo response contains no text message ID"))
    }

    async fn find_user_by_phone(&self, phone: &str) -> anyhow::Result<Option<NormalizedProfile>> {
        Ok(self.inner.find_user(phone, None).await?.map(|user| {
            let display_name = if !user.display_name.trim().is_empty() {
                Some(user.display_name.trim().to_string())
            } else if !user.zalo_name.trim().is_empty() {
                Some(user.zalo_name.trim().to_string())
            } else {
                None
            };
            let avatar_url =
                (!user.avatar.trim().is_empty()).then(|| user.avatar.trim().to_string());

            NormalizedProfile {
                external_id: user.uid,
                display_name,
                avatar_url,
            }
        }))
    }

    async fn fetch_user_profiles(
        &self,
        user_ids: &[String],
    ) -> anyhow::Result<Vec<NormalizedProfile>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for chunk in user_ids.chunks(50) {
            let resp = self
                .inner
                .get_user_info(chunk, None)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to fetch Zalo user info batch: {e}"))?;

            for (raw_id, user) in resp.changed_profiles {
                let clean_id = if !user.user_id.trim().is_empty() {
                    user.user_id.trim().to_string()
                } else {
                    raw_id.split('_').next().unwrap_or(&raw_id).to_string()
                };
                let display_name = if !user.display_name.trim().is_empty() {
                    Some(user.display_name.trim().to_string())
                } else if !user.zalo_name.trim().is_empty() {
                    Some(user.zalo_name.trim().to_string())
                } else {
                    None
                };
                let avatar_url = if !user.avatar.trim().is_empty() {
                    Some(user.avatar.trim().to_string())
                } else if !user.bgavatar.trim().is_empty() {
                    Some(user.bgavatar.trim().to_string())
                } else {
                    None
                };

                results.push(NormalizedProfile {
                    external_id: clean_id,
                    display_name,
                    avatar_url,
                });
            }

            for (raw_id, val) in resp.unchanged_profiles {
                let clean_id = raw_id.split('_').next().unwrap_or(&raw_id).to_string();
                let display_name = val
                    .get("displayName")
                    .or_else(|| val.get("display_name"))
                    .or_else(|| val.get("zaloName"))
                    .or_else(|| val.get("zalo_name"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());

                let avatar_url = val
                    .get("avatar")
                    .or_else(|| val.get("fullAvatar"))
                    .or_else(|| val.get("full_avatar"))
                    .or_else(|| val.get("avatar_240"))
                    .or_else(|| val.get("avatar_120"))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string());

                if display_name.is_some() || avatar_url.is_some() {
                    results.push(NormalizedProfile {
                        external_id: clean_id,
                        display_name,
                        avatar_url,
                    });
                }
            }
        }

        Ok(results)
    }

    async fn send_part(
        &self,
        request: ProviderSendRequest,
    ) -> Result<ProviderSendResult, ProviderSendError> {
        let cli_msg_id = request
            .provider_client_message_id
            .parse::<u64>()
            .map_err(|_| ProviderSendError {
                kind: ProviderSendErrorKind::Permanent,
                message: format!(
                    "Invalid provider_client_message_id {}",
                    request.provider_client_message_id
                ),
            })?;

        match request.content {
            ProviderOutboundContent::Text { text } => {
                let quote = request.quote.map(to_zalo_quote);
                let content = zca_rs::models::MessageContent {
                    msg: text,
                    styles: None,
                    urgency: None,
                    quote,
                    mentions: None,
                    attachments: None,
                    ttl: None,
                };
                let res = self
                    .inner
                    .send_message_with_client_id(
                        content,
                        &request.external_thread_id,
                        request.thread_type,
                        cli_msg_id,
                    )
                    .await?;
                let msg_res = res
                    .message
                    .or_else(|| res.attachment.into_iter().next())
                    .ok_or_else(|| ProviderSendError {
                        kind: ProviderSendErrorKind::Permanent,
                        message: "No message result returned from Zalo".into(),
                    })?;
                Ok(ProviderSendResult {
                    provider_message_id: msg_res.msg_id.to_string(),
                    provider_client_message_id: if msg_res.cli_msg_id != 0 {
                        msg_res.cli_msg_id.to_string()
                    } else {
                        request.provider_client_message_id
                    },
                })
            }
            ProviderOutboundContent::Attachment {
                data,
                file_name,
                size_bytes,
                ..
            } => {
                let quote = request.quote.map(to_zalo_quote);
                let content = zca_rs::models::MessageContent {
                    msg: String::new(),
                    styles: None,
                    urgency: None,
                    quote,
                    mentions: None,
                    attachments: Some(vec![zca_rs::models::AttachmentSource::Buffer {
                        data,
                        filename: file_name,
                        metadata: zca_rs::models::AttachmentMetadata {
                            total_size: size_bytes,
                            width: None,
                            height: None,
                        },
                    }]),
                    ttl: None,
                };
                let res = self
                    .inner
                    .send_message_with_client_id(
                        content,
                        &request.external_thread_id,
                        request.thread_type,
                        cli_msg_id,
                    )
                    .await?;
                let msg_res = res
                    .message
                    .or_else(|| res.attachment.into_iter().next())
                    .ok_or_else(|| ProviderSendError {
                        kind: ProviderSendErrorKind::Permanent,
                        message: "No message result returned from Zalo".into(),
                    })?;
                Ok(ProviderSendResult {
                    provider_message_id: msg_res.msg_id.to_string(),
                    provider_client_message_id: if msg_res.cli_msg_id != 0 {
                        msg_res.cli_msg_id.to_string()
                    } else {
                        request.provider_client_message_id
                    },
                })
            }
            ProviderOutboundContent::Sticker {
                sticker_id,
                cate_id,
                sticker_type,
            } => {
                let res = self
                    .inner
                    .send_sticker_with_client_id(
                        sticker_id,
                        cate_id,
                        sticker_type,
                        &request.external_thread_id,
                        request.thread_type,
                        cli_msg_id,
                    )
                    .await?;
                Ok(ProviderSendResult {
                    provider_message_id: res.msg_id.to_string(),
                    provider_client_message_id: if res.cli_msg_id != 0 {
                        res.cli_msg_id.to_string()
                    } else {
                        request.provider_client_message_id
                    },
                })
            }
        }
    }

    async fn search_stickers(
        &self,
        keyword: &str,
        limit: u32,
    ) -> Result<Vec<ZaloStickerReference>, ProviderSendError> {
        let basics = self
            .inner
            .search_sticker(keyword, Some(limit.clamp(1, 50)))
            .await?;
        if basics.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<u32> = basics.iter().map(|b| b.sticker_id).collect();
        let details = self.inner.get_stickers_detail(&ids).await?;
        let detail_map: std::collections::HashMap<u32, zca_rs::models::StickerDetail> =
            details.into_iter().map(|d| (d.id, d)).collect();

        let mut results = Vec::new();
        for b in basics {
            if let Some(d) = detail_map.get(&b.sticker_id) {
                results.push(ZaloStickerReference {
                    sticker_id: b.sticker_id,
                    cate_id: b.cate_id,
                    sticker_type: b.r#type,
                    url: d.sticker_url.clone(),
                });
            }
        }
        Ok(results)
    }

    async fn recall(&self, request: ProviderRecallRequest) -> Result<(), ProviderSendError> {
        self.inner
            .undo(
                &request.provider_message_id,
                &request.provider_client_message_id,
                &request.external_thread_id,
                request.thread_type,
            )
            .await?;
        Ok(())
    }

    fn own_external_id(&self) -> Option<String> {
        self.inner.get_own_id()
    }

    fn stop(&self) {
        self.inner.listener.stop();
    }
}

fn to_zalo_quote(value: ProviderQuoteContext) -> zca_rs::models::SendMessageQuote {
    zca_rs::models::SendMessageQuote {
        content: serde_json::Value::String(value.content),
        msg_type: value.message_type.to_string(),
        property_ext: value.property_ext.map(serde_json::Value::String),
        uid_from: value.sender_external_id,
        msg_id: value.provider_message_id,
        cli_msg_id: value.provider_client_message_id,
        ts: value.provider_timestamp_ms.to_string(),
        ttl: value.ttl as i32,
    }
}

pub struct RealZaloClientFactory;

#[async_trait]
impl ZaloClientFactory for RealZaloClientFactory {
    async fn connect(
        &self,
        session: crate::models::ZaloSession,
    ) -> anyhow::Result<Arc<dyn ZaloClient>> {
        let cookies: Vec<zca_rs::context::ZaloCookie> = serde_json::from_str(&session.cookies_json)
            .map_err(|e| anyhow::anyhow!("Invalid cookies JSON in session: {e}"))?;

        let options = zca_rs::Options {
            self_listen: true,
            auto_start_listener: false,
            ..Default::default()
        };

        let zalo = zca_rs::Zalo::login(session.imei.clone(), session.user_agent, cookies, options)
            .await
            .map_err(|e| anyhow::anyhow!("Zalo login failed: {e}"))?;

        Ok(Arc::new(RealZaloClient::new(Arc::new(zalo), session.imei)))
    }
}
