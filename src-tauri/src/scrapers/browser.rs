#![allow(dead_code)]

use anyhow::{Context, Result};
use cdp_lite::client::CdpClient;
use cdp_lite::protocol::NoParams;
use serde_json::json;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct BrowserSession {
    pub client: Arc<CdpClient>,
    pub(crate) in_use: Arc<Mutex<bool>>,
}

impl BrowserSession {
    pub fn new(client: CdpClient) -> Self {
        Self {
            client: Arc::new(client),
            in_use: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        use tokio_stream::StreamExt;
        let mut page_events = self.client.on_domain("Page");
        self.client.send_raw_command("Page.navigate", json!({"url": url})).await?;
        
        let wait_load = async {
            while let Some(event_res) = page_events.next().await {
                if let Ok(event) = event_res {
                    if event.method.as_deref() == Some("Page.loadEventFired") {
                        break;
                    }
                }
            }
        };

        tokio::time::timeout(Duration::from_secs(30), wait_load)
            .await
            .context("Navigation load event timed out")?;
        Ok(())
    }

    pub async fn evaluate(&self, script: &str) -> Result<serde_json::Value> {
        let response = self.client.send_raw_command(
            "Runtime.evaluate",
            json!({"expression": script, "returnByValue": true}),
        ).await?;
        if let Some(result) = response.result {
            if let Some(value) = result.get("result").and_then(|r| r.get("value")) {
                return Ok(value.clone());
            }
        }
        Ok(serde_json::Value::Null)
    }
}

pub struct BrowserManager {
    port: u16,
    endpoint: String,
    sessions: Arc<Mutex<Vec<Arc<BrowserSession>>>>,
    max_sessions: usize,
    process: Arc<Mutex<Option<Child>>>,
}

impl BrowserManager {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            port: 9222,
            endpoint: "127.0.0.1:9222".to_string(),
            sessions: Arc::new(Mutex::new(Vec::new())),
            max_sessions,
            process: Arc::new(Mutex::new(Option::None)),
        }
    }

    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self.endpoint = format!("127.0.0.1:{}", port);
        self
    }

    pub async fn start(&self, headless: bool) -> Result<()> {
        let candidates = vec![
            "/usr/bin/google-chrome-stable",
            "/usr/bin/google-chrome",
            "/usr/bin/chromium",
            "/home/arch/.cloakbrowser/chromium-146.0.7680.177.5/chrome",
        ];
        let mut path = "/usr/bin/chromium".to_string();
        for p in candidates {
            if std::path::Path::new(p).exists() {
                path = p.to_string();
                break;
            }
        }

        let mut cmd = Command::new(&path);
        cmd.arg(format!("--remote-debugging-port={}", self.port))
           .arg("--no-first-run")
           .arg("--no-default-browser-check")
           .arg("--disable-blink-features=AutomationControlled")
           .arg("--disable-extensions");

        if headless {
            cmd.arg("--headless=new");
        }

        let child = cmd.spawn().context("Failed to spawn browser")?;
        *self.process.lock().await = Some(child);

        // Wait for browser port to bind
        for _ in 0..10 {
            if CdpClient::new(&self.endpoint, Duration::from_secs(1)).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok(())
    }

    pub async fn acquire(&self) -> Result<Arc<BrowserSession>> {
        loop {
            let sessions = self.sessions.lock().await;

            // Check if there is an unused session we can reuse
            for session in sessions.iter() {
                let mut in_use = session.in_use.lock().await;
                if !*in_use {
                    *in_use = true;
                    return Ok(session.clone());
                }
            }

            // If all sessions are in use, but we haven't reached max_sessions, create a new one
            let can_create = sessions.len() < self.max_sessions;
            drop(sessions);

            if can_create {
                let client = CdpClient::new(&self.endpoint, Duration::from_secs(30)).await?;
                client.send_raw_command("Page.enable", NoParams).await?;
                let session = Arc::new(BrowserSession::new(client));
                *session.in_use.lock().await = true;

                let mut sessions_lock = self.sessions.lock().await;
                if sessions_lock.len() < self.max_sessions {
                    sessions_lock.push(session.clone());
                    return Ok(session);
                } else {
                    *session.in_use.lock().await = false;
                }
            }

            // Otherwise, wait for a session to be released
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn release(&self, session: Arc<BrowserSession>) -> Result<()> {
        let mut in_use = session.in_use.lock().await;
        *in_use = false;
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        if let Some(mut child) = self.process.lock().await.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub async fn process_id(&self) -> Option<u32> {
        let guard = self.process.lock().await;
        guard.as_ref().map(|c| c.id())
    }
}

impl Drop for BrowserManager {
    fn drop(&mut self) {
        if let Ok(mut guard) = self.process.try_lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
