use super::events::{ConversationEventPublisher, ConversationInvalidationReason};
use super::migration::migrate;
use super::repository::{ConversationRepository, NormalizedIdentity, NormalizedMessage};
use super::service::ConversationService;
use super::types::{Channel, ConversationKind, MessageDirection, MessageStatus};
use crate::db::open_db_connection;
use std::sync::Arc;

pub struct ServiceHarness {
    pub path: std::path::PathBuf,
    pub repository: Arc<ConversationRepository>,
    pub events: Arc<ConversationEventPublisher>,
    pub service: ConversationService,
}

impl Default for ServiceHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceHarness {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "rustsale-conversation-service-{}.db",
            rand::random::<u64>()
        ));
        let conn = open_db_connection(path.to_str().unwrap()).unwrap();
        conn.execute(
            "CREATE TABLE leads (id INTEGER PRIMARY KEY, name TEXT NOT NULL, phone TEXT, zalo_uid TEXT)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS zalo_sessions (
                imei TEXT PRIMARY KEY,
                user_agent TEXT NOT NULL,
                cookies_json TEXT NOT NULL,
                language TEXT NOT NULL,
                display_name TEXT,
                avatar TEXT
            )",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        drop(conn);

        let repository = Arc::new(ConversationRepository::new(
            path.to_string_lossy().to_string(),
        ));
        let events = Arc::new(ConversationEventPublisher::new(256));
        let spool_root = path.parent().unwrap().join("spool-harness");
        let service = ConversationService::new(repository.clone(), events.clone(), spool_root);

        Self {
            path,
            repository,
            events,
            service,
        }
    }

    pub fn create_conv_state(
        &self,
        service: Arc<ConversationService>,
        zalo_supervisor: Arc<crate::conversation::zalo::supervisor::ZaloSessionSupervisor>,
    ) -> super::commands::ConversationState {
        let facebook_supervisor = Arc::new(
            super::facebook::supervisor::FacebookMessengerSupervisor::new(
                self.path.to_str().unwrap().to_string(),
                self.events.clone(),
                Arc::new(super::facebook::client::RealFacebookGraphClient::new()),
            ),
        );
        let sessions_dir = self
            .path
            .parent()
            .unwrap()
            .join("whatsapp-sessions-harness");
        let whatsapp_supervisor = Arc::new(
            crate::conversation::whatsapp::supervisor::WhatsAppSupervisor::new(
                self.path.to_str().unwrap().to_string(),
                sessions_dir.clone(),
                self.events.clone(),
            ),
        );
        let whatsapp_login_manager = Arc::new(
            crate::conversation::whatsapp::login::WhatsAppLoginManager::new_mock_with_dir(
                sessions_dir,
            ),
        );
        super::commands::ConversationState {
            service,
            zalo_supervisor,
            facebook_supervisor,
            whatsapp_supervisor,
            whatsapp_login_manager,
        }
    }

    pub fn insert_lead(&self, id: i64, name: &str) {
        let conn = open_db_connection(self.path.to_str().unwrap()).unwrap();
        conn.execute(
            "INSERT INTO leads (id, name) VALUES (?1, ?2)",
            rusqlite::params![id, name],
        )
        .unwrap();
    }
}

impl Drop for ServiceHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_file(self.path.with_extension("db-wal"));
        let _ = std::fs::remove_file(self.path.with_extension("db-shm"));
    }
}

pub fn direct_message(account: &str, provider_message_id: &str) -> NormalizedMessage {
    direct_message_for(account, "peer-1", provider_message_id)
}

pub fn direct_message_for(
    account: &str,
    peer_id: &str,
    provider_message_id: &str,
) -> NormalizedMessage {
    NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: account.to_string(),
        account_display_name: Some(account.to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: peer_id.to_string(),
            display_name: Some(format!("Peer {peer_id}")),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: peer_id.to_string(),
        conversation_kind: ConversationKind::Direct,
        conversation_title: Some(format!("Peer {peer_id}")),
        direction: MessageDirection::Inbound,
        provider_message_id: Some(provider_message_id.to_string()),
        provider_client_message_id: Some(format!("client-{provider_message_id}")),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some(provider_message_id.to_string()),
        content: serde_json::json!({"text": provider_message_id}),
        attachments: Vec::new(),
        reply: None,
        mentions: Vec::new(),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T00:00:00.000Z".to_string()),
        ingested_at: "2026-07-31T00:00:01.000Z".to_string(),
        status: MessageStatus::Received,
    }
}

pub fn group_message(
    account: &str,
    group_thread_id: &str,
    provider_message_id: &str,
) -> NormalizedMessage {
    NormalizedMessage {
        channel: Channel::Zalo,
        provider_account_key: account.to_string(),
        account_display_name: Some(account.to_string()),
        participant: Some(NormalizedIdentity {
            identity_namespace: "zalo_uid".to_string(),
            external_id: "peer-g1".to_string(),
            display_name: Some("Group Member 1".to_string()),
            avatar_url: None,
            lead_id: None,
        }),
        sender_is_participant: true,
        external_thread_id: group_thread_id.to_string(),
        conversation_kind: ConversationKind::Group,
        conversation_title: Some(format!("Group {group_thread_id}")),
        direction: MessageDirection::Inbound,
        provider_message_id: Some(provider_message_id.to_string()),
        provider_client_message_id: Some(format!("client-{provider_message_id}")),
        local_request_id: None,
        content_type: "text".to_string(),
        text_content: Some("hello group".to_string()),
        content: serde_json::json!({"text": "hello group"}),
        attachments: Vec::new(),
        reply: None,
        mentions: Vec::new(),
        raw_payload: None,
        provider_timestamp: Some("2026-07-31T00:00:00.000Z".to_string()),
        ingested_at: "2026-07-31T00:00:01.000Z".to_string(),
        status: MessageStatus::Received,
    }
}

#[test]
fn list_inbox_returns_customer_unknown_and_group_rows() {
    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let linked = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "linked-message"))
        .unwrap();
    harness
        .repository
        .link_identity(linked.identity_id.unwrap(), 7)
        .unwrap();
    let unknown = harness
        .repository
        .apply_normalized_message(&direct_message_for("imei-a", "peer-2", "unknown-message"))
        .unwrap();
    let group = harness
        .repository
        .apply_normalized_message(&group_message("imei-a", "group-1", "group-message"))
        .unwrap();

    let rows = harness.service.list_inbox().unwrap();
    let keys = rows.iter().map(|row| row.key.as_str()).collect::<Vec<_>>();
    let unknown_key = format!("identity:{}", unknown.identity_id.unwrap());
    let group_key = format!("conversation:{}", group.conversation_id);
    assert_eq!(keys.iter().filter(|key| **key == "lead:7").count(), 1);
    assert_eq!(
        keys.iter()
            .filter(|key| **key == unknown_key.as_str())
            .count(),
        1
    );
    assert_eq!(
        keys.iter()
            .filter(|key| **key == group_key.as_str())
            .count(),
        1
    );
}

#[test]
fn link_identity_publishes_link_invalidation() {
    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let mut receiver = harness.events.subscribe();

    harness
        .service
        .link_identity(applied.identity_id.unwrap(), 7)
        .unwrap();

    let event = receiver.try_recv().unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Link);
    assert_eq!(event.lead_id, Some(7));
    assert_eq!(event.conversation_id, applied.conversation_id);
}

#[test]
fn unlink_identity_publishes_link_invalidation() {
    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let identity_id = applied.identity_id.unwrap();
    harness.service.link_identity(identity_id, 7).unwrap();

    let mut receiver = harness.events.subscribe();
    harness.service.unlink_identity(identity_id).unwrap();

    let event = receiver.try_recv().unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Link);
    assert_eq!(event.lead_id, None);
    assert_eq!(event.conversation_id, applied.conversation_id);
}

#[tokio::test]
async fn test_conversation_state_commands_integration() {
    use super::commands::{
        get_conversation_runtime_status, restart_conversation_account, send_conversation_message,
        ConversationState,
    };
    use crate::conversation::types::SendConversationMessageRequest;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-cmd1"),
    ));

    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));

    let conv_state = harness.create_conv_state(conversation_service, zalo_supervisor);

    let app = MockApp::new().manage(conv_state);

    let state = app.state::<ConversationState>();

    // 1. get_conversation_runtime_status
    let status = get_conversation_runtime_status(state.clone())
        .await
        .unwrap();
    assert!(status.accounts.is_empty());

    // 2. restart_conversation_account
    let res = restart_conversation_account(state.clone(), "non-existent-account".to_string()).await;
    assert!(res.is_err());

    // 3. send_conversation_message
    let req = SendConversationMessageRequest {
        conversation_id: 1,
        channel_account_id: 1,
        text: "hello tauri command".to_string(),
        reply_to_message_id: None,
    };
    let _ = send_conversation_message(state.clone(), req);
}

#[tokio::test]
async fn test_unlink_conversation_lead_command() {
    use super::commands::{unlink_conversation_lead, ConversationState};
    use super::types::UnlinkLeadRequest;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let identity_id = applied.identity_id.unwrap();
    harness.service.link_identity(identity_id, 7).unwrap();

    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-cmd2"),
    ));

    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));

    let conv_state = harness.create_conv_state(conversation_service, zalo_supervisor);

    let app = MockApp::new().manage(conv_state);

    let state = app.state::<ConversationState>();
    let mut receiver = harness.events.subscribe();

    unlink_conversation_lead(state, UnlinkLeadRequest { lead_id: 7 }).unwrap();

    let event = receiver.try_recv().unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Link);
    assert_eq!(event.lead_id, Some(7));
    assert_eq!(event.conversation_id, applied.conversation_id);
}

#[tokio::test]
async fn test_unlink_conversation_identity_command() {
    use super::commands::{unlink_conversation_identity, ConversationState};
    use super::types::UnlinkIdentityRequest;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let identity_id = applied.identity_id.unwrap();
    harness.service.link_identity(identity_id, 7).unwrap();

    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-cmd3"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    unlink_conversation_identity(state, UnlinkIdentityRequest { identity_id }).unwrap();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let lead_id: Option<i64> = conn
        .query_row(
            "SELECT lead_id FROM channel_identities WHERE id = ?1",
            [identity_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(lead_id, None);
}

#[tokio::test]
async fn test_relink_conversation_lead_command() {
    use super::commands::{relink_conversation_lead, ConversationState};
    use super::types::RelinkLeadRequest;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    harness.insert_lead(7, "Lead Seven");
    harness.insert_lead(8, "Lead Eight");
    let applied = harness
        .repository
        .apply_normalized_message(&direct_message("imei-a", "message-1"))
        .unwrap();
    let identity_id = applied.identity_id.unwrap();
    harness.service.link_identity(identity_id, 7).unwrap();

    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-cmd4"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();
    let mut receiver = harness.events.subscribe();

    relink_conversation_lead(
        state,
        RelinkLeadRequest {
            source_lead_id: 7,
            target_lead_id: 8,
        },
    )
    .unwrap();

    assert_eq!(
        harness
            .repository
            .lead_id_for_identity(identity_id)
            .unwrap(),
        Some(8)
    );
    let source_event = receiver.try_recv().unwrap();
    let target_event = receiver.try_recv().unwrap();
    assert_eq!(source_event.lead_id, Some(7));
    assert_eq!(target_event.lead_id, Some(8));
}

#[tokio::test]
async fn quick_template_command_list_active_and_all() {
    use super::commands::{
        create_conversation_quick_template, delete_conversation_quick_template,
        list_conversation_quick_templates, ConversationState,
    };
    use super::types::{
        CreateConversationQuickTemplateRequest, DeleteConversationQuickTemplateRequest,
        ListConversationQuickTemplatesRequest,
    };
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-qt1"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let t1 = create_conversation_quick_template(
        state.clone(),
        CreateConversationQuickTemplateRequest {
            shortcut: "/hello".into(),
            title: "Greeting".into(),
            content: "Hello world".into(),
            category: Some("General".into()),
        },
    )
    .unwrap();

    let t2 = create_conversation_quick_template(
        state.clone(),
        CreateConversationQuickTemplateRequest {
            shortcut: "/bye".into(),
            title: "Goodbye".into(),
            content: "Goodbye world".into(),
            category: Some("Support".into()),
        },
    )
    .unwrap();

    delete_conversation_quick_template(
        state.clone(),
        DeleteConversationQuickTemplateRequest { id: t2.id },
    )
    .unwrap();

    let list = list_conversation_quick_templates(
        state.clone(),
        ListConversationQuickTemplatesRequest {
            category: None,
            search_query: None,
            include_inactive: false,
        },
    )
    .unwrap();

    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, t1.id);
    assert_eq!(list[0].shortcut, "hello");

    let search_list = list_conversation_quick_templates(
        state.clone(),
        ListConversationQuickTemplatesRequest {
            category: None,
            search_query: Some("hello".into()),
            include_inactive: false,
        },
    )
    .unwrap();
    assert_eq!(search_list.len(), 1);
    assert_eq!(search_list[0].id, t1.id);

    let cat_list = list_conversation_quick_templates(
        state,
        ListConversationQuickTemplatesRequest {
            category: Some("General".into()),
            search_query: None,
            include_inactive: false,
        },
    )
    .unwrap();
    assert_eq!(cat_list.len(), 1);
    assert_eq!(cat_list[0].id, t1.id);
}

#[tokio::test]
async fn quick_template_command_create_success_and_conflict() {
    use super::commands::{create_conversation_quick_template, ConversationState};
    use super::types::CreateConversationQuickTemplateRequest;
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-qt2"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let req = CreateConversationQuickTemplateRequest {
        shortcut: "/thanks".into(),
        title: "Thank You".into(),
        content: "Thank you for contacting us!".into(),
        category: Some("General".into()),
    };

    let created = create_conversation_quick_template(state.clone(), req).unwrap();
    assert_eq!(created.shortcut, "thanks");
    assert_eq!(created.title, "Thank You");

    let dup_req = CreateConversationQuickTemplateRequest {
        shortcut: "THANKS".into(),
        title: "Another Thanks".into(),
        content: "Thanks again!".into(),
        category: None,
    };

    let err = create_conversation_quick_template(state, dup_req).unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::TemplateShortcutConflict);
}

#[tokio::test]
async fn quick_template_command_update_validation() {
    use super::commands::{
        create_conversation_quick_template, update_conversation_quick_template, ConversationState,
    };
    use super::types::{
        CreateConversationQuickTemplateRequest, UpdateConversationQuickTemplateRequest,
    };
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-qt3"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let created = create_conversation_quick_template(
        state.clone(),
        CreateConversationQuickTemplateRequest {
            shortcut: "/faq".into(),
            title: "FAQ Answer".into(),
            content: "Here is the answer.".into(),
            category: None,
        },
    )
    .unwrap();

    let updated = update_conversation_quick_template(
        state.clone(),
        UpdateConversationQuickTemplateRequest {
            id: created.id,
            shortcut: "/faq1".into(),
            title: "FAQ 1".into(),
            content: "Updated content".into(),
            category: Some("FAQ".into()),
            is_active: true,
        },
    )
    .unwrap();
    assert_eq!(updated.shortcut, "faq1");
    assert_eq!(updated.title, "FAQ 1");

    let err = update_conversation_quick_template(
        state.clone(),
        UpdateConversationQuickTemplateRequest {
            id: created.id,
            shortcut: "/faq1".into(),
            title: "   ".into(),
            content: "Updated content".into(),
            category: None,
            is_active: true,
        },
    )
    .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);

    let err = update_conversation_quick_template(
        state,
        UpdateConversationQuickTemplateRequest {
            id: 99999,
            shortcut: "/nonexistent".into(),
            title: "Title".into(),
            content: "Content".into(),
            category: None,
            is_active: true,
        },
    )
    .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[tokio::test]
async fn quick_template_command_delete_success() {
    use super::commands::{
        create_conversation_quick_template, delete_conversation_quick_template,
        list_conversation_quick_templates, ConversationState,
    };
    use super::types::{
        CreateConversationQuickTemplateRequest, DeleteConversationQuickTemplateRequest,
        ListConversationQuickTemplatesRequest,
    };
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-qt4"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let created = create_conversation_quick_template(
        state.clone(),
        CreateConversationQuickTemplateRequest {
            shortcut: "/delete_me".into(),
            title: "Temporary".into(),
            content: "To be deleted".into(),
            category: None,
        },
    )
    .unwrap();

    delete_conversation_quick_template(
        state.clone(),
        DeleteConversationQuickTemplateRequest { id: created.id },
    )
    .unwrap();

    let list = list_conversation_quick_templates(
        state.clone(),
        ListConversationQuickTemplatesRequest {
            category: None,
            search_query: None,
            include_inactive: false,
        },
    )
    .unwrap();
    assert!(list.is_empty());

    let err = delete_conversation_quick_template(
        state,
        DeleteConversationQuickTemplateRequest { id: created.id },
    )
    .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[tokio::test]
async fn quick_template_command_mark_used_updates_usage() {
    use super::commands::{
        create_conversation_quick_template, list_conversation_quick_templates,
        mark_conversation_quick_template_used, ConversationState,
    };
    use super::types::{
        CreateConversationQuickTemplateRequest, ListConversationQuickTemplatesRequest,
        MarkConversationQuickTemplateUsedRequest,
    };
    use crate::conversation::error::ConversationErrorCode;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-qt5"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let created = create_conversation_quick_template(
        state.clone(),
        CreateConversationQuickTemplateRequest {
            shortcut: "/promo".into(),
            title: "Promo Code".into(),
            content: "Use code SAVE10".into(),
            category: None,
        },
    )
    .unwrap();
    assert_eq!(created.usage_count, 0);

    mark_conversation_quick_template_used(
        state.clone(),
        MarkConversationQuickTemplateUsedRequest { id: created.id },
    )
    .unwrap();

    let list = list_conversation_quick_templates(
        state.clone(),
        ListConversationQuickTemplatesRequest {
            category: None,
            search_query: None,
            include_inactive: false,
        },
    )
    .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].usage_count, 1);
    assert!(list[0].last_used_at.is_some());

    let err = mark_conversation_quick_template_used(
        state,
        MarkConversationQuickTemplateUsedRequest { id: 99999 },
    )
    .unwrap_err();
    assert_eq!(err.code, ConversationErrorCode::InvalidRequest);
}

#[tokio::test]
async fn phase2a_command_registration() {
    use super::commands::{
        create_conversation_quick_template, delete_conversation_quick_template,
        forward_conversation_message, list_conversation_mention_candidates,
        list_conversation_quick_templates, mark_conversation_quick_template_used,
        retry_conversation_forward_target, set_conversation_reaction,
        update_conversation_quick_template, ConversationState,
    };
    use super::types::{
        ConversationReactionCode, CreateConversationQuickTemplateRequest,
        DeleteConversationQuickTemplateRequest, ForwardConversationMessageRequest,
        ListConversationMentionCandidatesRequest, ListConversationQuickTemplatesRequest,
        MarkConversationQuickTemplateUsedRequest, RetryConversationForwardTargetRequest,
        SetConversationReactionRequest, UpdateConversationQuickTemplateRequest,
    };
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-p2a-reg"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    // 1. list_conversation_quick_templates
    let templates = list_conversation_quick_templates(
        state.clone(),
        ListConversationQuickTemplatesRequest {
            category: None,
            search_query: None,
            include_inactive: false,
        },
    )
    .unwrap();
    assert!(templates.is_empty());

    // 2. create_conversation_quick_template
    let created = create_conversation_quick_template(
        state.clone(),
        CreateConversationQuickTemplateRequest {
            shortcut: "/hello".into(),
            title: "Hello".into(),
            content: "Hello World".into(),
            category: Some("greeting".into()),
        },
    )
    .unwrap();
    assert_eq!(created.shortcut, "hello");

    // 3. update_conversation_quick_template
    let updated = update_conversation_quick_template(
        state.clone(),
        UpdateConversationQuickTemplateRequest {
            id: created.id,
            shortcut: "/hello".into(),
            title: "Hello Updated".into(),
            content: "Hello World!".into(),
            category: Some("greeting".into()),
            is_active: true,
        },
    )
    .unwrap();
    assert_eq!(updated.title, "Hello Updated");

    // 4. mark_conversation_quick_template_used
    mark_conversation_quick_template_used(
        state.clone(),
        MarkConversationQuickTemplateUsedRequest { id: created.id },
    )
    .unwrap();

    // 5. delete_conversation_quick_template
    delete_conversation_quick_template(
        state.clone(),
        DeleteConversationQuickTemplateRequest { id: created.id },
    )
    .unwrap();

    // 6. list_conversation_mention_candidates
    let candidates = list_conversation_mention_candidates(
        state.clone(),
        ListConversationMentionCandidatesRequest {
            conversation_id: 1,
            source_message_id: None,
            query: None,
            limit: None,
        },
    )
    .unwrap();
    assert!(candidates.is_empty());

    // 7. forward_conversation_message (fails gracefully with invalid request since message doesn't exist)
    let fwd_err = forward_conversation_message(
        state.clone(),
        ForwardConversationMessageRequest {
            client_operation_id: "op-1".into(),
            source_message_id: 9999,
            target_conversation_ids: vec![1],
        },
    )
    .unwrap_err();
    assert_eq!(
        fwd_err.code,
        crate::conversation::error::ConversationErrorCode::ForwardSourceMissingProviderIds
    );

    // 8. retry_conversation_forward_target (fails gracefully with operation not found)
    let retry_err = retry_conversation_forward_target(
        state.clone(),
        RetryConversationForwardTargetRequest {
            client_operation_id: "op-nonexistent".into(),
            target_conversation_id: 1,
        },
    )
    .unwrap_err();
    assert_eq!(
        retry_err.code,
        crate::conversation::error::ConversationErrorCode::InvalidRequest
    );

    // 9. set_conversation_reaction (fails gracefully with message not found)
    let react_err = set_conversation_reaction(
        state.clone(),
        SetConversationReactionRequest {
            message_id: 9999,
            reaction: ConversationReactionCode::Like,
            active: true,
            count: None,
            client_mutation_id: "mut-command-missing".to_string(),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        react_err.code,
        crate::conversation::error::ConversationErrorCode::InvalidRequest
    );
}

#[tokio::test]
async fn mention_candidates_query_matching_and_all() {
    use super::commands::{list_conversation_mention_candidates, ConversationState};
    use super::types::ListConversationMentionCandidatesRequest;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // Create group conversation
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'g1', 'group', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Create lead with phone
    conn.execute(
        "INSERT INTO leads (id, name, phone) VALUES (10, 'Nguyen Van A', '0901234567')",
        [],
    )
    .unwrap();

    // Create identities
    conn.execute(
        "INSERT INTO channel_identities (id, lead_id, channel, identity_namespace, external_id, display_name, created_at, updated_at) VALUES (100, 10, 'zalo', 'zalo_uid', 'user-a', 'Nguyen Van A', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at) VALUES (101, 'zalo', 'zalo_uid', 'user-b', 'Tran Thi B', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, created_at, updated_at) VALUES (142, 'zalo', 'zalo_uid', '5025114793295720787', 'Nct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Add active participants
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at) VALUES (1, 100, '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at) VALUES (1, 101, '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, sender_identity_id, direction, content_type, text_content, ingested_at, reply_sender_identity_id) VALUES (4006, 1, 100, 'inbound', 'text', '@Nct còn không bác rep em với hic', '2026-08-13T02:04:27.762Z', 142)",
        [],
    ).unwrap();
    drop(conn);

    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-mc1"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    // Test 1: Empty query returns @all + all active participants
    let candidates = list_conversation_mention_candidates(
        state.clone(),
        ListConversationMentionCandidatesRequest {
            conversation_id: 1,
            source_message_id: None,
            query: None,
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(candidates.len(), 3);
    assert!(candidates[0].is_all);
    assert_eq!(candidates[1].display_name, "Nguyen Van A");
    assert_eq!(candidates[2].display_name, "Tran Thi B");

    // Test 2: Search by phone prefix
    let phone_match = list_conversation_mention_candidates(
        state.clone(),
        ListConversationMentionCandidatesRequest {
            conversation_id: 1,
            source_message_id: None,
            query: Some("0901".into()),
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(phone_match.len(), 1);
    assert_eq!(phone_match[0].display_name, "Nguyen Van A");

    // Test 3: Search with limit
    let limited = list_conversation_mention_candidates(
        state.clone(),
        ListConversationMentionCandidatesRequest {
            conversation_id: 1,
            source_message_id: None,
            query: None,
            limit: Some(2),
        },
    )
    .unwrap();
    assert_eq!(limited.len(), 2);

    let reply_alias_match = list_conversation_mention_candidates(
        state.clone(),
        ListConversationMentionCandidatesRequest {
            conversation_id: 1,
            source_message_id: Some(4006),
            query: Some("Nct".into()),
            limit: None,
        },
    )
    .unwrap();
    assert_eq!(reply_alias_match.len(), 1);
    assert_eq!(reply_alias_match[0].identity_id, Some(142));
    assert_eq!(
        reply_alias_match[0].external_id.as_deref(),
        Some("5025114793295720787")
    );
}

#[tokio::test]
async fn startup_recovery_sequence() {
    let harness = ServiceHarness::new();
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    // Insert channel account & conversation
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'c1', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Insert message
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (10, 1, 'outbound', 'text', 'hello', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Insert running reaction mutation for message 10
    conn.execute(
        "INSERT INTO reaction_mutations (client_mutation_id, message_id, reaction_code, active, status, created_at, updated_at) VALUES ('mut-run-1', 10, 'LIKE', 1, 'running', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Insert outbox job in running state for message 10
    conn.execute(
        "INSERT INTO message_outbox_jobs (id, message_id, channel_account_id, status, attempt_count, created_at, updated_at) VALUES (100, 10, 1, 'running', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    drop(conn);

    // Run recover_startup
    let count = harness.service.recover_startup().unwrap();
    assert_eq!(count, 1);

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let mut_status: String = conn
        .query_row(
            "SELECT status FROM reaction_mutations WHERE client_mutation_id = 'mut-run-1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mut_status, "delivery_unknown");

    let outbox_status: String = conn
        .query_row(
            "SELECT status FROM message_outbox_jobs WHERE id = 100",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outbox_status, "failed");

    let msg_status: String = conn
        .query_row("SELECT status FROM messages WHERE id = 10", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(msg_status, "failed");
}

#[tokio::test]
async fn test_delete_lead_conversation_does_not_delete_group_chats() {
    let harness = ServiceHarness::new();
    harness.insert_lead(10, "Lead Customer");
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, lead_id, created_at, updated_at) VALUES (100, 'zalo', 'zalo_uid', 'user-100', 'Lead User', 10, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Direct conversation (kind = 'direct')
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (1, 1, 'direct-1', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (1, 100, 'member')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (10, 1, 'inbound', 'text', 'Direct msg', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Group conversation (kind = 'group')
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (2, 1, 'group-2', 'group', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (2, 100, 'member')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (20, 2, 'inbound', 'text', 'Group msg', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: None,
        identity_id: None,
        lead_id: Some(10),
    };
    harness.service.delete_conversation(&del_req).unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    let direct_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE id = 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(direct_count, 0);

    let group_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE id = 2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(group_count, 1);

    let group_msg_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = 2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(group_msg_count, 1);

    let group_part_count: i64 = conn.query_row("SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = 2 AND identity_id = 100", [], |r| r.get(0)).unwrap();
    assert_eq!(group_part_count, 1);
}

#[tokio::test]
async fn test_delete_identity_conversation_does_not_corrupt_group() {
    let harness = ServiceHarness::new();
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_identities (id, channel, identity_namespace, external_id, display_name, lead_id, created_at, updated_at) VALUES (200, 'zalo', 'zalo_uid', 'user-200', 'Unknown User', NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Direct conversation
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (10, 1, 'direct-10', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (10, 200, 'member')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (100, 10, 'inbound', 'text', 'Inbound direct', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (101, 10, 'outbound', 'text', 'Outbound direct', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Group conversation
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (20, 1, 'group-20', 'group', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, participant_role) VALUES (20, 200, 'member')",
        [],
    ).unwrap();

    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: None,
        identity_id: Some(200),
        lead_id: None,
    };
    harness.service.delete_conversation(&del_req).unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    let direct_msg_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE conversation_id = 10",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(direct_msg_count, 0);

    let group_part: i64 = conn.query_row("SELECT COUNT(*) FROM conversation_participants WHERE conversation_id = 20 AND identity_id = 200", [], |r| r.get(0)).unwrap();
    assert_eq!(group_part, 1);
}

#[tokio::test]
async fn test_delete_conversation_cleans_up_attachment_files() {
    let harness = ServiceHarness::new();
    std::fs::create_dir_all(harness.service.stager.root()).unwrap();
    let tmp_file = harness
        .service
        .stager
        .root()
        .join(format!("rustsale-test-att-{}.tmp", rand::random::<u64>()));
    std::fs::write(&tmp_file, b"test file content").unwrap();
    assert!(tmp_file.exists());

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (30, 1, 'conv-30', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (300, 30, 'inbound', 'image', 'photo', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, local_path, created_at) VALUES (300, 'image', ?1, '2026-01-01T00:00:00Z')",
        [tmp_file.to_str().unwrap()],
    ).unwrap();
    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(30),
        identity_id: None,
        lead_id: None,
    };
    harness.service.delete_conversation(&del_req).unwrap();

    assert!(!tmp_file.exists());
}

#[tokio::test]
async fn test_delete_conversation_tombstone_prevents_replay_and_allows_future_message() {
    let harness = ServiceHarness::new();
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (40, 1, 'peer-40', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (400, 40, 'inbound', 'text', 'Hello', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(40),
        identity_id: None,
        lead_id: None,
    };
    harness.service.delete_conversation(&del_req).unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "UPDATE deleted_conversations
         SET deleted_at = '2026-08-12T10:00:00.500123+00:00'
         WHERE channel_account_id = 1 AND external_thread_id = 'peer-40' AND kind = 'direct'",
        [],
    )
    .unwrap();
    drop(conn);

    // Provider time is authoritative for history sync, even when ingestion happens much later.
    let mut msg_old = direct_message_for("acc1", "peer-40", "msg-replay-1");
    msg_old.provider_timestamp = Some("2026-01-02T00:00:00Z".to_string());
    msg_old.ingested_at = "2099-01-01T00:00:00Z".to_string();
    let result_old = harness
        .repository
        .apply_normalized_message(&msg_old)
        .unwrap();
    assert_eq!(result_old.conversation_id, 0);

    // Equivalent RFC3339 offsets and fractional precision must be compared chronologically.
    let mut msg_fractional = direct_message_for("acc1", "peer-40", "msg-replay-fractional");
    msg_fractional.provider_timestamp = Some("2026-08-12T10:00:00.500Z".to_string());
    msg_fractional.ingested_at = "2099-01-01T00:00:00Z".to_string();
    let result_fractional = harness
        .repository
        .apply_normalized_message(&msg_fractional)
        .unwrap();
    assert_eq!(result_fractional.conversation_id, 0);

    // An invalid provider timestamp must fail closed instead of clearing the tombstone.
    let mut msg_invalid = direct_message_for("acc1", "peer-40", "msg-replay-invalid");
    msg_invalid.provider_timestamp = Some("not-a-timestamp".to_string());
    msg_invalid.ingested_at = "2099-01-01T00:00:00Z".to_string();
    let result_invalid = harness
        .repository
        .apply_normalized_message(&msg_invalid)
        .unwrap();
    assert_eq!(result_invalid.conversation_id, 0);

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let conv_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE external_thread_id = 'peer-40'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(conv_count, 0);
    drop(conn);

    // A genuinely newer provider event clears the tombstone even if ingested_at is older.
    let mut msg_new = direct_message_for("acc1", "peer-40", "msg-new-future-1");
    msg_new.provider_timestamp = Some("2026-08-12T10:00:00.500124Z".to_string());
    msg_new.ingested_at = "2020-01-01T00:00:00Z".to_string();
    let result_new = harness
        .repository
        .apply_normalized_message(&msg_new)
        .unwrap();
    assert!(result_new.conversation_id > 0);

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let new_conv_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE external_thread_id = 'peer-40'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(new_conv_count, 1);
    let tombstone_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deleted_conversations
             WHERE channel_account_id = 1 AND external_thread_id = 'peer-40' AND kind = 'direct'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tombstone_count, 0);
}

#[tokio::test]
async fn test_delete_conversation_batch_rollback_preserves_files() {
    let harness = ServiceHarness::new();
    std::fs::create_dir_all(harness.service.stager.root()).unwrap();

    let file1 = harness.service.stager.root().join("att-1.tmp");
    std::fs::write(&file1, b"file 1").unwrap();
    assert!(file1.exists());

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (50, 1, 'conv-50', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (999999, 1, 'conv-999999', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (500, 50, 'inbound', 'image', 'photo', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, local_path, created_at) VALUES (500, 'image', ?1, '2026-01-01T00:00:00Z')",
        [file1.to_str().unwrap()],
    ).unwrap();

    // Attach trigger to force failure during transaction execution on conversation 999999
    conn.execute(
        "CREATE TRIGGER force_fail_on_999999 BEFORE DELETE ON channel_conversations
         BEGIN
             SELECT CASE WHEN OLD.id = 999999 THEN RAISE(FAIL, 'forced trigger failure mid-transaction') END;
         END;",
        [],
    ).unwrap();
    drop(conn);

    let reqs = vec![
        crate::conversation::types::DeleteConversationRequest {
            conversation_id: Some(50),
            identity_id: None,
            lead_id: None,
        },
        crate::conversation::types::DeleteConversationRequest {
            conversation_id: Some(999999),
            identity_id: None,
            lead_id: None,
        },
    ];

    let err = harness.service.delete_conversations_batch(&reqs);
    assert!(err.is_err());

    // File must be preserved because mid-transaction DB failure rolled back the transaction
    assert!(file1.exists());
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE id = 50",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_delete_conversation_queue_insert_failure_rolls_back_database_and_file() {
    let harness = ServiceHarness::new();
    std::fs::create_dir_all(harness.service.stager.root()).unwrap();
    let attachment_file = harness.service.stager.root().join(format!(
        "queue-insert-rollback-{}.tmp",
        rand::random::<u64>()
    ));
    std::fs::write(&attachment_file, b"keep on rollback").unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at)
         VALUES (1, 'zalo', 'queue-rollback-account', '2026-08-13T00:00:00Z', '2026-08-13T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at)
         VALUES (55, 1, 'queue-rollback-thread', 'direct', '2026-08-13T00:00:00Z', '2026-08-13T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at)
         VALUES (550, 55, 'inbound', 'image', 'attachment', '2026-08-13T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, local_path, created_at)
         VALUES (550, 'image', ?1, '2026-08-13T00:00:00Z')",
        [attachment_file.to_str().unwrap()],
    )
    .unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_pending_cleanup_insert
         BEFORE INSERT ON pending_spool_cleanups
         BEGIN
             SELECT RAISE(FAIL, 'forced pending cleanup insert failure');
         END;",
    )
    .unwrap();
    drop(conn);

    let result = harness.service.delete_conversation(
        &crate::conversation::types::DeleteConversationRequest {
            conversation_id: Some(55),
            identity_id: None,
            lead_id: None,
        },
    );
    assert!(result.is_err());

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let conversation_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations WHERE id = 55",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let message_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages WHERE id = 550", [], |row| {
            row.get(0)
        })
        .unwrap();
    let attachment_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM message_attachments WHERE message_id = 550",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let cleanup_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pending_spool_cleanups", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(conversation_count, 1);
    assert_eq!(message_count, 1);
    assert_eq!(attachment_count, 1);
    assert_eq!(cleanup_count, 0);
    assert!(attachment_file.exists());
    let _ = std::fs::remove_file(attachment_file);
}

#[tokio::test]
async fn test_recover_startup_replays_pending_spool_cleanup_queue() {
    let harness = ServiceHarness::new();
    std::fs::create_dir_all(harness.service.stager.root()).unwrap();
    let queued_file = harness
        .service
        .stager
        .root()
        .join(format!("startup-cleanup-{}.tmp", rand::random::<u64>()));
    std::fs::write(&queued_file, b"delete on startup").unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO pending_spool_cleanups (local_path, created_at)
         VALUES (?1, '2026-08-13T00:00:00Z')",
        [queued_file.to_str().unwrap()],
    )
    .unwrap();
    drop(conn);

    harness.service.recover_startup().unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let cleanup_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pending_spool_cleanups", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(!queued_file.exists());
    assert_eq!(cleanup_count, 0);
}

#[tokio::test]
async fn test_spool_cleanup_missing_root_fails_closed_and_keeps_queue() {
    let harness = ServiceHarness::new();
    let outside_file = std::env::temp_dir().join(format!(
        "rustsale-missing-spool-outside-{}.tmp",
        rand::random::<u64>()
    ));
    let missing_root = std::env::temp_dir().join(format!(
        "rustsale-missing-spool-root-{}",
        rand::random::<u64>()
    ));
    std::fs::write(&outside_file, b"must remain").unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO pending_spool_cleanups (local_path, created_at)
         VALUES (?1, '2026-08-13T00:00:00Z')",
        [outside_file.to_str().unwrap()],
    )
    .unwrap();
    drop(conn);

    harness
        .repository
        .cleanup_spool_files(&[], Some(&missing_root));

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let cleanup_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pending_spool_cleanups", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(outside_file.exists());
    assert_eq!(cleanup_count, 1);
    let _ = std::fs::remove_file(outside_file);
}

#[tokio::test]
async fn test_spool_cleanup_reference_query_failure_fails_closed() {
    let harness = ServiceHarness::new();
    std::fs::create_dir_all(harness.service.stager.root()).unwrap();
    let queued_file = harness.service.stager.root().join(format!(
        "reference-query-failure-{}.tmp",
        rand::random::<u64>()
    ));
    std::fs::write(&queued_file, b"must remain").unwrap();

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO pending_spool_cleanups (local_path, created_at)
         VALUES (?1, '2026-08-13T00:00:00Z')",
        [queued_file.to_str().unwrap()],
    )
    .unwrap();
    conn.execute("DROP TABLE message_attachments", []).unwrap();
    drop(conn);

    harness
        .repository
        .cleanup_spool_files(&[], Some(harness.service.stager.root()));

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let cleanup_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pending_spool_cleanups", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(queued_file.exists());
    assert_eq!(cleanup_count, 1);
    let _ = std::fs::remove_file(queued_file);
}

#[tokio::test]
async fn test_delete_conversation_file_outside_spool_not_deleted() {
    let harness = ServiceHarness::new();
    let outside_file =
        std::env::temp_dir().join(format!("rustsale-outside-{}.tmp", rand::random::<u64>()));
    std::fs::write(&outside_file, b"outside spool").unwrap();
    assert!(outside_file.exists());

    // Path traversal string targeting outside_file via relative segment
    let traversal_path = format!(
        "{}/../{}",
        harness.service.stager.root().display(),
        outside_file.file_name().unwrap().to_str().unwrap()
    );

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (60, 1, 'conv-60', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (600, 60, 'inbound', 'image', 'photo', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, local_path, created_at) VALUES (600, 'image', ?1, '2026-01-01T00:00:00Z')",
        [&traversal_path],
    ).unwrap();
    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(60),
        identity_id: None,
        lead_id: None,
    };
    harness.service.delete_conversation(&del_req).unwrap();

    // Traversal file outside spool root must NOT be deleted
    assert!(outside_file.exists());
    let _ = std::fs::remove_file(outside_file);
}

#[tokio::test]
async fn test_delete_conversation_shared_attachment_not_deleted() {
    let harness = ServiceHarness::new();
    std::fs::create_dir_all(harness.service.stager.root()).unwrap();
    let shared_file = harness.service.stager.root().join("shared.tmp");
    std::fs::write(&shared_file, b"shared attachment").unwrap();
    assert!(shared_file.exists());

    // Construct non-canonical path alias for conv 71 (e.g. /spool/./shared.tmp)
    let alias_path = format!("{}/./shared.tmp", harness.service.stager.root().display());

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (71, 1, 'conv-71', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, created_at, updated_at) VALUES (72, 1, 'conv-72', 'direct', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (701, 71, 'inbound', 'image', 'p1', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, local_path, created_at) VALUES (701, 'image', ?1, '2026-01-01T00:00:00Z')",
        [&alias_path],
    ).unwrap();

    conn.execute(
        "INSERT INTO messages (id, conversation_id, direction, content_type, text_content, ingested_at) VALUES (702, 72, 'inbound', 'image', 'p2', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO message_attachments (message_id, attachment_type, local_path, created_at) VALUES (702, 'image', ?1, '2026-01-01T00:00:00Z')",
        [shared_file.to_str().unwrap()],
    ).unwrap();
    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(71),
        identity_id: None,
        lead_id: None,
    };
    harness.service.delete_conversation(&del_req).unwrap();

    // Canonical comparison matches /spool/./shared.tmp with /spool/shared.tmp, so file is kept
    assert!(shared_file.exists());
    let _ = std::fs::remove_file(shared_file);
}

#[tokio::test]
async fn test_delete_group_conversation_tombstone_covers_group_snapshot() {
    let harness = ServiceHarness::new();
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, created_at, updated_at) VALUES (1, 'zalo', 'acc1', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_conversations (id, channel_account_id, external_thread_id, kind, title, created_at, updated_at) VALUES (80, 1, 'group-80', 'group', 'Test Group', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    drop(conn);

    let del_req = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(80),
        identity_id: None,
        lead_id: None,
    };
    harness.service.delete_conversation(&del_req).unwrap();

    // Historical group snapshot (occurred_at <= deletion_time) is suppressed
    let old_snapshot = crate::conversation::zalo::normalizer::NormalizedGroupSnapshot {
        provider_account_key: "acc1".to_string(),
        external_thread_id: "group-80".to_string(),
        title: "Stale Group".to_string(),
        avatar_url: None,
        occurred_at: "2026-01-01T00:00:00Z".to_string(),
        participants: vec![],
        membership_complete: true,
    };
    let res_old = harness
        .repository
        .apply_group_snapshot(&old_snapshot)
        .unwrap();
    assert_eq!(res_old, 0);

    // Background snapshots never prove new activity and must not reopen a deleted group.
    let new_snapshot = crate::conversation::zalo::normalizer::NormalizedGroupSnapshot {
        provider_account_key: "acc1".to_string(),
        external_thread_id: "group-80".to_string(),
        title: "Reopened Group".to_string(),
        avatar_url: None,
        occurred_at: "2099-01-01T00:00:00Z".to_string(),
        participants: vec![],
        membership_complete: true,
    };
    let res_new = harness
        .repository
        .apply_group_snapshot(&new_snapshot)
        .unwrap();
    assert_eq!(res_new, 0);

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let snapshot_conversation_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM channel_conversations
             WHERE channel_account_id = 1 AND external_thread_id = 'group-80' AND kind = 'group'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let snapshot_tombstone_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deleted_conversations
             WHERE channel_account_id = 1 AND external_thread_id = 'group-80' AND kind = 'group'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(snapshot_conversation_count, 0);
    assert_eq!(snapshot_tombstone_count, 1);
    drop(conn);

    // A real provider message newer than the tombstone may reopen the group.
    let mut new_group_message = group_message("acc1", "group-80", "group-new-message");
    new_group_message.provider_timestamp = Some("2099-01-01T00:00:01Z".to_string());
    new_group_message.ingested_at = "2099-01-01T00:00:02Z".to_string();
    let applied = harness
        .repository
        .apply_normalized_message(&new_group_message)
        .unwrap();
    assert!(applied.conversation_id > 0);

    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let tombstone_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deleted_conversations
             WHERE channel_account_id = 1 AND external_thread_id = 'group-80' AND kind = 'group'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(tombstone_count, 0);
}

#[tokio::test]
async fn test_merge_account_into_migrates_tombstones() {
    let harness = ServiceHarness::new();
    let conn = crate::db::open_db_connection(harness.path.to_str().unwrap()).unwrap();

    conn.execute(
        "DROP INDEX IF EXISTS idx_channel_accounts_zalo_self_ext_id",
        [],
    )
    .unwrap();

    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, self_external_id, created_at, updated_at) VALUES (1, 'zalo', 'acc1', 'user-canonical', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO channel_accounts (id, channel, provider_account_key, self_external_id, created_at, updated_at) VALUES (2, 'zalo', 'acc2', 'user-canonical', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    conn.execute(
        "INSERT INTO deleted_conversations (channel_account_id, external_thread_id, kind, deleted_at) VALUES (2, 'thread-del', 'direct', '2026-01-01T12:00:00Z')",
        [],
    ).unwrap();

    crate::conversation::migration::reconcile_duplicate_zalo_channel_accounts(&conn).unwrap();

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM deleted_conversations WHERE channel_account_id = 1 AND external_thread_id = 'thread-del'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_delete_conversation_request_validation() {
    let harness = ServiceHarness::new();

    // 1. ID <= 0
    let req1 = crate::conversation::types::DeleteConversationRequest {
        conversation_id: None,
        identity_id: Some(0),
        lead_id: None,
    };
    assert!(harness.service.delete_conversation(&req1).is_err());

    // 2. Malformed multi-target payload { conversationId: Some(0), identityId: Some(200) }
    let req_malformed = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(0),
        identity_id: Some(200),
        lead_id: None,
    };
    assert!(harness.service.delete_conversation(&req_malformed).is_err());

    // 3. Multiple valid targets
    let req2 = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(10),
        identity_id: Some(20),
        lead_id: None,
    };
    assert!(harness.service.delete_conversation(&req2).is_err());

    // 4. Valid single target
    let req3 = crate::conversation::types::DeleteConversationRequest {
        conversation_id: Some(10),
        identity_id: None,
        lead_id: None,
    };
    assert!(req3.validate().is_ok());
}

#[tokio::test]
async fn test_sync_facebook_conversations_command() {
    use super::commands::{sync_facebook_conversations, ConversationState};
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-fb-cmd"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    // With no connected Facebook accounts, sync returns 0 new messages
    let res = sync_facebook_conversations(state.clone(), None)
        .await
        .unwrap();
    assert_eq!(res, 0);

    // With non-existent page_id, returns error
    let err = sync_facebook_conversations(state, Some("page_does_not_exist".to_string())).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn test_whatsapp_commands_lifecycle() {
    use super::commands::{cancel_whatsapp_qr_login, start_whatsapp_qr_login, ConversationState};
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-wa-cmd1"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let attempt_id = "wa-attempt-unit-1".to_string();
    let start_res = start_whatsapp_qr_login(state.clone(), attempt_id.clone()).await;
    assert!(start_res.is_ok());

    let cancel_res = cancel_whatsapp_qr_login(state, attempt_id);
    assert!(cancel_res.is_ok());
}

#[tokio::test]
async fn test_get_managed_whatsapp_accounts() {
    use super::commands::{
        disconnect_whatsapp_account, get_managed_whatsapp_accounts, ConversationState,
    };
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-wa-cmd2"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    // Initial state: no accounts
    let accounts = get_managed_whatsapp_accounts(state.clone()).unwrap();
    assert_eq!(accounts.len(), 0);

    // Seed a connected WhatsApp account
    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO channel_accounts (
            channel, provider_account_key, display_name, avatar_url,
            enabled, connection_status, last_connected_at, created_at, updated_at
         ) VALUES ('whatsapp', 'wa-acc-1', 'WhatsApp Business 1', NULL, 1, 'connected', ?1, ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();
    let acc_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO whatsapp_sessions (
            channel_account_id, account_key, phone_number, session_db_filename, is_valid,
            last_connected_at, created_at, updated_at
         ) VALUES (?1, 'wa-acc-1', '+84987654321', 'session_wa-acc-1.db', 1, ?2, ?2, ?2)",
        rusqlite::params![acc_id, now],
    )
    .unwrap();

    let accounts = get_managed_whatsapp_accounts(state.clone()).unwrap();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].id, acc_id);
    assert_eq!(accounts[0].provider_account_key, "wa-acc-1");
    assert_eq!(
        accounts[0].display_name.as_deref(),
        Some("WhatsApp Business 1")
    );
    assert_eq!(accounts[0].phone_number.as_deref(), Some("+84987654321"));

    // Disconnect account
    disconnect_whatsapp_account(state.clone(), acc_id)
        .await
        .unwrap();

    let accounts_after = get_managed_whatsapp_accounts(state).unwrap();
    assert_eq!(accounts_after.len(), 0);
}

#[tokio::test]
async fn test_create_and_manage_whatsapp_campaign() {
    use super::commands::{
        cancel_whatsapp_campaign, create_whatsapp_campaign, get_whatsapp_campaign,
        get_whatsapp_campaign_recipients, list_whatsapp_campaigns, pause_whatsapp_campaign,
        populate_whatsapp_campaign_recipients, resume_whatsapp_campaign, start_whatsapp_campaign,
        ConversationState,
    };
    use crate::conversation::whatsapp::campaign::{CampaignRecipientInput, CreateCampaignRequest};
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-wa-cmd3"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    // 1. Create campaign
    let req = CreateCampaignRequest {
        name: "Q3 Promotional Campaign".to_string(),
        template_id: None,
        assigned_account_ids: vec![1, 2],
        rate_limit_min_delay_secs: Some(5),
        rate_limit_max_delay_secs: Some(15),
        burst_limit_per_batch: Some(20),
        burst_pause_secs: Some(45),
        quiet_hours_enabled: Some(true),
        quiet_hours_start: Some("22:00".to_string()),
        quiet_hours_end: Some("07:30".to_string()),
        timezone: Some("Asia/Ho_Chi_Minh".to_string()),
    };
    let campaign_id = create_whatsapp_campaign(state.clone(), req).unwrap();
    assert!(campaign_id > 0);

    // 2. Populate recipients
    let recipients = vec![
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "0912345678".to_string(),
            template: Some("Hello {name}, welcome!".to_string()),
            lead_vars: Some(std::collections::HashMap::from([(
                "name".to_string(),
                "Alice".to_string(),
            )])),
        },
        CampaignRecipientInput {
            lead_id: None,
            recipient_phone_raw: "+84987654321".to_string(),
            template: None,
            lead_vars: None,
        },
    ];
    let inserted = populate_whatsapp_campaign_recipients(
        state.clone(),
        campaign_id,
        recipients,
        Some("Default template {name}".to_string()),
    )
    .unwrap();
    assert_eq!(inserted, 2);

    // 3. Verify get_campaign (draft status)
    let camp = get_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    assert_eq!(camp.id, campaign_id);
    assert_eq!(camp.name, "Q3 Promotional Campaign");
    assert_eq!(camp.status, "draft");
    assert_eq!(camp.total_recipients, 2);
    assert_eq!(camp.assigned_account_ids, vec![1, 2]);

    // 4. Start campaign -> transitions to queued
    start_whatsapp_campaign(state.clone(), campaign_id)
        .await
        .unwrap();
    let camp_queued = get_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    assert_eq!(camp_queued.status, "queued");

    // 5. Pause campaign -> transitions to paused
    pause_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    let camp_paused = get_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    assert_eq!(camp_paused.status, "paused");

    // 6. Resume campaign -> transitions to queued
    resume_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    let camp_resumed = get_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    assert_eq!(camp_resumed.status, "queued");

    // 7. Get recipients
    let recs =
        get_whatsapp_campaign_recipients(state.clone(), campaign_id, Some(10), Some(0)).unwrap();
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].status, "pending");
    assert_eq!(recs[0].recipient_phone_e164, "84912345678");

    // 8. List campaigns
    let all_camps = list_whatsapp_campaigns(state.clone()).unwrap();
    assert!(!all_camps.is_empty());
    assert!(all_camps.iter().any(|c| c.id == campaign_id));

    // 9. Cancel campaign -> transitions to cancelled
    cancel_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    let camp_cancelled = get_whatsapp_campaign(state.clone(), campaign_id).unwrap();
    assert_eq!(camp_cancelled.status, "cancelled");
}

#[tokio::test]
async fn download_conversation_attachment_command_flow() {
    use super::commands::{download_conversation_attachment, ConversationState};
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use crate::host::mock::MockApp;

    let harness = ServiceHarness::new();
    let conversation_service = Arc::new(ConversationService::new(
        harness.repository.clone(),
        harness.events.clone(),
        harness.path.parent().unwrap().join("spool-wa-dl-cmd"),
    ));
    let zalo_supervisor = Arc::new(ZaloSessionSupervisor::new(
        harness.path.to_str().unwrap().to_string(),
        conversation_service.clone(),
        Arc::new(RealZaloClientFactory),
    ));
    let app =
        MockApp::new().manage(harness.create_conv_state(conversation_service, zalo_supervisor));
    let state = app.state::<ConversationState>();

    let conn = open_db_connection(harness.path.to_str().unwrap()).unwrap();
    let now = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO channel_accounts (
            channel, provider_account_key, display_name, avatar_url,
            enabled, connection_status, last_connected_at, created_at, updated_at
         ) VALUES ('whatsapp', 'wa-acc-dl-1', 'WhatsApp Business DL', NULL, 1, 'connected', ?1, ?1, ?1)",
        rusqlite::params![now],
    )
    .unwrap();
    let acc_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO channel_conversations (
            channel_account_id, external_thread_id, kind, title, status, created_at, updated_at
         ) VALUES (?1, '84901234567@s.whatsapp.net', 'direct', 'Peer', 'active', ?2, ?2)",
        rusqlite::params![acc_id, now],
    )
    .unwrap();
    let conv_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO messages (
            conversation_id, direction, provider_message_id, content_type, text_content,
            ingested_at, status
         ) VALUES (?1, 'inbound', 'WA_MSG_DL_001', 'image', 'Image caption', ?2, 'received')",
        rusqlite::params![conv_id, now],
    )
    .unwrap();
    let msg_id = conn.last_insert_rowid();

    // 1. Existing data URL attachment returns directly
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'photo.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/photo.jpg', 'data:image/jpeg;base64,/9j/4AAQSkZJRg==', '{\"width\":800,\"height\":600}', ?2)",
        rusqlite::params![msg_id, now],
    )
    .unwrap();
    let cached_att_id = conn.last_insert_rowid();

    let res = download_conversation_attachment(state.clone(), cached_att_id)
        .await
        .unwrap();
    assert_eq!(res.id, cached_att_id);
    assert_eq!(
        res.thumbnail_url.as_deref(),
        Some("data:image/jpeg;base64,/9j/4AAQSkZJRg==")
    );
    assert_eq!(res.width, Some(800));
    assert_eq!(res.height, Some(600));

    // 2. Encrypted attachment missing media_key returns a stable non-retryable code
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'missing_key.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/missing_key.jpg', NULL, '{}', ?2)",
        rusqlite::params![msg_id, now],
    )
    .unwrap();
    let missing_key_att_id = conn.last_insert_rowid();

    let err = download_conversation_attachment(state.clone(), missing_key_att_id)
        .await
        .unwrap_err();
    assert_eq!(err.code.as_str(), "media_metadata_unavailable");

    // 2b. Malformed key material is the same permanent metadata condition.
    let invalid_key_metadata = serde_json::json!({
        "direct_path": "/v/t62.7118-24/invalid-key.enc",
        "media_key": "not-valid-base64"
    })
    .to_string();
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'invalid_key.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/invalid_key.jpg', NULL, ?2, ?3)",
        rusqlite::params![msg_id, invalid_key_metadata, now],
    )
    .unwrap();
    let invalid_key_att_id = conn.last_insert_rowid();
    let invalid_key_error = download_conversation_attachment(state.clone(), invalid_key_att_id)
        .await
        .unwrap_err();
    assert_eq!(
        invalid_key_error.code.as_str(),
        "media_metadata_unavailable"
    );

    // 3. Encrypted attachment with valid media_key downloads and stores data URL
    let mock_wa_client =
        std::sync::Arc::new(crate::conversation::whatsapp::client::MockWhatsAppClient::new());
    state
        .whatsapp_supervisor
        .register_client(acc_id, mock_wa_client.clone())
        .await;

    use base64::Engine as _;
    let key_b64 = base64::engine::general_purpose::STANDARD.encode(vec![0x42; 32]);

    // 2c. Reject a provider-declared payload over the application limit before downloading it.
    let oversized_metadata = serde_json::json!({
        "direct_path": "/v/t62.7118-24/oversized.enc",
        "media_key": key_b64
    })
    .to_string();
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'oversized.jpg', 'image/jpeg', ?2,
                   'https://mmg.whatsapp.net/v/oversized.jpg', NULL, ?3, ?4)",
        rusqlite::params![
            msg_id,
            crate::conversation::staging::MAX_ATTACHMENT_BYTES as i64 + 1,
            oversized_metadata,
            now
        ],
    )
    .unwrap();
    let oversized_att_id = conn.last_insert_rowid();
    let oversized_error = download_conversation_attachment(state.clone(), oversized_att_id)
        .await
        .unwrap_err();
    assert_eq!(
        oversized_error.code,
        crate::conversation::error::ConversationErrorCode::FileTooLarge
    );

    let meta_json = serde_json::json!({
        "direct_path": "/v/t62.7118-24/img.enc",
        "media_key": key_b64,
        "width": 1024,
        "height": 768
    })
    .to_string();

    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'photo_enc.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/photo_enc.jpg',
                   'data:image/jpeg;base64,TE9X', ?2, ?3)",
        rusqlite::params![msg_id, meta_json, now],
    )
    .unwrap();
    let enc_att_id = conn.last_insert_rowid();

    let res_enc = download_conversation_attachment(state.clone(), enc_att_id)
        .await
        .unwrap();
    assert_eq!(res_enc.id, enc_att_id);
    assert!(res_enc
        .thumbnail_url
        .as_ref()
        .unwrap()
        .starts_with("data:image/"));
    assert_ne!(
        res_enc.thumbnail_url.as_deref(),
        Some("data:image/jpeg;base64,TE9X"),
        "an encrypted WhatsApp attachment must upgrade its low-resolution thumbnail"
    );
    assert_eq!(res_enc.width, Some(1024));
    assert_eq!(res_enc.height, Some(768));
    let resolved_metadata: String = conn
        .query_row(
            "SELECT metadata_json FROM message_attachments WHERE id = ?1",
            [enc_att_id],
            |row| row.get(0),
        )
        .unwrap();
    let resolved_metadata: serde_json::Value = serde_json::from_str(&resolved_metadata).unwrap();
    assert_eq!(resolved_metadata["preview_resolved"], true);

    // 3b. An expired CDN reference follows the WhatsApp Web media-retry flow,
    // persists the refreshed direct path, then retries the download once.
    mock_wa_client.set_download_results(vec![Err(anyhow::anyhow!(
        "media URL expired with status: 404"
    ))]);
    let expired_meta_json = serde_json::json!({
        "direct_path": "/v/t62.7118-24/expired.enc",
        "media_key": key_b64,
    })
    .to_string();
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'expired.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/expired.jpg', NULL, ?2, ?3)",
        rusqlite::params![msg_id, expired_meta_json, now],
    )
    .unwrap();
    let expired_att_id = conn.last_insert_rowid();

    let refreshed = download_conversation_attachment(state.clone(), expired_att_id)
        .await
        .unwrap();
    assert!(refreshed
        .thumbnail_url
        .as_ref()
        .unwrap()
        .starts_with("data:image/"));
    let reupload_requests = mock_wa_client.get_media_reupload_requests();
    assert_eq!(reupload_requests.len(), 1);
    assert_eq!(reupload_requests[0].provider_message_id, "WA_MSG_DL_001");
    assert_eq!(reupload_requests[0].chat_jid, "84901234567@s.whatsapp.net");
    let refreshed_metadata: String = conn
        .query_row(
            "SELECT metadata_json FROM message_attachments WHERE id = ?1",
            [expired_att_id],
            |row| row.get(0),
        )
        .unwrap();
    let refreshed_metadata: serde_json::Value = serde_json::from_str(&refreshed_metadata).unwrap();
    assert_eq!(
        refreshed_metadata["direct_path"],
        "/mock/refreshed-media.enc"
    );

    // 3c. WhatsApp confirming the media no longer exists is a permanent,
    // non-retryable condition rather than an endless provider retry loop.
    mock_wa_client.set_download_results(vec![Err(anyhow::anyhow!(
        "media URL expired with status: 410"
    ))]);
    mock_wa_client.set_next_media_reupload_result(Ok(
        crate::conversation::whatsapp::client::WhatsAppMediaReuploadResult::NotFound,
    ));
    let gone_metadata = serde_json::json!({
        "direct_path": "/v/t62.7118-24/gone.enc",
        "media_key": key_b64,
    })
    .to_string();
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'gone.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/gone.jpg', NULL, ?2, ?3)",
        rusqlite::params![msg_id, gone_metadata, now],
    )
    .unwrap();
    let gone_att_id = conn.last_insert_rowid();
    let gone_error = download_conversation_attachment(state.clone(), gone_att_id)
        .await
        .unwrap_err();
    assert_eq!(gone_error.code.as_str(), "media_metadata_unavailable");

    // 3d. Group media retry must retain the exact participant JID from the
    // original WhatsApp message key (especially a LID), not reconstruct it
    // from the normalized CRM identity.
    mock_wa_client.clear();
    mock_wa_client.set_download_results(vec![Err(anyhow::anyhow!(
        "media URL expired with status: 404"
    ))]);
    conn.execute(
        "INSERT INTO channel_conversations (
            channel_account_id, external_thread_id, kind, title, status, created_at, updated_at
         ) VALUES (?1, '120363000000000000@g.us', 'group', 'Media Group', 'active', ?2, ?2)",
        rusqlite::params![acc_id, now],
    )
    .unwrap();
    let group_conv_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO channel_identities (
            channel, channel_account_id, identity_namespace, external_id,
            normalized_address, display_name, created_at, updated_at
         ) VALUES ('whatsapp', ?1, 'phone', '84908887777', '84908887777', 'Group Sender', ?2, ?2)",
        rusqlite::params![acc_id, now],
    )
    .unwrap();
    let group_sender_id = conn.last_insert_rowid();
    let exact_participant_jid = "120363099988877766@lid";
    let group_raw_payload = serde_json::json!({
        "id": "WA_GROUP_MEDIA_001",
        "chat_jid": "120363000000000000@g.us",
        "sender_jid": exact_participant_jid,
        "attachments": [{
            "media_type": "image",
            "direct_path": "/v/t62.7118-24/group-expired.enc",
            "media_key": key_b64,
        }]
    })
    .to_string();
    conn.execute(
        "INSERT INTO messages (
            conversation_id, sender_identity_id, direction, provider_message_id,
            content_type, raw_payload_json, ingested_at, status
         ) VALUES (?1, ?2, 'inbound', 'WA_GROUP_MEDIA_001', 'image', ?3, ?4, 'received')",
        rusqlite::params![group_conv_id, group_sender_id, group_raw_payload, now],
    )
    .unwrap();
    let group_msg_id = conn.last_insert_rowid();
    let group_metadata = serde_json::json!({
        "direct_path": "/v/t62.7118-24/group-expired.enc",
        "media_key": key_b64,
    })
    .to_string();
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'group-expired.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/group-expired.jpg', NULL, ?2, ?3)",
        rusqlite::params![group_msg_id, group_metadata, now],
    )
    .unwrap();
    let group_att_id = conn.last_insert_rowid();

    download_conversation_attachment(state.clone(), group_att_id)
        .await
        .unwrap();
    let group_reupload_requests = mock_wa_client.get_media_reupload_requests();
    assert_eq!(group_reupload_requests.len(), 1);
    assert_eq!(
        group_reupload_requests[0].participant_jid.as_deref(),
        Some(exact_participant_jid)
    );

    // 4. Local attachment with valid local_path generates preview data URL and stores it
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_img_path = temp_dir.path().join("outbound_local.png");
    // Minimal 1x1 transparent PNG bytes
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&temp_img_path, &png_bytes).unwrap();
    let temp_path_str = temp_img_path.to_str().unwrap();

    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            local_path, remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'outbound_local.png', 'image/png', ?2,
                   ?3, NULL, 'data:image/jpeg;base64,TE9X', '{}', ?4)",
        rusqlite::params![msg_id, png_bytes.len() as i64, temp_path_str, now],
    )
    .unwrap();
    let local_att_id = conn.last_insert_rowid();

    let res_local = download_conversation_attachment(state.clone(), local_att_id)
        .await
        .unwrap();
    assert_eq!(res_local.id, local_att_id);
    assert!(res_local
        .thumbnail_url
        .as_ref()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert_ne!(
        res_local.thumbnail_url.as_deref(),
        Some("data:image/jpeg;base64,TE9X"),
        "the full outbound local image must replace the staging thumbnail"
    );

    // Verify thumbnail_url was saved in DB
    let saved_thumb: Option<String> = conn
        .query_row(
            "SELECT thumbnail_url FROM message_attachments WHERE id = ?1",
            rusqlite::params![local_att_id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(saved_thumb
        .as_ref()
        .unwrap()
        .starts_with("data:image/png;base64,"));

    // 4b. The bounded local reader checks the actual file size even when DB size is absent.
    let oversized_local_path = temp_dir.path().join("oversized-local.jpg");
    let oversized_local_file = std::fs::File::create(&oversized_local_path).unwrap();
    oversized_local_file
        .set_len(crate::conversation::staging::MAX_ATTACHMENT_BYTES + 1)
        .unwrap();
    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            local_path, remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'oversized-local.jpg', 'image/jpeg', NULL,
                   ?2, NULL, NULL, '{}', ?3)",
        rusqlite::params![msg_id, oversized_local_path.to_str().unwrap(), now],
    )
    .unwrap();
    let oversized_local_att_id = conn.last_insert_rowid();
    let oversized_local_error =
        download_conversation_attachment(state.clone(), oversized_local_att_id)
            .await
            .unwrap_err();
    assert_eq!(
        oversized_local_error.code,
        crate::conversation::error::ConversationErrorCode::FileTooLarge
    );

    // 5. Encrypted attachment missing media_key in metadata_json, but present in message raw_payload_json
    let key_b64_raw = base64::engine::general_purpose::STANDARD.encode(vec![0x77; 32]);
    let raw_payload_with_key = serde_json::json!({
        "id": "MSG_RAW_KEY_001",
        "attachments": [{
            "media_type": "image",
            "direct_path": "/v/t62.7118-24/raw_key.enc",
            "media_key": key_b64_raw,
        }]
    })
    .to_string();

    conn.execute(
        "INSERT INTO messages (
            conversation_id, direction, provider_message_id,
            content_type, text_content, content_json, raw_payload_json,
            provider_timestamp, ingested_at, status
         ) VALUES (?1, 'inbound', 'MSG_RAW_KEY_001', 'image', NULL, '{}', ?2, ?3, ?3, 'read')",
        rusqlite::params![conv_id, raw_payload_with_key, now],
    )
    .unwrap();
    let raw_key_msg_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO message_attachments (
            message_id, attachment_type, file_name, mime_type, size_bytes,
            remote_url, thumbnail_url, metadata_json, created_at
         ) VALUES (?1, 'image', 'photo_raw_key.jpg', 'image/jpeg', 12345,
                   'https://mmg.whatsapp.net/v/photo_raw_key.jpg', NULL, '{\"direct_path\":\"/v/t62.7118-24/raw_key.enc\"}', ?2)",
        rusqlite::params![raw_key_msg_id, now],
    )
    .unwrap();
    let raw_key_att_id = conn.last_insert_rowid();

    let res_raw_key = download_conversation_attachment(state.clone(), raw_key_att_id)
        .await
        .unwrap();
    assert_eq!(res_raw_key.id, raw_key_att_id);
    assert!(res_raw_key
        .thumbnail_url
        .as_ref()
        .unwrap()
        .starts_with("data:image/"));

    // 6. Non-existent attachment returns InvalidRequest
    let err = download_conversation_attachment(state.clone(), 99999)
        .await
        .unwrap_err();
    assert_eq!(
        err.code,
        crate::conversation::error::ConversationErrorCode::InvalidRequest
    );
}
