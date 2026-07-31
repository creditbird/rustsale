use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use zca_rs::models::ThreadType;

use crate::conversation::service::ConversationService;
use crate::conversation::types::{AccountConnectionStatus, ConversationRuntimeStatus};
use crate::conversation::zalo::client::ZaloClientFactory;
use crate::db::open_db_connection;
use crate::models::ZaloSession;

struct SessionTask {
    cancellation: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

pub struct ZaloSessionSupervisor {
    db_path: String,
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
    tasks: Mutex<HashMap<String, SessionTask>>,
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
        }
    }

    pub async fn start_all(&self) -> anyhow::Result<()> {
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

        let handle = tokio::spawn(async move {
            run_session_loop(service, factory, session, task_cancel).await;
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

            let handle = tokio::spawn(async move {
                run_session_loop(service, factory, session, task_cancel).await;
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
    }

    pub async fn runtime_status(&self) -> anyhow::Result<ConversationRuntimeStatus> {
        self.service.runtime_status()
    }
}

async fn run_session_loop(
    service: Arc<ConversationService>,
    factory: Arc<dyn ZaloClientFactory>,
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

        // 4. Mark account connected
        let account_id = service
            .repository
            .set_account_status(&imei, AccountConnectionStatus::Connected, None)
            .ok()
            .or(account_id);

        // 5. Fetch all group snapshots and apply them before history reconciliation
        if let Ok(snapshots) = client.get_all_group_snapshots().await {
            for snapshot in snapshots {
                let _ = service.apply_group_snapshot(&snapshot);
            }
        }

        // 6. Request direct/group old messages and reactions from stored checkpoints
        let direct_cursor = account_id.and_then(|id| {
            service
                .repository
                .get_checkpoint(id, "direct_messages")
                .ok()
                .flatten()
        });
        let group_cursor = account_id.and_then(|id| {
            service
                .repository
                .get_checkpoint(id, "group_messages")
                .ok()
                .flatten()
        });

        let _ = client.request_old_messages(ThreadType::User, direct_cursor.as_deref());
        let _ = client.request_old_reactions(ThreadType::User, direct_cursor.as_deref());
        let _ = client.request_old_messages(ThreadType::Group, group_cursor.as_deref());
        let _ = client.request_old_reactions(ThreadType::Group, group_cursor.as_deref());

        // 7. Drain receiver loop
        let should_reconnect = loop {
            tokio::select! {
                _ = cancellation.cancelled() => {
                    client.stop();
                    return;
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

                            // 9. Call service.ingest_zalo_event before publishing UI events
                            let display_name = session.display_name.as_deref();
                            let _ = service.ingest_zalo_event(&imei, display_name, event);

                            if is_disconnecting {
                                client.stop();
                                break true;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                            // 10. On Lagged, request direct/group reconciliation and mark account degraded
                            let _ = service.repository.set_account_status(
                                &imei,
                                AccountConnectionStatus::Degraded,
                                Some("Receiver lagged"),
                            );

                            let d_cursor = account_id.and_then(|id| {
                                service
                                    .repository
                                    .get_checkpoint(id, "direct_messages")
                                    .ok()
                                    .flatten()
                            });
                            let g_cursor = account_id.and_then(|id| {
                                service
                                    .repository
                                    .get_checkpoint(id, "group_messages")
                                    .ok()
                                    .flatten()
                            });

                            let _ = client.request_old_messages(ThreadType::User, d_cursor.as_deref());
                            let _ = client.request_old_reactions(ThreadType::User, d_cursor.as_deref());
                            let _ = client.request_old_messages(ThreadType::Group, g_cursor.as_deref());
                            let _ = client.request_old_reactions(ThreadType::Group, g_cursor.as_deref());
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            client.stop();
                            break true;
                        }
                    }
                }
            }
        };

        if should_reconnect {
            // 11. Drop client, sleep backoff, reconnect inside same task
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
