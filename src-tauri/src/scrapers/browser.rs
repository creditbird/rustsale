#![allow(dead_code)]

use anyhow::{Context, Result};
use cdp_lite::client::CdpClient;
use cdp_lite::protocol::NoParams;
use serde_json::json;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct BrowserSession {
    pub client: Arc<CdpClient>,
    pub(crate) in_use: AtomicBool,
}

impl BrowserSession {
    pub fn new(client: CdpClient) -> Self {
        Self {
            client: Arc::new(client),
            in_use: AtomicBool::new(false),
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

pub struct SessionGuard {
    pub(crate) session: Arc<BrowserSession>,
}

impl std::ops::Deref for SessionGuard {
    type Target = BrowserSession;
    fn deref(&self) -> &Self::Target {
        &self.session
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.session.in_use.store(false, Ordering::SeqCst);
    }
}

struct SessionPool {
    vec: Vec<Arc<BrowserSession>>,
    pending_creations: usize,
}

pub struct BrowserManager {
    port: u16,
    endpoint: String,
    sessions: Arc<Mutex<SessionPool>>,
    max_sessions: usize,
    process: Arc<std::sync::Mutex<Option<Child>>>,
}

impl BrowserManager {
    pub fn new(max_sessions: usize) -> Self {
        Self {
            port: 9222,
            endpoint: "127.0.0.1:9222".to_string(),
            sessions: Arc::new(Mutex::new(SessionPool {
                vec: Vec::new(),
                pending_creations: 0,
            })),
            max_sessions,
            process: Arc::new(std::sync::Mutex::new(Option::None)),
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
        *self.process.lock().unwrap() = Some(child);

        // Wait for browser port to bind
        for _ in 0..10 {
            if CdpClient::new(&self.endpoint, Duration::from_secs(1)).await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        Ok(())
    }

    pub async fn acquire(&self) -> Result<SessionGuard> {
        loop {
            let mut pool = self.sessions.lock().await;

            // 1. Check if there is an unused session we can reuse
            for session in pool.vec.iter() {
                if session.in_use.compare_exchange(
                    false,
                    true,
                    Ordering::SeqCst,
                    Ordering::SeqCst
                ).is_ok() {
                    return Ok(SessionGuard { session: session.clone() });
                }
            }

            // 2. Check if total active + pending sessions is less than max_sessions
            if pool.vec.len() + pool.pending_creations < self.max_sessions {
                pool.pending_creations += 1;
                drop(pool);

                let create_result = async {
                    let client = CdpClient::new(&self.endpoint, Duration::from_secs(30)).await?;
                    client.send_raw_command("Page.enable", NoParams).await?;
                    let session = Arc::new(BrowserSession::new(client));
                    session.in_use.store(true, Ordering::SeqCst);
                    Ok(session)
                }.await;

                let mut pool_lock = self.sessions.lock().await;
                pool_lock.pending_creations -= 1;

                match create_result {
                    Ok(session) => {
                        pool_lock.vec.push(session.clone());
                        return Ok(SessionGuard { session });
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }

            // 3. Otherwise, wait for a session to be released
            drop(pool);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    pub async fn release(&self, session: Arc<BrowserSession>) -> Result<()> {
        session.in_use.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        if let Some(mut child) = self.process.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }

    pub async fn process_id(&self) -> Option<u32> {
        let guard = self.process.lock().unwrap();
        guard.as_ref().map(|c| c.id())
    }
}

impl Drop for BrowserManager {
    fn drop(&mut self) {
        let mut guard = self.process.lock().unwrap();
        if let Some(mut child) = guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
