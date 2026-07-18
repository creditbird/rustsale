#![recursion_limit = "256"]

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Manager;

mod commands;
#[cfg(test)]
mod commands_test;
mod db;
#[cfg(test)]
mod db_test;
mod models;
mod scrapers;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            std::fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");
            let db_path = app_data_dir.join("rustsale.db");
            let db_path_str = db_path.to_str().expect("Failed to convert path to string");

            #[allow(unused_mut)]
            let mut conn = db::init_db(db_path_str).expect("Failed to initialize database");
            #[cfg(debug_assertions)]
            db::seed_data(&mut conn).expect("Failed to seed database");

            app.manage(commands::DbState {
                conn: std::sync::Mutex::new(conn),
            });

            let port = find_free_port(9333);
            let browser_manager =
                std::sync::Arc::new(scrapers::browser::BrowserManager::new(2).with_port(port));
            app.manage(commands::BrowserState {
                manager: browser_manager,
                start_lock: tokio::sync::Mutex::new(()),
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
            commands::update_deal_stage,
            commands::get_activities,
            commands::add_activity,
            commands::update_lead,
            commands::delete_lead,
            commands::start_google_maps_scrape,
            commands::stop_google_maps_scrape,
            commands::get_preset_provinces,
            commands::start_zalo_qr_login,
            commands::get_zalo_sessions,
            commands::delete_zalo_session,
            commands::get_outreach_templates,
            commands::create_outreach_template,
            commands::update_outreach_template,
            commands::delete_outreach_template,
            commands::send_zalo_outreach,
            commands::get_categories,
            commands::create_category,
            commands::delete_category,
            commands::add_leads_to_category,
            commands::remove_leads_from_category,
            commands::get_leads_by_category,
            commands::send_zalo_campaign
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn find_free_port(start_port: u16) -> u16 {
    for port in start_port..65535 {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return port;
        }
    }
    start_port
}
