use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{
    EventReactions, GroupMessage, Message, Reaction, TGroupMessage, TReaction, TReactionContent,
    TReactionMsg, ThreadType, UserMessage,
};

use crate::conversation::events::{ConversationEventPublisher, ConversationInvalidationReason};
use crate::conversation::migration::migrate;
use crate::conversation::repository::ConversationRepository;
use crate::conversation::service::ConversationService;
use crate::conversation::zalo::client::{ZaloClient, ZaloClientFactory};
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;
use crate::conversation::zalo::supervisor::{GroupMessageBackfillOutcome, ZaloSessionSupervisor};
use crate::models::ZaloSession;

pub fn direct_listener_event(msg_id: &str) -> ListenerEvent {
    let raw_msg = serde_json::from_value(serde_json::json!({
        "actionId": "action-1",
        "msgId": msg_id,
        "cliMsgId": format!("client-{msg_id}"),
        "msgType": "webchat",
        "uidFrom": "peer-uid",
        "idTo": "0",
        "dName": "Probe Sender",
        "ts": "1785456000000",
        "status": 1,
        "content": "hello",
        "notify": "",
        "ttl": 0,
        "userId": "",
        "uin": "",
        "propertyExt": null,
        "paramsExt": {"countUnread": 1, "containType": 0, "platformType": 1},
        "cmd": 501,
        "st": 0,
        "at": 0,
        "realMsgId": format!("real-{msg_id}"),
        "quote": null
    }))
    .unwrap();

    ListenerEvent::Message(Box::new(Message::User(UserMessage::new(
        "self-uid", raw_msg,
    ))))
}

fn direct_old_messages_event(message_ids: &[&str]) -> ListenerEvent {
    let messages = message_ids
        .iter()
        .map(|message_id| match direct_listener_event(message_id) {
            ListenerEvent::Message(message) => *message,
            _ => unreachable!(),
        })
        .collect();

    ListenerEvent::OldMessages {
        messages,
        thread_type: ThreadType::User,
    }
}

fn group_old_messages_event(message_ids: &[&str]) -> ListenerEvent {
    let messages = message_ids
        .iter()
        .map(|message_id| match direct_listener_event(message_id) {
            ListenerEvent::Message(message) => match *message {
                Message::User(message) => Message::Group(GroupMessage::new(
                    "self-uid",
                    TGroupMessage {
                        base: message.data,
                        mentions: None,
                    },
                )),
                Message::Group(_) => unreachable!(),
            },
            _ => unreachable!(),
        })
        .collect();

    ListenerEvent::OldMessages {
        messages,
        thread_type: ThreadType::Group,
    }
}

fn direct_old_reactions_event(page_ids: &[&str]) -> ListenerEvent {
    let reactions = page_ids
        .iter()
        .enumerate()
        .map(|(index, page_id)| {
            Reaction::new(
                "self-uid",
                TReaction {
                    action_id: format!("reaction-action-{index}"),
                    msg_id: format!("reaction-event-{page_id}"),
                    cli_msg_id: format!("reaction-client-{index}"),
                    msg_type: "1".to_string(),
                    uid_from: "peer-uid".to_string(),
                    id_to: "0".to_string(),
                    d_name: Some("Peer".to_string()),
                    content: TReactionContent {
                        r_msg: vec![TReactionMsg {
                            g_msg_id: format!("target-message-{page_id}"),
                            c_msg_id: format!("target-client-{index}"),
                            msg_type: 1,
                        }],
                        r_icon: EventReactions::Heart,
                        r_type: 1,
                        source: 1,
                    },
                    ts: "1785495600000".to_string(),
                    ttl: 0,
                },
                false,
            )
        })
        .collect();

    ListenerEvent::OldReactions {
        reactions,
        is_group: false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapturedHistoryRequestKind {
    Messages,
    Reactions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHistoryRequest {
    pub kind: CapturedHistoryRequestKind,
    pub thread_type: ThreadType,
    pub cursor: Option<String>,
}

pub struct HarnessClient {
    pub _imei: String,
    pub event_tx: broadcast::Sender<ListenerEvent>,
    pub history_requests: Mutex<Vec<CapturedHistoryRequest>>,
    pub stopped: Mutex<bool>,
}

impl HarnessClient {
    pub fn new(imei: String, capacity: usize) -> Self {
        let (event_tx, _) = broadcast::channel(capacity);
        Self {
            _imei: imei,
            event_tx,
            history_requests: Mutex::new(Vec::new()),
            stopped: Mutex::new(false),
        }
    }

    pub fn is_stopped(&self) -> bool {
        *self.stopped.lock().unwrap()
    }
}

#[async_trait]
impl ZaloClient for HarnessClient {
    fn subscribe(&self) -> broadcast::Receiver<ListenerEvent> {
        self.event_tx.subscribe()
    }

    fn start(&self) -> anyhow::Result<()> {
        let _ = self.event_tx.send(ListenerEvent::Connected);
        Ok(())
    }

    fn request_old_messages(
        &self,
        thread_type: ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.history_requests
            .lock()
            .unwrap()
            .push(CapturedHistoryRequest {
                kind: CapturedHistoryRequestKind::Messages,
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
        self.history_requests
            .lock()
            .unwrap()
            .push(CapturedHistoryRequest {
                kind: CapturedHistoryRequestKind::Reactions,
                thread_type,
                cursor: cursor.map(str::to_string),
            });
        Ok(())
    }

    async fn get_all_group_snapshots(&self) -> anyhow::Result<Vec<NormalizedGroupSnapshot>> {
        Ok(Vec::new())
    }

    async fn get_group_snapshot(
        &self,
        _group_id: &str,
    ) -> anyhow::Result<Option<NormalizedGroupSnapshot>> {
        Ok(None)
    }

    async fn send_text(
        &self,
        _thread_id: &str,
        _thread_type: ThreadType,
        _text: &str,
    ) -> anyhow::Result<String> {
        Ok("msg-id".to_string())
    }

    fn stop(&self) {
        *self.stopped.lock().unwrap() = true;
    }
}

pub struct FakeZaloClientFactory {
    listener_capacity: usize,
    clients: Mutex<HashMap<String, Vec<Arc<HarnessClient>>>>,
    connect_counts: Mutex<HashMap<String, usize>>,
    max_simultaneous: Mutex<HashMap<String, usize>>,
}

struct AlwaysFailZaloClientFactory;

#[async_trait]
impl ZaloClientFactory for AlwaysFailZaloClientFactory {
    async fn connect(&self, _session: ZaloSession) -> anyhow::Result<Arc<dyn ZaloClient>> {
        anyhow::bail!("forced Zalo connection failure")
    }
}

struct AuthError102ZaloClientFactory {
    attempts: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl ZaloClientFactory for AuthError102ZaloClientFactory {
    async fn connect(&self, _session: ZaloSession) -> anyhow::Result<Arc<dyn ZaloClient>> {
        self.attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!("Zalo login failed: Zalo API Error: The session key was improperly submitted or has reached its timeout. Direct the user to log in again to obtain another key (code: Some(102))")
    }
}

impl FakeZaloClientFactory {
    pub fn new(capacity: usize) -> Self {
        Self {
            listener_capacity: capacity,
            clients: Mutex::new(HashMap::new()),
            connect_counts: Mutex::new(HashMap::new()),
            max_simultaneous: Mutex::new(HashMap::new()),
        }
    }

    pub async fn emit(&self, imei: &str, event: ListenerEvent) {
        let clients = self.clients.lock().unwrap();
        if let Some(list) = clients.get(imei) {
            if let Some(client) = list.iter().rev().find(|c| !c.is_stopped()) {
                let _ = client.event_tx.send(event);
            }
        }
    }

    pub async fn emit_burst(&self, imei: &str, count: usize) {
        let clients = self.clients.lock().unwrap();
        if let Some(list) = clients.get(imei) {
            if let Some(client) = list.iter().rev().find(|c| !c.is_stopped()) {
                for i in 0..count {
                    let _ = client
                        .event_tx
                        .send(direct_listener_event(&format!("burst-{i}")));
                }
            }
        }
    }

    pub async fn wait_for_connect_count(&self, imei: &str, target: usize) {
        let start = std::time::Instant::now();
        loop {
            let count = *self.connect_counts.lock().unwrap().get(imei).unwrap_or(&0);
            if count >= target {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(1) {
                panic!(
                    "Timed out waiting for connect count {} (got {}) for {}",
                    target, count, imei
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn wait_for_stop_count(&self, imei: &str, target: usize) {
        let start = std::time::Instant::now();
        loop {
            let stopped_count = {
                let clients = self.clients.lock().unwrap();
                clients
                    .get(imei)
                    .map(|list| list.iter().filter(|c| c.is_stopped()).count())
                    .unwrap_or(0)
            };
            if stopped_count >= target {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(1) {
                panic!(
                    "Timed out waiting for stop count {} (got {}) for {}",
                    target, stopped_count, imei
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn wait_for_history_request_count(&self, imei: &str, target: usize) {
        let start = std::time::Instant::now();
        loop {
            let reqs = self.history_requests(imei).await;
            if reqs.len() >= target {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(1) {
                panic!(
                    "Timed out waiting for history request count {} (got {}) for {}",
                    target,
                    reqs.len(),
                    imei
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    pub async fn history_requests(&self, imei: &str) -> Vec<CapturedHistoryRequest> {
        let clients = self.clients.lock().unwrap();
        let mut reqs = Vec::new();
        if let Some(list) = clients.get(imei) {
            for c in list {
                reqs.extend(c.history_requests.lock().unwrap().clone());
            }
        }
        reqs
    }

    pub fn max_simultaneous_clients(&self, imei: &str) -> usize {
        *self
            .max_simultaneous
            .lock()
            .unwrap()
            .get(imei)
            .unwrap_or(&0)
    }
}

#[async_trait]
impl ZaloClientFactory for FakeZaloClientFactory {
    async fn connect(&self, session: ZaloSession) -> anyhow::Result<Arc<dyn ZaloClient>> {
        let imei = session.imei;
        let mut connect_map = self.connect_counts.lock().unwrap();
        *connect_map.entry(imei.clone()).or_insert(0) += 1;
        drop(connect_map);

        let mut clients_map = self.clients.lock().unwrap();
        let list = clients_map.entry(imei.clone()).or_default();

        let active_count = list.iter().filter(|c| !c.is_stopped()).count() + 1;

        let mut max_map = self.max_simultaneous.lock().unwrap();
        let current_max = max_map.entry(imei.clone()).or_insert(0);
        if active_count > *current_max {
            *current_max = active_count;
        }
        drop(max_map);

        let client = Arc::new(HarnessClient::new(imei.clone(), self.listener_capacity));
        list.push(client.clone());

        Ok(client)
    }
}

pub struct SupervisorHarness {
    pub _db_path: String,
    pub factory: Arc<FakeZaloClientFactory>,
    pub repository: Arc<ConversationRepository>,
    pub _service: Arc<ConversationService>,
    pub events: Arc<ConversationEventPublisher>,
    pub supervisor: Arc<ZaloSessionSupervisor>,
    _clean_path: std::path::PathBuf,
}

impl SupervisorHarness {
    pub async fn new() -> Self {
        Self::with_listener_capacity(100).await
    }

    pub async fn with_listener_capacity(capacity: usize) -> Self {
        let clean_path = std::env::temp_dir().join(format!(
            "rustsale-supervisor-test-{}.db",
            rand::random::<u64>()
        ));
        let db_path = clean_path.to_str().unwrap().to_string();

        let conn = crate::db::init_db(&db_path).unwrap();
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name)
             VALUES ('imei-a', 'UA/1.0', '[]', 'vi', 'Account A')",
            [],
        )
        .unwrap();

        let repository = Arc::new(ConversationRepository::new(db_path.clone()));
        let events = Arc::new(ConversationEventPublisher::new(100));
        let spool_root = std::path::Path::new(&db_path)
            .parent()
            .unwrap()
            .join("spool-sup");
        let service = Arc::new(ConversationService::new(
            repository.clone(),
            events.clone(),
            spool_root,
        ));
        let factory = Arc::new(FakeZaloClientFactory::new(capacity));

        let supervisor = Arc::new(ZaloSessionSupervisor::new(
            db_path.clone(),
            service.clone(),
            factory.clone(),
        ));

        Self {
            _db_path: db_path,
            factory,
            repository,
            _service: service,
            events,
            supervisor,
            _clean_path: clean_path,
        }
    }

    pub async fn wait_for_checkpoint(&self, scope: &str, expected: &str) {
        let start = std::time::Instant::now();
        loop {
            let account_id = self
                .repository
                .get_channel_account_id("imei-a")
                .unwrap()
                .unwrap();
            if self
                .repository
                .get_checkpoint(account_id, scope)
                .unwrap()
                .as_deref()
                == Some(expected)
            {
                return;
            }
            if start.elapsed() > std::time::Duration::from_secs(1) {
                panic!("Timed out waiting for checkpoint {scope}={expected}");
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }
}

impl Drop for SupervisorHarness {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self._clean_path);
    }
}

#[tokio::test]
async fn supervisor_persists_fake_direct_message_and_publishes_invalidation() {
    let harness = SupervisorHarness::new().await;
    let mut events = harness.events.subscribe();
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    harness
        .factory
        .emit("imei-a", direct_listener_event("message-1"))
        .await;

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Message);
    assert_eq!(harness.repository.message_count(), 1);
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn mapped_session_registers_and_ingests_under_stable_zalo_uid() {
    let harness = SupervisorHarness::new().await;
    let conn = crate::db::open_db_connection(&harness._db_path).unwrap();
    conn.execute_batch(
        "INSERT INTO channel_accounts
            (id, channel, provider_account_key, self_external_id, display_name, connection_status, capabilities_json, created_at, updated_at)
         VALUES
            (900, 'zalo', 'uid-stable', 'uid-stable', 'Stable Zalo', 'disconnected', '{}', '2026-08-09T00:00:00Z', '2026-08-09T00:00:00Z');
         UPDATE zalo_sessions
         SET zalo_uid = 'uid-stable', channel_account_id = 900
         WHERE imei = 'imei-a';",
    )
    .unwrap();
    drop(conn);

    let mut events = harness.events.subscribe();
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    assert!(harness
        .supervisor
        .connected_client("uid-stable")
        .await
        .is_some());

    harness
        .factory
        .emit("imei-a", direct_listener_event("message-stable"))
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
        .await
        .unwrap()
        .unwrap();

    let conn = crate::db::open_db_connection(&harness._db_path).unwrap();
    assert_eq!(
        conn.query_row(
            "SELECT COUNT(*) FROM channel_accounts WHERE channel = 'zalo'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT ca.provider_account_key
             FROM messages m
             JOIN channel_conversations cc ON cc.id = m.conversation_id
             JOIN channel_accounts ca ON ca.id = cc.channel_account_id
             WHERE m.provider_message_id = 'message-stable'",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap(),
        "uid-stable"
    );
    drop(conn);

    harness.supervisor.stop_account("uid-stable").await;
}

#[tokio::test]
async fn restart_account_fails_when_no_session_matches_account_key() {
    let harness = SupervisorHarness::new().await;
    let error = harness
        .supervisor
        .restart_account("missing-zalo-account")
        .await
        .expect_err("missing session must fail restart");
    assert!(error.to_string().contains("No Zalo session"));
}

#[tokio::test]
async fn restart_account_returns_error_when_runtime_never_becomes_ready() {
    let clean_path = std::env::temp_dir().join(format!(
        "rustsale-supervisor-failure-{}.db",
        rand::random::<u64>()
    ));
    let db_path = clean_path.to_string_lossy().to_string();
    let conn = crate::db::init_db(&db_path).unwrap();
    conn.execute(
        "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name)
         VALUES ('imei-fail', 'UA/1.0', '[]', 'vi', 'Failure Account')",
        [],
    )
    .unwrap();
    drop(conn);

    let repository = Arc::new(ConversationRepository::new(db_path.clone()));
    let events = Arc::new(ConversationEventPublisher::new(10));
    let service = Arc::new(ConversationService::new(
        repository,
        events,
        clean_path.parent().unwrap().join("spool-failure"),
    ));
    let supervisor =
        ZaloSessionSupervisor::new(db_path, service, Arc::new(AlwaysFailZaloClientFactory));

    let error = supervisor
        .restart_account("imei-fail")
        .await
        .expect_err("restart must fail when no client becomes ready");
    assert!(error
        .to_string()
        .contains("Timed out waiting for Zalo account"));

    let _ = std::fs::remove_file(&clean_path);
    let _ = std::fs::remove_file(clean_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(clean_path.with_extension("db-shm"));
}

#[tokio::test]
async fn packet_error_keeps_listener_alive_for_following_messages() {
    let harness = SupervisorHarness::new().await;
    let mut events = harness.events.subscribe();
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    harness
        .factory
        .emit(
            "imei-a",
            ListenerEvent::PacketError {
                cmd: 501,
                sub_cmd: 1,
                error: "invalid packet payload".to_string(),
            },
        )
        .await;
    harness
        .factory
        .emit(
            "imei-a",
            direct_listener_event("message-after-packet-error"),
        )
        .await;

    let event = tokio::time::timeout(std::time::Duration::from_secs(1), async {
        loop {
            let event = events.recv().await.unwrap();
            if event.reason == ConversationInvalidationReason::Message {
                return event;
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(event.reason, ConversationInvalidationReason::Message);
    assert_eq!(harness.repository.message_count(), 1);
    assert_eq!(
        *harness
            .factory
            .connect_counts
            .lock()
            .unwrap()
            .get("imei-a")
            .unwrap_or(&0),
        1
    );
    assert_eq!(
        harness
            .factory
            .clients
            .lock()
            .unwrap()
            .get("imei-a")
            .map(|clients| clients.iter().filter(|client| client.is_stopped()).count())
            .unwrap_or(0),
        0
    );

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn lagged_receiver_requests_direct_and_group_reconciliation() {
    let harness = SupervisorHarness::with_listener_capacity(1).await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness.factory.emit_burst("imei-a", 50).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 8)
        .await;

    let requests = harness.factory.history_requests("imei-a").await;
    assert!(requests
        .iter()
        .any(|request| request.thread_type == ThreadType::User));
    assert!(requests
        .iter()
        .any(|request| request.thread_type == ThreadType::Group));
    assert_eq!(harness.repository.account_status("imei-a"), "degraded");
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn reactive_history_requests_exactly_five_pages_and_persists_the_last_cursor() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    for page in 1..=5 {
        let message_id = format!("history-page-{page}");
        harness
            .factory
            .emit("imei-a", direct_old_messages_event(&[&message_id]))
            .await;

        if page < 5 {
            harness
                .factory
                .wait_for_history_request_count("imei-a", 4 + page)
                .await;
        }
    }

    harness
        .wait_for_checkpoint("direct_message_history", "history-page-5")
        .await;
    let requests = harness.factory.history_requests("imei-a").await;
    assert!((8..=9).contains(&requests.len()));
    let cursors = requests
        .iter()
        .filter_map(|request| request.cursor.as_deref())
        .collect::<Vec<_>>();
    assert_eq!(
        &cursors[..4],
        [
            "history-page-1",
            "history-page-2",
            "history-page-3",
            "history-page-4"
        ]
    );
    assert!(cursors
        .iter()
        .skip(4)
        .all(|cursor| *cursor == "history-page-5"));

    let account_id = harness
        .repository
        .get_channel_account_id("imei-a")
        .unwrap()
        .unwrap();
    assert_eq!(
        harness
            .repository
            .get_checkpoint(account_id, "direct_message_history")
            .unwrap(),
        Some("history-page-5".to_string())
    );

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn failed_history_ingest_does_not_request_the_next_page() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    let conn = crate::db::open_db_connection(&harness._db_path).unwrap();
    conn.execute(
        "CREATE TRIGGER fail_second_history_message BEFORE INSERT ON messages
         WHEN NEW.provider_message_id = 'history-page-fail'
         BEGIN SELECT RAISE(ABORT, 'history page ingest failure'); END;",
        [],
    )
    .unwrap();

    harness
        .factory
        .emit(
            "imei-a",
            direct_old_messages_event(&["history-page-ok", "history-page-fail"]),
        )
        .await;
    harness.factory.wait_for_stop_count("imei-a", 1).await;

    let requests = harness.factory.history_requests("imei-a").await;
    assert_eq!(requests.len(), 4);
    assert_eq!(harness.repository.message_count(), 1);

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn reaction_history_pauses_after_five_pages_then_resumes_from_next_cursor() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    for page in 1..=5 {
        let message_id = format!("reaction-page-{page}");
        harness
            .factory
            .emit("imei-a", direct_old_reactions_event(&[&message_id]))
            .await;

        if page < 5 {
            harness
                .factory
                .wait_for_history_request_count("imei-a", 4 + page)
                .await;
        }
    }

    harness
        .wait_for_checkpoint("direct_reaction_history", "reaction-event-reaction-page-5")
        .await;
    let requests = harness.factory.history_requests("imei-a").await;
    assert!((8..=9).contains(&requests.len()));
    assert_eq!(
        requests[..8]
            .iter()
            .filter_map(|request| request.cursor.as_deref())
            .collect::<Vec<_>>(),
        vec![
            "reaction-event-reaction-page-1",
            "reaction-event-reaction-page-2",
            "reaction-event-reaction-page-3",
            "reaction-event-reaction-page-4"
        ]
    );

    let account_id = harness
        .repository
        .get_channel_account_id("imei-a")
        .unwrap()
        .unwrap();
    assert_eq!(
        harness
            .repository
            .get_checkpoint(account_id, "direct_reaction_history")
            .unwrap(),
        Some("reaction-event-reaction-page-5".to_string())
    );

    harness
        .factory
        .wait_for_history_request_count("imei-a", 9)
        .await;
    let resumed_requests = harness.factory.history_requests("imei-a").await;
    assert_eq!(
        resumed_requests
            .last()
            .and_then(|request| request.cursor.as_deref()),
        Some("reaction-event-reaction-page-5")
    );

    harness
        .factory
        .emit("imei-a", direct_old_reactions_event(&["reaction-page-5"]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
    assert_eq!(harness.factory.history_requests("imei-a").await.len(), 9);

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn empty_reaction_history_page_completes_without_resume() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    harness
        .factory
        .emit("imei-a", direct_old_reactions_event(&[]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(350)).await;

    assert_eq!(harness.factory.history_requests("imei-a").await.len(), 4);
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn restart_account_cancels_old_task_before_connecting_again() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;

    harness.supervisor.restart_account("imei-a").await.unwrap();
    harness.factory.wait_for_stop_count("imei-a", 1).await;
    harness.factory.wait_for_connect_count("imei-a", 2).await;

    assert_eq!(harness.factory.max_simultaneous_clients("imei-a"), 1);
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn ready_client_is_registered_for_commands_and_removed_after_stop() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    assert!(harness
        .supervisor
        .connected_client("imei-a")
        .await
        .is_some());

    harness.supervisor.stop_account("imei-a").await;
    assert!(harness
        .supervisor
        .connected_client("imei-a")
        .await
        .is_none());
}

#[tokio::test]
async fn stop_account_terminates_session_task_and_removes_from_supervisor() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;

    harness.supervisor.stop_account("imei-a").await;
    harness.factory.wait_for_stop_count("imei-a", 1).await;

    // Further restarts can create clean task
    harness.supervisor.restart_account("imei-a").await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 2).await;
    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn explicit_group_history_backfill_rearms_completed_stream() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;

    harness
        .factory
        .emit("imei-a", group_old_messages_event(&[]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let supervisor = harness.supervisor.clone();
    let backfill = tokio::spawn(async move {
        supervisor
            .backfill_group_messages("imei-a", std::time::Duration::from_secs(1))
            .await
    });

    harness
        .factory
        .wait_for_history_request_count("imei-a", 5)
        .await;
    harness
        .factory
        .emit("imei-a", group_old_messages_event(&[]))
        .await;

    let report = backfill.await.unwrap();
    assert_eq!(report.outcome, GroupMessageBackfillOutcome::Completed);
    assert!(!report.joined_existing_run);
    assert_eq!(report.pages_requested, 1);
    assert_eq!(report.pages_received, 1);
    assert_eq!(report.stop_reason.as_deref(), Some("empty_page"));

    let requests = harness.factory.history_requests("imei-a").await;
    assert_eq!(
        requests
            .iter()
            .filter(|request| {
                request.kind == CapturedHistoryRequestKind::Messages
                    && request.thread_type == ThreadType::Group
            })
            .count(),
        2
    );

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn concurrent_group_history_backfills_join_one_supervisor_run() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;
    harness
        .factory
        .emit("imei-a", group_old_messages_event(&[]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let first_supervisor = harness.supervisor.clone();
    let first = tokio::spawn(async move {
        first_supervisor
            .backfill_group_messages("imei-a", std::time::Duration::from_secs(1))
            .await
    });
    harness
        .factory
        .wait_for_history_request_count("imei-a", 5)
        .await;

    let second_supervisor = harness.supervisor.clone();
    let second = tokio::spawn(async move {
        second_supervisor
            .backfill_group_messages("imei-a", std::time::Duration::from_secs(1))
            .await
    });
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert_eq!(harness.factory.history_requests("imei-a").await.len(), 5);

    harness
        .factory
        .emit("imei-a", group_old_messages_event(&[]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let first_report = first.await.unwrap();
    let second_report = second.await.unwrap();
    assert_eq!(first_report.outcome, GroupMessageBackfillOutcome::Completed);
    assert_eq!(
        second_report.outcome,
        GroupMessageBackfillOutcome::Completed
    );
    assert_ne!(
        first_report.joined_existing_run,
        second_report.joined_existing_run
    );

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn group_history_backfill_timeout_is_bounded_and_keeps_run_alive() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    harness
        .factory
        .wait_for_history_request_count("imei-a", 4)
        .await;
    harness
        .factory
        .emit("imei-a", group_old_messages_event(&[]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;

    let report = harness
        .supervisor
        .backfill_group_messages("imei-a", std::time::Duration::from_millis(50))
        .await;
    assert_eq!(report.outcome, GroupMessageBackfillOutcome::TimedOut);
    assert_eq!(report.pages_requested, 1);

    harness
        .factory
        .emit("imei-a", group_old_messages_event(&[]))
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    assert!(harness
        .supervisor
        .connected_client("imei-a")
        .await
        .is_some());

    harness.supervisor.stop_account("imei-a").await;
}

#[tokio::test]
async fn group_history_backfill_reports_unavailable_without_running_session() {
    let harness = SupervisorHarness::new().await;

    let report = harness
        .supervisor
        .backfill_group_messages("imei-a", std::time::Duration::from_millis(50))
        .await;

    assert_eq!(report.outcome, GroupMessageBackfillOutcome::Unavailable);
    assert!(report.warning.is_some());
}

#[tokio::test]
async fn test_code_102_error_transitions_to_expired_and_stops_retries() {
    let clean_path = std::env::temp_dir().join(format!(
        "rustsale-supervisor-102-{}.db",
        rand::random::<u64>()
    ));
    let db_path = clean_path.to_string_lossy().to_string();
    let conn = crate::db::init_db(&db_path).unwrap();
    conn.execute(
        "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name, auth_status)
         VALUES ('imei-102', 'UA/1.0', '[]', 'vi', '102 Expired Account', 'valid')",
        [],
    )
    .unwrap();
    drop(conn);

    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let factory = Arc::new(AuthError102ZaloClientFactory {
        attempts: attempts.clone(),
    });

    let repository = Arc::new(ConversationRepository::new(db_path.clone()));
    let events = Arc::new(ConversationEventPublisher::new(10));
    let service = Arc::new(ConversationService::new(
        repository,
        events,
        clean_path.parent().unwrap().join("spool-102"),
    ));
    let supervisor = ZaloSessionSupervisor::new(db_path.clone(), service, factory);

    supervisor.start_all().await.unwrap();

    // Poll until the supervisor detects code 102 and persists the expired status
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let conn = crate::db::open_db_connection(&db_path).unwrap();
        let status: String = conn
            .query_row(
                "SELECT auth_status FROM zalo_sessions WHERE imei = 'imei-102'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);
        if status == "expired" || std::time::Instant::now() > deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let conn = crate::db::open_db_connection(&db_path).unwrap();
    let (auth_status, auth_error): (String, Option<String>) = conn
        .query_row(
            "SELECT auth_status, auth_error FROM zalo_sessions WHERE imei = 'imei-102'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(auth_status, "expired");
    assert_eq!(auth_error.as_deref(), Some("auth_expired_102"));
    // Verify that retries stopped immediately and did not retry in a loop
    assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);

    supervisor.stop_all().await;

    let _ = std::fs::remove_file(&clean_path);
    let _ = std::fs::remove_file(clean_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(clean_path.with_extension("db-shm"));
}

struct TransientFailClientFactory {
    attempts: Arc<std::sync::atomic::AtomicUsize>,
}

#[async_trait]
impl ZaloClientFactory for TransientFailClientFactory {
    async fn connect(&self, _session: ZaloSession) -> anyhow::Result<Arc<dyn ZaloClient>> {
        self.attempts
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        anyhow::bail!("transient network timeout")
    }
}

#[tokio::test]
async fn test_transient_error_does_not_expire_and_continues_retries() {
    let clean_path = std::env::temp_dir().join(format!(
        "rustsale-supervisor-transient-{}.db",
        rand::random::<u64>()
    ));
    let db_path = clean_path.to_string_lossy().to_string();
    let conn = crate::db::init_db(&db_path).unwrap();
    conn.execute(
        "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name, auth_status)
         VALUES ('imei-transient', 'UA/1.0', '[]', 'vi', 'Transient Account', 'valid')",
        [],
    )
    .unwrap();
    drop(conn);

    let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let factory = Arc::new(TransientFailClientFactory {
        attempts: attempts.clone(),
    });

    let repository = Arc::new(ConversationRepository::new(db_path.clone()));
    let events = Arc::new(ConversationEventPublisher::new(10));
    let service = Arc::new(ConversationService::new(
        repository,
        events,
        clean_path.parent().unwrap().join("spool-transient"),
    ));
    let supervisor = ZaloSessionSupervisor::new(db_path.clone(), service, factory);

    supervisor.start_all().await.unwrap();

    // Poll until at least 2 attempts are made (confirming retry loop is active)
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while attempts.load(std::sync::atomic::Ordering::SeqCst) < 2
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        attempts.load(std::sync::atomic::Ordering::SeqCst) >= 2,
        "Transient errors must be retried by the supervisor loop"
    );

    let conn = crate::db::open_db_connection(&db_path).unwrap();
    let auth_status: String = conn
        .query_row(
            "SELECT auth_status FROM zalo_sessions WHERE imei = 'imei-transient'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_ne!(
        auth_status, "expired",
        "Transient error must not mark auth_status as expired"
    );

    supervisor.stop_all().await;

    let _ = std::fs::remove_file(&clean_path);
    let _ = std::fs::remove_file(clean_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(clean_path.with_extension("db-shm"));
}

#[tokio::test]
async fn test_steady_state_error_102_event_transitions_to_expired() {
    let clean_path = std::env::temp_dir().join(format!(
        "rustsale-supervisor-steady102-{}.db",
        rand::random::<u64>()
    ));
    let db_path = clean_path.to_string_lossy().to_string();
    let conn = crate::db::init_db(&db_path).unwrap();
    conn.execute(
        "INSERT INTO zalo_sessions (imei, user_agent, cookies_json, language, display_name, auth_status)
         VALUES ('imei-steady-102', 'UA/1.0', '[]', 'vi', 'Steady 102 Account', 'valid')",
        [],
    )
    .unwrap();
    drop(conn);

    let factory = Arc::new(FakeZaloClientFactory::new(10));
    let repository = Arc::new(ConversationRepository::new(db_path.clone()));
    let events = Arc::new(ConversationEventPublisher::new(10));
    let service = Arc::new(ConversationService::new(
        repository,
        events,
        clean_path.parent().unwrap().join("spool-steady-102"),
    ));
    let supervisor = ZaloSessionSupervisor::new(db_path.clone(), service, factory.clone());

    supervisor.start_all().await.unwrap();

    // Ensure client is connected and ready
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while supervisor
        .connected_client("imei-steady-102")
        .await
        .is_none()
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(supervisor
        .connected_client("imei-steady-102")
        .await
        .is_some());

    // Emit steady-state 102 error event
    factory.emit(
        "imei-steady-102",
        ListenerEvent::Error("Zalo API Error: The session key was improperly submitted or has reached its timeout (code: Some(102))".to_string()),
    ).await;

    // Poll until auth_status becomes expired
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let conn = crate::db::open_db_connection(&db_path).unwrap();
        let status: String = conn
            .query_row(
                "SELECT auth_status FROM zalo_sessions WHERE imei = 'imei-steady-102'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        drop(conn);
        if status == "expired" || std::time::Instant::now() > deadline {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    let conn = crate::db::open_db_connection(&db_path).unwrap();
    let (auth_status, auth_error): (String, Option<String>) = conn
        .query_row(
            "SELECT auth_status, auth_error FROM zalo_sessions WHERE imei = 'imei-steady-102'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();

    assert_eq!(auth_status, "expired");
    assert_eq!(auth_error.as_deref(), Some("auth_expired_102"));

    supervisor.stop_all().await;

    let _ = std::fs::remove_file(&clean_path);
    let _ = std::fs::remove_file(clean_path.with_extension("db-wal"));
    let _ = std::fs::remove_file(clean_path.with_extension("db-shm"));
}

#[test]
fn test_is_terminal_auth_error_matches_valid_patterns_and_rejects_false_positives() {
    use super::supervisor::is_terminal_auth_error;

    // Valid positive cases
    assert!(is_terminal_auth_error(
        "Zalo API Error: The session key was improperly submitted or has reached its timeout. Direct the user to log in again to obtain another key (code: Some(102))"
    ));
    assert!(is_terminal_auth_error(
        "{\"error_code\": 102, \"message\": \"Session timeout\"}"
    ));
    assert!(is_terminal_auth_error(
        "{\"error_code\":102,\"message\":\"Expired\"}"
    ));
    assert!(is_terminal_auth_error("Zalo error_code: 102"));
    assert!(is_terminal_auth_error("Error (code: 102)"));
    assert!(is_terminal_auth_error(
        "The session key was improperly submitted"
    ));
    assert!(is_terminal_auth_error(
        "The user session has reached its timeout"
    ));
    assert!(is_terminal_auth_error(
        "Please direct the user to log in again for this session"
    ));

    // False positives that MUST NOT be classified as terminal auth error
    assert!(!is_terminal_auth_error("Zalo error code: 1020"));
    assert!(!is_terminal_auth_error("HTTP error code: 1024"));
    assert!(!is_terminal_auth_error("gateway response code: 10299"));
    assert!(!is_terminal_auth_error(
        "network connection has reached its timeout"
    ));
    assert!(!is_terminal_auth_error(
        "http request has reached its timeout"
    ));
    assert!(!is_terminal_auth_error("transient proxy timeout (504)"));
    assert!(!is_terminal_auth_error(
        "Zalo network error: connection reset"
    ));
}

#[tokio::test]
async fn test_sync_group_chat_history_behavior() {
    let harness = SupervisorHarness::new().await;
    harness.supervisor.start_all().await.unwrap();
    harness.factory.wait_for_connect_count("imei-a", 1).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let conv_id = {
        let conn = crate::db::open_db_connection(&harness._db_path).unwrap();
        let account_id: i64 = conn.query_row(
            "SELECT id FROM channel_accounts WHERE channel = 'zalo' AND provider_account_key = 'imei-a'",
            [],
            |r| r.get(0),
        ).unwrap();

        conn.execute(
            "INSERT INTO channel_conversations (channel_account_id, external_thread_id, kind, title, created_at, updated_at)
             VALUES (?1, 'group_sync_123', 'group', 'Test Sync Group', '2026-08-27T00:00:00Z', '2026-08-27T00:00:00Z')",
            rusqlite::params![account_id],
        ).unwrap();

        conn.query_row(
            "SELECT id FROM channel_conversations WHERE external_thread_id = 'group_sync_123'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };

    // Calling sync_group_chat_history should succeed
    let result = harness.supervisor.sync_group_chat_history(conv_id).await;
    assert!(result.is_ok(), "Expected Ok but got: {:?}", result);

    // Calling on non-existent group conversation returns error
    let invalid_result = harness.supervisor.sync_group_chat_history(99999).await;
    assert!(invalid_result.is_err());

    harness.supervisor.stop_all().await;
}
