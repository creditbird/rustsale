#[cfg(test)]
mod tests {
    use crate::scrapers::browser::BrowserManager;
    use std::process::Command;
    use std::sync::Arc;
    use std::time::Duration;

    fn get_random_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(9222)
    }

    #[tokio::test]
    async fn test_browser_manager_lifecycle() {
        let manager = BrowserManager::new(2).with_port(get_random_port());

        // Start the browser in headless mode
        let start_res = manager.start(true).await;
        assert!(
            start_res.is_ok(),
            "Failed to start browser: {:?}",
            start_res.err()
        );

        // Acquire a session
        let session_res = manager.acquire().await;
        assert!(
            session_res.is_ok(),
            "Failed to acquire session: {:?}",
            session_res.err()
        );
        let session = session_res.unwrap();

        // Navigate to about:blank
        let nav_res = session.navigate("about:blank").await;
        assert!(nav_res.is_ok(), "Failed to navigate: {:?}", nav_res.err());

        // Evaluate some script
        let eval_res = session.evaluate("1 + 2").await;
        assert!(
            eval_res.is_ok(),
            "Failed to evaluate script: {:?}",
            eval_res.err()
        );
        let val = eval_res.unwrap();
        assert_eq!(val, serde_json::json!(3));

        // Evaluate location.href
        let eval_href = session.evaluate("window.location.href").await;
        assert!(
            eval_href.is_ok(),
            "Failed to evaluate href: {:?}",
            eval_href.err()
        );
        let href = eval_href.unwrap();
        assert_eq!(href, serde_json::json!("about:blank"));

        // Close the manager
        let close_res = manager.close().await;
        assert!(
            close_res.is_ok(),
            "Failed to close browser: {:?}",
            close_res.err()
        );
    }

    #[tokio::test]
    async fn test_browser_manager_session_pooling() {
        let manager = Arc::new(BrowserManager::new(1).with_port(get_random_port())); // max_sessions = 1

        // Start the browser
        manager.start(true).await.expect("Failed to start browser");

        // Acquire the first and only allowed session
        let session1 = manager
            .acquire()
            .await
            .expect("Failed to acquire session 1");

        // Retain pointer to verify session reuse
        let session1_ptr = Arc::as_ptr(&session1.session);

        // Spawn a task that will drop the session after a short delay to trigger auto-release
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            drop(session1);
        });

        // Try to acquire another session. This should block/wait until the first session is released
        let start_time = std::time::Instant::now();
        let session2 = manager
            .acquire()
            .await
            .expect("Failed to acquire session 2");
        let elapsed = start_time.elapsed();

        // Verify that it actually waited for at least some time (roughly > 150ms)
        assert!(
            elapsed >= Duration::from_millis(150),
            "Should have blocked waiting for release, but finished in {:?}",
            elapsed
        );

        // Verify that we reused the same session structure since it was released
        assert_eq!(session1_ptr, Arc::as_ptr(&session2.session));

        // Clean up
        manager.close().await.expect("Failed to close manager");
    }

    #[tokio::test]
    async fn test_browser_manager_raii_drop() {
        let manager = BrowserManager::new(1).with_port(get_random_port());
        manager.start(true).await.expect("Failed to start browser");

        // Retrieve process ID
        let pid = manager.process_id().await.unwrap();

        // Verify it is currently running (kill -0 should return success)
        let status = Command::new("kill").arg("-0").arg(pid.to_string()).status();
        assert!(status.is_ok());
        assert!(status.unwrap().success(), "Process should be alive");

        // Drop the manager to trigger RAII drop implementation
        drop(manager);

        // Wait a short moment to ensure the process has completed exit handling
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify process is dead/reaped (kill -0 should fail)
        let status = Command::new("kill").arg("-0").arg(pid.to_string()).status();
        assert!(status.is_ok());
        assert!(
            !status.unwrap().success(),
            "Process should have been killed and reaped by drop()"
        );
    }
}
