use crate::host::{AppHandle, State};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrayConfig {
    pub close_to_tray: bool,
    pub dnd_enabled: bool,
}

impl Default for TrayConfig {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            dnd_enabled: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrayState {
    close_to_tray: Arc<AtomicBool>,
    dnd_enabled: Arc<AtomicBool>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self::new(true, false)
    }
}

impl TrayState {
    pub fn new(close_to_tray: bool, dnd_enabled: bool) -> Self {
        Self {
            close_to_tray: Arc::new(AtomicBool::new(close_to_tray)),
            dnd_enabled: Arc::new(AtomicBool::new(dnd_enabled)),
        }
    }

    pub fn is_close_to_tray_enabled(&self) -> bool {
        self.close_to_tray.load(Ordering::SeqCst)
    }

    pub fn set_close_to_tray(&self, enabled: bool) {
        self.close_to_tray.store(enabled, Ordering::SeqCst);
    }

    pub fn is_dnd_enabled(&self) -> bool {
        self.dnd_enabled.load(Ordering::SeqCst)
    }

    pub fn set_dnd_enabled(&self, enabled: bool) {
        self.dnd_enabled.store(enabled, Ordering::SeqCst);
    }
}

/// Actions behind the tray menu. The shell draws the tray; these hold the
/// behavior so every shell (Tauri, NSH) acts identically.
pub mod actions {
    use super::{save_tray_config, TrayConfig, TrayState};
    use crate::commands::DbState;
    use crate::host::AppHandle;

    /// Menu "Mở RustSale".
    pub fn open_app(app: &AppHandle) {
        if let Err(error) = app.ports().window.show_main() {
            tracing::warn!(target: "tray", %error, "failed to show main window");
        }
    }

    /// Menu "Đánh dấu tất cả đã đọc": the UI performs the actual marking.
    pub fn mark_all_read(app: &AppHandle) {
        let _ = app.emit("tray-mark-all-read", ());
    }

    /// Menu "Bật/Tắt Không làm phiền (DND)".
    pub fn toggle_dnd(app: &AppHandle) {
        if let Some(state) = app.try_state::<TrayState>() {
            let new_val = !state.is_dnd_enabled();
            state.set_dnd_enabled(new_val);
            let _ = app.emit("tray-dnd-changed", new_val);
            if let Some(db_state) = app.try_state::<DbState>() {
                if let Ok(conn) = db_state.conn.lock() {
                    let config = TrayConfig {
                        close_to_tray: state.is_close_to_tray_enabled(),
                        dnd_enabled: new_val,
                    };
                    let _ = save_tray_config(&conn, &config);
                }
            }
        }
    }

    /// Menu "Thoát ứng dụng".
    pub fn quit(app: &AppHandle) {
        app.ports().window.exit(0);
    }

    /// Left click on the tray icon: hide when visible, otherwise show.
    pub fn toggle_main_window(app: &AppHandle) {
        let window = &app.ports().window;
        if !window.has_main_window() {
            return;
        }
        let result = if window.is_main_visible() {
            window.hide_main()
        } else {
            window.show_main()
        };
        if let Err(error) = result {
            tracing::warn!(target: "tray", %error, "failed to toggle main window");
        }
    }

    /// Window close requested: true when the window must be hidden instead
    /// of closed (close-to-tray enabled).
    pub fn should_hide_on_close(app: &AppHandle) -> bool {
        app.try_state::<TrayState>()
            .is_some_and(|tray_state| tray_state.is_close_to_tray_enabled())
    }
}

pub fn load_tray_config(conn: &rusqlite::Connection) -> TrayConfig {
    conn.query_row(
        "SELECT value FROM app_metadata WHERE key = 'tray_config'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|raw| serde_json::from_str::<TrayConfig>(&raw).ok())
    .unwrap_or_default()
}

pub fn save_tray_config(
    conn: &rusqlite::Connection,
    config: &TrayConfig,
) -> Result<(), rusqlite::Error> {
    let json = serde_json::to_string(config).unwrap_or_default();
    conn.execute(
        "INSERT INTO app_metadata (key, value) VALUES ('tray_config', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![json],
    )?;
    Ok(())
}
pub fn get_tray_state(state: State<'_, TrayState>) -> Result<TrayConfig, String> {
    Ok(TrayConfig {
        close_to_tray: state.is_close_to_tray_enabled(),
        dnd_enabled: state.is_dnd_enabled(),
    })
}
pub fn set_close_to_tray(
    app: AppHandle,
    enabled: bool,
    state: State<'_, TrayState>,
) -> Result<(), String> {
    state.set_close_to_tray(enabled);
    if let Some(db_state) = app.try_state::<crate::commands::DbState>() {
        if let Ok(conn) = db_state.conn.lock() {
            let config = TrayConfig {
                close_to_tray: enabled,
                dnd_enabled: state.is_dnd_enabled(),
            };
            if let Err(err) = save_tray_config(&conn, &config) {
                tracing::warn!(target: "tray", %err, "Failed to persist tray config to database");
            }
        }
    }
    Ok(())
}
pub fn set_tray_dnd(
    app: AppHandle,
    enabled: bool,
    state: State<'_, TrayState>,
) -> Result<(), String> {
    state.set_dnd_enabled(enabled);
    let _ = app.emit("tray-dnd-changed", enabled);
    if let Some(db_state) = app.try_state::<crate::commands::DbState>() {
        if let Ok(conn) = db_state.conn.lock() {
            let config = TrayConfig {
                close_to_tray: state.is_close_to_tray_enabled(),
                dnd_enabled: enabled,
            };
            if let Err(err) = save_tray_config(&conn, &config) {
                tracing::warn!(target: "tray", %err, "Failed to persist tray config to database");
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tray_test.rs"]
mod tray_test;
