//! Tauri implementations of the `rustsale-core` ports (temporary adapter: v2
//! keeps running on Tauri until the NSH shell replaces it).

use async_trait::async_trait;
use rustsale_core::host::{
    AppHandle as CoreHandle, AutostartPort, DesktopNotifier, EventSink, FileDialog, FileFilter,
    OpenerPort, PickFileRequest, PortError, Ports, SaveFileRequest, TrayPort, UpdateInfo,
    UpdaterPort, WindowPort,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use tauri::{Emitter, Manager, Wry};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_notification::NotificationExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::UpdaterExt;

const MAIN_WINDOW: &str = "main";

fn failed(error: impl std::fmt::Display) -> PortError {
    PortError::failed(error.to_string())
}

/// One object implementing every port on top of a Tauri `AppHandle`.
pub struct TauriHost {
    app: tauri::AppHandle<Wry>,
    /// Core handle, bound after `Core::start` (the tray menu calls core actions).
    core: OnceLock<CoreHandle>,
    pending_update: Mutex<Option<tauri_plugin_updater::Update>>,
}

impl TauriHost {
    pub fn new(app: tauri::AppHandle<Wry>) -> Arc<Self> {
        Arc::new(Self {
            app,
            core: OnceLock::new(),
            pending_update: Mutex::new(None),
        })
    }

    pub fn ports(self: &Arc<Self>) -> Ports {
        Ports {
            events: self.clone(),
            notifier: self.clone(),
            dialog: self.clone(),
            window: self.clone(),
            tray: self.clone(),
            opener: self.clone(),
            updater: self.clone(),
            autostart: self.clone(),
        }
    }

    /// Give the tray access to the started core.
    pub fn bind_core(&self, core: CoreHandle) {
        let _ = self.core.set(core);
    }

    fn core(&self) -> Option<&CoreHandle> {
        self.core.get()
    }
}

impl EventSink for TauriHost {
    fn emit(&self, event: &str, payload: Value) -> Result<(), PortError> {
        self.app.emit(event, payload).map_err(failed)
    }
}

impl DesktopNotifier for TauriHost {
    fn show(&self, title: &str, body: &str) -> Result<(), PortError> {
        self.app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(failed)
    }
}

fn apply_filters<R: tauri::Runtime>(
    mut builder: tauri_plugin_dialog::FileDialogBuilder<R>,
    filters: &[FileFilter],
) -> tauri_plugin_dialog::FileDialogBuilder<R> {
    for filter in filters {
        let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
        builder = builder.add_filter(&filter.name, &extensions);
    }
    builder
}

fn into_path(picked: Option<tauri_plugin_dialog::FilePath>) -> Result<Option<PathBuf>, PortError> {
    match picked {
        Some(path) => path.into_path().map(Some).map_err(failed),
        None => Ok(None),
    }
}

impl FileDialog for TauriHost {
    fn pick_file(&self, request: &PickFileRequest) -> Result<Option<PathBuf>, PortError> {
        let builder = apply_filters(self.app.dialog().file(), &request.filters);
        into_path(builder.blocking_pick_file())
    }

    fn save_file(&self, request: &SaveFileRequest) -> Result<Option<PathBuf>, PortError> {
        let mut builder = self.app.dialog().file();
        if let Some(name) = request.default_file_name.as_deref() {
            if !name.trim().is_empty() {
                builder = builder.set_file_name(name);
            }
        }
        let builder = apply_filters(builder, &request.filters);
        into_path(builder.blocking_save_file())
    }
}

impl WindowPort for TauriHost {
    fn has_main_window(&self) -> bool {
        self.app.get_webview_window(MAIN_WINDOW).is_some()
    }

    fn is_main_visible(&self) -> bool {
        self.app
            .get_webview_window(MAIN_WINDOW)
            .is_some_and(|window| window.is_visible().unwrap_or(false))
    }

    fn show_main(&self) -> Result<(), PortError> {
        let window = self
            .app
            .get_webview_window(MAIN_WINDOW)
            .ok_or(PortError::Unsupported("main window"))?;
        window.show().map_err(failed)?;
        let _ = window.unminimize();
        let _ = window.set_focus();
        Ok(())
    }

    fn hide_main(&self) -> Result<(), PortError> {
        let window = self
            .app
            .get_webview_window(MAIN_WINDOW)
            .ok_or(PortError::Unsupported("main window"))?;
        window.hide().map_err(failed)
    }

    fn exit(&self, code: i32) {
        self.app.exit(code);
    }
}

impl TrayPort for TauriHost {
    fn init(&self) -> Result<(), PortError> {
        let core = self
            .core()
            .cloned()
            .ok_or(PortError::Unsupported("tray before core start"))?;
        crate::tray_ui::init_tray(&self.app, core).map_err(failed)
    }
}

impl OpenerPort for TauriHost {
    fn open_url(&self, url: &str) -> Result<(), PortError> {
        self.app
            .opener()
            .open_url(url, None::<&str>)
            .map_err(failed)
    }

    fn reveal_path(&self, path: &Path) -> Result<(), PortError> {
        self.app.opener().reveal_item_in_dir(path).map_err(failed)
    }
}

#[async_trait]
impl UpdaterPort for TauriHost {
    async fn check(&self) -> Result<Option<UpdateInfo>, PortError> {
        let updater = self.app.updater().map_err(failed)?;
        let update = updater.check().await.map_err(failed)?;
        let info = update.as_ref().map(|u| UpdateInfo {
            version: u.version.clone(),
            notes: u.body.clone(),
        });
        *self
            .pending_update
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = update;
        Ok(info)
    }

    async fn download_and_install(&self) -> Result<(), PortError> {
        let update = self
            .pending_update
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or_else(|| PortError::failed("no update checked"))?;
        update
            .download_and_install(|_, _| {}, || {})
            .await
            .map_err(failed)
    }

    fn restart(&self) {
        self.app.restart();
    }
}

impl AutostartPort for TauriHost {
    fn is_enabled(&self) -> Result<bool, PortError> {
        self.app.autolaunch().is_enabled().map_err(failed)
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        let launcher = self.app.autolaunch();
        if enabled {
            launcher.enable().map_err(failed)
        } else {
            launcher.disable().map_err(failed)
        }
    }
}
