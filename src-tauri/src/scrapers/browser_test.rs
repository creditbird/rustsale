#[cfg(test)]
mod tests {
    use crate::scrapers::browser::BrowserManager;

    #[tokio::test]
    async fn test_browser_manager_lifecycle() {
        let manager = BrowserManager::new(2);

        // Start the browser in headless mode
        let start_res = manager.start(true).await;
        assert!(start_res.is_ok(), "Failed to start browser: {:?}", start_res.err());

        // Acquire a session
        let session_res = manager.acquire().await;
        assert!(session_res.is_ok(), "Failed to acquire session: {:?}", session_res.err());
        let session = session_res.unwrap();

        // Navigate to about:blank
        let nav_res = session.navigate("about:blank").await;
        assert!(nav_res.is_ok(), "Failed to navigate: {:?}", nav_res.err());

        // Evaluate some script
        let eval_res = session.evaluate("1 + 2").await;
        assert!(eval_res.is_ok(), "Failed to evaluate script: {:?}", eval_res.err());
        let val = eval_res.unwrap();
        assert_eq!(val, serde_json::json!(3));

        // Evaluate location.href
        let eval_href = session.evaluate("window.location.href").await;
        assert!(eval_href.is_ok(), "Failed to evaluate href: {:?}", eval_href.err());
        let href = eval_href.unwrap();
        assert_eq!(href, serde_json::json!("about:blank"));

        // Close the manager
        let close_res = manager.close().await;
        assert!(close_res.is_ok(), "Failed to close browser: {:?}", close_res.err());
    }
}
