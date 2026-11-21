#[cfg(test)]
mod tests {
    use super::super::browser::{
        BrowserAutomation, BrowserManager, BrowserSupervisor, GoogleMapsPageState, LaunchRequest,
        ProcessControl, RuntimeStatus, SupervisorTimeouts,
    };
    use crate::google_maps_jobs::types::{ScrapeErrorKind, ScrapeJobError};
    use async_trait::async_trait;
    use serde_json::{json, Value};
    use std::io;
    use std::path::Path;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex as StdMutex};
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};
    use tokio::sync::Notify;

    struct FakeProcessControl {
        killed: AtomicBool,
        terminated: AtomicBool,
        exited: AtomicBool,
        exit_notify: Notify,
    }

    impl FakeProcessControl {
        fn new() -> Self {
            Self {
                killed: AtomicBool::new(false),
                terminated: AtomicBool::new(false),
                exited: AtomicBool::new(false),
                exit_notify: Notify::new(),
            }
        }

        fn exit(&self) {
            self.exited.store(true, Ordering::SeqCst);
            self.exit_notify.notify_waiters();
        }
    }

    #[async_trait]
    impl ProcessControl for FakeProcessControl {
        fn pid(&self) -> Option<u32> {
            Some(4242)
        }

        async fn wait(&self) -> io::Result<()> {
            while !self.exited.load(Ordering::SeqCst) {
                self.exit_notify.notified().await;
            }
            Ok(())
        }

        async fn terminate(&self) -> io::Result<()> {
            self.terminated.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn kill(&self) -> io::Result<()> {
            self.killed.store(true, Ordering::SeqCst);
            self.exit();
            Ok(())
        }
    }

    fn supervisor_fixture() -> (
        BrowserSupervisor,
        BufReader<tokio::io::ReadHalf<DuplexStream>>,
        tokio::io::WriteHalf<DuplexStream>,
        Arc<FakeProcessControl>,
    ) {
        supervisor_fixture_with(SupervisorTimeouts {
            request: Duration::from_secs(1),
            shutdown: Duration::from_millis(50),
            exit_grace: Duration::from_millis(50),
        })
    }

    fn supervisor_fixture_with(
        timeouts: SupervisorTimeouts,
    ) -> (
        BrowserSupervisor,
        BufReader<tokio::io::ReadHalf<DuplexStream>>,
        tokio::io::WriteHalf<DuplexStream>,
        Arc<FakeProcessControl>,
    ) {
        let (client, sidecar) = tokio::io::duplex(16 * 1024);
        let (client_reader, client_writer) = tokio::io::split(client);
        let (sidecar_reader, sidecar_writer) = tokio::io::split(sidecar);
        let control = Arc::new(FakeProcessControl::new());
        let supervisor = BrowserSupervisor::from_io(
            BufReader::new(client_reader),
            client_writer,
            control.clone(),
            timeouts,
        );
        (
            supervisor,
            BufReader::new(sidecar_reader),
            sidecar_writer,
            control,
        )
    }

    async fn read_request(reader: &mut BufReader<tokio::io::ReadHalf<DuplexStream>>) -> Value {
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        serde_json::from_str(&line).unwrap()
    }

    async fn write_response(writer: &mut tokio::io::WriteHalf<DuplexStream>, value: Value) {
        writer
            .write_all(format!("{}\n", value).as_bytes())
            .await
            .unwrap();
        writer.flush().await.unwrap();
    }

    #[tokio::test]
    async fn maps_correlated_sidecar_error_to_scrape_error_kind() {
        let (supervisor, mut reader, mut writer, _) = supervisor_fixture();
        let sidecar = tokio::spawn(async move {
            let request = read_request(&mut reader).await;
            writer.write_all(b"\n").await.unwrap();
            writer.flush().await.unwrap();
            write_response(
                &mut writer,
                json!({
                    "id": request["id"],
                    "ok": false,
                    "error": {
                        "kind": "navigation_timeout",
                        "message": "navigation timed out",
                        "retryable": true
                    }
                }),
            )
            .await;
        });

        let error = supervisor
            .navigate("https://example.test", Duration::from_secs(5))
            .await
            .unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::NavigationTimeout);
        assert_eq!(error.message, "navigation timed out");
        assert!(error.retryable);
        sidecar.await.unwrap();
    }

    #[tokio::test]
    async fn captcha_health_snapshot_preserves_state_url_and_title() {
        let (supervisor, mut reader, mut writer, _) = supervisor_fixture();
        let sidecar = tokio::spawn(async move {
            let request = read_request(&mut reader).await;
            assert_eq!(request["command"], "health_check");
            write_response(
                &mut writer,
                json!({
                    "id": request["id"],
                    "ok": true,
                    "result": {
                        "state": "captcha",
                        "url": "https://www.google.com/sorry/index",
                        "title": "Google verification"
                    }
                }),
            )
            .await;
        });

        let health = supervisor.health_snapshot().await.unwrap();

        assert_eq!(health.state, GoogleMapsPageState::Captcha);
        assert_eq!(health.url, "https://www.google.com/sorry/index");
        assert_eq!(health.title, "Google verification");
        sidecar.await.unwrap();
    }

    #[tokio::test]
    async fn shutdown_times_out_then_terminates_sidecar() {
        let (supervisor, mut reader, _writer, control) = supervisor_fixture();
        let sidecar = tokio::spawn(async move {
            let request = read_request(&mut reader).await;
            assert_eq!(request["command"], "shutdown");
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        supervisor.shutdown().await.unwrap();

        assert!(control.killed.load(Ordering::SeqCst));
        sidecar.abort();
    }

    /// C1: the sidecar needs time after stdin closes to close Chromium and exit.
    /// A SIGKILL while it is still cleaning up strands the browser and its
    /// profile lock, so a slow-but-alive sidecar must not be killed.
    #[tokio::test]
    async fn kill_after_shutdown_timeout_does_not_leave_browser_orphan() {
        let (supervisor, mut reader, _writer, control) =
            supervisor_fixture_with(SupervisorTimeouts {
                request: Duration::from_secs(1),
                shutdown: Duration::from_millis(50),
                exit_grace: Duration::from_millis(600),
            });
        let exiting = control.clone();
        let sidecar = tokio::spawn(async move {
            let request = read_request(&mut reader).await;
            assert_eq!(request["command"], "shutdown");
            // Slower than the shutdown reply timeout, faster than the grace.
            tokio::time::sleep(Duration::from_millis(250)).await;
            exiting.exit();
        });

        supervisor.shutdown().await.unwrap();

        assert!(!control.killed.load(Ordering::SeqCst));
        sidecar.await.unwrap();
    }

    #[tokio::test]
    async fn stuck_sidecar_is_terminated_before_it_is_killed() {
        let (supervisor, mut reader, _writer, control) = supervisor_fixture();
        let sidecar = tokio::spawn(async move {
            let _ = read_request(&mut reader).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        });

        supervisor.shutdown().await.unwrap();

        assert!(control.terminated.load(Ordering::SeqCst));
        assert!(control.killed.load(Ordering::SeqCst));
        sidecar.abort();
    }

    #[tokio::test]
    async fn only_one_in_flight_request_owns_a_correlation_id() {
        let (supervisor, mut reader, mut writer, _) = supervisor_fixture();
        let supervisor = Arc::new(supervisor);
        let first_client = supervisor.clone();
        let first =
            tokio::spawn(
                async move { first_client.evaluate("1 + 1", Duration::from_secs(1)).await },
            );

        let first_request = read_request(&mut reader).await;
        let second_client = supervisor.clone();
        let second = tokio::spawn(async move {
            second_client
                .evaluate("2 + 2", Duration::from_secs(1))
                .await
        });

        let mut second_line = String::new();
        assert!(
            tokio::time::timeout(
                Duration::from_millis(50),
                reader.read_line(&mut second_line)
            )
            .await
            .is_err(),
            "second request must wait until the first response is correlated"
        );

        write_response(
            &mut writer,
            json!({"id": first_request["id"], "ok": true, "result": {"value": 2}}),
        )
        .await;
        let second_request = read_request(&mut reader).await;
        assert_ne!(first_request["id"], second_request["id"]);
        write_response(
            &mut writer,
            json!({"id": second_request["id"], "ok": true, "result": {"value": 4}}),
        )
        .await;

        assert_eq!(first.await.unwrap().unwrap(), json!(2));
        assert_eq!(second.await.unwrap().unwrap(), json!(4));
    }

    #[tokio::test]
    async fn closed_stdout_fails_all_pending_requests_as_disconnected() {
        let (supervisor, mut reader, writer, _) = supervisor_fixture();
        let sidecar = tokio::spawn(async move {
            let _request = read_request(&mut reader).await;
            drop(writer);
        });

        let error = supervisor
            .evaluate("document.title", Duration::from_secs(1))
            .await
            .unwrap_err();

        assert_eq!(error.kind, ScrapeErrorKind::Disconnected);
        assert!(error.retryable);
        sidecar.await.unwrap();
    }

    struct FakeAutomation {
        runtime_status_checks: AtomicUsize,
        runtime_prepares: AtomicUsize,
        launches: AtomicUsize,
        shutdowns: AtomicUsize,
        launch_requests: StdMutex<Vec<LaunchRequest>>,
    }

    impl FakeAutomation {
        fn new() -> Self {
            Self {
                runtime_status_checks: AtomicUsize::new(0),
                runtime_prepares: AtomicUsize::new(0),
                launches: AtomicUsize::new(0),
                shutdowns: AtomicUsize::new(0),
                launch_requests: StdMutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl BrowserAutomation for FakeAutomation {
        async fn runtime_status(&self) -> Result<RuntimeStatus, ScrapeJobError> {
            self.runtime_status_checks.fetch_add(1, Ordering::SeqCst);
            Ok(RuntimeStatus::installed_for_test())
        }

        async fn ensure_runtime(&self) -> Result<RuntimeStatus, ScrapeJobError> {
            self.runtime_prepares.fetch_add(1, Ordering::SeqCst);
            Ok(RuntimeStatus::installed_for_test())
        }

        async fn launch(&self, request: LaunchRequest) -> Result<(), ScrapeJobError> {
            self.launches.fetch_add(1, Ordering::SeqCst);
            self.launch_requests.lock().unwrap().push(request);
            Ok(())
        }

        async fn navigate(&self, _url: &str, _timeout: Duration) -> Result<(), ScrapeJobError> {
            Ok(())
        }

        async fn evaluate(
            &self,
            script: &str,
            _timeout: Duration,
        ) -> Result<Value, ScrapeJobError> {
            Ok(match script {
                "1 + 2" => json!(3),
                "window.location.href" => json!("about:blank"),
                _ => Value::Null,
            })
        }

        async fn screenshot(&self, _path: &Path) -> Result<(), ScrapeJobError> {
            Ok(())
        }

        async fn health_check(&self) -> Result<GoogleMapsPageState, ScrapeJobError> {
            Ok(GoogleMapsPageState::Ready)
        }

        async fn shutdown(&self) -> Result<(), ScrapeJobError> {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn browser_manager_prepares_runtime_without_launching_browser() {
        let automation = Arc::new(FakeAutomation::new());
        let manager = BrowserManager::with_automation(1, automation.clone());

        let status = manager.prepare_runtime().await.unwrap();

        assert!(status.installed);
        assert_eq!(automation.runtime_prepares.load(Ordering::SeqCst), 1);
        assert_eq!(automation.launches.load(Ordering::SeqCst), 0);
        assert_eq!(automation.shutdowns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn browser_manager_reads_runtime_status_without_downloading_or_launching() {
        let automation = Arc::new(FakeAutomation::new());
        let manager = BrowserManager::with_automation(1, automation.clone());

        let status = manager.runtime_status().await.unwrap();

        assert!(status.installed);
        assert_eq!(automation.runtime_status_checks.load(Ordering::SeqCst), 1);
        assert_eq!(automation.runtime_prepares.load(Ordering::SeqCst), 0);
        assert_eq!(automation.launches.load(Ordering::SeqCst), 0);
        assert_eq!(automation.shutdowns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn browser_manager_starts_before_exposing_a_session() {
        let automation = Arc::new(FakeAutomation::new());
        let manager = BrowserManager::with_automation(1, automation.clone());

        manager.start(true).await.unwrap();
        let session = manager.acquire().await.unwrap();
        session.navigate("about:blank").await.unwrap();
        assert_eq!(session.evaluate("1 + 2").await.unwrap(), json!(3));
        manager.close().await.unwrap();

        assert_eq!(automation.launches.load(Ordering::SeqCst), 1);
        assert_eq!(automation.shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(
            automation.launch_requests.lock().unwrap()[0].start_url,
            None
        );
    }

    #[tokio::test]
    async fn captcha_browser_manager_relaunches_same_profile_headed_at_blocked_url() {
        let automation = Arc::new(FakeAutomation::new());
        let profile_dir = std::env::temp_dir().join("rustsale-captcha-profile");
        let manager = BrowserManager::with_automation(1, automation.clone())
            .with_profile_dir(profile_dir.clone());

        manager.start(true).await.unwrap();
        manager
            .relaunch_for_action("https://www.google.com/sorry/index")
            .await
            .unwrap();

        let launches = automation.launch_requests.lock().unwrap();
        assert_eq!(launches.len(), 2);
        assert_eq!(launches[0].profile_dir, profile_dir);
        assert_eq!(launches[1].profile_dir, launches[0].profile_dir);
        assert!(launches[0].headless);
        assert!(!launches[1].headless);
        assert_eq!(
            launches[1].start_url.as_deref(),
            Some("https://www.google.com/sorry/index")
        );
    }

    #[tokio::test]
    async fn browser_manager_serializes_session_ownership() {
        let automation = Arc::new(FakeAutomation::new());
        let manager = Arc::new(BrowserManager::with_automation(1, automation));
        manager.start(true).await.unwrap();
        let first = manager.acquire().await.unwrap();
        let first_session = Arc::as_ptr(&first.session);

        let waiting_manager = manager.clone();
        let waiting = tokio::spawn(async move { waiting_manager.acquire().await.unwrap() });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!waiting.is_finished());
        drop(first);

        let second = waiting.await.unwrap();
        assert_eq!(first_session, Arc::as_ptr(&second.session));
        manager.close().await.unwrap();
    }
}
