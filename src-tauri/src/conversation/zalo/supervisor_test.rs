use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::{Message, ThreadType, UserMessage};

use crate::conversation::events::{ConversationEventPublisher, ConversationInvalidationReason};
use crate::conversation::migration::migrate;
use crate::conversation::repository::ConversationRepository;
use crate::conversation::service::ConversationService;
use crate::conversation::zalo::client::{ZaloClient, ZaloClientFactory};
use crate::conversation::zalo::normalizer::NormalizedGroupSnapshot;
use crate::conversation::zalo::supervisor::ZaloSessionSupervisor;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedHistoryRequest {
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

    fn request_old_messages(
        &self,
        thread_type: ThreadType,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        self.history_requests
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
        self.history_requests
            .lock()
            .unwrap()
            .push(CapturedHistoryRequest {
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
            let clients = self.clients.lock().unwrap();
            let stopped_count = clients
                .get(imei)
                .map(|list| list.iter().filter(|c| c.is_stopped()).count())
                .unwrap_or(0);
            if stopped_count >= target {
                break;
            }
            if start.elapsed() > std::time::Duration::from_secs(1) {
                panic!(
                    "Timed out waiting for stop count {} (got {}) for {}",
                    target, stopped_count, imei
                );
            }
            drop(clients);
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
        let list = clients_map.entry(imei.clone()).or_insert_with(Vec::new);

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
        let service = Arc::new(ConversationService::new(repository.clone(), events.clone()));
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
