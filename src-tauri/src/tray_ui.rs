//! System tray UI (Tauri-specific). Behaviour lives in
//! `rustsale_core::tray::actions`; this file only builds the menu and routes events.

use rustsale_core::host::AppHandle as CoreHandle;
use rustsale_core::tray::actions;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Runtime;

pub fn init_tray<R: Runtime>(app: &tauri::AppHandle<R>, core: CoreHandle) -> anyhow::Result<()> {
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

    let menu_core = core.clone();
    let click_core = core;
    let _tray = builder
        .on_menu_event(move |_app, event| match event.id().as_ref() {
            "open_app" => actions::open_app(&menu_core),
            "mark_all_read" => actions::mark_all_read(&menu_core),
            "toggle_dnd" => actions::toggle_dnd(&menu_core),
            "quit" => actions::quit(&menu_core),
            _ => {}
        })
        .on_tray_icon_event(move |_tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                actions::toggle_main_window(&click_core);
            }
        })
        .build(app)?;

    Ok(())
}
