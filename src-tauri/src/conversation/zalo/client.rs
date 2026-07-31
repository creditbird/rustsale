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

#[async_trait]
pub trait ZaloClient: Send + Sync {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent>;
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

        let zalo = zca_rs::Zalo::login(session.imei.clone(), session.user_agent, cookies, options)
            .await
            .map_err(|e| anyhow::anyhow!("Zalo login failed: {e}"))?;

        Ok(Arc::new(RealZaloClient::new(Arc::new(zalo), session.imei)))
    }
}
