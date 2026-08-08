use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::ThreadType;

use crate::conversation::zalo::client::{
    build_forward_message_request, normalize_group_info, ProviderForwardRequest,
    ProviderRecallRequest, ProviderSendError, ProviderSendRequest, ProviderSendResult, ZaloClient,
    ZaloStickerReference,
};
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedSendRequest {
    pub thread_id: String,
    pub thread_type: ThreadType,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHistoryRequest {
    pub thread_type: ThreadType,
    pub cursor: Option<String>,
}

pub struct FakeZaloClient {
    pub event_tx: broadcast::Sender<ListenerEvent>,
    pub captured_history_requests: Mutex<Vec<CapturedHistoryRequest>>,
    pub captured_reaction_requests: Mutex<Vec<CapturedHistoryRequest>>,
    pub captured_send_requests: Mutex<Vec<CapturedSendRequest>>,
    pub captured_part_requests: Mutex<Vec<ProviderSendRequest>>,
    pub captured_recall_requests: Mutex<Vec<ProviderRecallRequest>>,
    pub captured_set_reaction_requests:
        Mutex<Vec<crate::conversation::zalo::client::ProviderReactionRequest>>,
    pub captured_forward_requests:
        Mutex<Vec<crate::conversation::zalo::client::ProviderForwardRequest>>,
    pub captured_sticker_searches: Mutex<Vec<String>>,
    pub captured_group_snapshot_requests: Mutex<Vec<Option<String>>>,
    pub group_snapshots: Mutex<HashMap<String, NormalizedGroupSnapshot>>,
    pub sticker_responses: Mutex<Vec<ZaloStickerReference>>,
    pub own_id: Mutex<Option<String>>,
    pub next_msg_id: Mutex<u64>,
    pub stopped: Mutex<bool>,
}

impl FakeZaloClient {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            event_tx,
            captured_history_requests: Mutex::new(Vec::new()),
            captured_reaction_requests: Mutex::new(Vec::new()),
            captured_send_requests: Mutex::new(Vec::new()),
            captured_part_requests: Mutex::new(Vec::new()),
            captured_recall_requests: Mutex::new(Vec::new()),
            captured_set_reaction_requests: Mutex::new(Vec::new()),
            captured_forward_requests: Mutex::new(Vec::new()),
            captured_sticker_searches: Mutex::new(Vec::new()),
            captured_group_snapshot_requests: Mutex::new(Vec::new()),
            group_snapshots: Mutex::new(HashMap::new()),
            sticker_responses: Mutex::new(Vec::new()),
            own_id: Mutex::new(None),
            next_msg_id: Mutex::new(1000),
            stopped: Mutex::new(false),
        }
    }

    pub fn is_stopped(&self) -> bool {
        *self.stopped.lock().unwrap()
    }
}

#[async_trait]
impl ZaloClient for FakeZaloClient {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        self.event_tx.subscribe()
    }

    fn request_old_messages(
        &self,
        thread_type: ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.captured_history_requests
            .lock()
            .unwrap()
            .push(CapturedHistoryRequest {
                thread_type,
                cursor: cursor.map(str::to_string),
            });
        Ok(())
    }

    fn request_old_reactions(
        &self,
        thread_type: ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.captured_reaction_requests
            .lock()
            .unwrap()
            .push(CapturedHistoryRequest {
                thread_type,
                cursor: cursor.map(str::to_string),
            });
        Ok(())
    }

    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>> {
        self.captured_group_snapshot_requests
            .lock()
            .unwrap()
            .push(None);
        let map = self.group_snapshots.lock().unwrap();
        Ok(map.values().cloned().collect())
    }

    async fn get_group_snapshot(
        &self,
        group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>> {
        self.captured_group_snapshot_requests
            .lock()
            .unwrap()
            .push(Some(group_id.to_string()));
        let map = self.group_snapshots.lock().unwrap();
        Ok(map.get(group_id).cloned())
    }

    async fn send_text(
        &self,
        thread_id: &str,
        thread_type: ThreadType,
        text: &str,
    ) -> anyhow::Result<String> {
        self.captured_send_requests
            .lock()
            .unwrap()
            .push(CapturedSendRequest {
                thread_id: thread_id.to_string(),
                thread_type,
                text: text.to_string(),
            });
        let mut id = self.next_msg_id.lock().unwrap();
        *id += 1;
        Ok(id.to_string())
    }

    async fn send_part(
        &self,
        request: ProviderSendRequest,
    ) -> Result<ProviderSendResult, ProviderSendError> {
        self.captured_part_requests
            .lock()
            .unwrap()
            .push(request.clone());
        let mut id = self.next_msg_id.lock().unwrap();
        *id += 1;
        let provider_message_id = id.to_string();
        let provider_client_message_id = if !request.provider_client_message_id.is_empty() {
            request.provider_client_message_id
        } else {
            provider_message_id.clone()
        };
        Ok(ProviderSendResult {
            provider_message_id,
            provider_client_message_id,
        })
    }

    async fn forward_message(
        &self,
        request: crate::conversation::zalo::client::ProviderForwardRequest,
    ) -> Result<ProviderSendResult, ProviderSendError> {
        self.captured_forward_requests
            .lock()
            .unwrap()
            .push(request.clone());
        let mut id = self.next_msg_id.lock().unwrap();
        *id += 1;
        let provider_message_id = id.to_string();
        let provider_client_message_id = if !request.provider_client_message_id.is_empty() {
            request.provider_client_message_id
        } else {
            provider_message_id.clone()
        };
        Ok(ProviderSendResult {
            provider_message_id,
            provider_client_message_id,
        })
    }

    async fn search_stickers(
        &self,
        keyword: &str,
        _limit: u32,
    ) -> Result<Vec<ZaloStickerReference>, ProviderSendError> {
        self.captured_sticker_searches
            .lock()
            .unwrap()
            .push(keyword.to_string());
        Ok(self.sticker_responses.lock().unwrap().clone())
    }

    async fn recall(&self, request: ProviderRecallRequest) -> Result<(), ProviderSendError> {
        self.captured_recall_requests.lock().unwrap().push(request);
        Ok(())
    }

    async fn set_reaction(
        &self,
        request: crate::conversation::zalo::client::ProviderReactionRequest,
    ) -> Result<(), ProviderSendError> {
        self.captured_set_reaction_requests
            .lock()
            .unwrap()
            .push(request);
        Ok(())
    }

    fn own_external_id(&self) -> Option<String> {
        self.own_id.lock().unwrap().clone()
    }

    fn stop(&self) {
        let mut stopped = self.stopped.lock().unwrap();
        *stopped = true;
    }
}

#[test]
fn native_forward_request_preserves_source_body_and_reference_metadata() {
    let request = ProviderForwardRequest {
        external_thread_id: "target-user".to_string(),
        thread_type: ThreadType::User,
        provider_client_message_id: "1785750219147".to_string(),
        source_provider_message_id: "8110983721631".to_string(),
        source_message: "Nội dung gốc cần chuyển tiếp".to_string(),
        source_timestamp_ms: 1_785_667_688_951,
    };

    let provider_request = build_forward_message_request(&request);
    let reference = provider_request.reference.expect("forward reference");

    assert_eq!(provider_request.message, "Nội dung gốc cần chuyển tiếp");
    assert_eq!(provider_request.client_id, Some(1_785_750_219_147));
    assert_eq!(reference.id, "8110983721631");
    assert_eq!(reference.ts, 1_785_667_688_951);
    assert_eq!(reference.log_src_type, 0);
    assert_eq!(reference.fw_lvl, 0);
}

#[tokio::test]
async fn test_fake_client_forwards_thread_types_and_captures_requests() {
    let client = FakeZaloClient::new();

    // Verify history request forwarding for User and Group
    client
        .request_old_messages(ThreadType::User, Some("cursor_1"))
        .unwrap();
    client
        .request_old_messages(ThreadType::Group, None)
        .unwrap();

    let history_reqs = client.captured_history_requests.lock().unwrap().clone();
    assert_eq!(history_reqs.len(), 2);
    assert_eq!(history_reqs[0].thread_type, ThreadType::User);
    assert_eq!(history_reqs[0].cursor.as_deref(), Some("cursor_1"));
    assert_eq!(history_reqs[1].thread_type, ThreadType::Group);
    assert_eq!(history_reqs[1].cursor, None);

    // Verify reaction request forwarding
    client
        .request_old_reactions(ThreadType::User, None)
        .unwrap();
    client
        .request_old_reactions(ThreadType::Group, Some("cursor_2"))
        .unwrap();

    let rxn_reqs = client.captured_reaction_requests.lock().unwrap().clone();
    assert_eq!(rxn_reqs.len(), 2);
    assert_eq!(rxn_reqs[0].thread_type, ThreadType::User);
    assert_eq!(rxn_reqs[1].thread_type, ThreadType::Group);
    assert_eq!(rxn_reqs[1].cursor.as_deref(), Some("cursor_2"));

    // Verify send_text request forwarding and deterministic msg_id
    let msg_id1 = client
        .send_text("user_123", ThreadType::User, "Hello direct")
        .await
        .unwrap();
    let msg_id2 = client
        .send_text("group_456", ThreadType::Group, "Hello group")
        .await
        .unwrap();

    assert_eq!(msg_id1, "1001");
    assert_eq!(msg_id2, "1002");

    let send_reqs = client.captured_send_requests.lock().unwrap().clone();
    assert_eq!(send_reqs.len(), 2);
    assert_eq!(
        send_reqs[0],
        CapturedSendRequest {
            thread_id: "user_123".to_string(),
            thread_type: ThreadType::User,
            text: "Hello direct".to_string(),
        }
    );
    assert_eq!(
        send_reqs[1],
        CapturedSendRequest {
            thread_id: "group_456".to_string(),
            thread_type: ThreadType::Group,
            text: "Hello group".to_string(),
        }
    );

    // Verify stop flag
    assert!(!client.is_stopped());
    client.stop();
    assert!(client.is_stopped());
}

#[tokio::test]
async fn test_fake_client_group_snapshots_complete_and_partial() {
    let client = FakeZaloClient::new();

    let complete_snapshot = NormalizedGroupSnapshot {
        provider_account_key: "imei_1".to_string(),
        external_thread_id: "g_complete".to_string(),
        title: "Complete Group".to_string(),
        avatar_url: Some("https://avatar/full.jpg".to_string()),
        participants: vec![],
        membership_complete: true,
        occurred_at: "2026-07-31T00:00:00.000Z".to_string(),
    };

    let partial_snapshot = NormalizedGroupSnapshot {
        provider_account_key: "imei_1".to_string(),
        external_thread_id: "g_partial".to_string(),
        title: "Partial Group".to_string(),
        avatar_url: None,
        participants: vec![],
        membership_complete: false,
        occurred_at: "2026-07-31T00:00:00.000Z".to_string(),
    };

    client
        .group_snapshots
        .lock()
        .unwrap()
        .insert("g_complete".to_string(), complete_snapshot);
    client
        .group_snapshots
        .lock()
        .unwrap()
        .insert("g_partial".to_string(), partial_snapshot);

    // Fetch all snapshots
    let all = client.get_all_group_snapshots().await.unwrap();
    assert_eq!(all.len(), 2);

    let snapshot_reqs = client
        .captured_group_snapshot_requests
        .lock()
        .unwrap()
        .clone();
    assert_eq!(snapshot_reqs, vec![None]);

    // Fetch complete snapshot explicitly
    let res_complete = client.get_group_snapshot("g_complete").await.unwrap();
    assert!(res_complete.is_some());
    assert!(res_complete.unwrap().membership_complete);

    // Fetch partial snapshot explicitly
    let res_partial = client.get_group_snapshot("g_partial").await.unwrap();
    assert!(res_partial.is_some());
    assert!(!res_partial.unwrap().membership_complete);

    // Fetch unknown group snapshot
    let res_unknown = client.get_group_snapshot("g_unknown").await.unwrap();
    assert!(res_unknown.is_none());

    let snapshot_reqs2 = client
        .captured_group_snapshot_requests
        .lock()
        .unwrap()
        .clone();
    assert_eq!(
        snapshot_reqs2,
        vec![
            None,
            Some("g_complete".to_string()),
            Some("g_partial".to_string()),
            Some("g_unknown".to_string())
        ]
    );
}

#[test]
fn test_normalize_group_info_mapping() {
    let setting = zca_rs::models::GroupSetting {
        block_name: 0,
        sign_admin_msg: 0,
        add_member_only: 0,
        set_topic_only: 0,
        enable_msg_history: 0,
        join_appr: 0,
        lock_create_post: 0,
        lock_create_poll: 0,
        lock_send_msg: 0,
        lock_view_member: 0,
        bann_feature: 0,
        dirty_media: 0,
        ban_duration: 0,
    };

    let group_info = zca_rs::models::GroupInfo {
        group_id: "g_100".to_string(),
        name: "Test Team".to_string(),
        desc: "Description".to_string(),
        r#type: zca_rs::models::GroupType::Group,
        creator_id: "user_owner".to_string(),
        version: "1.0".to_string(),
        avt: "http://avatar/small.jpg".to_string(),
        full_avt: "http://avatar/full.jpg".to_string(),
        member_ids: vec!["user_owner".to_string(), "user_mem".to_string()],
        admin_ids: vec!["user_owner".to_string()],
        current_mems: vec![
            zca_rs::models::GroupCurrentMem {
                id: "user_owner".to_string(),
                d_name: "Owner Display".to_string(),
                zalo_name: "Owner Zalo".to_string(),
                avatar: "http://user/owner.jpg".to_string(),
                avatar_25: "http://user/owner_25.jpg".to_string(),
                account_status: 0,
                r#type: 0,
            },
            zca_rs::models::GroupCurrentMem {
                id: "user_mem".to_string(),
                d_name: "".to_string(),
                zalo_name: "Member Zalo".to_string(),
                avatar: "".to_string(),
                avatar_25: "http://user/mem_25.jpg".to_string(),
                account_status: 0,
                r#type: 0,
            },
        ],
        update_mems: vec![],
        admins: vec![],
        has_more_member: 0,
        sub_type: 0,
        total_member: 2,
        max_member: 1000,
        setting,
        created_time: 1700000000000,
        visibility: 0,
        global_id: "gid_100".to_string(),
        e2ee: 0,
        extra_info: zca_rs::models::GroupExtraInfo {
            enable_media_store: 0,
        },
    };

    let info_with_ver = zca_rs::apis::group::GroupInfoWithVer {
        info: group_info,
        mem_ver_list: Some(vec!["user_owner_0".to_string(), "user_mem_0".to_string()]),
        pending_approve: None,
    };
    let snapshot = normalize_group_info("imei_test", info_with_ver);

    assert_eq!(snapshot.provider_account_key, "imei_test");
    assert_eq!(snapshot.external_thread_id, "g_100");
    assert_eq!(snapshot.title, "Test Team");
    assert_eq!(
        snapshot.avatar_url.as_deref(),
        Some("http://avatar/full.jpg")
    );
    assert!(snapshot.membership_complete);
    assert_eq!(snapshot.participants.len(), 2);

    // Admin member check
    let p0 = &snapshot.participants[0];
    assert_eq!(p0.identity.external_id, "user_owner");
    assert_eq!(p0.identity.identity_namespace, "zalo_uid");
    assert_eq!(p0.identity.display_name.as_deref(), Some("Owner Display"));
    assert_eq!(
        p0.identity.avatar_url.as_deref(),
        Some("http://user/owner.jpg")
    );
    assert_eq!(p0.identity.lead_id, None);
    assert_eq!(p0.participant_role, "admin");

    // Non-admin member check (falls back to zalo_name and avatar_25)
    let p1 = &snapshot.participants[1];
    assert_eq!(p1.identity.external_id, "user_mem");
    assert_eq!(p1.identity.identity_namespace, "zalo_uid");
    assert_eq!(p1.identity.display_name.as_deref(), Some("Member Zalo"));
    assert_eq!(
        p1.identity.avatar_url.as_deref(),
        Some("http://user/mem_25.jpg")
    );
    assert_eq!(p1.identity.lead_id, None);
    assert_eq!(p1.participant_role, "member");
}

#[tokio::test]
async fn test_fake_client_captures_provider_send_request_and_recall() {
    use crate::conversation::zalo::client::{ProviderOutboundContent, ProviderQuoteContext};

    let client = FakeZaloClient::new();
    *client.own_id.lock().unwrap() = Some("own-123".into());

    assert_eq!(client.own_external_id(), Some("own-123".into()));

    let quote = ProviderQuoteContext {
        provider_message_id: "p-msg-1".into(),
        provider_client_message_id: "cli-msg-1".into(),
        sender_external_id: "sender-1".into(),
        content: "quoted text".into(),
        message_type: 1,
        property_ext: None,
        provider_timestamp_ms: 1700000000000,
        ttl: 0,
    };

    let req = ProviderSendRequest {
        external_thread_id: "thread-1".into(),
        thread_type: ThreadType::User,
        provider_client_message_id: "1785491200123".into(),
        content: ProviderOutboundContent::Attachment {
            media_type: "image".into(),
            mime_type: "image/png".into(),
            file_name: "test.png".into(),
            size_bytes: 50,
            data: vec![1, 2, 3],
        },
        quote: Some(quote),
    };

    let res = client.send_part(req.clone()).await.unwrap();
    assert_eq!(res.provider_client_message_id, "1785491200123");

    {
        let captured = client.captured_part_requests.lock().unwrap();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].provider_client_message_id, "1785491200123");
        assert!(matches!(
            captured[0].content,
            ProviderOutboundContent::Attachment { .. }
        ));
        assert_eq!(
            captured[0].quote.as_ref().unwrap().provider_message_id,
            "p-msg-1"
        );
    }

    let recall_req = ProviderRecallRequest {
        external_thread_id: "thread-1".into(),
        thread_type: ThreadType::User,
        provider_message_id: res.provider_message_id.clone(),
        provider_client_message_id: res.provider_client_message_id.clone(),
    };
    client.recall(recall_req.clone()).await.unwrap();

    let recalled = client.captured_recall_requests.lock().unwrap();
    assert_eq!(recalled.len(), 1);
    assert_eq!(recalled[0], recall_req);
}

#[tokio::test]
async fn quote_reply_with_mentions_serializes_both() {
    use crate::conversation::zalo::client::{
        ProviderMention, ProviderOutboundContent, ProviderQuoteContext,
    };

    let client = FakeZaloClient::new();
    let quote = ProviderQuoteContext {
        provider_message_id: "p-msg-quote".into(),
        provider_client_message_id: "cli-msg-quote".into(),
        sender_external_id: "sender-1".into(),
        content: "quoted text".into(),
        message_type: 1,
        property_ext: None,
        provider_timestamp_ms: 1700000000000,
        ttl: 0,
    };

    let mentions = vec![
        ProviderMention {
            pos: 6,
            len: 14,
            target_external_id: Some("user-501".into()),
            is_all: false,
        },
        ProviderMention {
            pos: 21,
            len: 4,
            target_external_id: None,
            is_all: true,
        },
    ];

    let req = ProviderSendRequest {
        external_thread_id: "group-101".into(),
        thread_type: ThreadType::Group,
        provider_client_message_id: "1785491200999".into(),
        content: ProviderOutboundContent::Text {
            text: "Hello @Nguyễn Văn A @all".into(),
            mentions: mentions.clone(),
        },
        quote: Some(quote),
    };

    let res = client.send_part(req.clone()).await.unwrap();
    assert_eq!(res.provider_client_message_id, "1785491200999");

    let captured = client.captured_part_requests.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].quote.as_ref().unwrap().provider_message_id,
        "p-msg-quote"
    );
    if let ProviderOutboundContent::Text {
        text,
        mentions: cap_mentions,
    } = &captured[0].content
    {
        assert_eq!(text, "Hello @Nguyễn Văn A @all");
        assert_eq!(cap_mentions.len(), 2);
        assert_eq!(cap_mentions[0], mentions[0]);
        assert_eq!(cap_mentions[1], mentions[1]);
    } else {
        panic!("Expected ProviderOutboundContent::Text");
    }
}
