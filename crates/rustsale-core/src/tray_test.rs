use super::*;

#[test]
fn test_tray_config_default() {
    let config = TrayConfig::default();
    assert!(config.close_to_tray);
    assert!(!config.dnd_enabled);
}

#[test]
fn test_tray_state_toggle_dnd() {
    let state = TrayState::default();
    assert!(!state.is_dnd_enabled());

    state.set_dnd_enabled(true);
    assert!(state.is_dnd_enabled());

    state.set_dnd_enabled(false);
    assert!(!state.is_dnd_enabled());
}

#[test]
fn test_tray_state_close_to_tray() {
    let state = TrayState::default();
    assert!(state.is_close_to_tray_enabled());

    state.set_close_to_tray(false);
    assert!(!state.is_close_to_tray_enabled());

    state.set_close_to_tray(true);
    assert!(state.is_close_to_tray_enabled());
}

#[test]
fn test_tray_config_sqlite_roundtrip_and_restart() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE app_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )
    .unwrap();

    // 1. Initial load from empty table yields defaults
    let initial = load_tray_config(&conn);
    assert!(initial.close_to_tray);
    assert!(!initial.dnd_enabled);

    // 2. Save modified config (user disables close to tray, enables DND)
    let modified = TrayConfig {
        close_to_tray: false,
        dnd_enabled: true,
    };
    save_tray_config(&conn, &modified).unwrap();

    // 3. Reload from SQLite and assert exact persistence
    let loaded = load_tray_config(&conn);
    assert!(!loaded.close_to_tray);
    assert!(loaded.dnd_enabled);

    // 4. Simulate application restart: instantiate default state, then restore from SQLite
    let fresh_state = TrayState::default();
    assert!(fresh_state.is_close_to_tray_enabled()); // starts as true by default
    fresh_state.set_close_to_tray(loaded.close_to_tray);
    fresh_state.set_dnd_enabled(loaded.dnd_enabled);

    // Asserts values restored properly after simulated restart
    assert!(!fresh_state.is_close_to_tray_enabled());
    assert!(fresh_state.is_dnd_enabled());
}

#[tokio::test]
async fn test_set_close_to_tray_command_persists_to_db() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute(
        "CREATE TABLE app_metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        [],
    )
    .unwrap();

    let db_state = crate::commands::DbState {
        conn: std::sync::Mutex::new(conn),
    };

    let app = crate::host::mock::MockApp::new()
        .manage(TrayState::default())
        .manage(db_state);

    let tray_state = app.state::<TrayState>();

    // Toggle close_to_tray to false
    let res = set_close_to_tray(app.handle().clone(), false, tray_state);
    assert!(res.is_ok());

    // Verify DB received the update
    let db_state_ref = app.state::<crate::commands::DbState>();
    let db_conn = db_state_ref.conn.lock().unwrap();
    let loaded = load_tray_config(&db_conn);
    assert!(!loaded.close_to_tray);
}
