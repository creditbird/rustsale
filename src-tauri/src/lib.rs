#![recursion_limit = "256"]

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Manager;

mod commands;
#[cfg(test)]
mod commands_test;
pub mod conversation;
mod db;
#[cfg(test)]
mod db_test;
pub mod google_maps_jobs;
mod models;
pub mod scrapers;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

pub(crate) fn install_rustls_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_rustls_crypto_provider();
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
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
                    conversation_repository,
                    conversation_events.clone(),
                ));
            let zalo_supervisor = std::sync::Arc::new(
                conversation::zalo::supervisor::ZaloSessionSupervisor::new(
                    db_path_str.to_string(),
                    conversation_service.clone(),
                    std::sync::Arc::new(conversation::zalo::client::RealZaloClientFactory),
                ),
            );
            app.manage(conversation::commands::ConversationState {
                service: conversation_service.clone(),
                zalo_supervisor: zalo_supervisor.clone(),
            });
            conversation_events.spawn_tauri_forwarder(app.handle().clone());
            let startup_service = conversation_service.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = startup_service.recover_outbox_startup() {
                    tracing::error!(target: "conversation", %error, "outbox recovery failed");
                }
                if let Err(error) = zalo_supervisor.start_all().await {
                    tracing::error!(target: "conversation", %error, "Zalo supervisor startup failed");
                }
            });

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
            commands::start_google_maps_scrape,
            commands::stop_google_maps_scrape,
            commands::get_preset_provinces,
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
            conversation::commands::get_contact_timeline,
            conversation::commands::get_conversation_timeline,
            conversation::commands::link_conversation_identity,
            conversation::commands::unlink_conversation_identity,
            conversation::commands::unlink_conversation_lead,
            conversation::commands::relink_conversation_lead,
            conversation::commands::send_conversation_message,
            conversation::commands::get_conversation_runtime_status,
            conversation::commands::restart_conversation_account
        ])

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
