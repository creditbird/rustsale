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
    async fn find_user_by_phone(
        &self,
        _phone: &str,
    ) -> anyhow::Result<Option<NormalizedProfile>> {
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

    async fn find_user_by_phone(
        &self,
        phone: &str,
    ) -> anyhow::Result<Option<NormalizedProfile>> {
        Ok(self.inner.find_user(phone, None).await?.map(|user| {
            let display_name = if !user.display_name.trim().is_empty() {
                Some(user.display_name.trim().to_string())
            } else if !user.zalo_name.trim().is_empty() {
                Some(user.zalo_name.trim().to_string())
            } else {
                None
            };
            let avatar_url = (!user.avatar.trim().is_empty())
                .then(|| user.avatar.trim().to_string());

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

    fn stop(&self) {
        self.inner.listener.stop();
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

        let mut options = zca_rs::Options::default();
        options.self_listen = true;
        options.auto_start_listener = false;

        let zalo = zca_rs::Zalo::login(session.imei.clone(), session.user_agent, cookies, options)
            .await
            .map_err(|e| anyhow::anyhow!("Zalo login failed: {e}"))?;

        Ok(Arc::new(RealZaloClient::new(Arc::new(zalo), session.imei)))
    }
}
