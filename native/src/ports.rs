//! The core's ports on NSH 0.6.1.
//!
//! NSH's platform services (notifications, dialogs, tray, window, launch at
//! login, open URL) answer only on the engine thread, in the reactive
//! context. The core calls its ports from tokio and blocking threads. So
//! every port here is a thin sender: it posts a [`Request`] to the **pump**,
//! a task on the engine thread (see [`crate::host`]), and blocking ports wait
//! for the answer. The pump is the only code that touches NSH services.
//!
//! The updater has no Rust-callable face in NSH 0.6.1: `nsh/updater` exists
//! for TSX only. The updater port therefore asks TSX, which calls
//! `checkForUpdate`, `downloadUpdate` and `installAndRestart` of
//! `nsh/updater` (the API of the packaged native build) and answers back;
//! without a listener it reports `Unsupported`.

use async_trait::async_trait;
use rustsale_core::host::PortError;
use rustsale_core::host::ports::{
    AutostartPort, DesktopNotifier, EventSink, FileDialog, OpenerPort, PickFileRequest, Ports,
    SaveFileRequest, TrayPort, UpdateInfo, UpdaterPort, WindowPort,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};

/// How long the core waits for TSX to answer an updater request.
const UPDATER_CHECK_TIMEOUT: Duration = Duration::from_secs(60);
const UPDATER_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

type Reply<T> = std::sync::mpsc::Sender<Result<T, PortError>>;

/// What a port asks the pump to do on the engine thread.
pub(crate) enum Request {
    Event {
        name: String,
        payload: Value,
    },
    Notify {
        title: String,
        body: String,
        reply: Reply<()>,
    },
    PickFile {
        request: PickFileRequest,
        reply: Reply<Option<PathBuf>>,
    },
    SaveFile {
        request: SaveFileRequest,
        reply: Reply<Option<PathBuf>>,
    },
    OpenUrl {
        url: String,
        reply: Reply<()>,
    },
    RevealPath {
        path: PathBuf,
        reply: Reply<()>,
    },
    TrayInit {
        reply: Reply<()>,
    },
    MainVisible {
        reply: Reply<bool>,
    },
    ShowMain {
        reply: Reply<()>,
    },
    HideMain {
        reply: Reply<()>,
    },
    Exit,
    AutostartGet {
        reply: Reply<bool>,
    },
    AutostartSet {
        enabled: bool,
        reply: Reply<()>,
    },
    /// TSX performs an `nsh/updater` step; `id` pairs the answer.
    Updater {
        id: u64,
        op: &'static str,
    },
}

/// The sending half the ports share.
#[derive(Clone)]
pub(crate) struct Bridge {
    tx: mpsc::UnboundedSender<Request>,
    updater: Arc<UpdaterAnswers>,
}

impl Bridge {
    pub(crate) fn new(tx: mpsc::UnboundedSender<Request>, updater: Arc<UpdaterAnswers>) -> Self {
        Self { tx, updater }
    }

    fn send(&self, request: Request) -> Result<(), PortError> {
        self.tx
            .send(request)
            .map_err(|_| PortError::Failed("the engine is gone".into()))
    }

    /// Sends a request that carries a reply channel and waits for it. Only
    /// blocking threads call this, never the engine thread.
    fn ask<T>(&self, make: impl FnOnce(Reply<T>) -> Request) -> Result<T, PortError> {
        let (reply, answer) = std::sync::mpsc::channel();
        self.send(make(reply))?;
        answer
            .recv()
            .map_err(|_| PortError::Failed("the engine did not answer".into()))?
    }
}

/// Whether a listener of TSX can serve updater requests, and the answers
/// they owe.
#[derive(Default)]
pub(crate) struct UpdaterAnswers {
    next: std::sync::atomic::AtomicU64,
    waiting:
        std::sync::Mutex<std::collections::HashMap<u64, oneshot::Sender<Result<Value, String>>>>,
    listening: AtomicBool,
}

impl UpdaterAnswers {
    pub(crate) fn set_listening(&self, on: bool) {
        self.listening.store(on, Ordering::SeqCst);
    }

    pub(crate) fn is_listening(&self) -> bool {
        self.listening.load(Ordering::SeqCst)
    }

    fn register(&self) -> (u64, oneshot::Receiver<Result<Value, String>>) {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let (tx, rx) = oneshot::channel();
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.insert(id, tx);
        }
        (id, rx)
    }

    fn forget(&self, id: u64) {
        if let Ok(mut waiting) = self.waiting.lock() {
            waiting.remove(&id);
        }
    }

    /// TSX answers request `id`. `false` when nobody waits for it any more.
    pub(crate) fn answer(&self, id: u64, result: Result<Value, String>) -> bool {
        let sender = self
            .waiting
            .lock()
            .ok()
            .and_then(|mut waiting| waiting.remove(&id));
        sender.is_some_and(|sender| sender.send(result).is_ok())
    }
}

pub(crate) struct NshEvents(pub(crate) Bridge);
pub(crate) struct NshNotifier(pub(crate) Bridge);
pub(crate) struct NshDialog(pub(crate) Bridge);
pub(crate) struct NshWindow(pub(crate) Bridge);
pub(crate) struct NshTray(pub(crate) Bridge);
pub(crate) struct NshOpener(pub(crate) Bridge);
pub(crate) struct NshUpdater(pub(crate) Bridge);
pub(crate) struct NshAutostart(pub(crate) Bridge);

impl EventSink for NshEvents {
    fn emit(&self, event: &str, payload: Value) -> Result<(), PortError> {
        self.0.send(Request::Event {
            name: event.to_owned(),
            payload,
        })
    }
}

impl DesktopNotifier for NshNotifier {
    fn show(&self, title: &str, body: &str) -> Result<(), PortError> {
        self.0.ask(|reply| Request::Notify {
            title: title.to_owned(),
            body: body.to_owned(),
            reply,
        })
    }
}

impl FileDialog for NshDialog {
    fn pick_file(&self, request: &PickFileRequest) -> Result<Option<PathBuf>, PortError> {
        self.0.ask(|reply| Request::PickFile {
            request: request.clone(),
            reply,
        })
    }

    fn save_file(&self, request: &SaveFileRequest) -> Result<Option<PathBuf>, PortError> {
        self.0.ask(|reply| Request::SaveFile {
            request: request.clone(),
            reply,
        })
    }
}

impl WindowPort for NshWindow {
    fn has_main_window(&self) -> bool {
        true
    }

    fn is_main_visible(&self) -> bool {
        self.0
            .ask(|reply| Request::MainVisible { reply })
            .unwrap_or(true)
    }

    fn show_main(&self) -> Result<(), PortError> {
        self.0.ask(|reply| Request::ShowMain { reply })
    }

    fn hide_main(&self) -> Result<(), PortError> {
        self.0.ask(|reply| Request::HideMain { reply })
    }

    fn exit(&self, _code: i32) {
        // NSH's `quit` has no exit code; a failed send means the engine is gone.
        let _ = self.0.send(Request::Exit);
    }
}

impl TrayPort for NshTray {
    fn init(&self) -> Result<(), PortError> {
        self.0.ask(|reply| Request::TrayInit { reply })
    }
}

impl OpenerPort for NshOpener {
    fn open_url(&self, url: &str) -> Result<(), PortError> {
        self.0.ask(|reply| Request::OpenUrl {
            url: url.to_owned(),
            reply,
        })
    }

    fn reveal_path(&self, path: &Path) -> Result<(), PortError> {
        self.0.ask(|reply| Request::RevealPath {
            path: path.to_path_buf(),
            reply,
        })
    }
}

impl AutostartPort for NshAutostart {
    fn is_enabled(&self) -> Result<bool, PortError> {
        self.0.ask(|reply| Request::AutostartGet { reply })
    }

    fn set_enabled(&self, enabled: bool) -> Result<(), PortError> {
        self.0.ask(|reply| Request::AutostartSet { enabled, reply })
    }
}

impl NshUpdater {
    async fn step(&self, op: &'static str, limit: Duration) -> Result<Value, PortError> {
        let answers = &self.0.updater;
        if !answers.is_listening() {
            return Err(PortError::Unsupported(
                "updater (no nsh/updater bridge in TSX)",
            ));
        }
        let (id, answer) = answers.register();
        if let Err(error) = self.0.send(Request::Updater { id, op }) {
            answers.forget(id);
            return Err(error);
        }
        match tokio::time::timeout(limit, answer).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(message))) => Err(PortError::Failed(message)),
            Ok(Err(_)) => Err(PortError::Failed("the updater bridge was closed".into())),
            Err(_) => {
                answers.forget(id);
                Err(PortError::Failed(format!("updater step `{op}` timed out")))
            }
        }
    }
}

#[async_trait]
impl UpdaterPort for NshUpdater {
    async fn check(&self) -> Result<Option<UpdateInfo>, PortError> {
        let value = self.step("check", UPDATER_CHECK_TIMEOUT).await?;
        Ok(parse_update(&value))
    }

    async fn download_and_install(&self) -> Result<(), PortError> {
        self.step("download", UPDATER_DOWNLOAD_TIMEOUT)
            .await
            .map(|_| ())
    }

    fn restart(&self) {
        // Fire and forget: `installAndRestart` quits this run when it works.
        let answers = &self.0.updater;
        if answers.is_listening() {
            let (id, _answer) = answers.register();
            if self.0.send(Request::Updater { id, op: "restart" }).is_err() {
                answers.forget(id);
            }
        }
    }
}

/// `{ "version": "1.2.0", "notes": "..." }` from `checkForUpdate`, or `null`.
fn parse_update(value: &Value) -> Option<UpdateInfo> {
    let version = value.get("version")?.as_str()?.to_owned();
    let notes = value
        .get("notes")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some(UpdateInfo { version, notes })
}

/// The ports the core runs on: every one forwards to the pump.
pub(crate) fn nsh_ports(bridge: &Bridge) -> Ports {
    Ports {
        events: Arc::new(NshEvents(bridge.clone())),
        notifier: Arc::new(NshNotifier(bridge.clone())),
        dialog: Arc::new(NshDialog(bridge.clone())),
        window: Arc::new(NshWindow(bridge.clone())),
        tray: Arc::new(NshTray(bridge.clone())),
        opener: Arc::new(NshOpener(bridge.clone())),
        updater: Arc::new(NshUpdater(bridge.clone())),
        autostart: Arc::new(NshAutostart(bridge.clone())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bridge() -> (Bridge, mpsc::UnboundedReceiver<Request>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Bridge::new(tx, Arc::new(UpdaterAnswers::default())), rx)
    }

    #[test]
    fn events_are_forwarded_to_the_pump() {
        let (bridge, mut rx) = bridge();
        let ports = nsh_ports(&bridge);
        assert!(ports.events.emit("lead-updated", json!({"id": 1})).is_ok());
        match rx.try_recv() {
            Ok(Request::Event { name, payload }) => {
                assert_eq!(name, "lead-updated");
                assert_eq!(payload["id"], 1);
            }
            _ => panic!("expected an event request"),
        }
    }

    #[test]
    fn blocking_port_waits_for_the_pump_answer() {
        let (bridge, mut rx) = bridge();
        let ports = nsh_ports(&bridge);
        let pump = std::thread::spawn(move || {
            if let Some(Request::PickFile { reply, .. }) = rx.blocking_recv() {
                let _ = reply.send(Ok(Some(PathBuf::from("/tmp/a.csv"))));
            }
        });
        let picked = ports.dialog.pick_file(&PickFileRequest::default());
        assert!(matches!(picked, Ok(Some(path)) if path == Path::new("/tmp/a.csv")));
        assert!(pump.join().is_ok());
    }

    #[test]
    fn dead_engine_is_an_error_not_a_hang() {
        let (bridge, rx) = bridge();
        drop(rx);
        let ports = nsh_ports(&bridge);
        assert!(ports.notifier.show("t", "b").is_err());
        assert!(ports.autostart.is_enabled().is_err());
    }

    #[test]
    fn pump_that_drops_the_reply_does_not_hang_the_caller() {
        let (bridge, mut rx) = bridge();
        let ports = nsh_ports(&bridge);
        let pump = std::thread::spawn(move || drop(rx.blocking_recv()));
        assert!(ports.opener.open_url("https://example.com").is_err());
        assert!(pump.join().is_ok());
    }

    #[tokio::test]
    async fn updater_without_a_tsx_bridge_is_unsupported() {
        let (bridge, _rx) = bridge();
        let ports = nsh_ports(&bridge);
        assert!(matches!(
            ports.updater.check().await,
            Err(PortError::Unsupported(_))
        ));
    }

    #[tokio::test]
    async fn updater_check_round_trips_through_the_listener() {
        let (bridge, mut rx) = bridge();
        bridge.updater.set_listening(true);
        let answers = bridge.updater.clone();
        let ports = nsh_ports(&bridge);
        let tsx = tokio::spawn(async move {
            if let Some(Request::Updater { id, op }) = rx.recv().await {
                assert_eq!(op, "check");
                answers.answer(id, Ok(json!({"version": "9.9.9", "notes": "n"})));
            }
        });
        let update = ports.updater.check().await;
        assert!(
            matches!(update, Ok(Some(info)) if info.version == "9.9.9" && info.notes.as_deref() == Some("n"))
        );
        assert!(tsx.await.is_ok());
    }

    #[tokio::test]
    async fn updater_null_means_up_to_date() {
        let (bridge, mut rx) = bridge();
        bridge.updater.set_listening(true);
        let answers = bridge.updater.clone();
        let ports = nsh_ports(&bridge);
        tokio::spawn(async move {
            if let Some(Request::Updater { id, .. }) = rx.recv().await {
                answers.answer(id, Ok(Value::Null));
            }
        });
        assert!(matches!(ports.updater.check().await, Ok(None)));
    }

    #[tokio::test]
    async fn updater_rejection_becomes_a_port_failure() {
        let (bridge, mut rx) = bridge();
        bridge.updater.set_listening(true);
        let answers = bridge.updater.clone();
        let ports = nsh_ports(&bridge);
        tokio::spawn(async move {
            if let Some(Request::Updater { id, .. }) = rx.recv().await {
                answers.answer(id, Err("bad signature".into()));
            }
        });
        assert!(
            matches!(ports.updater.download_and_install().await, Err(PortError::Failed(m)) if m == "bad signature")
        );
    }
}
