use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;

use crate::conversation::repository::NormalizedIdentity;
use crate::conversation::zalo::normalizer::{NormalizedGroupParticipant, NormalizedGroupSnapshot};

pub fn normalize_group_info(
    provider_account_key: &str,
    info_with_ver: zca_rs::apis::group::GroupInfoWithVer,
) -> NormalizedGroupSnapshot {
    let info = info_with_ver.info;
    let title = info.name;
    let avatar_url = if !info.full_avt.is_empty() {
        Some(info.full_avt)
    } else if !info.avt.is_empty() {
        Some(info.avt)
    } else {
        None
    };

    let mut mem_map: std::collections::HashMap<String, zca_rs::models::GroupCurrentMem> = info
        .current_mems
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect();

    let mut all_uids: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    if let Some(ver_list) = info_with_ver.mem_ver_list {
        for entry in ver_list {
            let uid = entry
                .rsplit_once('_')
                .map(|(u, _v)| u)
                .unwrap_or(&entry)
                .to_string();
            if !uid.is_empty() && seen.insert(uid.clone()) {
                all_uids.push(uid);
            }
        }
    }

    for uid in &info.member_ids {
        if !uid.is_empty() && seen.insert(uid.clone()) {
            all_uids.push(uid.clone());
        }
    }

    for uid in mem_map.keys() {
        if !uid.is_empty() && seen.insert(uid.clone()) {
            all_uids.push(uid.clone());
        }
    }

    let participants: Vec<NormalizedGroupParticipant> = all_uids
        .into_iter()
        .map(|uid| {
            let role = if info.admin_ids.contains(&uid) || info.creator_id == uid {
                "admin".to_string()
            } else {
                "member".to_string()
            };

            let (display_name, avatar) = if let Some(mem) = mem_map.remove(&uid) {
                let dname = if !mem.d_name.is_empty() {
                    Some(mem.d_name)
                } else if !mem.zalo_name.is_empty() {
                    Some(mem.zalo_name)
                } else {
                    None
                };
                let avt = if !mem.avatar.is_empty() {
                    Some(mem.avatar)
                } else if !mem.avatar_25.is_empty() {
                    Some(mem.avatar_25)
                } else {
                    None
                };
                (dname, avt)
            } else {
                (None, None)
            };

            NormalizedGroupParticipant {
                identity: NormalizedIdentity {
                    identity_namespace: "zalo_uid".to_string(),
                    external_id: uid,
                    display_name,
                    avatar_url: avatar,
                    lead_id: None,
                },
                participant_role: role,
            }
        })
        .collect();

    let reported_member_count = info.total_member as usize;
    let membership_complete = info.has_more_member == 0
        && (reported_member_count == 0 || participants.len() >= reported_member_count);
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

const GROUP_BOARD_PAGE_SIZE: i64 = 50;
const GROUP_BOARD_MAX_PAGES: usize = 100;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupBoardWarning {
    pub source: String,
    pub code: String,
    pub context: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupBoardUidReport {
    pub uids: Vec<String>,
    pub pages_attempted: usize,
    pub pages_succeeded: usize,
    pub items_inspected: usize,
    pub polls_attempted: usize,
    pub polls_succeeded: usize,
    pub stop_reason: Option<String>,
    pub warnings: Vec<GroupBoardWarning>,
}

#[async_trait]
pub(crate) trait GroupBoardSource: Send + Sync {
    async fn get_list_board_page(
        &self,
        group_id: &str,
        page: i64,
        count: i64,
    ) -> anyhow::Result<zca_rs::apis::group::GetListBoardResponse>;

    async fn get_poll_detail(&self, poll_id: u64) -> anyhow::Result<zca_rs::models::PollDetail>;
}

#[async_trait]
impl GroupBoardSource for zca_rs::Zalo {
    async fn get_list_board_page(
        &self,
        group_id: &str,
        page: i64,
        count: i64,
    ) -> anyhow::Result<zca_rs::apis::group::GetListBoardResponse> {
        self.get_list_board(group_id, Some(page), Some(count))
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }

    async fn get_poll_detail(&self, poll_id: u64) -> anyhow::Result<zca_rs::models::PollDetail> {
        zca_rs::Zalo::get_poll_detail(self, poll_id)
            .await
            .map_err(|error| anyhow::anyhow!(error.to_string()))
    }
}

fn normalized_uid(value: &serde_json::Value) -> Option<String> {
    let normalized = match value {
        serde_json::Value::String(value) => value.trim().to_string(),
        serde_json::Value::Number(value) => value.to_string(),
        _ => return None,
    };
    let lowercase = normalized.to_ascii_lowercase();
    (!normalized.is_empty() && !matches!(lowercase.as_str(), "0" | "-1" | "null" | "undefined"))
        .then_some(normalized)
}

fn poll_id(value: &serde_json::Value) -> Option<u64> {
    match value {
        serde_json::Value::Number(value) => value.as_u64().filter(|value| *value > 0),
        serde_json::Value::String(value) => {
            value.trim().parse::<u64>().ok().filter(|value| *value > 0)
        }
        _ => None,
    }
}

fn collect_board_payload_candidates(
    payload: &serde_json::Value,
    uids: &mut std::collections::HashSet<String>,
    poll_ids: &mut std::collections::HashSet<u64>,
) {
    let Some(object) = payload.as_object() else {
        return;
    };

    for key in [
        "creator",
        "creator_id",
        "creatorId",
        "owner_id",
        "ownerId",
        "uid",
        "user_id",
        "userId",
        "editor_id",
        "editorId",
    ] {
        if let Some(uid) = object.get(key).and_then(normalized_uid) {
            uids.insert(uid);
        }
    }

    for key in ["poll_id", "pollId"] {
        if let Some(id) = object.get(key).and_then(poll_id) {
            poll_ids.insert(id);
        }
    }

    if let Some(options) = object.get("options").and_then(serde_json::Value::as_array) {
        for option in options {
            if let Some(voters) = option.get("voters").and_then(serde_json::Value::as_array) {
                for voter in voters {
                    if let Some(uid) = normalized_uid(voter) {
                        uids.insert(uid);
                    }
                }
            }
        }
    }
}

fn decoded_board_params(
    data: &serde_json::Value,
) -> Result<Option<serde_json::Value>, serde_json::Error> {
    let Some(params) = data.get("params") else {
        return Ok(None);
    };
    match params {
        serde_json::Value::String(value) => serde_json::from_str(value).map(Some),
        serde_json::Value::Object(_) | serde_json::Value::Array(_) => Ok(Some(params.clone())),
        serde_json::Value::Null => Ok(None),
        _ => Ok(None),
    }
}

pub(crate) async fn fetch_group_board_uid_report_with_limits(
    source: &dyn GroupBoardSource,
    group_id: &str,
    page_size: i64,
    max_pages: usize,
) -> GroupBoardUidReport {
    let mut report = GroupBoardUidReport::default();
    let mut uids = std::collections::HashSet::new();
    let mut fetched_poll_ids = std::collections::HashSet::new();
    let mut seen_page_signatures = std::collections::HashSet::new();

    for page_index in 0..max_pages {
        let page = page_index as i64 + 1;
        report.pages_attempted += 1;
        let response = match source.get_list_board_page(group_id, page, page_size).await {
            Ok(response) => response,
            Err(error) => {
                report.stop_reason = Some("board_page_failed".to_string());
                report.warnings.push(GroupBoardWarning {
                    source: "board".to_string(),
                    code: "board_page_failed".to_string(),
                    context: Some(format!("page={page}")),
                    message: error.to_string(),
                });
                break;
            }
        };
        report.pages_succeeded += 1;

        if response.items.is_empty() {
            report.stop_reason = Some("empty_page".to_string());
            break;
        }

        let signature = serde_json::to_string(&response.items)
            .unwrap_or_else(|_| format!("page={page},count={}", response.items.len()));
        if !seen_page_signatures.insert(signature) {
            report.stop_reason = Some("repeated_page".to_string());
            break;
        }

        let item_count = response.items.len();
        for (item_index, item) in response.items.into_iter().enumerate() {
            report.items_inspected += 1;
            let mut item_poll_ids = std::collections::HashSet::new();
            collect_board_payload_candidates(&item.data, &mut uids, &mut item_poll_ids);

            match decoded_board_params(&item.data) {
                Ok(Some(params)) => {
                    collect_board_payload_candidates(&params, &mut uids, &mut item_poll_ids);
                }
                Ok(None) => {}
                Err(error) => report.warnings.push(GroupBoardWarning {
                    source: "board".to_string(),
                    code: "invalid_params_json".to_string(),
                    context: Some(format!("page={page},item={item_index}")),
                    message: error.to_string(),
                }),
            }

            for poll_id in item_poll_ids {
                if !fetched_poll_ids.insert(poll_id) {
                    continue;
                }
                report.polls_attempted += 1;
                match source.get_poll_detail(poll_id).await {
                    Ok(detail) => {
                        report.polls_succeeded += 1;
                        if let Some(uid) =
                            normalized_uid(&serde_json::Value::String(detail.creator))
                        {
                            uids.insert(uid);
                        }
                        for option in detail.options {
                            for voter in option.voters {
                                if let Some(uid) = normalized_uid(&serde_json::Value::String(voter))
                                {
                                    uids.insert(uid);
                                }
                            }
                        }
                    }
                    Err(error) => report.warnings.push(GroupBoardWarning {
                        source: "poll".to_string(),
                        code: "poll_detail_failed".to_string(),
                        context: Some(format!("poll_id={poll_id}")),
                        message: error.to_string(),
                    }),
                }
            }
        }

        if item_count < page_size.max(1) as usize {
            report.stop_reason = Some("short_page".to_string());
            break;
        }
    }

    if report.stop_reason.is_none() {
        report.stop_reason = Some("max_pages_reached".to_string());
    }
    report.uids = uids.into_iter().collect();
    report.uids.sort();
    report
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderMention {
    pub pos: usize,
    pub len: usize,
    pub target_external_id: Option<String>,
    pub is_all: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderOutboundContent {
    Text {
        text: String,
        mentions: Vec<ProviderMention>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderReactionRequest {
    pub provider_message_id: String,
    pub provider_client_message_id: String,
    pub external_thread_id: String,
    pub thread_type: zca_rs::models::ThreadType,
    pub reaction_code: Option<crate::conversation::types::ConversationReactionCode>,
    pub client_mutation_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderForwardRequest {
    pub external_thread_id: String,
    pub thread_type: zca_rs::models::ThreadType,
    pub provider_client_message_id: String,
    pub source_provider_message_id: String,
    pub source_message: String,
    pub source_timestamp_ms: u64,
}

pub(crate) fn build_forward_message_request(
    request: &ProviderForwardRequest,
) -> zca_rs::models::ForwardMessageRequest {
    zca_rs::models::ForwardMessageRequest {
        client_id: request.provider_client_message_id.parse::<u64>().ok(),
        message: request.source_message.clone(),
        thread_id: request.external_thread_id.clone(),
        thread_type: request.thread_type,
        reference: Some(zca_rs::models::ForwardReference {
            id: request.source_provider_message_id.clone(),
            ts: request.source_timestamp_ms,
            log_src_type: 0,
            fw_lvl: 0,
        }),
        ttl: Some(0),
    }
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
    async fn forward_message(
        &self,
        _request: ProviderForwardRequest,
    ) -> Result<ProviderSendResult, ProviderSendError> {
        Err(ProviderSendError {
            kind: ProviderSendErrorKind::Permanent,
            message: "forward_message not implemented".into(),
        })
    }
    async fn search_stickers(
        &self,
        _keyword: &str,
        _limit: u32,
    ) -> Result<Vec<ZaloStickerReference>, ProviderSendError> {
        Ok(Vec::new())
    }
    async fn get_sticker_detail(
        &self,
        _sticker_id: u32,
    ) -> Result<Option<ZaloStickerReference>, ProviderSendError> {
        Ok(None)
    }
    async fn recall(&self, _request: ProviderRecallRequest) -> Result<(), ProviderSendError> {
        Ok(())
    }
    async fn set_reaction(
        &self,
        _request: ProviderReactionRequest,
    ) -> Result<(), ProviderSendError> {
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
    async fn fetch_group_member_profiles(
        &self,
        user_ids: &[String],
    ) -> anyhow::Result<Vec<NormalizedProfile>> {
        self.fetch_user_profiles(user_ids).await
    }

    async fn fetch_group_link_detail(&self, _group_id: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    async fn fetch_group_chat_history_with_cursor(
        &self,
        _group_id: &str,
        _count: Option<i64>,
        _last_action_id: Option<&str>,
    ) -> anyhow::Result<zca_rs::apis::group::GetGroupChatHistoryResponse> {
        Ok(zca_rs::apis::group::GetGroupChatHistoryResponse {
            last_action_id: String::new(),
            last_action_id_other: String::new(),
            more: 0,
            group_msgs: Vec::new(),
        })
    }

    async fn fetch_group_chat_history(
        &self,
        group_id: &str,
        count: Option<i64>,
    ) -> anyhow::Result<Vec<zca_rs::models::GroupMessage>> {
        let resp = self
            .fetch_group_chat_history_with_cursor(group_id, count, None)
            .await?;
        Ok(resp.group_msgs)
    }

    async fn fetch_group_link_members(
        &self,
        _link: &str,
        _page: u32,
    ) -> anyhow::Result<Option<(Vec<NormalizedProfile>, bool)>> {
        Ok(None)
    }

    async fn fetch_group_invite_box_members(
        &self,
        _group_id: &str,
        _page: u32,
    ) -> anyhow::Result<Option<Vec<NormalizedProfile>>> {
        Ok(None)
    }

    async fn fetch_group_board_uids(&self, _group_id: &str) -> anyhow::Result<Vec<String>> {
        Ok(self.fetch_group_board_uid_report(_group_id).await?.uids)
    }

    async fn fetch_group_board_uid_report(
        &self,
        _group_id: &str,
    ) -> anyhow::Result<GroupBoardUidReport> {
        Ok(GroupBoardUidReport::default())
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
                    info_with_ver,
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
                info_with_ver,
            )))
        } else if let Some(info_with_ver) = info_response.grid_info_map.into_values().next() {
            Ok(Some(normalize_group_info(
                &self.provider_account_key,
                info_with_ver,
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

    async fn fetch_group_chat_history_with_cursor(
        &self,
        group_id: &str,
        count: Option<i64>,
        last_action_id: Option<&str>,
    ) -> anyhow::Result<zca_rs::apis::group::GetGroupChatHistoryResponse> {
        let mut resp = self
            .inner
            .get_group_chat_history_with_cursor(group_id, count, last_action_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch group chat history: {e}"))?;
        for msg in &mut resp.group_msgs {
            if msg.thread_id.is_empty() || msg.thread_id == "0" {
                msg.thread_id = group_id.to_string();
            }
        }
        Ok(resp)
    }

    async fn fetch_group_member_profiles(
        &self,
        user_ids: &[String],
    ) -> anyhow::Result<Vec<NormalizedProfile>> {
        if user_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        for chunk in user_ids.chunks(50) {
            let response = self
                .inner
                .get_group_members_info(chunk.to_vec())
                .await
                .map_err(|error| {
                    anyhow::anyhow!("Failed to fetch Zalo group member profiles: {error}")
                })?;

            for (raw_id, profile) in response.profiles {
                let external_id = if !profile.id.trim().is_empty() {
                    profile.id.trim().to_string()
                } else {
                    raw_id
                        .rsplit_once('_')
                        .map(|(uid, _)| uid)
                        .unwrap_or(&raw_id)
                        .to_string()
                };
                let display_name = if !profile.display_name.trim().is_empty() {
                    Some(profile.display_name.trim().to_string())
                } else if !profile.zalo_name.trim().is_empty() {
                    Some(profile.zalo_name.trim().to_string())
                } else {
                    None
                };
                let avatar_url =
                    (!profile.avatar.trim().is_empty()).then(|| profile.avatar.trim().to_string());

                results.push(NormalizedProfile {
                    external_id,
                    display_name,
                    avatar_url,
                });
            }
        }

        Ok(results)
    }

    async fn fetch_group_link_detail(&self, group_id: &str) -> anyhow::Result<Option<String>> {
        let resp = self
            .inner
            .get_group_link_detail(group_id)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch group link detail: {e}"))?;
        Ok(resp.link)
    }

    async fn fetch_group_link_members(
        &self,
        link: &str,
        page: u32,
    ) -> anyhow::Result<Option<(Vec<NormalizedProfile>, bool)>> {
        let resp = self
            .inner
            .get_group_link_info(link, Some(page))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch group link info: {e}"))?;

        let mut profiles = Vec::new();
        for mem in resp.current_mems {
            let clean_id = mem.id.trim().to_string();
            let display_name = if !mem.d_name.trim().is_empty() {
                Some(mem.d_name.trim().to_string())
            } else if !mem.zalo_name.trim().is_empty() {
                Some(mem.zalo_name.trim().to_string())
            } else {
                None
            };
            let avatar_url = if !mem.avatar.trim().is_empty() {
                Some(mem.avatar.trim().to_string())
            } else {
                None
            };

            profiles.push(NormalizedProfile {
                external_id: clean_id,
                display_name,
                avatar_url,
            });
        }

        let has_more = resp.has_more_member == 1;
        Ok(Some((profiles, has_more)))
    }

    async fn fetch_group_invite_box_members(
        &self,
        group_id: &str,
        page: u32,
    ) -> anyhow::Result<Option<Vec<NormalizedProfile>>> {
        let resp = self
            .inner
            .get_group_invite_box_info(group_id, Some(50), Some(page))
            .await
            .map_err(|e| anyhow::anyhow!("Failed to fetch group invite box info: {e}"))?;

        let mut profiles = Vec::new();

        if let Some(inviter) = resp.inviter_info {
            let id = inviter.id.trim().to_string();
            if !id.is_empty() {
                let display_name = if !inviter.d_name.trim().is_empty() {
                    Some(inviter.d_name.trim().to_string())
                } else if !inviter.zalo_name.trim().is_empty() {
                    Some(inviter.zalo_name.trim().to_string())
                } else {
                    None
                };
                let avatar_url =
                    (!inviter.avatar.trim().is_empty()).then(|| inviter.avatar.trim().to_string());
                profiles.push(NormalizedProfile {
                    external_id: id,
                    display_name,
                    avatar_url,
                });
            }
        }

        if let Some(creator) = resp.gr_creator_info {
            let id = creator.id.trim().to_string();
            if !id.is_empty() {
                let display_name = if !creator.d_name.trim().is_empty() {
                    Some(creator.d_name.trim().to_string())
                } else if !creator.zalo_name.trim().is_empty() {
                    Some(creator.zalo_name.trim().to_string())
                } else {
                    None
                };
                let avatar_url =
                    (!creator.avatar.trim().is_empty()).then(|| creator.avatar.trim().to_string());
                profiles.push(NormalizedProfile {
                    external_id: id,
                    display_name,
                    avatar_url,
                });
            }
        }

        if let Some(group_info) = resp.group_info {
            let snapshot = normalize_group_info(&self.provider_account_key, group_info);
            for p in snapshot.participants {
                profiles.push(NormalizedProfile {
                    external_id: p.identity.external_id,
                    display_name: p.identity.display_name,
                    avatar_url: p.identity.avatar_url,
                });
            }
        }

        Ok(Some(profiles))
    }

    async fn fetch_group_board_uid_report(
        &self,
        group_id: &str,
    ) -> anyhow::Result<GroupBoardUidReport> {
        Ok(fetch_group_board_uid_report_with_limits(
            self.inner.as_ref(),
            group_id,
            GROUP_BOARD_PAGE_SIZE,
            GROUP_BOARD_MAX_PAGES,
        )
        .await)
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
            ProviderOutboundContent::Text { text, mentions } => {
                let quote = request.quote.map(to_zalo_quote);
                let zalo_mentions = if mentions.is_empty() {
                    None
                } else {
                    Some(
                        mentions
                            .iter()
                            .map(|m| zca_rs::models::MessageMention {
                                pos: m.pos as i32,
                                uid: if m.is_all {
                                    "0".to_string()
                                } else {
                                    m.target_external_id
                                        .clone()
                                        .unwrap_or_else(|| "0".to_string())
                                },
                                len: m.len as i32,
                            })
                            .collect(),
                    )
                };
                let content = zca_rs::models::MessageContent {
                    msg: text,
                    styles: None,
                    urgency: None,
                    quote,
                    mentions: zalo_mentions,
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

    async fn forward_message(
        &self,
        request: ProviderForwardRequest,
    ) -> Result<ProviderSendResult, ProviderSendError> {
        let forward_req = build_forward_message_request(&request);

        let receipt = self.inner.forward_message_one(forward_req).await?;
        Ok(ProviderSendResult {
            provider_message_id: receipt.msg_id.to_string(),
            provider_client_message_id: if !receipt.cli_msg_id.is_empty() {
                receipt.cli_msg_id
            } else {
                request.provider_client_message_id
            },
        })
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

    async fn get_sticker_detail(
        &self,
        sticker_id: u32,
    ) -> Result<Option<ZaloStickerReference>, ProviderSendError> {
        let details = self.inner.get_stickers_detail(&[sticker_id]).await?;
        if let Some(d) = details.first() {
            Ok(Some(ZaloStickerReference {
                sticker_id: d.id,
                cate_id: d.cate_id,
                sticker_type: d.r#type,
                url: if !d.sticker_url.is_empty() {
                    d.sticker_url.clone()
                } else {
                    d.sticker_sprite_url.clone()
                },
            }))
        } else {
            Ok(None)
        }
    }

    async fn recall(&self, request: ProviderRecallRequest) -> Result<(), ProviderSendError> {
        let delays = [1500, 3000];
        execute_recall_with_retry(&delays, || async {
            self.inner
                .undo(
                    &request.provider_message_id,
                    &request.provider_client_message_id,
                    &request.external_thread_id,
                    request.thread_type,
                )
                .await
        })
        .await
        .map(|_| ())
    }

    async fn set_reaction(
        &self,
        request: ProviderReactionRequest,
    ) -> Result<(), ProviderSendError> {
        use crate::conversation::types::ConversationReactionCode;
        let reaction = request.reaction_code.map(|code| match code {
            ConversationReactionCode::Heart => {
                zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Heart)
            }
            ConversationReactionCode::Like => {
                zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Like)
            }
            ConversationReactionCode::Haha => {
                zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Haha)
            }
            ConversationReactionCode::Wow => {
                zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Wow)
            }
            ConversationReactionCode::Cry => {
                zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Cry)
            }
            ConversationReactionCode::Angry => {
                zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Angry)
            }
            _ => zca_rs::models::ReactionType::Standard(zca_rs::models::Reactions::Like),
        });

        let client_id = request
            .client_mutation_id
            .as_deref()
            .map(|id| id.strip_prefix("mut-").unwrap_or(id))
            .and_then(|s| s.split('-').next())
            .and_then(|s| s.parse::<u64>().ok());

        let req = zca_rs::models::ReactionRequest {
            client_id,
            reaction,
            msg_id: request.provider_message_id,
            cli_msg_id: request.provider_client_message_id,
            thread_id: request.external_thread_id,
            thread_type: request.thread_type,
        };

        self.inner.set_reaction(req).await?;
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

        let provider_account_key = zalo
            .get_own_id()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                session
                    .zalo_uid
                    .clone()
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or_else(|| session.imei.clone());

        Ok(Arc::new(RealZaloClient::new(
            Arc::new(zalo),
            provider_account_key,
        )))
    }
}

pub async fn execute_recall_with_retry<F, Fut, T>(
    delays: &[u64],
    mut undo_fn: F,
) -> Result<T, ProviderSendError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, zca_rs::error::ZaloError>>,
{
    let mut last_err = None;
    for (i, &delay_ms) in std::iter::once(&0).chain(delays.iter()).enumerate() {
        if delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
        }
        match undo_fn().await {
            Ok(res) => return Ok(res),
            Err(err) => {
                let is_code_112 = match &err {
                    zca_rs::error::ZaloError::ApiError { code, .. } => *code == Some(112),
                    _ => {
                        let s = err.to_string();
                        s.contains("[zalo:112]")
                            || s.contains("code: 112")
                            || s.contains("\"error_code\":112")
                            || s.contains("\"error_code\": 112")
                    }
                };
                if is_code_112 && i < delays.len() {
                    tracing::warn!(
                        "Zalo recall returned code 112 (indexing propagation delay), retrying {}/{} after {}ms",
                        i + 1,
                        delays.len(),
                        delays[i],
                    );
                    last_err = Some(err);
                    continue;
                }
                return Err(err.into());
            }
        }
    }
    if let Some(err) = last_err {
        Err(err.into())
    } else {
        Err(ProviderSendError {
            kind: crate::conversation::zalo::client::ProviderSendErrorKind::Permanent,
            message: "Recall failed without returning error".into(),
        })
    }
}
