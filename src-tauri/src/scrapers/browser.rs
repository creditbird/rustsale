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
    in_use: Arc<Mutex<bool>>,
}

impl BrowserSession {
    pub fn new(client: CdpClient) -> Self {
        Self {
            client: Arc::new(client),
            in_use: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn navigate(&self, url: &str) -> Result<()> {
        self.client.send_raw_command("Page.navigate", json!({"url": url})).await?;
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
    endpoint: String,
    sessions: Arc<Mutex<Vec<Arc<BrowserSession>>>>,
    max_sessions: usize,
    process: Arc<Mutex<Option<Child>>>,
}

impl BrowserManager {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            endpoint: "127.0.0.1:9222".to_string(),
            sessions: Arc::new(Mutex::new(Vec::new())),
            max_sessions,
            process: Arc::new(Mutex::new(Option::None)),
        }
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
        cmd.arg("--remote-debugging-port=9222")
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
        let client = CdpClient::new(&self.endpoint, Duration::from_secs(30)).await?;
        client.send_raw_command("Page.enable", NoParams).await?;
        let session = Arc::new(BrowserSession::new(client));
        self.sessions.lock().await.push(session.clone());
        Ok(session)
    }

    pub async fn close(&self) -> Result<()> {
        if let Some(mut child) = self.process.lock().await.take() {
            let _ = child.kill();
        }
        Ok(())
    }
}
