//! `Host`: starting the core, the engine-thread pump and the events TSX hears.
//!
//! NSH services answer only on the engine thread, so the ports of
//! [`crate::ports`] post [`Request`]s here and the pump carries them out.

use crate::error::CoreApiError;
use crate::ports::{Bridge, Request, UpdaterAnswers};
use crate::runtime;
use nsh_core::JsonValue;
use nsh_core::desktop::LaunchAtLoginError;
use nsh_core::element::desktop::{
    MenuItem, launch_at_login, main_window, quit, set_launch_at_login, set_tray, tray,
};
use nsh_core::file_dialog::{FileFilter as NshFilter, OpenDialog, SaveDialog};
use nsh_core::notifications::Notification;
use nsh_core::prelude::*;
use rustsale_core::host::PortError;
use rustsale_core::host::ports::FileFilter;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;

/// Where the core started and what it listens on.
#[derive(Debug, Clone, JsObject)]
pub struct StartInfo {
    /// The data folder in use (always a v3 folder, never the live app's).
    pub data_dir: String,
    /// Port of the local HTTP API (it stays off without a token or clients file).
    pub local_api_port: f64,
    /// Ports the host could not serve, as `name`s: what runs as a no-op here.
    pub noop_ports: Vec<String>,
}

/// Starts the core and reaches it: the first call starts it, later calls answer the same.
#[module]
pub struct Host;

type EventListeners = Rc<RefCell<Vec<(u64, Callback<(String, JsonValue)>)>>>;
type UpdaterListeners = Rc<RefCell<Vec<(u64, Callback<(f64, String)>)>>>;

thread_local! {
    static EVENTS: EventListeners = Rc::new(RefCell::new(Vec::new()));
    static UPDATER: UpdaterListeners = Rc::new(RefCell::new(Vec::new()));
    static PUMPED: Cell<bool> = const { Cell::new(false) };
}

static NEXT_LISTENER: AtomicU64 = AtomicU64::new(1);

fn updater_answers() -> &'static Arc<UpdaterAnswers> {
    static ANSWERS: std::sync::OnceLock<Arc<UpdaterAnswers>> = std::sync::OnceLock::new();
    ANSWERS.get_or_init(|| Arc::new(UpdaterAnswers::default()))
}

fn port_err(error: String) -> PortError {
    PortError::Failed(error)
}

fn filters(list: &[FileFilter]) -> Vec<NshFilter> {
    list.iter()
        .filter(|f| !f.extensions.is_empty())
        .map(|f| NshFilter::new(f.name.clone(), f.extensions.clone()))
        .collect()
}

/// Carries out one request on the engine thread.
fn serve(request: Request) {
    match request {
        Request::Event { name, payload } => EVENTS.with(|listeners| {
            for (_, listener) in listeners.borrow().iter() {
                listener.post((name.clone(), payload.clone()));
            }
        }),
        Request::Notify { title, body, reply } => {
            let shown = show_notification(Notification::new("rustsale", title, body));
            drop(spawn(async move {
                let _ = reply.send(match shown.await {
                    true => Ok(()),
                    false => Err(PortError::Failed(
                        "the system did not show the notification".into(),
                    )),
                });
            }));
        }
        Request::PickFile { request, reply } => {
            let mut dialog = OpenDialog::files();
            for filter in filters(&request.filters) {
                dialog = dialog.filter(filter);
            }
            let picked = pick_paths(dialog);
            drop(spawn(async move {
                let _ = reply.send(match picked.await {
                    Ok(paths) => Ok(paths.and_then(|p| p.into_iter().next())),
                    Err(error) => Err(port_err(error)),
                });
            }));
        }
        Request::SaveFile { request, reply } => {
            let mut dialog = SaveDialog::new();
            if let Some(name) = request.default_file_name {
                dialog = dialog.file_name(name);
            }
            for filter in filters(&request.filters) {
                dialog = dialog.filter(filter);
            }
            let picked = pick_save_path(dialog);
            drop(spawn(async move {
                let _ = reply.send(picked.await.map_err(port_err));
            }));
        }
        Request::OpenUrl { url, reply } => {
            let opened = open_url(url);
            drop(spawn(async move {
                let _ = reply.send(match opened.await {
                    true => Ok(()),
                    false => Err(PortError::Failed("no app opened the URL".into())),
                });
            }));
        }
        Request::RevealPath { path, reply } => {
            let shown = reveal_path(path);
            drop(spawn(async move {
                let _ = reply.send(shown.await.map_err(port_err));
            }));
        }
        Request::TrayInit { reply } => {
            set_tray(tray().tooltip("RustSale").menu([
                MenuItem::new("Mở RustSale").on_select(|| main_window().show()),
                MenuItem::separator(),
                MenuItem::new("Thoát").on_select(quit),
            ]));
            let _ = reply.send(Ok(()));
        }
        Request::MainVisible { reply } => {
            let visible = main_window().state().is_none_or(|state| state.visible);
            let _ = reply.send(Ok(visible));
        }
        Request::ShowMain { reply } => {
            let window = main_window();
            window.show();
            window.focus();
            let _ = reply.send(Ok(()));
        }
        Request::HideMain { reply } => {
            main_window().hide();
            let _ = reply.send(Ok(()));
        }
        Request::Exit => quit(),
        Request::AutostartGet { reply } => {
            let enabled = launch_at_login();
            drop(spawn(async move {
                let _ = reply.send(Ok(enabled.await));
            }));
        }
        Request::AutostartSet { enabled, reply } => {
            let done = set_launch_at_login(enabled);
            drop(spawn(async move {
                let _ = reply.send(match done.await {
                    Ok(()) => Ok(()),
                    Err(LaunchAtLoginError::Unavailable) => Err(PortError::Unsupported(
                        "launch at login (not in a packaged app)",
                    )),
                    Err(error) => Err(PortError::Failed(format!("{error:?}"))),
                });
            }));
        }
        Request::Updater { id, op } => UPDATER.with(|listeners| {
            let listeners = listeners.borrow();
            if listeners.is_empty() {
                updater_answers().answer(id, Err("no nsh/updater bridge is listening".into()));
            }
            for (_, listener) in listeners.iter() {
                listener.post((id as f64, op.to_owned()));
            }
        }),
    }
}

/// The pump: the only reader of the ports' requests. Runs on the engine thread.
async fn pump(mut requests: mpsc::UnboundedReceiver<Request>) {
    while let Some(request) = requests.recv().await {
        serve(request);
    }
}

/// Ports with no NSH 0.6.1 service behind them, by name.
fn noop_ports() -> Vec<String> {
    // Every port has a real NSH service or a TSX bridge; the updater serves
    // only while a TSX listener (`onUpdaterRequest`) is registered, and the
    // tray, notifications and launch at login need a desktop app.
    vec![]
}

#[module]
impl Host {
    /// Starts the core on `dataDir` (default: the v3-dev folder beside the live app's, which is never
    /// used) and resolves once it runs. Calling it again resolves with the same answer. Rejects
    /// with `invalid_request` for the live app's folder or when the Zalo webhook is switched on.
    pub async fn start(data_dir: Option<String>) -> Result<StartInfo, CoreApiError> {
        let dir = match data_dir.filter(|d| !d.trim().is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => runtime::default_data_dir()?,
        };
        runtime::check_data_dir(&dir)?;
        if !PUMPED.with(Cell::get) {
            let (tx, rx) = mpsc::unbounded_channel();
            BRIDGE.with(|b| *b.borrow_mut() = Some(Bridge::new(tx, updater_answers().clone())));
            drop(spawn(pump(rx)));
            PUMPED.with(|p| p.set(true));
        }
        let bridge = BRIDGE
            .with(|b| b.borrow().clone())
            .ok_or_else(|| CoreApiError::internal("the pump is not running"))?;
        let port = runtime::DEV_LOCAL_API_PORT;
        let running = blocking(move || runtime::boot(&bridge, dir, port))
            .await
            .map_err(|e| CoreApiError::internal(format!("background task failed: {e}")))??;
        Ok(StartInfo {
            data_dir: running.data_dir().display().to_string(),
            local_api_port: f64::from(port),
            noop_ports: noop_ports(),
        })
    }

    /// Calls `listener` with the name and payload of every event the core emits
    /// (`conversation-updated`, `scrape-job-event`, ...). Stop with the returned function.
    pub fn on_event(#[args(name, payload)] listener: Callback<(String, JsonValue)>) -> Stop {
        let id = NEXT_LISTENER.fetch_add(1, Ordering::Relaxed);
        EVENTS.with(|l| l.borrow_mut().push((id, listener)));
        Stop::new(move || EVENTS.with(|l| l.borrow_mut().retain(|(i, _)| *i != id)))
    }

    /// Calls `listener` with `(id, op)` when the core wants an update step done through
    /// `nsh/updater`: `op` is `"check"` (answer with the update or `null`), `"download"`
    /// (`downloadUpdate`) or `"restart"` (`installAndRestart`). Answer each with `answerUpdater`.
    pub fn on_updater_request(#[args(id, op)] listener: Callback<(f64, String)>) -> Stop {
        let id = NEXT_LISTENER.fetch_add(1, Ordering::Relaxed);
        UPDATER.with(|l| l.borrow_mut().push((id, listener)));
        updater_answers().set_listening(true);
        Stop::new(move || {
            UPDATER.with(|l| {
                let mut listeners = l.borrow_mut();
                listeners.retain(|(i, _)| *i != id);
                updater_answers().set_listening(!listeners.is_empty());
            })
        })
    }

    /// Answers updater request `id`: `error` set rejects it, else `update` is what
    /// `checkForUpdate` found (`{ version, notes }`) or `null`. Resolves with whether the core still waited.
    pub fn answer_updater(id: f64, update: Option<JsonValue>, error: Option<String>) -> bool {
        let result = match error {
            Some(message) => Err(message),
            None => Ok(update.unwrap_or(JsonValue::Null)),
        };
        updater_answers().answer(id as u64, result)
    }
}

thread_local! {
    static BRIDGE: RefCell<Option<Bridge>> = const { RefCell::new(None) };
}
