#![recursion_limit = "256"]
//! `rustsale-core`: all RustSale business logic, independent of any UI shell.
//!
//! The shell (Tauri adapter today, NSH native module tomorrow) supplies
//! [`host::Ports`] and calls [`Core::start`]; commands are plain functions in
//! the `*::commands` modules that take an [`host::AppHandle`] / [`host::State`].

use std::path::PathBuf;

pub mod administrative_catalog;
pub mod ai;
pub mod campaign;
pub mod commands;
#[cfg(test)]
mod commands_test;
pub mod conversation;
pub mod db;
#[cfg(test)]
mod db_test;
pub mod demo_seed;
pub mod events;
pub mod facebook;
pub mod google_maps_jobs;
pub mod host;
pub mod instance_lock;
#[cfg(test)]
mod int7_core_test;
#[cfg(test)]
mod int8_core_test;
pub mod lead_importer;
pub mod local_api;
pub mod models;
pub mod notification;
pub mod paging;
pub mod scrapers;
pub mod tray;
#[cfg(test)]
mod ui2_core_test;

use host::{AppHandle, Ports};

/// Errors that can prevent the core from starting.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("cannot create data directory {path}: {source}")]
    DataDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("data path is not valid UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error("another RustSale instance is already using {0}")]
    AlreadyRunning(PathBuf),
    #[error("cannot take the instance lock: {0}")]
    InstanceLock(String),
    #[error("cannot initialize logging: {0}")]
    Logging(String),
    #[error("cannot initialize database: {0}")]
    Database(String),
    #[error("cannot initialize administrative catalog: {0}")]
    Catalog(String),
    #[error("cannot initialize Google Maps jobs: {0}")]
    GoogleMaps(String),
    #[error("cannot spawn thread {name}: {source}")]
    Thread {
        name: &'static str,
        #[source]
        source: std::io::Error,
    },
}

/// Static configuration supplied by the shell.
#[derive(Debug, Clone)]
pub struct CoreConfig {
    /// Application data directory (database, logs, sessions, spool).
    pub data_dir: PathBuf,
    /// Port of the local HTTP API; `None` keeps the environment/default
    /// (`RUSTSALE_LOCAL_API_PORT`, else 8787).
    pub local_api_port: Option<u16>,
}

/// A started core: owns the [`AppHandle`] with every managed state.
#[derive(Debug, Clone)]
pub struct Core {
    handle: AppHandle,
}

impl Core {
    pub fn handle(&self) -> &AppHandle {
        &self.handle
    }

    /// Initialize storage, register all states and spawn background workers
    /// on `runtime`. Behaviour mirrors the v2 Tauri `setup` hook (minus the
    /// tray and window handling, which belong to the shell).
    pub fn start(
        config: CoreConfig,
        ports: Ports,
        runtime: &tokio::runtime::Handle,
    ) -> Result<Self, CoreError> {
        let _guard = runtime.enter();
        let handle = AppHandle::new(ports);

        let tray_state = tray::TrayState::default();
        let notification_manager =
            std::sync::Arc::new(notification::NotificationManager::default());
        notification_manager.set_tray_state(tray_state.clone());
        let notif_mgr_for_setup = notification_manager.clone();
        let tray_state_for_setup = tray_state.clone();
        handle.manage(tray_state);
        handle.manage(notification_manager.clone());

        let app_data_dir = config.data_dir.clone();
        std::fs::create_dir_all(&app_data_dir).map_err(|source| CoreError::DataDir {
            path: app_data_dir.clone(),
            source,
        })?;
        // A second process on this data directory would interrupt this one's
        // scrape jobs and share its browser profile (A-U2). Refuse it before
        // anything else touches the directory.
        let instance_lock =
            instance_lock::InstanceLock::acquire(&app_data_dir).map_err(|error| match error {
                instance_lock::InstanceLockError::AlreadyRunning { path } => {
                    CoreError::AlreadyRunning(path)
                }
                other => CoreError::InstanceLock(other.to_string()),
            })?;
        handle.manage(instance_lock);
        let diagnostics = google_maps_jobs::diagnostics::init_structured_logging_or_inherit(
            &app_data_dir.join("logs"),
        )
        .map_err(|source| CoreError::Logging(source.to_string()))?;
        handle.manage(diagnostics);
        let db_path = app_data_dir.join("rustsale.db");
        let db_path_str = db_path
            .to_str()
            .ok_or_else(|| CoreError::NonUtf8Path(db_path.clone()))?;

        #[allow(unused_mut)]
        let mut conn =
            db::init_db(db_path_str).map_err(|source| CoreError::Database(source.to_string()))?;
        #[cfg(debug_assertions)]
        db::seed_data(&mut conn).map_err(|source| CoreError::Database(source.to_string()))?;

        let loaded_settings = notification::load_notification_settings(&conn);
        notif_mgr_for_setup.update_settings(loaded_settings);

        let loaded_tray = tray::load_tray_config(&conn);
        tray_state_for_setup.set_close_to_tray(loaded_tray.close_to_tray);
        tray_state_for_setup.set_dnd_enabled(loaded_tray.dnd_enabled);

        handle.manage(commands::DbState {
            conn: std::sync::Mutex::new(conn),
        });

        let administrative_catalog_repository =
            administrative_catalog::repository::AdministrativeCatalogRepository::new(
                app_data_dir
                    .join("administrative-catalog")
                    .join("catalog.db"),
            )
            .map_err(|source| CoreError::Catalog(source.to_string()))?;
        let administrative_catalog_service = std::sync::Arc::new(
            administrative_catalog::service::AdministrativeCatalogService::new(
                administrative_catalog_repository,
            )
            .map_err(|source| CoreError::Catalog(source.to_string()))?,
        );
        handle.manage(
            administrative_catalog::commands::AdministrativeCatalogState {
                service: administrative_catalog_service,
            },
        );

        // `init_db` above already migrated this file; do not migrate it again.
        let repository = std::sync::Arc::new(
            google_maps_jobs::repository::ScrapeJobRepository::for_initialized_db(
                db_path_str.to_string(),
            ),
        );
        let events = std::sync::Arc::new(google_maps_jobs::events::ScrapeEventPublisher::default());
        let processor =
            std::sync::Arc::new(google_maps_jobs::processor::GoogleMapsUnitProcessor::new(
                repository.clone(),
                events.clone(),
            ));
        let runner = std::sync::Arc::new(google_maps_jobs::runner::ScrapeJobRunner::new(
            repository.clone(),
            processor,
            events.clone(),
            google_maps_jobs::runner::RetryPolicy::default(),
        ));
        // Job starts wait for start-up recovery (A-U4); see `recover_startup_exclusive`.
        runner.expect_startup_recovery();
        let service = std::sync::Arc::new(google_maps_jobs::commands::GoogleMapsJobService::new(
            repository,
            runner,
            app_data_dir.join("google-maps-profiles"),
        ));
        let runtime = std::sync::Arc::new(scrapers::browser::BrowserManager::new(1));
        handle.manage(google_maps_jobs::commands::GoogleMapsJobState {
            service: service.clone(),
            runtime,
        });
        events.clone().spawn_event_forwarder(handle.clone());
        let retention_service = service.clone();
        let mut retention_events = events.subscribe();
        tokio::spawn(async move {
            loop {
                match retention_events.recv().await {
                    Ok(event) if event.status.is_terminal() => {
                        if let Err(error) = retention_service.cleanup_retention().await {
                            tracing::error!(
                                target: "google_maps_scraper",
                                event = "retention_cleanup_failed",
                                job_id = %event.job_id,
                                error = %error,
                            );
                        }
                    }
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(
                            target: "google_maps_scraper",
                            event = "retention_listener_lagged",
                            skipped,
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
        let scheduled_service = service.clone();
        tokio::spawn(async move {
            // Start-up cleanup can lose a lock race; the schedule picks it up later.
            let mut ticker = tokio::time::interval_at(
                tokio::time::Instant::now() + std::time::Duration::from_secs(6 * 60 * 60),
                std::time::Duration::from_secs(6 * 60 * 60),
            );
            loop {
                ticker.tick().await;
                if let Err(error) = scheduled_service.cleanup_retention().await {
                    tracing::error!(
                        target: "google_maps_scraper",
                        event = "scheduled_retention_cleanup_failed",
                        error = %error,
                    );
                }
            }
        });
        tokio::spawn(async move {
            // The instance lock is held, so every `running` job in the
            // database belongs to a process that has died.
            if let Err(error) = service.recover_startup_exclusive().await {
                tracing::error!(
                    target: "google_maps_scraper",
                    event = "startup_recovery_failed",
                    error = %error,
                );
            }
            if let Err(error) = service.cleanup_retention().await {
                tracing::error!(
                    target: "google_maps_scraper",
                    event = "startup_retention_cleanup_failed",
                    error = %error,
                );
            }
        });

        let conversation_repository = std::sync::Arc::new(
            conversation::repository::ConversationRepository::new(db_path_str.to_string()),
        );
        let conversation_events =
            std::sync::Arc::new(conversation::events::ConversationEventPublisher::new(256));
        let conversation_service =
            std::sync::Arc::new(conversation::service::ConversationService::new(
                conversation_repository.clone(),
                conversation_events.clone(),
                app_data_dir.join("attachments-spool"),
            ));
        let zalo_supervisor =
            std::sync::Arc::new(conversation::zalo::supervisor::ZaloSessionSupervisor::new(
                db_path_str.to_string(),
                conversation_service.clone(),
                std::sync::Arc::new(conversation::zalo::client::RealZaloClientFactory),
            ));
        let facebook_graph_client =
            std::sync::Arc::new(conversation::facebook::client::RealFacebookGraphClient::new());
        let facebook_supervisor = std::sync::Arc::new(
            conversation::facebook::supervisor::FacebookMessengerSupervisor::new(
                db_path_str.to_string(),
                conversation_events.clone(),
                facebook_graph_client,
            ),
        );
        let whatsapp_sessions_dir = app_data_dir.join("whatsapp-sessions");
        let whatsapp_supervisor =
            std::sync::Arc::new(conversation::whatsapp::supervisor::WhatsAppSupervisor::new(
                db_path_str.to_string(),
                whatsapp_sessions_dir.clone(),
                conversation_events.clone(),
            ));
        let whatsapp_login_manager =
            std::sync::Arc::new(conversation::whatsapp::login::WhatsAppLoginManager::new(
                db_path_str.to_string(),
                whatsapp_sessions_dir,
                Some(handle.clone()),
            ));
        whatsapp_login_manager.with_supervisor(whatsapp_supervisor.clone());
        whatsapp_supervisor.with_service(conversation_service.clone());

        let ws_startup = whatsapp_supervisor.clone();
        tokio::spawn(async move {
            let _ = ws_startup.load_active_sessions_on_startup().await;
        });
        let conversation_state = conversation::commands::ConversationState {
            service: conversation_service.clone(),
            zalo_supervisor: zalo_supervisor.clone(),
            facebook_supervisor: facebook_supervisor.clone(),
            whatsapp_supervisor: whatsapp_supervisor.clone(),
            whatsapp_login_manager: whatsapp_login_manager.clone(),
        };
        local_api::server::spawn(
            std::sync::Arc::new(conversation_state.clone()),
            config.local_api_port,
        );
        handle.manage(conversation_state);
        conversation_events
            .clone()
            .spawn_event_forwarder(handle.clone());
        let notif_service = notification_manager.clone();
        let notif_repo = conversation_repository.clone();
        let notif_app = handle.clone();
        let mut notif_events = conversation_events.subscribe();

        let (notif_tx, notif_rx) = tokio::sync::mpsc::channel::<(i64, Option<i64>)>(128);
        let notif_rx = std::sync::Arc::new(tokio::sync::Mutex::new(notif_rx));

        for _ in 0..4 {
            let rx = notif_rx.clone();
            let repo = notif_repo.clone();
            let mgr = notif_service.clone();
            let handle = notif_app.clone();
            tokio::spawn(async move {
                loop {
                    let job = {
                        let mut lock = rx.lock().await;
                        lock.recv().await
                    };
                    match job {
                        Some((msg_id, lead_id)) => {
                            let repo = repo.clone();
                            let mgr = mgr.clone();
                            let handle = handle.clone();
                            match tokio::task::spawn_blocking(move || {
                                if let Ok(Some(msg)) = repo.get_message_by_id(msg_id) {
                                    if msg.direction
                                        == conversation::types::MessageDirection::Inbound
                                    {
                                        let sender = msg
                                            .sender_display_name
                                            .as_deref()
                                            .unwrap_or("Khách hàng");
                                        let text = msg.text_content.as_deref().unwrap_or("");
                                        mgr.notify_inbound(
                                            &handle,
                                            msg.conversation_id,
                                            lead_id,
                                            msg.channel,
                                            sender,
                                            text,
                                            &msg.content_type,
                                            false,
                                            None,
                                        );
                                    }
                                }
                            })
                            .await
                            {
                                Ok(_) => {}
                                Err(join_err) => {
                                    tracing::error!(target: "notification", %msg_id, %join_err, "Notification worker blocking task panicked or aborted");
                                }
                            }
                        }
                        None => break,
                    }
                }
            });
        }

        tokio::spawn(async move {
            loop {
                match notif_events.recv().await {
                    Ok(event) => {
                        if event.reason
                            == conversation::events::ConversationInvalidationReason::Message
                        {
                            if let Some(msg_id) = event.message_id {
                                match notif_tx.try_send((msg_id, event.lead_id)) {
                                    Ok(_) => {}
                                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                        tracing::warn!(target: "notification", %msg_id, "Notification queue full (128 items); message dropped to protect runtime");
                                    }
                                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                                        tracing::warn!(target: "notification", %msg_id, "Notification worker queue closed");
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(target: "notification", skipped, "Notification event receiver lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let startup_service = conversation_service.clone();
        let startup_fb_supervisor = facebook_supervisor.clone();
        let startup_wa_supervisor = whatsapp_supervisor.clone();
        facebook_supervisor.with_service(conversation_service.clone());
        tokio::spawn(async move {
            if let Err(error) = startup_service.recover_outbox_startup() {
                tracing::error!(target: "conversation", %error, "outbox recovery failed");
            }
            if let Err(error) = zalo_supervisor.start_all().await {
                tracing::error!(target: "conversation", %error, "Zalo supervisor startup failed");
            }
            if let Err(error) = startup_fb_supervisor.sync_all().await {
                tracing::error!(target: "conversation", %error, "Facebook Messenger initial sync failed");
            }
            drop(startup_fb_supervisor.start_background_loop(std::time::Duration::from_secs(60)));
            tokio::spawn(async move {
                startup_wa_supervisor
                    .run_loop(std::time::Duration::from_millis(500))
                    .await;
            });
        });

        let fb_db_path = db_path_str.to_string();
        std::thread::Builder::new()
        .name("facebook-scheduler".to_string())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to build runtime for facebook-scheduler: {}", e);
                    return;
                }
            };
            rt.block_on(async move {
                let scheduler = facebook::scheduler::FacebookScheduler::new();
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                    if let Ok(mut conn) = rusqlite::Connection::open(&fb_db_path) {
                        let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
                        let _ = conn.execute_batch(
                            "PRAGMA foreign_keys = ON;
                             PRAGMA journal_mode = WAL;
                             PRAGMA synchronous = NORMAL;",
                        );
                        if let Err(e) = scheduler.run_promotion_cycle(&conn, 10).await {
                            tracing::warn!(target: "facebook_scheduler", error = %e, "promotion cycle error");
                        }
                        if let Err(e) = scheduler.run_reconciliation_cycle(&conn, 10).await {
                            tracing::warn!(target: "facebook_scheduler", error = %e, "reconciliation cycle error");
                        }
                        if let Err(e) = scheduler.reconcile_upload_sessions(&mut conn, 10).await {
                            tracing::warn!(target: "facebook_scheduler", error = %e, "upload session reconciler error");
                        }
                    }
                }
            });
        })
        .map_err(|source| CoreError::Thread { name: "facebook-scheduler", source })?;

        Ok(Self { handle })
    }
}

/// Apply the autostart window policy once the shell knows whether the tray
/// came up (v2 `setup` behaviour).
pub fn apply_boot_window_policy(app: &AppHandle, has_autostart_arg: bool, tray_ready: bool) {
    let window = &app.ports().window;
    let action =
        determine_autostart_action(has_autostart_arg, tray_ready, window.has_main_window());
    match action {
        AutostartBootAction::HiddenToTray => {
            if let Err(e) = window.hide_main() {
                tracing::error!("Failed to hide main window during autostart boot: {:?}", e);
                let _ = window.show_main();
            } else {
                tracing::info!("Application launched in autostart mode, window minimized to tray");
            }
        }
        AutostartBootAction::ShowDueToTrayFailure => {
            tracing::warn!(
                "System tray failed to initialize; keeping main window visible to prevent stranded process"
            );
            let _ = window.show_main();
        }
        AutostartBootAction::NormalLaunch => {
            tracing::debug!("Application launched normally, main window visible");
        }
        AutostartBootAction::NoMainWindow => {
            tracing::warn!("Autostart launch detected but main window was not found");
        }
    }
}

/// Smoke command kept from the v2 template.
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AutostartBootAction {
    NormalLaunch,
    HiddenToTray,
    ShowDueToTrayFailure,
    NoMainWindow,
}

pub fn determine_autostart_action(
    has_autostart_arg: bool,
    tray_ready: bool,
    has_window: bool,
) -> AutostartBootAction {
    if !has_autostart_arg {
        return AutostartBootAction::NormalLaunch;
    }
    if !has_window {
        return AutostartBootAction::NoMainWindow;
    }
    if !tray_ready {
        return AutostartBootAction::ShowDueToTrayFailure;
    }
    AutostartBootAction::HiddenToTray
}

pub fn install_rustls_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

const ALLOWED_DOTENV_KEYS: &[&str] = &[
    "APP_ID",
    "APP_SECRET",
    "FACEBOOK_APP_ID",
    "FACEBOOK_APP_SECRET",
    "META_APP_ID",
    "META_APP_SECRET",
    "VITE_FACEBOOK_APP_ID",
    "VITE_FACEBOOK_APP_SECRET",
    "RUSTSALE_LOCAL_API_TOKEN",
    "RUSTSALE_LOCAL_API_PORT",
    "RUSTSALE_ZALO_WEBHOOK_URL",
    "RUSTSALE_ZALO_WEBHOOK_INCLUDE_GROUPS",
];

static DOTENV_INIT: std::sync::Once = std::sync::Once::new();

pub fn load_dotenv_if_present() {
    DOTENV_INIT.call_once(|| {
        let mut candidates = Vec::new();
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(".env"));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(p) = exe.parent() {
                candidates.push(p.join(".env"));
            }
        }

        for path in candidates {
            if path.is_file() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if trimmed.is_empty() || trimmed.starts_with('#') {
                            continue;
                        }
                        if let Some((key, val)) = trimmed.split_once('=') {
                            let k = key.trim();
                            let mut v = val.trim();
                            if ((v.starts_with('"') && v.ends_with('"'))
                                || (v.starts_with('\'') && v.ends_with('\'')))
                                && v.len() >= 2
                            {
                                v = &v[1..v.len() - 1];
                            }
                            if !k.is_empty()
                                && ALLOWED_DOTENV_KEYS.contains(&k)
                                && std::env::var(k).is_err()
                            {
                                std::env::set_var(k, v);
                            }
                        }
                    }
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_normal_launch() {
        let action = determine_autostart_action(false, true, true);
        assert_eq!(action, AutostartBootAction::NormalLaunch);
    }

    #[test]
    fn test_autostart_hidden_to_tray_when_ready() {
        let action = determine_autostart_action(true, true, true);
        assert_eq!(action, AutostartBootAction::HiddenToTray);
    }

    #[test]
    fn test_autostart_shows_window_when_tray_fails() {
        // Crucial safety invariant: If system tray fails to initialize on OS boot,
        // we MUST NOT hide the window, otherwise the user has no way to interact with the app.
        let action = determine_autostart_action(true, false, true);
        assert_eq!(action, AutostartBootAction::ShowDueToTrayFailure);
    }

    #[test]
    fn test_autostart_no_main_window() {
        let action = determine_autostart_action(true, true, false);
        assert_eq!(action, AutostartBootAction::NoMainWindow);
    }
}
