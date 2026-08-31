use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime};

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

pub fn init_tray<R: Runtime>(app: &AppHandle<R>) -> anyhow::Result<()> {
    let open_i = MenuItem::with_id(app, "open_app", "Mở RustSale", true, None::<&str>)?;
    let mark_read_i = MenuItem::with_id(
        app,
        "mark_all_read",
        "Đánh dấu tất cả đã đọc",
        true,
        None::<&str>,
    )?;
    let dnd_i = MenuItem::with_id(
        app,
        "toggle_dnd",
        "Bật/Tắt Không làm phiền (DND)",
        true,
        None::<&str>,
    )?;
    let quit_i = MenuItem::with_id(app, "quit", "Thoát ứng dụng", true, None::<&str>)?;
    let separator_1 = PredefinedMenuItem::separator(app)?;
    let separator_2 = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &open_i,
            &separator_1,
            &mark_read_i,
            &dnd_i,
            &separator_2,
            &quit_i,
        ],
    )?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("RustSale CRM");

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    let _tray = builder
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open_app" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                }
            }
            "mark_all_read" => {
                let _ = app.emit("tray-mark-all-read", ());
            }
            "toggle_dnd" => {
                if let Some(state) = app.try_state::<TrayState>() {
                    let new_val = !state.is_dnd_enabled();
                    state.set_dnd_enabled(new_val);
                    let _ = app.emit("tray-dnd-changed", new_val);
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

#[tauri::command]
pub fn get_tray_state(state: tauri::State<'_, TrayState>) -> Result<TrayConfig, String> {
    Ok(TrayConfig {
        close_to_tray: state.is_close_to_tray_enabled(),
        dnd_enabled: state.is_dnd_enabled(),
    })
}

#[tauri::command]
pub fn set_close_to_tray(enabled: bool, state: tauri::State<'_, TrayState>) -> Result<(), String> {
    state.set_close_to_tray(enabled);
    Ok(())
}

#[tauri::command]
pub fn set_tray_dnd<R: Runtime>(
    app: AppHandle<R>,
    enabled: bool,
    state: tauri::State<'_, TrayState>,
) -> Result<(), String> {
    state.set_dnd_enabled(enabled);
    let _ = app.emit("tray-dnd-changed", enabled);
    Ok(())
}

#[cfg(test)]
#[path = "tray_test.rs"]
mod tray_test;
