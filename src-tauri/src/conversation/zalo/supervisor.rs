use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use zca_rs::listener::ListenerEvent;
use zca_rs::models::ThreadType;

use crate::conversation::service::ConversationService;
use crate::conversation::types::{AccountConnectionStatus, ConversationRuntimeStatus};
use crate::conversation::zalo::client::ZaloClientFactory;
use crate::conversation::zalo::{
    DIRECT_MESSAGE_HISTORY_SCOPE, DIRECT_MESSAGE_WATERMARK_SCOPE, DIRECT_REACTION_HISTORY_SCOPE,
    DIRECT_REACTION_WATERMARK_SCOPE, GROUP_MESSAGE_HISTORY_SCOPE, GROUP_MESSAGE_WATERMARK_SCOPE,
    GROUP_REACTION_HISTORY_SCOPE, GROUP_REACTION_WATERMARK_SCOPE,
};
use crate::db::open_db_connection;
use crate::models::ZaloSession;

const MAX_HISTORY_PAGES: usize = 5;
type ConnectedClients =
    Arc<Mutex<HashMap<String, Arc<dyn crate::conversation::zalo::client::ZaloClient>>>>;

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
    last_requested_cursor: Option<String>,
    completed: bool,
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
                    last_requested_cursor: cursor,
                    completed: false,
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
    ) -> anyhow::Result<()> {
        let state = self.streams.entry(stream).or_default();
        if state.completed
            || state.pages_requested >= MAX_HISTORY_PAGES
            || state.last_requested_cursor.as_deref() == Some(cursor)
        {
            state.completed = true;
            return Ok(());
        }

        stream.request(client, Some(cursor))?;
        state.pages_requested += 1;
        state.last_requested_cursor = Some(cursor.to_string());
        Ok(())
    }

    fn complete(&mut self, stream: HistoryStream) {
        self.streams.entry(stream).or_default().completed = true;
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

fn mark_account_degraded(service: &ConversationService, imei: &str, error: &dyn std::fmt::Display) {
    tracing::warn!(provider_account_key = imei, error = %error, "Zalo conversation session degraded");
    let _ = service.repository.set_account_status(
        imei,
        AccountConnectionStatus::Degraded,
        Some(&error.to_string()),
    );
}

struct SessionTask {
    cancellation: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

pub struct ZaloSessionSupervisor {
    db_path: String,
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
    tasks: Mutex<HashMap<String, SessionTask>>,
    clients: ConnectedClients,
    sticker_cache: std::sync::Mutex<crate::conversation::zalo::sticker_cache::ZaloStickerCache>,
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
        }
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
        let mut tasks = self.tasks.lock().await;

        if tasks.contains_key(&imei) {
            return;
        }

        let cancellation = CancellationToken::new();
        let task_cancel = cancellation.clone();
        let service = self.service.clone();
        let factory = self.factory.clone();
        let clients = self.clients.clone();

        let handle = tokio::spawn(async move {
            run_session_loop(service, factory, clients, session, task_cancel).await;
        });

        tasks.insert(
            imei,
            SessionTask {
                cancellation,
                handle,
            },
        );
    }

    pub async fn restart_account(&self, provider_account_key: &str) -> anyhow::Result<()> {
        // MUST NEVER await a task while holding tasks lock!
        let old_task = {
            let mut tasks = self.tasks.lock().await;
            tasks.remove(provider_account_key)
        };

        if let Some(task) = old_task {
            task.cancellation.cancel();
            let _ = task.handle.await;
        }
        self.clients.lock().await.remove(provider_account_key);

        let conn = open_db_connection(&self.db_path)?;
        let sessions = crate::db::get_all_zalo_sessions(&conn)
            .map_err(|e| anyhow::anyhow!("Failed to fetch Zalo sessions: {e}"))?;

        if let Some(session) = sessions
            .into_iter()
            .find(|s| s.imei == provider_account_key)
        {
            let cancellation = CancellationToken::new();
            let task_cancel = cancellation.clone();
            let service = self.service.clone();
            let factory = self.factory.clone();
            let clients = self.clients.clone();

            let handle = tokio::spawn(async move {
                run_session_loop(service, factory, clients, session, task_cancel).await;
            });

            let mut tasks = self.tasks.lock().await;
            tasks.insert(
                provider_account_key.to_string(),
                SessionTask {
                    cancellation,
                    handle,
                },
            );
        }

        Ok(())
    }

    pub async fn stop_account(&self, provider_account_key: &str) {
        let old_task = {
            let mut tasks = self.tasks.lock().await;
            tasks.remove(provider_account_key)
        };

        if let Some(task) = old_task {
            task.cancellation.cancel();
            let _ = task.handle.await;
        }
        self.clients.lock().await.remove(provider_account_key);
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
}

async fn run_session_loop(
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
    connected_clients: ConnectedClients,
    session: ZaloSession,
    cancellation: CancellationToken,
) {
    let imei = session.imei.clone();
    let mut backoff_secs = 1u64;

    loop {
        if cancellation.is_cancelled() {
            return;
        }

        // 1. Mark account connecting
        let account_id = service
            .repository
            .set_account_status(&imei, AccountConnectionStatus::Connecting, None)
            .ok();

        // 2. Connect through factory
        let client = match factory.connect(session.clone()).await {
            Ok(client) => client,
            Err(e) => {
                let err_msg = e.to_string();
                service
                    .repository
                    .set_account_status(
                        &imei,
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
            mark_account_degraded(service.as_ref(), &imei, &error);
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
            mark_account_degraded(service.as_ref(), &imei, &error);
            client.stop();
            tokio::select! {
                _ = cancellation.cancelled() => return,
                _ = tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)) => {}
            }
            backoff_secs = (backoff_secs * 2).min(30);
            continue;
        }

        connected_clients
            .lock()
            .await
            .insert(imei.clone(), client.clone());

        // 4. Mark account connected after the protocol cipher is ready
        let account_id = service
            .repository
            .set_account_status(&imei, AccountConnectionStatus::Connected, None)
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

        // 6. Request initial history and reactions from stored checkpoints
        let mut pagination =
            match send_initial_history_requests(client.as_ref(), service.as_ref(), account_id) {
                Ok(pagination) => pagination,
                Err(error) => {
                    unregister_connected_client(&connected_clients, &imei, &client).await;
                    mark_account_degraded(service.as_ref(), &imei, &error);
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
        let mut outbox_ticker = tokio::time::interval(std::time::Duration::from_millis(50));
        let mut profile_ticker = tokio::time::interval(std::time::Duration::from_secs(5));

        let should_reconnect = loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    if let Some(h) = profile_sync_handle.take() {
                        h.abort();
                    }
                    unregister_connected_client(&connected_clients, &imei, &client).await;
                    client.stop();
                    return;
                }
                _ = outbox_ticker.tick() => {
                    if let Some(acc_id) = account_id {
                        while let Ok(true) = service.process_next_outbox_job(acc_id, client.as_ref()).await {
                            if cancellation.is_cancelled() {
                                if let Some(h) = profile_sync_handle.take() {
                                    h.abort();
                                }
                                unregister_connected_client(&connected_clients, &imei, &client).await;
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

                            let history_batch = history_batch(&event);

                            // 10. Call service.ingest_zalo_event before publishing UI events
                            let display_name = session.display_name.as_deref();
                            if let Err(error) = service.ingest_zalo_event(&imei, display_name, event) {
                                mark_account_degraded(service.as_ref(), &imei, &error);
                                client.stop();
                                break true;
                            }

                            // 11. Trigger next page request if pagination state machine derived next_cursor
                            if let Some(batch) = history_batch {
                                if let Some(cursor) = batch.cursor {
                                    let Some(account_id) = account_id else {
                                        let error = anyhow::anyhow!("Missing channel account ID during history backfill");
                                        mark_account_degraded(service.as_ref(), &imei, &error);
                                        client.stop();
                                        break true;
                                    };
                                    if let Err(error) = service.repository.update_history_cursor(
                                        account_id,
                                        batch.stream.checkpoint_scope(),
                                        &cursor,
                                    ) {
                                        mark_account_degraded(service.as_ref(), &imei, &error);
                                        client.stop();
                                        break true;
                                    }
                                    if let Err(error) = pagination.request_next(
                                        client.as_ref(),
                                        batch.stream,
                                        &cursor,
                                    ) {
                                        mark_account_degraded(service.as_ref(), &imei, &error);
                                        client.stop();
                                        break true;
                                    }
                                } else {
                                    pagination.complete(batch.stream);
                                }
                            }

                            if is_disconnecting {
                                client.stop();
                                break true;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // 12. On Lagged, request direct/group reconciliation and mark account degraded
                            let _ = service.repository.set_account_status(
                                &imei,
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
                                    mark_account_degraded(service.as_ref(), &imei, &error);
                                    client.stop();
                                    break true;
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            client.stop();
                            break true;
                        }
                    }
                }
            }
        };

        unregister_connected_client(&connected_clients, &imei, &client).await;

        if should_reconnect {
            // 13. Drop client, sleep backoff, reconnect inside same task
            let _ = service.repository.set_account_status(
                &imei,
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
