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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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

    let conv_state = ConversationState {
        service: conversation_service,
        zalo_supervisor: zalo_supervisor.clone(),
    };

    let app = mock_builder()
        .manage(conv_state)
        .build(mock_context(noop_assets()))
        .unwrap();

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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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

    let conv_state = ConversationState {
        service: conversation_service,
        zalo_supervisor,
    };

    let app = mock_builder()
        .manage(conv_state)
        .build(mock_context(noop_assets()))
        .unwrap();

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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
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
        crate::conversation::error::ConversationErrorCode::ProviderFailure
    );
}

#[tokio::test]
async fn mention_candidates_query_matching_and_all() {
    use super::commands::{list_conversation_mention_candidates, ConversationState};
    use super::types::ListConversationMentionCandidatesRequest;
    use crate::conversation::zalo::client::RealZaloClientFactory;
    use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::Manager;

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

    // Add active participants
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at) VALUES (1, 100, '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();
    conn.execute(
        "INSERT INTO conversation_participants (conversation_id, identity_id, joined_at) VALUES (1, 101, '2026-01-01T00:00:00Z')",
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
    let app = mock_builder()
        .manage(ConversationState {
            service: conversation_service,
            zalo_supervisor,
        })
        .build(mock_context(noop_assets()))
        .unwrap();
    let state = app.state::<ConversationState>();

    // Test 1: Empty query returns @all + all active participants
    let candidates = list_conversation_mention_candidates(
        state.clone(),
        ListConversationMentionCandidatesRequest {
            conversation_id: 1,
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
            query: None,
            limit: Some(2),
        },
    )
    .unwrap();
    assert_eq!(limited.len(), 2);
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
