#![recursion_limit = "256"]
//! Temporary Tauri shell for v2: wires `rustsale-core` to Tauri through ports
//! and exposes every core command as a Tauri command (generated wrappers).

use rustsale_core::host::AppHandle as CoreAppHandle;
use rustsale_core::{apply_boot_window_policy, Core, CoreConfig};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

mod host;
mod tray_ui;
pub mod wrappers;

pub use rustsale_core::{determine_autostart_action, load_dotenv_if_present, AutostartBootAction};

/// Tauri-managed handle to the started core (injected into every wrapper).
pub struct CoreHandle(CoreAppHandle);

impl CoreHandle {
    pub fn handle(&self) -> &CoreAppHandle {
        &self.0
    }
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    let tauri_host = host::TauriHost::new(app.handle().clone());
    let runtime = tauri::async_runtime::handle();
    let core = Core::start(CoreConfig { data_dir }, tauri_host.ports(), runtime.inner())?;
    tauri_host.bind_core(core.handle().clone());

    let tray_ready = match core.handle().ports().tray.init() {
        Ok(()) => true,
        Err(error) => {
            tracing::error!("Failed to initialize system tray: {:?}", error);
            false
        }
    };
    let has_autostart_arg = std::env::args().any(|arg| arg == "--autostart");
    apply_boot_window_policy(core.handle(), has_autostart_arg, tray_ready);

    app.manage(CoreHandle(core.handle().clone()));
    Ok(())
}

pub fn run() {
    load_dotenv_if_present();
    rustsale_core::install_rustls_crypto_provider();

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| setup(app))
        .invoke_handler(handler_list!())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(core) = window.app_handle().try_state::<CoreHandle>() {
                    if rustsale_core::tray::actions::should_hide_on_close(core.handle()) {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .run(tauri::generate_context!());
    if let Err(error) = result {
        tracing::error!("error while running tauri application: {error}");
        eprintln!("error while running tauri application: {error}");
    }
}
