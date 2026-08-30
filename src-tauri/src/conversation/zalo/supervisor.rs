use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::sync::CancellationToken;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::ThreadType;

use crate::conversation::service::ConversationService;
use crate::conversation::types::{AccountConnectionStatus, ConversationRuntimeStatus};
use crate::conversation::zalo::client::{ZaloClient, ZaloClientFactory};
use crate::conversation::zalo::{
    DIRECT_MESSAGE_HISTORY_SCOPE, DIRECT_MESSAGE_WATERMARK_SCOPE, DIRECT_REACTION_HISTORY_SCOPE,
    DIRECT_REACTION_WATERMARK_SCOPE, GROUP_MESSAGE_HISTORY_SCOPE, GROUP_MESSAGE_WATERMARK_SCOPE,
    GROUP_REACTION_HISTORY_SCOPE, GROUP_REACTION_WATERMARK_SCOPE,
};
use crate::db::open_db_connection;
use crate::models::ZaloSession;

const MAX_HISTORY_PAGES: usize = 5;
#[cfg(not(test))]
const HISTORY_RESUME_INTERVAL: std::time::Duration = std::time::Duration::from_secs(2);
#[cfg(test)]
const HISTORY_RESUME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);
#[cfg(not(test))]
const ACCOUNT_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);
#[cfg(test)]
const ACCOUNT_READY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);
type ConnectedClients =
    Arc<Mutex<HashMap<String, Arc<dyn crate::conversation::zalo::client::ZaloClient>>>>;

const GROUP_MESSAGE_BACKFILL_SCOPE: &str = "account_group_messages";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum GroupMessageBackfillOutcome {
    Completed,
    TimedOut,
    #[default]
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct GroupMessageBackfillReport {
    pub provider_account_key: String,
    pub scope: String,
    pub outcome: GroupMessageBackfillOutcome,
    pub joined_existing_run: bool,
    pub pages_requested: usize,
    pub pages_received: usize,
    pub start_cursor: Option<String>,
    pub durable_cursor: Option<String>,
    pub stop_reason: Option<String>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum HistoryStream {
    DirectMessages,
    DirectReactions,
    GroupMessages,
    GroupReactions,
}

impl HistoryStream {
    const ALL: [Self; 4] = [
        Self::DirectMessages,
        Self::DirectReactions,
        Self::GroupMessages,
        Self::GroupReactions,
    ];

    fn checkpoint_scope(self) -> &'static str {
        match self {
            Self::DirectMessages => DIRECT_MESSAGE_HISTORY_SCOPE,
            Self::DirectReactions => DIRECT_REACTION_HISTORY_SCOPE,
            Self::GroupMessages => GROUP_MESSAGE_HISTORY_SCOPE,
            Self::GroupReactions => GROUP_REACTION_HISTORY_SCOPE,
        }
    }

    fn thread_type(self) -> ThreadType {
        match self {
            Self::DirectMessages | Self::DirectReactions => ThreadType::User,
            Self::GroupMessages | Self::GroupReactions => ThreadType::Group,
        }
    }

    fn request(
        self,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
        cursor: Option<&str>,
    ) -> anyhow::Result<()> {
        match self {
            Self::DirectMessages | Self::GroupMessages => {
                client.request_old_messages(self.thread_type(), cursor)
            }
            Self::DirectReactions | Self::GroupReactions => {
                client.request_old_reactions(self.thread_type(), cursor)
            }
        }
    }

    fn initial_cursor(
        self,
        service: &ConversationService,
        account_id: i64,
    ) -> anyhow::Result<Option<String>> {
        if let Some(cursor) = service
            .repository
            .get_checkpoint(account_id, self.checkpoint_scope())?
        {
            return Ok(Some(cursor));
        }

        let watermark_scope = match self {
            Self::DirectMessages => DIRECT_MESSAGE_WATERMARK_SCOPE,
            Self::DirectReactions => DIRECT_REACTION_WATERMARK_SCOPE,
            Self::GroupMessages => GROUP_MESSAGE_WATERMARK_SCOPE,
            Self::GroupReactions => GROUP_REACTION_WATERMARK_SCOPE,
        };
        Ok(service
            .repository
            .get_checkpoint(account_id, watermark_scope)?)
    }
}

#[derive(Debug, Default)]
struct HistoryStreamState {
    pages_requested: usize,
    total_pages_requested: usize,
    pages_received: usize,
    last_requested_cursor: Option<String>,
    paused_cursor: Option<String>,
    completed: bool,
    stop_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct HistoryStreamSnapshot {
    pages_requested: usize,
    pages_received: usize,
    start_cursor: Option<String>,
    stop_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HistoryAdvance {
    Requested,
    Paused,
    Completed,
}

#[derive(Debug, Default)]
struct HistoryPaginationState {
    streams: HashMap<HistoryStream, HistoryStreamState>,
}

impl HistoryPaginationState {
    fn start(
        client: &dyn crate::conversation::zalo::client::ZaloClient,
        service: &ConversationService,
        account_id: Option<i64>,
    ) -> anyhow::Result<Self> {
        let Some(account_id) = account_id else {
            return Ok(Self::default());
        };

        let mut pagination = Self::default();
        for stream in HistoryStream::ALL {
            let cursor = stream.initial_cursor(service, account_id)?;
            stream.request(client, cursor.as_deref())?;
            pagination.streams.insert(
                stream,
                HistoryStreamState {
                    pages_requested: 1,
                    total_pages_requested: 1,
                    pages_received: 0,
                    last_requested_cursor: cursor,
                    paused_cursor: None,
                    completed: false,
                    stop_reason: None,
                },
            );
        }
        Ok(pagination)
    }

    fn request_next(
        &mut self,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
        stream: HistoryStream,
        cursor: &str,
    ) -> anyhow::Result<HistoryAdvance> {
        let state = self.streams.entry(stream).or_default();
        let cursor = cursor.trim();
        if state.completed {
            return Ok(HistoryAdvance::Completed);
        }
        if cursor.is_empty() {
            state.completed = true;
            state.paused_cursor = None;
            state.stop_reason = Some("empty_cursor".to_string());
            return Ok(HistoryAdvance::Completed);
        }
        if state.last_requested_cursor.as_deref() == Some(cursor) {
            state.completed = true;
            state.paused_cursor = None;
            state.stop_reason = Some("repeated_cursor".to_string());
            return Ok(HistoryAdvance::Completed);
        }
        if state.pages_requested >= MAX_HISTORY_PAGES {
            state.paused_cursor = Some(cursor.to_string());
            return Ok(HistoryAdvance::Paused);
        }

        stream.request(client, Some(cursor))?;
        state.pages_requested += 1;
        state.total_pages_requested += 1;
        state.last_requested_cursor = Some(cursor.to_string());
        state.paused_cursor = None;
        Ok(HistoryAdvance::Requested)
    }

    fn complete(&mut self, stream: HistoryStream, reason: &str) {
        let state = self.streams.entry(stream).or_default();
        state.completed = true;
        state.paused_cursor = None;
        state.stop_reason = Some(reason.to_string());
    }

    fn note_page_received(&mut self, stream: HistoryStream) {
        self.streams.entry(stream).or_default().pages_received += 1;
    }

    fn is_active(&self, stream: HistoryStream) -> bool {
        self.streams
            .get(&stream)
            .is_some_and(|state| !state.completed)
    }

    fn snapshot(&self, stream: HistoryStream) -> HistoryStreamSnapshot {
        let state = self.streams.get(&stream);
        HistoryStreamSnapshot {
            pages_requested: state.map_or(0, |value| value.total_pages_requested),
            pages_received: state.map_or(0, |value| value.pages_received),
            start_cursor: state.and_then(|value| value.last_requested_cursor.clone()),
            stop_reason: state.and_then(|value| value.stop_reason.clone()),
        }
    }

    fn rearm(
        &mut self,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
        service: &ConversationService,
        account_id: i64,
        stream: HistoryStream,
    ) -> anyhow::Result<HistoryStreamSnapshot> {
        let cursor = stream.initial_cursor(service, account_id)?;
        stream.request(client, cursor.as_deref())?;
        self.streams.insert(
            stream,
            HistoryStreamState {
                pages_requested: 1,
                total_pages_requested: 1,
                pages_received: 0,
                last_requested_cursor: cursor.clone(),
                paused_cursor: None,
                completed: false,
                stop_reason: None,
            },
        );
        Ok(HistoryStreamSnapshot {
            pages_requested: 1,
            pages_received: 0,
            start_cursor: cursor,
            stop_reason: None,
        })
    }

    fn resume_paused(
        &mut self,
        client: &dyn crate::conversation::zalo::client::ZaloClient,
    ) -> anyhow::Result<usize> {
        let mut resumed = 0;
        for stream in HistoryStream::ALL {
            let paused_cursor = self.streams.get(&stream).and_then(|state| {
                (!state.completed)
                    .then(|| state.paused_cursor.clone())
                    .flatten()
            });
            let Some(cursor) = paused_cursor else {
                continue;
            };

            stream.request(client, Some(&cursor))?;
            let state = self.streams.entry(stream).or_default();
            state.pages_requested = 1;
            state.total_pages_requested += 1;
            state.last_requested_cursor = Some(cursor);
            state.paused_cursor = None;
            resumed += 1;
        }
        Ok(resumed)
    }
}

struct HistoryBatch {
    stream: HistoryStream,
    cursor: Option<String>,
}

fn history_batch(event: &ListenerEvent) -> Option<HistoryBatch> {
    match event {
        ListenerEvent::OldMessages {
            messages,
            thread_type,
        } => {
            let stream = match thread_type {
                ThreadType::User => HistoryStream::DirectMessages,
                ThreadType::Group => HistoryStream::GroupMessages,
            };
            let cursor = messages.last().and_then(|message| match message {
                zca_rs::models::Message::User(message) => {
                    (!message.data.msg_id.is_empty()).then(|| message.data.msg_id.clone())
                }
                zca_rs::models::Message::Group(message) => {
                    (!message.data.base.msg_id.is_empty()).then(|| message.data.base.msg_id.clone())
                }
            });
            Some(HistoryBatch { stream, cursor })
        }
        ListenerEvent::OldReactions {
            reactions,
            is_group,
        } => {
            let stream = if *is_group {
                HistoryStream::GroupReactions
            } else {
                HistoryStream::DirectReactions
            };
            let cursor = reactions.last().and_then(|reaction| {
                (!reaction.data.msg_id.is_empty()).then(|| reaction.data.msg_id.clone())
            });
            Some(HistoryBatch { stream, cursor })
        }
        _ => None,
    }
}

pub(crate) fn is_terminal_auth_error(error_msg: &str) -> bool {
    let lower = error_msg.to_ascii_lowercase();

    // Check for explicit Zalo error code 102 with non-digit boundary
    let has_code_102 = lower.contains("code: some(102)")
        || lower.contains("\"error_code\":102")
        || lower.contains("\"error_code\": 102")
        || lower.contains("error_code: 102")
        || lower.match_indices("code: 102").any(|(idx, s)| {
            let after = &lower[idx + s.len()..];
            after.is_empty() || !after.chars().next().unwrap_or(' ').is_ascii_digit()
        });

    if has_code_102 {
        return true;
    }

    // Specific Zalo API error phrasing for session expiration
    if lower.contains("session key was improperly submitted") {
        return true;
    }

    // Generic phrases must co-occur with session key / auth context
    if lower.contains("session")
        && (lower.contains("has reached its timeout")
            || lower.contains("direct the user to log in again"))
    {
        return true;
    }

    false
}

fn mark_account_auth_expired(
    service: &ConversationService,
    provider_account_key: &str,
    imei: &str,
    error_msg: &str,
) {
    tracing::warn!(
        provider_account_key,
        imei,
        error = %error_msg,
        "Zalo authentication session expired or invalidated by another login"
    );
    match open_db_connection(service.repository.db_path()) {
        Ok(conn) => {
            if let Err(e) = crate::db::update_zalo_session_auth_status(
                &conn,
                imei,
                "expired",
                Some("auth_expired_102"),
            ) {
                tracing::error!(imei, error = %e, "Failed to persist expired auth status");
            }
        }
        Err(e) => {
            tracing::error!(imei, error = %e, "Failed to open DB to persist expired auth status");
        }
    }
    if let Err(e) = service.repository.set_account_status(
        provider_account_key,
        AccountConnectionStatus::Disconnected,
        Some(error_msg),
    ) {
        tracing::error!(provider_account_key, error = %e, "Failed to set account status to Disconnected");
    }
}

async fn handle_terminal_auth_error(
    service: &ConversationService,
    connected_clients: &ConnectedClients,
    client: Option<&Arc<dyn ZaloClient>>,
    profile_sync_handle: &mut Option<tokio::task::JoinHandle<()>>,
    provider_account_key: &str,
    imei: &str,
    error_msg: &str,
) {
    mark_account_auth_expired(service, provider_account_key, imei, error_msg);
    if let Some(h) = profile_sync_handle.take() {
        h.abort();
    }
    if let Some(client) = client {
        unregister_connected_client(connected_clients, provider_account_key, client).await;
        client.stop();
    }
}

fn mark_account_degraded(
    service: &ConversationService,
    provider_account_key: &str,
    error: &dyn std::fmt::Display,
) {
    tracing::warn!(provider_account_key, error = %error, "Zalo conversation session degraded");
    let _ = service.repository.set_account_status(
        provider_account_key,
        AccountConnectionStatus::Degraded,
        Some(&error.to_string()),
    );
}

struct SessionTask {
    cancellation: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
    control_tx: mpsc::Sender<SupervisorControl>,
}

struct GroupMessageBackfillCompletion {
    outcome: GroupMessageBackfillOutcome,
    pages_requested: usize,
    pages_received: usize,
    durable_cursor: Option<String>,
    stop_reason: Option<String>,
    warning: Option<String>,
}

struct GroupMessageBackfillRegistration {
    joined_existing_run: bool,
    account_id: i64,
    initial: HistoryStreamSnapshot,
    completion_rx: oneshot::Receiver<GroupMessageBackfillCompletion>,
}

enum SupervisorControl {
    BackfillGroupMessages {
        registration_tx: oneshot::Sender<Result<GroupMessageBackfillRegistration, String>>,
    },
}

struct ActiveGroupMessageBackfill {
    account_id: i64,
    waiters: Vec<oneshot::Sender<GroupMessageBackfillCompletion>>,
}

pub struct ZaloSessionSupervisor {
    db_path: String,
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
    tasks: Mutex<HashMap<String, SessionTask>>,
    clients: ConnectedClients,
    sticker_cache: std::sync::Mutex<crate::conversation::zalo::sticker_cache::ZaloStickerCache>,
    outbox_notify: Arc<tokio::sync::Notify>,
}

impl ZaloSessionSupervisor {
    pub fn new(
        db_path: String,
        service: Arc<ConversationService>,
        factory: Arc<dyn ZaloClientFactory>,
    ) -> Self {
        Self {
            db_path,
            service,
            factory,
            tasks: Mutex::new(HashMap::new()),
            clients: Arc::new(Mutex::new(HashMap::new())),
            sticker_cache: std::sync::Mutex::new(
                crate::conversation::zalo::sticker_cache::ZaloStickerCache::new(),
            ),
            outbox_notify: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Notifies the supervisor to immediately process outbox jobs for connected Zalo accounts.
    pub fn notify_outbox(&self) {
        self.outbox_notify.notify_waiters();
        self.outbox_notify.notify_one();
    }

    pub async fn connected_client(
        &self,
        provider_account_key: &str,
    ) -> Option<Arc<dyn crate::conversation::zalo::client::ZaloClient>> {
        let clients = self.clients.lock().await;
        clients.get(provider_account_key).cloned()
    }

    pub async fn search_stickers(
        &self,
        provider_account_key: &str,
        query: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<crate::conversation::zalo::client::ZaloStickerReference>> {
        let key = crate::conversation::zalo::sticker_cache::CacheKey {
            provider_account_key: provider_account_key.to_string(),
            query: query.to_lowercase(),
            limit,
        };
        let now = std::time::Instant::now();
        {
            let mut cache = self.sticker_cache.lock().unwrap();
            if let Some(cached) = cache.get(&key, now) {
                return Ok(cached);
            }
        }
        let client = self
            .connected_client(provider_account_key)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("Zalo account {provider_account_key} is not connected")
            })?;
        let stickers = client
            .search_stickers(query, limit)
            .await
            .map_err(|e| anyhow::anyhow!(e.message))?;
        {
            let mut cache = self.sticker_cache.lock().unwrap();
            cache.insert(key, stickers.clone(), now);
        }
        Ok(stickers)
    }

    pub async fn get_sticker_detail(
        &self,
        provider_account_key: &str,
        sticker_id: u32,
    ) -> anyhow::Result<Option<crate::conversation::zalo::client::ZaloStickerReference>> {
        let client = self
            .connected_client(provider_account_key)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("Zalo account {provider_account_key} is not connected")
            })?;
        client
            .get_sticker_detail(sticker_id)
            .await
            .map_err(|e| anyhow::anyhow!(e.message))
    }

    pub async fn start_all(&self) -> anyhow::Result<()> {
        let _ = self.service.recover_outbox_startup();
        let conn = open_db_connection(&self.db_path)?;
        let sessions = crate::db::get_all_zalo_sessions(&conn)
            .map_err(|e| anyhow::anyhow!("Failed to fetch Zalo sessions: {e}"))?;

        for session in sessions {
            self.start_account_task(session).await;
        }

        Ok(())
    }

    async fn start_account_task(&self, session: ZaloSession) {
        let imei = session.imei.clone();
        let provider_account_key = session.provider_account_key().to_string();
        let mut tasks = self.tasks.lock().await;

        if tasks.contains_key(&provider_account_key) || tasks.contains_key(&imei) {
            return;
        }

        let cancellation = CancellationToken::new();
        let task_cancel = cancellation.clone();
        let service = self.service.clone();
        let factory = self.factory.clone();
        let clients = self.clients.clone();
        let outbox_notify = self.outbox_notify.clone();
        let (control_tx, control_rx) = mpsc::channel(16);

        let handle = tokio::spawn(async move {
            run_session_loop(
                service,
                factory,
                clients,
                session,
                task_cancel,
                control_rx,
                outbox_notify,
            )
            .await;
        });

        tasks.insert(
            provider_account_key,
            SessionTask {
                cancellation,
                handle,
                control_tx,
            },
        );
    }

    pub async fn restart_account(&self, provider_account_key: &str) -> anyhow::Result<()> {
        let conn = open_db_connection(&self.db_path)?;
        let target_imei: Option<String> = conn
            .query_row(
                "SELECT zs.imei FROM zalo_sessions zs
                 LEFT JOIN channel_accounts ca ON ca.id = zs.channel_account_id OR (ca.self_external_id IS NOT NULL AND ca.self_external_id = zs.zalo_uid)
                 WHERE zs.imei = ?1 OR zs.zalo_uid = ?1 OR ca.provider_account_key = ?1 OR ca.self_external_id = ?1
                 ORDER BY zs.rowid DESC LIMIT 1",
                rusqlite::params![provider_account_key],
                |row| row.get(0),
            )
            .optional()?;
        let target_imei = target_imei.ok_or_else(|| {
            anyhow::anyhow!("No Zalo session matches account key {provider_account_key}")
        })?;
        let session = crate::db::get_zalo_session(&conn, &target_imei)?
            .ok_or_else(|| anyhow::anyhow!("No Zalo session found for IMEI {target_imei}"))?;
        let runtime_key = session.provider_account_key().to_string();

        let old_tasks = {
            let mut tasks = self.tasks.lock().await;
            let mut removed = Vec::new();
            for key in [
                provider_account_key,
                target_imei.as_str(),
                runtime_key.as_str(),
            ] {
                if let Some(task) = tasks.remove(key) {
                    removed.push(task);
                }
            }
            removed
        };

        for task in old_tasks {
            task.cancellation.cancel();
            let _ = task.handle.await;
        }
        {
            let mut clients = self.clients.lock().await;
            clients.remove(provider_account_key);
            clients.remove(&target_imei);
            clients.remove(&runtime_key);
        }

        let cancellation = CancellationToken::new();
        let task_cancel = cancellation.clone();
        let service = self.service.clone();
        let factory = self.factory.clone();
        let clients = self.clients.clone();
        let outbox_notify = self.outbox_notify.clone();
        let (control_tx, control_rx) = mpsc::channel(16);

        let handle = tokio::spawn(async move {
            run_session_loop(
                service,
                factory,
                clients,
                session,
                task_cancel,
                control_rx,
                outbox_notify,
            )
            .await;
        });

        self.tasks.lock().await.insert(
            runtime_key.clone(),
            SessionTask {
                cancellation,
                handle,
                control_tx,
            },
        );

        let ready = tokio::time::timeout(ACCOUNT_READY_TIMEOUT, async {
            loop {
                if self.connected_client(&runtime_key).await.is_some() {
                    return Ok::<(), anyhow::Error>(());
                }
                let task_finished = self
                    .tasks
                    .lock()
                    .await
                    .get(&runtime_key)
                    .is_none_or(|task| task.handle.is_finished());
                if task_finished {
                    anyhow::bail!("Zalo session task stopped before becoming ready");
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await;

        match ready {
            Ok(result) => result?,
            Err(_) => {
                self.stop_account(&runtime_key).await;
                anyhow::bail!("Timed out waiting for Zalo account {runtime_key} to become ready");
            }
        }

        Ok(())
    }

    pub async fn stop_account(&self, provider_account_key: &str) {
        let mut aliases = vec![provider_account_key.to_string()];
        if let Ok(conn) = open_db_connection(&self.db_path) {
            if let Ok(Some((imei, zalo_uid))) = conn
                .query_row(
                    "SELECT imei, zalo_uid FROM zalo_sessions
                     WHERE imei = ?1 OR zalo_uid = ?1
                        OR channel_account_id IN (
                            SELECT id FROM channel_accounts
                            WHERE channel = 'zalo' AND (provider_account_key = ?1 OR self_external_id = ?1)
                        )
                     ORDER BY rowid DESC LIMIT 1",
                    [provider_account_key],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
                )
                .optional()
            {
                aliases.push(imei);
                if let Some(zalo_uid) = zalo_uid {
                    aliases.push(zalo_uid);
                }
            }
        }

        let old_tasks = {
            let mut tasks = self.tasks.lock().await;
            aliases
                .iter()
                .filter_map(|key| tasks.remove(key))
                .collect::<Vec<_>>()
        };

        for task in old_tasks {
            task.cancellation.cancel();
            let _ = task.handle.await;
        }
        let mut clients = self.clients.lock().await;
        for key in aliases {
            clients.remove(&key);
        }
    }

    pub async fn stop_all(&self) {
        let tasks_map = {
            let mut tasks = self.tasks.lock().await;
            std::mem::take(&mut *tasks)
        };

        for (_, task) in tasks_map {
            task.cancellation.cancel();
            let _ = task.handle.await;
        }
        self.clients.lock().await.clear();
    }

    pub async fn runtime_status(&self) -> anyhow::Result<ConversationRuntimeStatus> {
        self.service.runtime_status()
    }

    pub async fn backfill_group_messages(
        &self,
        provider_account_key: &str,
        timeout: std::time::Duration,
    ) -> GroupMessageBackfillReport {
        let unavailable = |warning: String| GroupMessageBackfillReport {
            provider_account_key: provider_account_key.to_string(),
            scope: GROUP_MESSAGE_BACKFILL_SCOPE.to_string(),
            outcome: GroupMessageBackfillOutcome::Unavailable,
            joined_existing_run: false,
            pages_requested: 0,
            pages_received: 0,
            start_cursor: None,
            durable_cursor: None,
            stop_reason: Some("session_unavailable".to_string()),
            warning: Some(warning),
        };

        if self.connected_client(provider_account_key).await.is_none() {
            return unavailable(format!(
                "Zalo account {provider_account_key} is not connected"
            ));
        }

        let control_tx = {
            let tasks = self.tasks.lock().await;
            tasks
                .get(provider_account_key)
                .map(|task| task.control_tx.clone())
        };
        let Some(control_tx) = control_tx else {
            return unavailable(format!(
                "Zalo account {provider_account_key} has no running supervisor task"
            ));
        };

        let started_at = tokio::time::Instant::now();
        let (registration_tx, registration_rx) = oneshot::channel();
        if control_tx
            .send(SupervisorControl::BackfillGroupMessages { registration_tx })
            .await
            .is_err()
        {
            return unavailable(format!(
                "Zalo account {provider_account_key} supervisor control channel is closed"
            ));
        }

        let registration = match tokio::time::timeout(timeout, registration_rx).await {
            Ok(Ok(Ok(registration))) => registration,
            Ok(Ok(Err(error))) => {
                return GroupMessageBackfillReport {
                    provider_account_key: provider_account_key.to_string(),
                    scope: GROUP_MESSAGE_BACKFILL_SCOPE.to_string(),
                    outcome: GroupMessageBackfillOutcome::Failed,
                    joined_existing_run: false,
                    pages_requested: 0,
                    pages_received: 0,
                    start_cursor: None,
                    durable_cursor: None,
                    stop_reason: Some("start_failed".to_string()),
                    warning: Some(error),
                }
            }
            Ok(Err(_)) => {
                return unavailable(format!(
                    "Zalo account {provider_account_key} supervisor stopped before accepting backfill"
                ));
            }
            Err(_) => {
                return GroupMessageBackfillReport {
                    provider_account_key: provider_account_key.to_string(),
                    scope: GROUP_MESSAGE_BACKFILL_SCOPE.to_string(),
                    outcome: GroupMessageBackfillOutcome::TimedOut,
                    joined_existing_run: false,
                    pages_requested: 0,
                    pages_received: 0,
                    start_cursor: None,
                    durable_cursor: None,
                    stop_reason: Some("accept_timeout".to_string()),
                    warning: Some("Timed out waiting for supervisor acceptance".to_string()),
                }
            }
        };

        let elapsed = started_at.elapsed();
        let remaining = timeout.saturating_sub(elapsed);
        let durable_cursor = || {
            self.service
                .repository
                .get_checkpoint(registration.account_id, GROUP_MESSAGE_HISTORY_SCOPE)
                .ok()
                .flatten()
        };

        match tokio::time::timeout(remaining, registration.completion_rx).await {
            Ok(Ok(completion)) => GroupMessageBackfillReport {
                provider_account_key: provider_account_key.to_string(),
                scope: GROUP_MESSAGE_BACKFILL_SCOPE.to_string(),
                outcome: completion.outcome,
                joined_existing_run: registration.joined_existing_run,
                pages_requested: completion.pages_requested,
                pages_received: completion.pages_received,
                start_cursor: registration.initial.start_cursor,
                durable_cursor: completion.durable_cursor,
                stop_reason: completion.stop_reason,
                warning: completion.warning,
            },
            Ok(Err(_)) => GroupMessageBackfillReport {
                provider_account_key: provider_account_key.to_string(),
                scope: GROUP_MESSAGE_BACKFILL_SCOPE.to_string(),
                outcome: GroupMessageBackfillOutcome::Failed,
                joined_existing_run: registration.joined_existing_run,
                pages_requested: registration.initial.pages_requested,
                pages_received: registration.initial.pages_received,
                start_cursor: registration.initial.start_cursor,
                durable_cursor: durable_cursor(),
                stop_reason: Some("session_stopped".to_string()),
                warning: Some("Supervisor stopped before history backfill completed".to_string()),
            },
            Err(_) => GroupMessageBackfillReport {
                provider_account_key: provider_account_key.to_string(),
                scope: GROUP_MESSAGE_BACKFILL_SCOPE.to_string(),
                outcome: GroupMessageBackfillOutcome::TimedOut,
                joined_existing_run: registration.joined_existing_run,
                pages_requested: registration.initial.pages_requested,
                pages_received: registration.initial.pages_received,
                start_cursor: registration.initial.start_cursor,
                durable_cursor: durable_cursor(),
                stop_reason: Some("timeout".to_string()),
                warning: Some(
                    "Timed out waiting for account-wide Zalo group history backfill".to_string(),
                ),
            },
        }
    }

    /// Synchronizes recent chat history for a specific Zalo group from the Zalo server API.
    pub async fn sync_group_chat_history(&self, conversation_id: i64) -> anyhow::Result<usize> {
        let (_channel_account_id, external_thread_id, provider_account_key, account_display_name) = {
            let conn = open_db_connection(&self.db_path)?;
            let mut stmt = conn.prepare(
                "SELECT cc.channel_account_id, cc.external_thread_id, ca.provider_account_key, ca.display_name
                 FROM channel_conversations cc
                 JOIN channel_accounts ca ON ca.id = cc.channel_account_id
                 WHERE cc.id = ?1 AND cc.kind = 'group' AND ca.channel = 'zalo'",
            )?;
            stmt.query_row(rusqlite::params![conversation_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
        };

        let client = self
            .connected_client(&provider_account_key)
            .await
            .ok_or_else(|| {
                anyhow::anyhow!("Zalo account {provider_account_key} is not connected")
            })?;
        let mut ingested_count = 0;
        let now_str = crate::conversation::types::utc_now();
        let mut current_cursor: Option<String> = None;
        let mut page_count = 0;

        while page_count < 3 {
            page_count += 1;
            let history_resp = match client
                .fetch_group_chat_history_with_cursor(
                    &external_thread_id,
                    Some(50),
                    current_cursor.as_deref(),
                )
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    if page_count == 1 {
                        return Err(anyhow::anyhow!(
                            "Failed to fetch group chat history from Zalo: {e}"
                        ));
                    } else {
                        tracing::warn!(target: "zalo_supervisor", "Pagination stopped at page {page_count}: {e}");
                        break;
                    }
                }
            };

            let has_more = history_resp.more > 0 && !history_resp.last_action_id.is_empty();
            let next_cursor = history_resp.last_action_id.clone();
            let msgs_count = history_resp.group_msgs.len();

            for mut group_msg in history_resp.group_msgs {
                if group_msg.thread_id.is_empty() || group_msg.thread_id == "0" {
                    group_msg.thread_id = external_thread_id.clone();
                }
                let model_msg = zca_rs::models::Message::Group(group_msg);
                match crate::conversation::zalo::normalizer::normalize_message(
                    &provider_account_key,
                    account_display_name.as_deref(),
                    model_msg,
                ) {
                    Ok(normalized_msg) => {
                        if let Ok(applied) = self
                            .service
                            .repository
                            .apply_normalized_message(&normalized_msg)
                        {
                            if !applied.duplicate {
                                ingested_count += 1;
                            }
                        }
                    }
                    Err(err) => {
                        tracing::warn!(target: "zalo_supervisor", "Failed to normalize group history message: {err}");
                    }
                }
            }

            if !has_more
                || msgs_count == 0
                || next_cursor == current_cursor.clone().unwrap_or_default()
            {
                break;
            }
            current_cursor = Some(next_cursor);
        }

        if let Ok(conn) = open_db_connection(&self.db_path) {
            let _ = conn.execute(
                "UPDATE channel_conversations SET last_synced_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![now_str, conversation_id],
            );
        }

        self.service
            .events
            .publish(crate::conversation::events::ConversationInvalidatedEvent {
                conversation_id,
                lead_id: None,
                channel: crate::conversation::types::Channel::Zalo,
                reason: crate::conversation::events::ConversationInvalidationReason::Message,
                message_id: None,
            });

        Ok(ingested_count)
    }

    pub(crate) async fn sync_active_group_histories(
        service: &ConversationService,
        client: &dyn ZaloClient,
        account_id: i64,
        provider_account_key: &str,
        account_display_name: Option<&str>,
        db_path: &str,
        max_groups: usize,
    ) -> anyhow::Result<usize> {
        let group_convs: Vec<(i64, String)> = {
            let conn = open_db_connection(db_path)?;
            let mut stmt = conn.prepare(
                "SELECT id, external_thread_id
                 FROM channel_conversations
                 WHERE channel_account_id = ?1 AND kind = 'group'
                 ORDER BY COALESCE(last_message_at, updated_at) DESC
                 LIMIT ?2",
            )?;
            let rows = stmt
                .query_map(rusqlite::params![account_id, max_groups as i64], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };

        let mut total_ingested = 0;
        let now_str = crate::conversation::types::utc_now();

        for (conv_id, group_id) in group_convs {
            let history_resp = match client
                .fetch_group_chat_history_with_cursor(&group_id, Some(50), None)
                .await
            {
                Ok(resp) => resp,
                Err(err) => {
                    tracing::warn!(target: "zalo_supervisor", group_id = %group_id, "Failed to fetch active group history: {err}");
                    continue;
                }
            };

            let mut group_ingested = 0;
            for mut group_msg in history_resp.group_msgs {
                if group_msg.thread_id.is_empty() || group_msg.thread_id == "0" {
                    group_msg.thread_id = group_id.clone();
                }
                let model_msg = zca_rs::models::Message::Group(group_msg);
                if let Ok(normalized_msg) = crate::conversation::zalo::normalizer::normalize_message(
                    provider_account_key,
                    account_display_name,
                    model_msg,
                ) {
                    if let Ok(applied) =
                        service.repository.apply_normalized_message(&normalized_msg)
                    {
                        if !applied.duplicate {
                            group_ingested += 1;
                        }
                    }
                }
            }

            if group_ingested > 0 {
                total_ingested += group_ingested;
                if let Ok(conn) = open_db_connection(db_path) {
                    let _ = conn.execute(
                        "UPDATE channel_conversations SET last_synced_at = ?1, updated_at = ?1 WHERE id = ?2",
                        rusqlite::params![now_str, conv_id],
                    );
                }
                service
                    .events
                    .publish(crate::conversation::events::ConversationInvalidatedEvent {
                        conversation_id: conv_id,
                        lead_id: None,
                        channel: crate::conversation::types::Channel::Zalo,
                        reason:
                            crate::conversation::events::ConversationInvalidationReason::Message,
                        message_id: None,
                    });
            }
        }

        Ok(total_ingested)
    }
}

async fn run_session_loop(
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
    connected_clients: ConnectedClients,
    session: ZaloSession,
    cancellation: CancellationToken,
    mut control_rx: mpsc::Receiver<SupervisorControl>,
    outbox_notify: Arc<tokio::sync::Notify>,
) {
    let imei = session.imei.clone();
    let mut provider_account_key = session.provider_account_key().to_string();
    let mut backoff_secs = 1u64;

    loop {
        if cancellation.is_cancelled() {
            return;
        }

        // 1. Mark account connecting
        let mut account_id = service
            .repository
            .set_account_status(
                &provider_account_key,
                AccountConnectionStatus::Connecting,
                None,
            )
            .ok();

        // 2. Connect through factory
        let client = match factory.connect(session.clone()).await {
            Ok(client) => client,
            Err(e) => {
                let err_msg = e.to_string();
                if is_terminal_auth_error(&err_msg) {
                    handle_terminal_auth_error(
                        service.as_ref(),
                        &connected_clients,
                        None,
                        &mut None,
                        &provider_account_key,
                        &imei,
                        &err_msg,
                    )
                    .await;
                    return;
                }
                service
                    .repository
                    .set_account_status(
                        &provider_account_key,
                        AccountConnectionStatus::Disconnected,
                        Some(&err_msg),
                    )
                    .ok();

                tokio::select! {
                    _ = cancellation.cancelled() => return,
                    _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
                }
                backoff_secs = (backoff_secs * 2).min(30);
                continue;
            }
        };

        backoff_secs = 1;

        // 3. Subscribe immediately
        let mut rx = client.subscribe();

        if let Err(error) = client.start() {
            let err_msg = error.to_string();
            if is_terminal_auth_error(&err_msg) {
                handle_terminal_auth_error(
                    service.as_ref(),
                    &connected_clients,
                    Some(&client),
                    &mut None,
                    &provider_account_key,
                    &imei,
                    &err_msg,
                )
                .await;
                return;
            }
            mark_account_degraded(service.as_ref(), &provider_account_key, &error);
            client.stop();
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
            }
            backoff_secs = (backoff_secs * 2).min(30);
            continue;
        }

        let ready_result = tokio::select! {
            _ = cancellation.cancelled() => {
                client.stop();
                return;
            }
            result = tokio::time::timeout(std::time::Duration::from_secs(20), async {
                loop {
                    match rx.recv().await {
                        Ok(ListenerEvent::Connected) => return Ok(()),
                        Ok(ListenerEvent::Error(error)) => anyhow::bail!("Zalo listener failed before ready: {error}"),
                        Ok(ListenerEvent::Disconnected { code, reason })
                        | Ok(ListenerEvent::Closed { code, reason }) => {
                            anyhow::bail!("Zalo listener closed before ready ({code}): {reason}")
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            anyhow::bail!("Zalo listener lagged by {skipped} events before ready")
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            anyhow::bail!("Zalo listener event stream closed before ready")
                        }
                    }
                }
            }) => match result {
                Ok(result) => result,
                Err(_) => Err(anyhow::anyhow!("Timed out waiting for Zalo listener readiness")),
            }
        };

        if let Err(error) = ready_result {
            let err_msg = error.to_string();
            if is_terminal_auth_error(&err_msg) {
                handle_terminal_auth_error(
                    service.as_ref(),
                    &connected_clients,
                    Some(&client),
                    &mut None,
                    &provider_account_key,
                    &imei,
                    &err_msg,
                )
                .await;
                return;
            }
            mark_account_degraded(service.as_ref(), &provider_account_key, &error);
            client.stop();
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
            }
            backoff_secs = (backoff_secs * 2).min(30);
            continue;
        }

        if let Some(own_external_id) = client
            .own_external_id()
            .filter(|value| !value.trim().is_empty())
        {
            match service.repository.reconcile_zalo_channel_account(
                &imei,
                Some(&own_external_id),
                session.display_name.as_deref(),
                None,
            ) {
                Ok(reconciled_account_id) => {
                    provider_account_key = own_external_id;
                    account_id = Some(reconciled_account_id);
                }
                Err(error) => {
                    mark_account_degraded(service.as_ref(), &provider_account_key, &error);
                    client.stop();
                    tokio::select! {
                        _ = cancellation.cancelled() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
                    }
                    backoff_secs = (backoff_secs * 2).min(30);
                    continue;
                }
            }
        }

        connected_clients
            .lock()
            .await
            .insert(provider_account_key.clone(), client.clone());

        // 4. Mark account connected after the protocol cipher is ready
        account_id = service
            .repository
            .set_account_status(
                &provider_account_key,
                AccountConnectionStatus::Connected,
                None,
            )
            .ok()
            .or(account_id);

        if let Some(acc_id) = account_id {
            let cap_json = serde_json::to_string(
                &crate::conversation::types::ConversationCapabilities::zalo_phase_one(),
            )
            .unwrap_or_default();
            let _ = service.repository.update_account_identity_and_capabilities(
                acc_id,
                client.own_external_id().as_deref(),
                &cap_json,
                &crate::conversation::types::utc_now(),
            );
        }

        // 5. Fetch all group snapshots and apply them before history reconciliation
        if let Ok(snapshots) = client.get_all_group_snapshots().await {
            for snapshot in snapshots {
                let _ = service.apply_group_snapshot(&snapshot);
            }
        }

        // 5.5 Initial sync of recent group chat messages for active groups
        if let Some(acc_id) = account_id {
            let initial_service = service.clone();
            let initial_client = client.clone();
            let initial_key = provider_account_key.clone();
            let initial_name = session.display_name.clone();
            let initial_db = service.repository.db_path().to_string();
            tokio::spawn(async move {
                let _ = ZaloSessionSupervisor::sync_active_group_histories(
                    initial_service.as_ref(),
                    initial_client.as_ref(),
                    acc_id,
                    &initial_key,
                    initial_name.as_deref(),
                    &initial_db,
                    20,
                )
                .await;
            });
        }

        // 6. Request initial history and reactions from stored checkpoints
        let mut pagination =
            match send_initial_history_requests(client.as_ref(), service.as_ref(), account_id) {
                Ok(pagination) => pagination,
                Err(error) => {
                    unregister_connected_client(&connected_clients, &provider_account_key, &client)
                        .await;
                    mark_account_degraded(service.as_ref(), &provider_account_key, &error);
                    client.stop();
                    tokio::select! {
                        _ = cancellation.cancelled() => return,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
                    }
                    backoff_secs = (backoff_secs * 2).min(30);
                    continue;
                }
            };

        let mut profile_sync_handle: Option<tokio::task::JoinHandle<()>> = None;
        let mut active_group_backfill: Option<ActiveGroupMessageBackfill> = None;

        // 6.5 Run initial profile enrichment sync for unresolved identities
        if let Some(acc_id) = account_id {
            let is_idle = profile_sync_handle.as_ref().is_none_or(|h| h.is_finished());
            if is_idle {
                let service = service.clone();
                let client = client.clone();
                profile_sync_handle = Some(tokio::spawn(async move {
                    if let Err(err) = service
                        .sync_identity_profiles_for_account(acc_id, client.as_ref())
                        .await
                    {
                        eprintln!("[ZaloSupervisor] Initial profile sync error for account {acc_id}: {err}");
                    }
                }));
            }
        }

        // 7. Drain receiver loop with reactive pagination state machine
        let mut outbox_ticker = tokio::time::interval(std::time::Duration::from_millis(200));
        let mut profile_ticker = tokio::time::interval(std::time::Duration::from_secs(60));
        let mut group_sync_ticker = tokio::time::interval_at(
            tokio::time::Instant::now() + std::time::Duration::from_secs(180),
            std::time::Duration::from_secs(180),
        );
        group_sync_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut history_ticker = tokio::time::interval_at(
            tokio::time::Instant::now() + HISTORY_RESUME_INTERVAL,
            HISTORY_RESUME_INTERVAL,
        );
        history_ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        let should_reconnect = loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    if let Some(h) = profile_sync_handle.take() {
                        h.abort();
                    }
                    unregister_connected_client(
                        &connected_clients,
                        &provider_account_key,
                        &client,
                    )
                    .await;
                    client.stop();
                    return;
                }
                _ = outbox_notify.notified() => {
                    if let Some(acc_id) = account_id {
                        while let Ok(true) = service.process_next_outbox_job(acc_id, client.as_ref()).await {
                            if cancellation.is_cancelled() {
                                if let Some(h) = profile_sync_handle.take() {
                                    h.abort();
                                }
                                unregister_connected_client(
                                    &connected_clients,
                                    &provider_account_key,
                                    &client,
                                )
                                .await;
                                client.stop();
                                return;
                            }
                        }
                    }
                }
                _ = outbox_ticker.tick() => {
                    if let Some(acc_id) = account_id {
                        while let Ok(true) = service.process_next_outbox_job(acc_id, client.as_ref()).await {
                            if cancellation.is_cancelled() {
                                if let Some(h) = profile_sync_handle.take() {
                                    h.abort();
                                }
                                unregister_connected_client(
                                    &connected_clients,
                                    &provider_account_key,
                                    &client,
                                )
                                .await;
                                client.stop();
                                return;
                            }
                        }
                    }
                }
                _ = profile_ticker.tick() => {
                    if let Some(acc_id) = account_id {
                        let is_idle = profile_sync_handle.as_ref().is_none_or(|h| h.is_finished());
                        if is_idle {
                            let service = service.clone();
                            let client = client.clone();
                            profile_sync_handle = Some(tokio::spawn(async move {
                                if let Err(err) = service.sync_identity_profiles_for_account(acc_id, client.as_ref()).await {
                                    eprintln!("[ZaloSupervisor] Profile ticker sync error for account {acc_id}: {err}");
                                }
                            }));
                        }
                    }
                }
                _ = group_sync_ticker.tick() => {
                    if let Some(acc_id) = account_id {
                        let service = service.clone();
                        let client = client.clone();
                        let key = provider_account_key.clone();
                        let name = session.display_name.clone();
                        let db = service.repository.db_path().to_string();
                        tokio::spawn(async move {
                            let _ = ZaloSessionSupervisor::sync_active_group_histories(
                                service.as_ref(),
                                client.as_ref(),
                                acc_id,
                                &key,
                                name.as_deref(),
                                &db,
                                20,
                            )
                            .await;
                        });
                    }
                }
                _ = history_ticker.tick() => {
                    if let Err(error) = pagination.resume_paused(client.as_ref()) {
                        finish_group_message_backfill(
                            &mut active_group_backfill,
                            &pagination,
                            service.as_ref(),
                            GroupMessageBackfillOutcome::Failed,
                            Some("request_failed".to_string()),
                            Some(error.to_string()),
                        );
                        mark_account_degraded(
                            service.as_ref(),
                            &provider_account_key,
                            &error,
                        );
                        client.stop();
                        break true;
                    }
                }
                control = control_rx.recv() => {
                    let Some(control) = control else {
                        continue;
                    };
                    match control {
                        SupervisorControl::BackfillGroupMessages { registration_tx } => {
                            let Some(account_id) = account_id else {
                                let _ = registration_tx.send(Err(
                                    "Missing channel account ID during history backfill".to_string(),
                                ));
                                continue;
                            };

                            let (completion_tx, completion_rx) = oneshot::channel();
                            let (joined_existing_run, initial) = if let Some(active) =
                                active_group_backfill.as_mut()
                            {
                                active.waiters.push(completion_tx);
                                (true, pagination.snapshot(HistoryStream::GroupMessages))
                            } else if pagination.is_active(HistoryStream::GroupMessages) {
                                let initial = pagination.snapshot(HistoryStream::GroupMessages);
                                active_group_backfill = Some(ActiveGroupMessageBackfill {
                                    account_id,
                                    waiters: vec![completion_tx],
                                });
                                (true, initial)
                            } else {
                                match pagination.rearm(
                                    client.as_ref(),
                                    service.as_ref(),
                                    account_id,
                                    HistoryStream::GroupMessages,
                                ) {
                                    Ok(initial) => {
                                        active_group_backfill = Some(ActiveGroupMessageBackfill {
                                            account_id,
                                            waiters: vec![completion_tx],
                                        });
                                        (false, initial)
                                    }
                                    Err(error) => {
                                        let _ = registration_tx.send(Err(error.to_string()));
                                        continue;
                                    }
                                }
                            };

                            let _ = registration_tx.send(Ok(GroupMessageBackfillRegistration {
                                joined_existing_run,
                                account_id,
                                initial,
                                completion_rx,
                            }));
                        }
                    }
                }
                res = rx.recv() => {
                    match res {
                        Ok(event) => {
                            // 8. For GroupEvent, fetch and apply referenced group snapshot
                            if let zca_rs::listener::ListenerEvent::GroupEvent(ref group_ev) = event {
                                let group_id = group_ev.thread_id.clone();
                                if let Ok(Some(snapshot)) = client.get_group_snapshot(&group_id).await {
                                    let _ = service.apply_group_snapshot(&snapshot);
                                }
                            }

                            let is_disconnecting = matches!(
                                event,
                                zca_rs::listener::ListenerEvent::Disconnected { .. }
                                    | zca_rs::listener::ListenerEvent::Closed { .. }
                                    | zca_rs::listener::ListenerEvent::Error(_)
                            );
                            let event_err_str = match &event {
                                zca_rs::listener::ListenerEvent::Error(err) => err.clone(),
                                zca_rs::listener::ListenerEvent::Closed { reason, .. }
                                | zca_rs::listener::ListenerEvent::Disconnected { reason, .. } => reason.clone(),
                                _ => String::new(),
                            };

                            let history_batch = history_batch(&event);

                            // 10. Call service.ingest_zalo_event before publishing UI events
                            let display_name = session.display_name.as_deref();
                            if let Err(error) = service.ingest_zalo_event(
                                &provider_account_key,
                                display_name,
                                event,
                            ) {
                                if history_batch.as_ref().is_some_and(|batch| {
                                    batch.stream == HistoryStream::GroupMessages
                                }) {
                                    finish_group_message_backfill(
                                        &mut active_group_backfill,
                                        &pagination,
                                        service.as_ref(),
                                        GroupMessageBackfillOutcome::Failed,
                                        Some("ingest_failed".to_string()),
                                        Some(error.to_string()),
                                    );
                                }
                                let err_msg = error.to_string();
                                if is_terminal_auth_error(&err_msg) {
                                    handle_terminal_auth_error(
                                        service.as_ref(),
                                        &connected_clients,
                                        Some(&client),
                                        &mut profile_sync_handle,
                                        &provider_account_key,
                                        &imei,
                                        &err_msg,
                                    )
                                    .await;
                                    return;
                                }
                                mark_account_degraded(
                                    service.as_ref(),
                                    &provider_account_key,
                                    &error,
                                );
                                client.stop();
                                break true;
                            }

                            // 11. Trigger next page request if pagination state machine derived next_cursor
                            if let Some(batch) = history_batch {
                                pagination.note_page_received(batch.stream);
                                if let Some(cursor) = batch.cursor {
                                    let Some(account_id) = account_id else {
                                        let error = anyhow::anyhow!("Missing channel account ID during history backfill");
                                        mark_account_degraded(
                                            service.as_ref(),
                                            &provider_account_key,
                                            &error,
                                        );
                                        client.stop();
                                        break true;
                                    };
                                    if let Err(error) = service.repository.update_history_cursor(
                                        account_id,
                                        batch.stream.checkpoint_scope(),
                                        &cursor,
                                    ) {
                                        mark_account_degraded(
                                            service.as_ref(),
                                            &provider_account_key,
                                            &error,
                                        );
                                        client.stop();
                                        break true;
                                    }
                                    let advance = match pagination.request_next(
                                        client.as_ref(),
                                        batch.stream,
                                        &cursor,
                                    ) {
                                        Ok(advance) => advance,
                                        Err(error) => {
                                            if batch.stream == HistoryStream::GroupMessages {
                                                finish_group_message_backfill(
                                                    &mut active_group_backfill,
                                                    &pagination,
                                                    service.as_ref(),
                                                    GroupMessageBackfillOutcome::Failed,
                                                    Some("request_failed".to_string()),
                                                    Some(error.to_string()),
                                                );
                                            }
                                            mark_account_degraded(
                                                service.as_ref(),
                                                &provider_account_key,
                                                &error,
                                            );
                                            client.stop();
                                            break true;
                                        }
                                    };
                                    if batch.stream == HistoryStream::GroupMessages
                                        && advance == HistoryAdvance::Completed
                                    {
                                        finish_group_message_backfill(
                                            &mut active_group_backfill,
                                            &pagination,
                                            service.as_ref(),
                                            GroupMessageBackfillOutcome::Completed,
                                            None,
                                            None,
                                        );
                                    }
                                } else {
                                    pagination.complete(batch.stream, "empty_page");
                                    if batch.stream == HistoryStream::GroupMessages {
                                        finish_group_message_backfill(
                                            &mut active_group_backfill,
                                            &pagination,
                                            service.as_ref(),
                                            GroupMessageBackfillOutcome::Completed,
                                            None,
                                            None,
                                        );
                                    }
                                }
                            }

                            if is_disconnecting {
                                finish_group_message_backfill(
                                    &mut active_group_backfill,
                                    &pagination,
                                    service.as_ref(),
                                    GroupMessageBackfillOutcome::Failed,
                                    Some("session_disconnected".to_string()),
                                    Some("Zalo session disconnected during history backfill".to_string()),
                                );
                                if is_terminal_auth_error(&event_err_str) {
                                    handle_terminal_auth_error(
                                        service.as_ref(),
                                        &connected_clients,
                                        Some(&client),
                                        &mut profile_sync_handle,
                                        &provider_account_key,
                                        &imei,
                                        &event_err_str,
                                    )
                                    .await;
                                    return;
                                }
                                client.stop();
                                break true;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            finish_group_message_backfill(
                                &mut active_group_backfill,
                                &pagination,
                                service.as_ref(),
                                GroupMessageBackfillOutcome::Failed,
                                Some("receiver_lagged".to_string()),
                                Some("Zalo listener receiver lagged during history backfill".to_string()),
                            );
                            // 12. On Lagged, request direct/group reconciliation and mark account degraded
                            let _ = service.repository.set_account_status(
                                &provider_account_key,
                                AccountConnectionStatus::Degraded,
                                Some("Receiver lagged"),
                            );

                            match send_initial_history_requests(
                                client.as_ref(),
                                service.as_ref(),
                                account_id,
                            ) {
                                Ok(next_pagination) => pagination = next_pagination,
                                Err(error) => {
                                    mark_account_degraded(
                                        service.as_ref(),
                                        &provider_account_key,
                                        &error,
                                    );
                                    client.stop();
                                    break true;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            finish_group_message_backfill(
                                &mut active_group_backfill,
                                &pagination,
                                service.as_ref(),
                                GroupMessageBackfillOutcome::Failed,
                                Some("event_stream_closed".to_string()),
                                Some("Zalo listener event stream closed during history backfill".to_string()),
                            );
                            client.stop();
                            break true;
                        }
                    }
                }
            }
        };

        unregister_connected_client(&connected_clients, &provider_account_key, &client).await;

        if should_reconnect {
            // 13. Drop client, sleep backoff, reconnect inside same task
            let _ = service.repository.set_account_status(
                &provider_account_key,
                AccountConnectionStatus::Disconnected,
                None,
            );

            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
            }
            backoff_secs = (backoff_secs * 2).min(30);
        }
    }
}

fn finish_group_message_backfill(
    active: &mut Option<ActiveGroupMessageBackfill>,
    pagination: &HistoryPaginationState,
    service: &ConversationService,
    outcome: GroupMessageBackfillOutcome,
    stop_reason: Option<String>,
    warning: Option<String>,
) {
    let Some(active) = active.take() else {
        return;
    };
    let snapshot = pagination.snapshot(HistoryStream::GroupMessages);
    let durable_cursor = service
        .repository
        .get_checkpoint(active.account_id, GROUP_MESSAGE_HISTORY_SCOPE)
        .ok()
        .flatten();
    let completion = GroupMessageBackfillCompletion {
        outcome,
        pages_requested: snapshot.pages_requested,
        pages_received: snapshot.pages_received,
        durable_cursor,
        stop_reason: stop_reason.or(snapshot.stop_reason),
        warning,
    };

    for waiter in active.waiters {
        let _ = waiter.send(GroupMessageBackfillCompletion {
            outcome: completion.outcome.clone(),
            pages_requested: completion.pages_requested,
            pages_received: completion.pages_received,
            durable_cursor: completion.durable_cursor.clone(),
            stop_reason: completion.stop_reason.clone(),
            warning: completion.warning.clone(),
        });
    }
}

async fn unregister_connected_client(
    connected_clients: &ConnectedClients,
    provider_account_key: &str,
    client: &Arc<dyn crate::conversation::zalo::client::ZaloClient>,
) {
    let mut clients = connected_clients.lock().await;
    let is_current = clients
        .get(provider_account_key)
        .is_some_and(|registered| Arc::ptr_eq(registered, client));
    if is_current {
        clients.remove(provider_account_key);
    }
}

fn send_initial_history_requests(
    client: &dyn crate::conversation::zalo::client::ZaloClient,
    service: &ConversationService,
    account_id: Option<i64>,
) -> anyhow::Result<HistoryPaginationState> {
    HistoryPaginationState::start(client, service, account_id)
}
