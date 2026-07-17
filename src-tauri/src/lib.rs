// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
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
    let conn = db::init_db("rustsale.db").expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(commands::DbState {
            conn: std::sync::Mutex::new(conn),
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
