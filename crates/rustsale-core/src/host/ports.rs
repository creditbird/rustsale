//! Ports: everything a desktop shell (Tauri, NSH, tests) provides to the core.
//!
//! The core never talks to a UI framework. It calls these traits; the shell
//! crate implements them. All traits are object-safe and `Send + Sync` so one
//! [`Ports`] bundle can be shared between tasks.

use super::error::PortError;
use async_trait::async_trait;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Pushes named events (JSON payloads) from the core to the UI.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value) -> Result<(), PortError>;
}

/// Shows a desktop (OS level) notification.
pub trait DesktopNotifier: Send + Sync {
    fn show(&self, title: &str, body: &str) -> Result<(), PortError>;
}

/// A named group of file extensions for a dialog filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileFilter {
    pub fn new(name: &str, extensions: &[&str]) -> Self {
        Self {
            name: name.to_string(),
            extensions: extensions.iter().map(|ext| (*ext).to_string()).collect(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickFileRequest {
    pub filters: Vec<FileFilter>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveFileRequest {
    pub default_file_name: Option<String>,
    pub filters: Vec<FileFilter>,
}

/// Native file dialogs. Both calls block the calling thread until the user
/// answers; the core always calls them from `spawn_blocking`.
pub trait FileDialog: Send + Sync {
    fn pick_file(&self, request: &PickFileRequest) -> Result<Option<PathBuf>, PortError>;
    fn save_file(&self, request: &SaveFileRequest) -> Result<Option<PathBuf>, PortError>;
}

/// The main application window.
pub trait WindowPort: Send + Sync {
    /// True when the shell has a main window at all.
    fn has_main_window(&self) -> bool;
    fn is_main_visible(&self) -> bool;
    /// Show, un-minimize and focus the main window.
    fn show_main(&self) -> Result<(), PortError>;
    fn hide_main(&self) -> Result<(), PortError>;
    /// Ask the shell to terminate the process.
    fn exit(&self, code: i32);
}

/// System tray icon owned by the shell.
pub trait TrayPort: Send + Sync {
    /// Create the tray icon and its menu. An error means the tray is unusable.
    fn init(&self) -> Result<(), PortError>;
}

/// Open URLs and reveal files with the OS default handler.
pub trait OpenerPort: Send + Sync {
    fn open_url(&self, url: &str) -> Result<(), PortError>;
    fn reveal_path(&self, path: &Path) -> Result<(), PortError>;
}

/// Metadata of an available application update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Application self-update.
#[async_trait]
pub trait UpdaterPort: Send + Sync {
    async fn check(&self) -> Result<Option<UpdateInfo>, PortError>;
    async fn download_and_install(&self) -> Result<(), PortError>;
    fn restart(&self);
}

/// Launch-at-login registration.
pub trait AutostartPort: Send + Sync {
    fn is_enabled(&self) -> Result<bool, PortError>;
    fn set_enabled(&self, enabled: bool) -> Result<(), PortError>;
}

/// Every port the core may use, bundled. Clone is cheap (`Arc`s).
#[derive(Clone)]
pub struct Ports {
    pub events: Arc<dyn EventSink>,
    pub notifier: Arc<dyn DesktopNotifier>,
    pub dialog: Arc<dyn FileDialog>,
    pub window: Arc<dyn WindowPort>,
    pub tray: Arc<dyn TrayPort>,
    pub opener: Arc<dyn OpenerPort>,
    pub updater: Arc<dyn UpdaterPort>,
    pub autostart: Arc<dyn AutostartPort>,
}

/// Port implementation that supports nothing: events are dropped, dialogs
/// return "cancelled", every other capability reports `Unsupported`.
/// Used by headless runs and as a base for tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopPorts;

impl EventSink for NoopPorts {
    fn emit(&self, _event: &str, _payload: Value) -> Result<(), PortError> {
        Ok(())
    }
}

impl DesktopNotifier for NoopPorts {
    fn show(&self, _title: &str, _body: &str) -> Result<(), PortError> {
        Err(PortError::Unsupported("desktop notification"))
    }
}

impl FileDialog for NoopPorts {
    fn pick_file(&self, _request: &PickFileRequest) -> Result<Option<PathBuf>, PortError> {
        Ok(None)
    }
    fn save_file(&self, _request: &SaveFileRequest) -> Result<Option<PathBuf>, PortError> {
        Ok(None)
    }
}

impl WindowPort for NoopPorts {
    fn has_main_window(&self) -> bool {
        false
    }
    fn is_main_visible(&self) -> bool {
        false
    }
    fn show_main(&self) -> Result<(), PortError> {
        Err(PortError::Unsupported("main window"))
    }
    fn hide_main(&self) -> Result<(), PortError> {
        Err(PortError::Unsupported("main window"))
    }
    fn exit(&self, _code: i32) {}
}

impl TrayPort for NoopPorts {
    fn init(&self) -> Result<(), PortError> {
        Err(PortError::Unsupported("system tray"))
    }
}

impl OpenerPort for NoopPorts {
    fn open_url(&self, _url: &str) -> Result<(), PortError> {
        Err(PortError::Unsupported("opener"))
    }
    fn reveal_path(&self, _path: &Path) -> Result<(), PortError> {
        Err(PortError::Unsupported("opener"))
    }
}

#[async_trait]
impl UpdaterPort for NoopPorts {
    async fn check(&self) -> Result<Option<UpdateInfo>, PortError> {
        Ok(None)
    }
    async fn download_and_install(&self) -> Result<(), PortError> {
        Err(PortError::Unsupported("updater"))
    }
    fn restart(&self) {}
}

impl AutostartPort for NoopPorts {
    fn is_enabled(&self) -> Result<bool, PortError> {
        Ok(false)
    }
    fn set_enabled(&self, _enabled: bool) -> Result<(), PortError> {
        Err(PortError::Unsupported("autostart"))
    }
}

impl Ports {
    /// All ports are [`NoopPorts`].
    pub fn noop() -> Self {
        let noop = Arc::new(NoopPorts);
        Self {
            events: noop.clone(),
            notifier: noop.clone(),
            dialog: noop.clone(),
            window: noop.clone(),
            tray: noop.clone(),
            opener: noop.clone(),
            updater: noop.clone(),
            autostart: noop,
        }
    }
}
