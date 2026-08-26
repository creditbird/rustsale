#![recursion_limit = "256"]

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Manager;

pub mod administrative_catalog;
mod commands;
#[cfg(test)]
mod commands_test;
pub mod conversation;
mod db;
#[cfg(test)]
mod db_test;
pub mod facebook;
pub mod google_maps_jobs;
mod models;
pub mod notification;
pub mod scrapers;
pub mod tray;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub(crate) fn install_rustls_crypto_provider() {
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
                            if (v.starts_with('"') && v.ends_with('"'))
                                || (v.starts_with('\'') && v.ends_with('\''))
                            {
                                if v.len() >= 2 {
                                    v = &v[1..v.len() - 1];
                                }
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    load_dotenv_if_present();
    install_rustls_crypto_provider();
    let tray_state = tray::TrayState::default();
    let notification_manager = std::sync::Arc::new(notification::NotificationManager::default());

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .manage(tray_state)
        .manage(notification_manager.clone())
        .setup(move |app| {
            let _ = tray::init_tray(app.handle());
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
            let diagnostics =
                google_maps_jobs::diagnostics::init_structured_logging(&app_data_dir.join("logs"))
                    .expect("Failed to initialize structured logging");
            app.manage(diagnostics);
            let db_path = app_data_dir.join("rustsale.db");
            let db_path_str = db_path.to_str().expect("Failed to convert path to string");

            #[allow(unused_mut)]
            let mut conn = db::init_db(db_path_str).expect("Failed to initialize database");
            #[cfg(debug_assertions)]
            db::seed_data(&mut conn).expect("Failed to seed database");

            app.manage(commands::DbState {
                conn: std::sync::Mutex::new(conn),
            });

            let administrative_catalog_repository =
                administrative_catalog::repository::AdministrativeCatalogRepository::new(
                    app_data_dir
                        .join("administrative-catalog")
                        .join("catalog.db"),
                )
                .expect("Failed to initialize administrative catalog repository");
            let administrative_catalog_service = std::sync::Arc::new(
                administrative_catalog::service::AdministrativeCatalogService::new(
                    administrative_catalog_repository,
                )
                .expect("Failed to initialize administrative catalog service"),
            );
            app.manage(
                administrative_catalog::commands::AdministrativeCatalogState {
                    service: administrative_catalog_service,
                },
            );

            let repository = std::sync::Arc::new(
                google_maps_jobs::repository::ScrapeJobRepository::new(db_path_str.to_string())
                    .expect("Failed to initialize Google Maps job repository"),
            );
            let events =
                std::sync::Arc::new(google_maps_jobs::events::ScrapeEventPublisher::default());
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
            let service =
                std::sync::Arc::new(google_maps_jobs::commands::GoogleMapsJobService::new(
                    repository,
                    runner,
                    app_data_dir.join("google-maps-profiles"),
                ));
            let runtime = std::sync::Arc::new(scrapers::browser::BrowserManager::new(1));
            app.manage(google_maps_jobs::commands::GoogleMapsJobState {
                service: service.clone(),
                runtime,
            });
            events.clone().spawn_tauri_forwarder(app.handle().clone());
            let retention_service = service.clone();
            let mut retention_events = events.subscribe();
            tauri::async_runtime::spawn(async move {
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
            tauri::async_runtime::spawn(async move {
                if let Err(error) = service.recover_startup().await {
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
            let zalo_supervisor = std::sync::Arc::new(
                conversation::zalo::supervisor::ZaloSessionSupervisor::new(
                    db_path_str.to_string(),
                    conversation_service.clone(),
                    std::sync::Arc::new(conversation::zalo::client::RealZaloClientFactory),
                ),
            );
            let facebook_graph_client = std::sync::Arc::new(
                conversation::facebook::client::RealFacebookGraphClient::new(),
            );
            let facebook_supervisor = std::sync::Arc::new(
                conversation::facebook::supervisor::FacebookMessengerSupervisor::new(
                    db_path_str.to_string(),
                    conversation_events.clone(),
                    facebook_graph_client,
                ),
            );
            let whatsapp_supervisor = std::sync::Arc::new(
                conversation::whatsapp::supervisor::WhatsAppSupervisor::new(
                    db_path_str.to_string(),
                    conversation_events.clone(),
                ),
            );
            let whatsapp_login_manager = std::sync::Arc::new(
                conversation::whatsapp::login::WhatsAppLoginManager::new(
                    app_data_dir.join("whatsapp-sessions"),
                    Some(app.handle().clone()),
                ),
            );
            whatsapp_supervisor.with_service(conversation_service.clone());
            app.manage(conversation::commands::ConversationState {
                service: conversation_service.clone(),
                zalo_supervisor: zalo_supervisor.clone(),
                facebook_supervisor: facebook_supervisor.clone(),
                whatsapp_supervisor: whatsapp_supervisor.clone(),
                whatsapp_login_manager: whatsapp_login_manager.clone(),
            });
            conversation_events.clone().spawn_tauri_forwarder(app.handle().clone());
            let notif_service = notification_manager.clone();
            let notif_repo = conversation_repository.clone();
            let notif_app = app.handle().clone();
            let mut notif_events = conversation_events.subscribe();
            tauri::async_runtime::spawn(async move {
                loop {
                    match notif_events.recv().await {
                        Ok(event) => {
                            if event.reason == conversation::events::ConversationInvalidationReason::Message {
                                if let Some(msg_id) = event.message_id {
                                    let repo = notif_repo.clone();
                                    let mgr = notif_service.clone();
                                    let handle = notif_app.clone();
                                    tokio::task::spawn_blocking(move || {
                                        if let Ok(Some(msg)) = repo.get_message_by_id(msg_id) {
                                            if msg.direction == conversation::types::MessageDirection::Inbound {
                                                let sender = msg.sender_display_name.as_deref().unwrap_or("Khách hàng");
                                                let text = msg.text_content.as_deref().unwrap_or("");
                                                mgr.notify_inbound(
                                                    &handle,
                                                    msg.conversation_id,
                                                    event.lead_id,
                                                    msg.channel,
                                                    sender,
                                                    text,
                                                    &msg.content_type,
                                                    false,
                                                    None,
                                                );
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });

            let startup_service = conversation_service.clone();
            let startup_fb_supervisor = facebook_supervisor.clone();
            let startup_wa_supervisor = whatsapp_supervisor.clone();
            facebook_supervisor.with_service(conversation_service.clone());
            tauri::async_runtime::spawn(async move {
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
                tauri::async_runtime::spawn(async move {
                    startup_wa_supervisor
                        .run_loop(std::time::Duration::from_secs(60))
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
                .expect("Failed to spawn facebook-scheduler thread");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            commands::get_leads,
            commands::get_lead_details,
            commands::add_lead,
            commands::update_lead_status,
            commands::get_deals,
            commands::create_deal,
            commands::update_deal_stage,
            commands::delete_deal,
            commands::get_activities,
            commands::get_recent_activities,
            commands::add_activity,
            commands::update_lead,
            commands::delete_lead,
            commands::get_currency_settings,
            commands::update_currency_settings,
            commands::start_google_maps_scrape,
            commands::stop_google_maps_scrape,
            commands::get_preset_provinces,
            commands::plan_grid_crawl,
            administrative_catalog::commands::list_administrative_countries,
            administrative_catalog::commands::get_administrative_country_status,
            administrative_catalog::commands::load_administrative_country,
            administrative_catalog::commands::list_administrative_areas,
            administrative_catalog::commands::search_administrative_areas,
            administrative_catalog::commands::resolve_administrative_bounds,
            administrative_catalog::commands::geocode_bounding_box,
            google_maps_jobs::commands::get_google_maps_runtime_status,
            google_maps_jobs::commands::prepare_google_maps_runtime,
            google_maps_jobs::commands::create_google_maps_scrape_job,
            google_maps_jobs::commands::pause_google_maps_scrape_job,
            google_maps_jobs::commands::resume_google_maps_scrape_job,
            google_maps_jobs::commands::cancel_google_maps_scrape_job,
            google_maps_jobs::commands::retry_google_maps_failed_units,
            google_maps_jobs::commands::retry_google_maps_email_enrichment,
            google_maps_jobs::commands::get_google_maps_scrape_job,
            google_maps_jobs::commands::list_google_maps_scrape_jobs,
            google_maps_jobs::commands::delete_google_maps_scrape_job,
            commands::start_zalo_qr_login,
            commands::get_managed_zalo_accounts,
            commands::get_managed_smtp_accounts,
            commands::get_managed_facebook_accounts,
            commands::save_managed_facebook_account,
            commands::fetch_facebook_pages_from_token,
            commands::delete_facebook_account,
            commands::delete_zalo_session,
            commands::verify_zalo_session,
            commands::verify_all_zalo_sessions,
            commands::get_outreach_templates,
            commands::create_outreach_template,
            commands::update_outreach_template,
            commands::delete_outreach_template,
            commands::send_zalo_outreach,
            commands::start_zalo_outreach,
            commands::get_categories,
            commands::get_categories_for_lead,
            commands::create_category,
            commands::delete_category,
            commands::add_leads_to_category,
            commands::remove_leads_from_category,
            commands::get_leads_by_category,
            commands::send_zalo_campaign,
            commands::get_campaign_leads,
            commands::get_outreach_leads_page,
            commands::resolve_outreach_lead_ids,
            commands::save_smtp_server,
            commands::delete_smtp_server,
            commands::test_smtp_connection,
            commands::get_email_templates,
            commands::create_email_template,
            commands::update_email_template,
            commands::delete_email_template,
            commands::send_email_outreach,
            conversation::commands::list_conversation_inbox,
            conversation::commands::mark_conversation_read,
            conversation::commands::get_contact_timeline,
            conversation::commands::get_conversation_timeline,
            conversation::commands::get_identity_timeline,
            conversation::commands::link_conversation_identity,
            conversation::commands::unlink_conversation_identity,
            conversation::commands::unlink_conversation_lead,
            conversation::commands::relink_conversation_lead,
            conversation::commands::send_conversation_message,
            conversation::commands::get_conversation_runtime_status,
            conversation::commands::restart_conversation_account,
            conversation::commands::delete_conversation,
            conversation::commands::delete_conversations_batch,
            conversation::commands::start_direct_conversation,
            conversation::commands::send_conversation_batch,
            conversation::commands::stage_conversation_attachment_paths,
            conversation::commands::stage_conversation_clipboard_image,
            conversation::commands::discard_conversation_staged_attachment,
            conversation::commands::search_zalo_stickers,
            conversation::commands::get_zalo_sticker_detail,
            conversation::commands::recall_conversation_message,
            conversation::commands::list_conversation_quick_templates,
            conversation::commands::create_conversation_quick_template,
            conversation::commands::update_conversation_quick_template,
            conversation::commands::delete_conversation_quick_template,
            conversation::commands::mark_conversation_quick_template_used,
            conversation::commands::set_conversation_reaction,
            conversation::commands::forward_conversation_message,
            conversation::commands::get_conversation_forward_operation,
            conversation::commands::retry_conversation_forward_target,
            conversation::commands::list_conversation_mention_candidates,
            conversation::commands::pin_conversation_message,
            conversation::commands::unpin_conversation_message,
            conversation::commands::list_conversation_pinned_messages,
            conversation::commands::sync_facebook_conversations,
            conversation::commands::ingest_facebook_webhook_events,
            conversation::commands::start_whatsapp_qr_login,
            conversation::commands::cancel_whatsapp_qr_login,
            conversation::commands::get_managed_whatsapp_accounts,
            conversation::commands::disconnect_whatsapp_account,
            conversation::commands::create_whatsapp_campaign,
            conversation::commands::populate_whatsapp_campaign_recipients,
            conversation::commands::start_whatsapp_campaign,
            conversation::commands::pause_whatsapp_campaign,
            conversation::commands::resume_whatsapp_campaign,
            conversation::commands::cancel_whatsapp_campaign,
            conversation::commands::get_whatsapp_campaign,
            conversation::commands::list_whatsapp_campaigns,
            conversation::commands::get_whatsapp_campaign_recipients,
            facebook::commands::schedule_facebook_post,
            facebook::commands::publish_facebook_post_now,
            facebook::commands::list_facebook_scheduled_posts,
            facebook::commands::cancel_facebook_scheduled_post,
            facebook::commands::create_comment_automation,
            facebook::commands::list_comment_automations,
            facebook::commands::update_comment_automation,
            facebook::commands::delete_comment_automation,
            facebook::commands::list_comment_logs,
            facebook::commands::get_facebook_page_health,
            commands::get_group_members_unrestricted,
            commands::batch_import_group_members_to_leads,
            commands::sync_and_deep_scan_group_members,
            commands::get_hardware_id,
            commands::export_leads_csv_file,
            commands::save_image_file,
            commands::install_portable_linux_update,
            tray::get_tray_state,
            tray::set_close_to_tray,
            tray::set_tray_dnd,
            notification::get_notification_settings,
            notification::update_notification_settings,
            notification::set_active_conversation_focus,
            notification::test_desktop_notification
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(tray_state) = window.app_handle().try_state::<tray::TrayState>() {
                    if tray_state.is_close_to_tray_enabled() {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
