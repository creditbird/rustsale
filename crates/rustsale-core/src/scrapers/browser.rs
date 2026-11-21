use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinHandle;

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(45);
const RUNTIME_INSTALL_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const BROWSER_LAUNCH_TIMEOUT: Duration = Duration::from_secs(120);
/// How long the sidecar may take to answer `shutdown` (it closes the browser
/// context and lets Chromium flush the profile first).
const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(25);
/// How long the sidecar process gets to exit after stdin closes, and again
/// after SIGTERM, before it is killed (a SIGKILL strands Chromium, C1).
const DEFAULT_EXIT_GRACE: Duration = Duration::from_secs(5);

type BrowserResult<T> = Result<T, ScrapeJobError>;
type PendingResponse = oneshot::Sender<BrowserResult<Value>>;
type BoxWriter = Pin<Box<dyn AsyncWrite + Send>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoogleMapsPageState {
    Ready,
    EmptyResults,
    Captcha,
    ConsentRequired,
    RateLimited,
    BlankPage,
    SelectorChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleMapsPageHealth {
    pub state: GoogleMapsPageState,
    pub url: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    #[serde(default)]
    pub installed: bool,
    pub version: Option<String>,
    pub bundled_version: Option<String>,
    pub tier: Option<String>,
    pub platform: Option<String>,
    pub binary_path: Option<String>,
    pub cache_dir: Option<String>,
    pub download_url: Option<String>,
    pub wrapper_version: Option<String>,
}

impl RuntimeStatus {
    #[cfg(test)]
    pub(crate) fn installed_for_test() -> Self {
        Self {
            installed: true,
            version: Some("146.0.7680.177.5".to_string()),
            bundled_version: Some("146.0.7680.177.5".to_string()),
            tier: Some("free".to_string()),
            platform: Some("test".to_string()),
            binary_path: None,
            cache_dir: None,
            download_url: None,
            wrapper_version: Some("0.5.1".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequest {
    pub profile_dir: PathBuf,
    pub headless: bool,
    pub start_url: Option<String>,
}

#[async_trait]
pub trait BrowserAutomation: Send + Sync {
    async fn runtime_status(&self) -> BrowserResult<RuntimeStatus>;
    async fn ensure_runtime(&self) -> BrowserResult<RuntimeStatus>;
    async fn launch(&self, request: LaunchRequest) -> BrowserResult<()>;
    async fn navigate(&self, url: &str, timeout: Duration) -> BrowserResult<()>;
    async fn evaluate(&self, script: &str, timeout: Duration) -> BrowserResult<Value>;
    async fn screenshot(&self, path: &Path) -> BrowserResult<()>;
    async fn health_check(&self) -> BrowserResult<GoogleMapsPageState>;
    async fn health_snapshot(&self) -> BrowserResult<GoogleMapsPageHealth> {
        Ok(GoogleMapsPageHealth {
            state: self.health_check().await?,
            url: String::new(),
            title: String::new(),
        })
    }
    async fn shutdown(&self) -> BrowserResult<()>;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SupervisorTimeouts {
    pub request: Duration,
    pub shutdown: Duration,
    pub exit_grace: Duration,
}

impl Default for SupervisorTimeouts {
    fn default() -> Self {
        Self {
            request: DEFAULT_REQUEST_TIMEOUT,
            shutdown: DEFAULT_SHUTDOWN_TIMEOUT,
            exit_grace: DEFAULT_EXIT_GRACE,
        }
    }
}

#[async_trait]
pub(crate) trait ProcessControl: Send + Sync {
    fn pid(&self) -> Option<u32>;
    async fn wait(&self) -> io::Result<()>;
    /// Asks the process to exit (SIGTERM) so it can run its own cleanup.
    async fn terminate(&self) -> io::Result<()>;
    async fn kill(&self) -> io::Result<()>;
}

struct TokioProcessControl {
    pid: u32,
    child: Mutex<Option<Child>>,
}

#[async_trait]
impl ProcessControl for TokioProcessControl {
    fn pid(&self) -> Option<u32> {
        Some(self.pid)
    }

    async fn wait(&self) -> io::Result<()> {
        let mut child = self.child.lock().await;
        if let Some(process) = child.as_mut() {
            process.wait().await?;
        }
        *child = None;
        Ok(())
    }

    async fn terminate(&self) -> io::Result<()> {
        #[cfg(unix)]
        {
            let child = self.child.lock().await;
            if child.is_some() {
                let pid = i32::try_from(self.pid)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "pid out of range"))?;
                // SAFETY: plain `kill(2)` on the sidecar pid we spawned and
                // still own (the child handle is held), no memory is touched.
                if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        Ok(())
    }

    async fn kill(&self) -> io::Result<()> {
        let mut child = self.child.lock().await;
        if let Some(process) = child.as_mut() {
            process.kill().await?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct RuntimeWireResponse {
    id: String,
    ok: bool,
    result: Option<Value>,
    error: Option<RuntimeWireError>,
}

#[derive(Debug, Deserialize)]
struct RuntimeWireError {
    kind: ScrapeErrorKind,
    message: String,
    retryable: bool,
}

pub struct BrowserSupervisor {
    writer: Mutex<Option<BoxWriter>>,
    pending: Arc<Mutex<HashMap<String, PendingResponse>>>,
    request_serial: Mutex<()>,
    request_sequence: AtomicU64,
    process: Arc<dyn ProcessControl>,
    reader_task: StdMutex<Option<JoinHandle<()>>>,
    stderr_task: StdMutex<Option<JoinHandle<()>>>,
    timeouts: SupervisorTimeouts,
    stdout_closed: Arc<AtomicBool>,
    shutdown_started: AtomicBool,
}

impl BrowserSupervisor {
    pub async fn spawn(path: &Path) -> BrowserResult<Self> {
        if !path.is_absolute() || !path.is_file() {
            return Err(scrape_error(
                ScrapeErrorKind::RuntimeUnavailable,
                format!("CloakBrowser sidecar not found: {}", path.display()),
                false,
            ));
        }

        let mut command = Command::new(path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command.spawn().map_err(|error| {
            scrape_error(
                ScrapeErrorKind::BrowserStartup,
                format!("failed to spawn CloakBrowser sidecar: {error}"),
                false,
            )
        })?;
        let pid = child.id().ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::BrowserStartup,
                "CloakBrowser sidecar did not expose a process id",
                false,
            )
        })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::BrowserStartup,
                "CloakBrowser sidecar stdin is unavailable",
                false,
            )
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::BrowserStartup,
                "CloakBrowser sidecar stdout is unavailable",
                false,
            )
        })?;
        let stderr = child.stderr.take();
        let process = Arc::new(TokioProcessControl {
            pid,
            child: Mutex::new(Some(child)),
        });
        let supervisor = Self::from_io(
            BufReader::new(stdout),
            stdin,
            process,
            SupervisorTimeouts::default(),
        );
        if let Some(stderr) = stderr {
            let task = tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[cloak-runtime] {line}");
                }
            });
            *supervisor
                .stderr_task
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(task);
        }
        Ok(supervisor)
    }

    pub(crate) fn from_io<R, W>(
        reader: R,
        writer: W,
        process: Arc<dyn ProcessControl>,
        timeouts: SupervisorTimeouts,
    ) -> Self
    where
        R: AsyncBufRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + 'static,
    {
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let stdout_closed = Arc::new(AtomicBool::new(false));
        let reader_task = tokio::spawn(response_loop(
            reader,
            pending.clone(),
            stdout_closed.clone(),
        ));
        Self {
            writer: Mutex::new(Some(Box::pin(writer))),
            pending,
            request_serial: Mutex::new(()),
            request_sequence: AtomicU64::new(1),
            process,
            reader_task: StdMutex::new(Some(reader_task)),
            stderr_task: StdMutex::new(None),
            timeouts,
            stdout_closed,
            shutdown_started: AtomicBool::new(false),
        }
    }

    pub fn pid(&self) -> Option<u32> {
        self.process.pid()
    }

    async fn request(
        &self,
        command: &'static str,
        payload: Value,
        timeout: Duration,
    ) -> BrowserResult<Value> {
        let _request_guard = self.request_serial.lock().await;
        if self.stdout_closed.load(Ordering::SeqCst) {
            return Err(disconnected_error("CloakBrowser sidecar stdout is closed"));
        }

        let id = self
            .request_sequence
            .fetch_add(1, Ordering::SeqCst)
            .to_string();
        let request = json!({
            "id": id,
            "command": command,
            "payload": payload,
        });
        let encoded = serde_json::to_vec(&request).map_err(|error| {
            scrape_error(
                ScrapeErrorKind::InvalidInput,
                format!("failed to encode CloakBrowser request: {error}"),
                false,
            )
        })?;
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().await.insert(id.clone(), sender);

        let write_result = async {
            let mut writer = self.writer.lock().await;
            let writer = writer.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "sidecar stdin is closed")
            })?;
            writer.write_all(&encoded).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await
        }
        .await;
        if let Err(error) = write_result {
            self.pending.lock().await.remove(&id);
            return Err(disconnected_error(format!(
                "failed to write CloakBrowser request: {error}"
            )));
        }

        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => Err(disconnected_error(
                "CloakBrowser response channel closed unexpectedly",
            )),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(timeout_error(command, timeout))
            }
        }
    }

    async fn shutdown_gracefully(&self) -> BrowserResult<()> {
        if self.shutdown_started.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let shutdown_result = self
            .request("shutdown", json!({}), self.timeouts.shutdown)
            .await;
        self.writer.lock().await.take();

        let exited = match tokio::time::timeout(self.timeouts.exit_grace, self.process.wait()).await
        {
            Ok(Ok(())) => true,
            Ok(Err(error)) => {
                return Err(scrape_error(
                    ScrapeErrorKind::BrowserStartup,
                    format!("failed waiting for CloakBrowser sidecar: {error}"),
                    false,
                ));
            }
            Err(_) => false,
        };
        // Give the sidecar a chance to run its own cleanup (it kills the
        // Chromium process tree on SIGTERM) before resorting to SIGKILL.
        let exited = exited
            || (self.process.terminate().await.is_ok()
                && matches!(
                    tokio::time::timeout(self.timeouts.exit_grace, self.process.wait()).await,
                    Ok(Ok(()))
                ));
        match exited {
            true => {}
            false => {
                self.process.kill().await.map_err(|error| {
                    scrape_error(
                        ScrapeErrorKind::BrowserStartup,
                        format!("failed to terminate CloakBrowser sidecar: {error}"),
                        false,
                    )
                })?;
                tokio::time::timeout(self.timeouts.shutdown, self.process.wait())
                    .await
                    .map_err(|_| {
                        scrape_error(
                            ScrapeErrorKind::BrowserStartup,
                            "CloakBrowser sidecar did not exit after forced termination",
                            false,
                        )
                    })?
                    .map_err(|error| {
                        scrape_error(
                            ScrapeErrorKind::BrowserStartup,
                            format!("failed reaping CloakBrowser sidecar: {error}"),
                            false,
                        )
                    })?;
            }
        }

        self.stdout_closed.store(true, Ordering::SeqCst);
        fail_all_pending(
            &self.pending,
            scrape_error(
                ScrapeErrorKind::Cancelled,
                "CloakBrowser sidecar shut down",
                false,
            ),
        )
        .await;
        if let Err(error) = shutdown_result {
            if !matches!(
                error.kind,
                ScrapeErrorKind::Disconnected
                    | ScrapeErrorKind::Cancelled
                    | ScrapeErrorKind::EvaluationTimeout
                    | ScrapeErrorKind::NavigationTimeout
            ) {
                return Err(error);
            }
        }
        Ok(())
    }
}

#[async_trait]
impl BrowserAutomation for BrowserSupervisor {
    async fn runtime_status(&self) -> BrowserResult<RuntimeStatus> {
        decode_result(
            self.request("runtime_status", json!({}), self.timeouts.request)
                .await?,
            "runtime_status",
        )
    }

    async fn ensure_runtime(&self) -> BrowserResult<RuntimeStatus> {
        decode_result(
            self.request("ensure_runtime", json!({}), RUNTIME_INSTALL_TIMEOUT)
                .await?,
            "ensure_runtime",
        )
    }

    async fn launch(&self, request: LaunchRequest) -> BrowserResult<()> {
        let payload = serde_json::to_value(request).map_err(|error| {
            scrape_error(
                ScrapeErrorKind::InvalidInput,
                format!("invalid browser launch request: {error}"),
                false,
            )
        })?;
        self.request("launch", payload, BROWSER_LAUNCH_TIMEOUT)
            .await?;
        Ok(())
    }

    async fn navigate(&self, url: &str, timeout: Duration) -> BrowserResult<()> {
        self.request(
            "navigate",
            json!({"url": url, "timeoutMs": duration_millis(timeout)}),
            timeout + Duration::from_secs(2),
        )
        .await?;
        Ok(())
    }

    async fn evaluate(&self, script: &str, timeout: Duration) -> BrowserResult<Value> {
        let result = self
            .request(
                "evaluate",
                json!({"expression": script, "timeoutMs": duration_millis(timeout)}),
                timeout + Duration::from_secs(2),
            )
            .await?;
        result.get("value").cloned().ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::InvalidResponse,
                "CloakBrowser evaluate response omitted value",
                false,
            )
        })
    }

    async fn screenshot(&self, path: &Path) -> BrowserResult<()> {
        let path = path.to_str().ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::InvalidInput,
                "screenshot path is not valid UTF-8",
                false,
            )
        })?;
        self.request(
            "screenshot",
            json!({"path": path, "fullPage": true}),
            self.timeouts.request,
        )
        .await?;
        Ok(())
    }

    async fn health_check(&self) -> BrowserResult<GoogleMapsPageState> {
        Ok(self.health_snapshot().await?.state)
    }

    async fn health_snapshot(&self) -> BrowserResult<GoogleMapsPageHealth> {
        decode_result(
            self.request("health_check", json!({}), self.timeouts.request)
                .await?,
            "health_check",
        )
    }

    async fn shutdown(&self) -> BrowserResult<()> {
        self.shutdown_gracefully().await
    }
}

impl Drop for BrowserSupervisor {
    fn drop(&mut self) {
        if let Ok(task) = self.reader_task.get_mut() {
            if let Some(task) = task.take() {
                task.abort();
            }
        }
        if let Ok(task) = self.stderr_task.get_mut() {
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }
}

pub struct BrowserSession {
    automation: Arc<dyn BrowserAutomation>,
}

impl BrowserSession {
    pub fn new(automation: Arc<dyn BrowserAutomation>) -> Self {
        Self { automation }
    }

    pub async fn navigate(&self, url: &str) -> BrowserResult<()> {
        self.automation.navigate(url, Duration::from_secs(30)).await
    }

    pub async fn evaluate(&self, script: &str) -> BrowserResult<Value> {
        self.automation
            .evaluate(script, Duration::from_secs(30))
            .await
    }

    pub async fn screenshot(&self, path: &Path) -> BrowserResult<()> {
        self.automation.screenshot(path).await
    }

    pub async fn health_check(&self) -> BrowserResult<GoogleMapsPageState> {
        self.automation.health_check().await
    }

    pub async fn health_snapshot(&self) -> BrowserResult<GoogleMapsPageHealth> {
        self.automation.health_snapshot().await
    }
}

pub struct SessionGuard {
    pub(crate) session: Arc<BrowserSession>,
    _permit: OwnedSemaphorePermit,
}

impl std::ops::Deref for SessionGuard {
    type Target = BrowserSession;

    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

pub struct BrowserManager {
    sidecar_path: PathBuf,
    profile_dir: PathBuf,
    injected: Option<Arc<dyn BrowserAutomation>>,
    automation: Mutex<Option<Arc<dyn BrowserAutomation>>>,
    supervisor: Mutex<Option<Arc<BrowserSupervisor>>>,
    session: Mutex<Option<Arc<BrowserSession>>>,
    session_permit: Arc<Semaphore>,
}

impl BrowserManager {
    pub fn new(_max_sessions: usize) -> Self {
        Self {
            sidecar_path: default_sidecar_path(),
            profile_dir: std::env::temp_dir().join("rustsale-cloakbrowser-profile"),
            injected: None,
            automation: Mutex::new(None),
            supervisor: Mutex::new(None),
            session: Mutex::new(None),
            session_permit: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn with_automation(_max_sessions: usize, automation: Arc<dyn BrowserAutomation>) -> Self {
        Self {
            sidecar_path: PathBuf::new(),
            profile_dir: std::env::temp_dir().join("rustsale-cloakbrowser-test-profile"),
            injected: Some(automation),
            automation: Mutex::new(None),
            supervisor: Mutex::new(None),
            session: Mutex::new(None),
            session_permit: Arc::new(Semaphore::new(1)),
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.profile_dir = std::env::temp_dir().join(format!("rustsale-cloakbrowser-{port}"));
        self
    }

    pub fn with_profile_dir(mut self, profile_dir: PathBuf) -> Self {
        self.profile_dir = profile_dir;
        self
    }

    pub fn with_sidecar_path(mut self, sidecar_path: PathBuf) -> Self {
        self.sidecar_path = sidecar_path;
        self
    }

    pub async fn start(&self, headless: bool) -> BrowserResult<()> {
        self.start_with_url(headless, None).await
    }

    pub async fn runtime_status(&self) -> BrowserResult<RuntimeStatus> {
        let automation: Arc<dyn BrowserAutomation> = if let Some(injected) = &self.injected {
            injected.clone()
        } else {
            Arc::new(BrowserSupervisor::spawn(&self.sidecar_path).await?)
        };
        let status = automation.runtime_status().await;
        let shutdown = automation.shutdown().await;
        match (status, shutdown) {
            (Ok(status), Ok(())) => Ok(status),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    pub async fn prepare_runtime(&self) -> BrowserResult<RuntimeStatus> {
        let automation: Arc<dyn BrowserAutomation> = if let Some(injected) = &self.injected {
            injected.clone()
        } else {
            Arc::new(BrowserSupervisor::spawn(&self.sidecar_path).await?)
        };
        let prepared = automation.ensure_runtime().await;
        let shutdown = automation.shutdown().await;
        match (prepared, shutdown) {
            (Ok(status), Ok(())) => Ok(status),
            (Err(error), _) => Err(error),
            (Ok(_), Err(error)) => Err(error),
        }
    }

    pub async fn relaunch_for_action(&self, blocked_url: &str) -> BrowserResult<()> {
        let blocked_url = blocked_url.trim();
        if !(blocked_url.starts_with("https://") || blocked_url.starts_with("http://")) {
            return Err(scrape_error(
                ScrapeErrorKind::InvalidInput,
                "blocked URL must use HTTP or HTTPS",
                false,
            ));
        }
        self.start_with_url(false, Some(blocked_url.to_string()))
            .await
    }

    async fn start_with_url(&self, headless: bool, start_url: Option<String>) -> BrowserResult<()> {
        self.close().await?;
        let (automation, supervisor): (Arc<dyn BrowserAutomation>, Option<Arc<BrowserSupervisor>>) =
            if let Some(injected) = &self.injected {
                (injected.clone(), None)
            } else {
                let supervisor = Arc::new(BrowserSupervisor::spawn(&self.sidecar_path).await?);
                (supervisor.clone(), Some(supervisor))
            };

        let startup = async {
            automation.ensure_runtime().await?;
            automation
                .launch(LaunchRequest {
                    profile_dir: self.profile_dir.clone(),
                    headless,
                    start_url,
                })
                .await
        }
        .await;
        if let Err(error) = startup {
            if let Err(shutdown_error) = automation.shutdown().await {
                eprintln!("failed to close browser after startup error: {shutdown_error}");
            }
            return Err(error);
        }

        *self.session.lock().await = Some(Arc::new(BrowserSession::new(automation.clone())));
        *self.automation.lock().await = Some(automation);
        *self.supervisor.lock().await = supervisor;
        Ok(())
    }

    pub async fn acquire(&self) -> BrowserResult<SessionGuard> {
        let permit = self
            .session_permit
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| disconnected_error("browser session semaphore is closed"))?;
        let session = self.session.lock().await.clone().ok_or_else(|| {
            scrape_error(
                ScrapeErrorKind::BrowserStartup,
                "browser is not started",
                false,
            )
        })?;
        Ok(SessionGuard {
            session,
            _permit: permit,
        })
    }

    pub async fn release(&self, session: Arc<BrowserSession>) -> BrowserResult<()> {
        drop(session);
        Ok(())
    }

    pub async fn close(&self) -> BrowserResult<()> {
        self.session.lock().await.take();
        let automation = self.automation.lock().await.take();
        let result = if let Some(automation) = automation {
            automation.shutdown().await
        } else {
            Ok(())
        };
        self.supervisor.lock().await.take();
        result
    }

    pub async fn process_id(&self) -> Option<u32> {
        self.supervisor
            .lock()
            .await
            .as_ref()
            .and_then(|supervisor| supervisor.pid())
    }
}

async fn response_loop<R>(
    reader: R,
    pending: Arc<Mutex<HashMap<String, PendingResponse>>>,
    stdout_closed: Arc<AtomicBool>,
) where
    R: AsyncBufRead + Send + Unpin + 'static,
{
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) if line.trim().is_empty() => continue,
            Ok(Some(line)) => match serde_json::from_str::<RuntimeWireResponse>(&line) {
                Ok(response) => deliver_response(&pending, response).await,
                Err(error) => {
                    // A library printing to stdout must not kill the in-flight request:
                    // the real response is still coming. Only EOF/IO errors fail pending.
                    tracing::warn!(
                        target: "google_maps_scraper",
                        error = %error,
                        line_bytes = line.len(),
                        "ignored non-JSON line on CloakBrowser stdout"
                    );
                }
            },
            Ok(None) => break,
            Err(error) => {
                fail_all_pending(
                    &pending,
                    disconnected_error(format!("failed reading CloakBrowser stdout: {error}")),
                )
                .await;
                break;
            }
        }
    }
    stdout_closed.store(true, Ordering::SeqCst);
    fail_all_pending(
        &pending,
        disconnected_error("CloakBrowser sidecar closed stdout"),
    )
    .await;
}

/// A sidecar bundle that cannot resolve its own modules is a broken installation: retrying
/// the launch three times cannot fix it, so report it as a non-retryable runtime fault.
fn classify_sidecar_error(error: RuntimeWireError) -> ScrapeJobError {
    let broken_bundle = error.kind == ScrapeErrorKind::BrowserStartup
        && (error.message.contains("ResolveMessage")
            || error.message.contains("Cannot find module")
            || error.message.contains("Cannot find package"));
    if broken_bundle {
        return scrape_error(
            ScrapeErrorKind::RuntimeUnavailable,
            format!("CloakBrowser sidecar bundle is broken: {}", error.message),
            false,
        );
    }
    scrape_error(error.kind, error.message, error.retryable)
}

async fn deliver_response(
    pending: &Arc<Mutex<HashMap<String, PendingResponse>>>,
    response: RuntimeWireResponse,
) {
    let sender = pending.lock().await.remove(&response.id);
    if let Some(sender) = sender {
        let result = if response.ok {
            Ok(response.result.unwrap_or(Value::Null))
        } else if let Some(error) = response.error {
            Err(classify_sidecar_error(error))
        } else {
            Err(scrape_error(
                ScrapeErrorKind::InvalidResponse,
                "CloakBrowser error response omitted error details",
                false,
            ))
        };
        if sender.send(result).is_err() {
            eprintln!("CloakBrowser response receiver was dropped");
        }
    }
}

async fn fail_all_pending(
    pending: &Arc<Mutex<HashMap<String, PendingResponse>>>,
    error: ScrapeJobError,
) {
    let senders: Vec<_> = pending
        .lock()
        .await
        .drain()
        .map(|(_, sender)| sender)
        .collect();
    for sender in senders {
        if sender.send(Err(error.clone())).is_err() {
            eprintln!("CloakBrowser pending response receiver was dropped");
        }
    }
}

fn decode_result<T: for<'de> Deserialize<'de>>(value: Value, command: &str) -> BrowserResult<T> {
    serde_json::from_value(value).map_err(|error| {
        scrape_error(
            ScrapeErrorKind::InvalidResponse,
            format!("invalid {command} response: {error}"),
            false,
        )
    })
}

fn duration_millis(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

fn timeout_error(command: &str, timeout: Duration) -> ScrapeJobError {
    let kind = match command {
        "navigate" => ScrapeErrorKind::NavigationTimeout,
        "evaluate" => ScrapeErrorKind::EvaluationTimeout,
        "launch" => ScrapeErrorKind::BrowserStartup,
        "ensure_runtime" | "runtime_status" => ScrapeErrorKind::RuntimeUnavailable,
        _ => ScrapeErrorKind::EvaluationTimeout,
    };
    scrape_error(
        kind,
        format!(
            "CloakBrowser command {command} timed out after {} ms",
            duration_millis(timeout)
        ),
        true,
    )
}

fn scrape_error(
    kind: ScrapeErrorKind,
    message: impl Into<String>,
    retryable: bool,
) -> ScrapeJobError {
    ScrapeJobError {
        kind,
        message: message.into(),
        retryable,
    }
}

fn disconnected_error(message: impl Into<String>) -> ScrapeJobError {
    scrape_error(ScrapeErrorKind::Disconnected, message, true)
}

fn default_sidecar_path() -> PathBuf {
    if let Some(path) = std::env::var_os("RUSTSALE_CLOAK_RUNTIME_PATH") {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return path;
        }
        if let Ok(cwd) = std::env::current_dir() {
            return cwd.join(path);
        }
        return path;
    }

    let executable_name = if cfg!(windows) {
        "cloak-runtime.exe"
    } else {
        "cloak-runtime"
    };
    let triple = host_target_triple();
    let extension = if cfg!(windows) { ".exe" } else { "" };
    let triple_name = format!("cloak-runtime-{triple}{extension}");

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let candidates = [
                parent.join(executable_name),
                parent.join(&triple_name),
                parent.join("binaries").join(executable_name),
                parent.join("binaries").join(&triple_name),
                parent.join("resources").join(executable_name),
                parent.join("resources").join(&triple_name),
                parent
                    .join("resources")
                    .join("binaries")
                    .join(executable_name),
                parent.join("resources").join("binaries").join(&triple_name),
            ];
            for candidate in candidates {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    {
        if let Ok(cwd) = std::env::current_dir() {
            let candidates = [
                cwd.join("src-tauri").join("binaries").join(&triple_name),
                cwd.join("../../src-tauri/binaries").join(&triple_name),
                cwd.join("binaries").join(&triple_name),
                cwd.join("src-tauri").join("binaries").join(executable_name),
                cwd.join("../../src-tauri/binaries").join(executable_name),
                cwd.join("binaries").join(executable_name),
            ];
            for candidate in candidates {
                if candidate.is_file() {
                    return candidate;
                }
            }
        }

        let manifest_candidate = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("binaries")
            .join(&triple_name);
        if manifest_candidate.is_file() {
            return manifest_candidate;
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            return parent.join(executable_name);
        }
    }

    if cfg!(windows) {
        PathBuf::from(r"C:\untrusted_sidecar_not_found\cloak-runtime.exe")
    } else {
        PathBuf::from("/untrusted_sidecar_not_found/cloak-runtime")
    }
}

fn host_target_triple() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => "unsupported-target",
    }
}
