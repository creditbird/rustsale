// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use tauri::Manager;

mod db;
mod models;
mod commands;
#[cfg(test)]
mod db_test;
#[cfg(test)]
mod commands_test;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("Failed to get app data directory");
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
            commands::add_activity
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
