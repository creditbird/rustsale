# 2026-07-17 Google Maps Lead Search & Zalo Outreach Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate Google Maps business scraping via CloakBrowser (keyword & grid modes) and Zalo template outreach messaging via `zca-rs` directly into the CRM application.

**Architecture:**
- **Scraper Engine:** Port the browser control module from the legacy `RustSale` reference using `cdp-lite` to launch and automate `cloakbrowser` (stealth Chromium) in headless mode. The scraper automatically writes extracted businesses into the local SQLite database.
- **Outreach Engine:** Add the local `zca-rs` crate as a dependency in Tauri, implement QR code login flow, serialize session credentials, search users by phone number, format templates dynamically, and send direct messages.
- **UI Screens:** Add two sidebar navigation options: "Lead Scraper" (with keyword/grid controls) and "Zalo Outreach" (template manager and queue sender).

**Tech Stack:**
- Backend: Rust, Tauri v2, rusqlite, cdp-lite v0.1, zca-rs (local path dependency)
- Frontend: Svelte 5, TypeScript, Vanilla CSS variables

## Global Constraints
- Tauri version: v2 (latest stable)
- Frontend package manager: bun
- Database: local SQLite file (saved under app data directory)
- Styling: strict Light Mode, Vanilla CSS variables, zero Tailwind CSS.
- Svelte MCP: verify Svelte 5 syntax before writing complex runes.

---

### Task 1: Scraper Scaffolding and Database Extensions

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Test: `src-tauri/src/db_test.rs`

**Interfaces:**
- Consumes: Existing database initialization functions.
- Produces: New database tables: `zalo_sessions`, `outreach_templates`, and extended database connection handlers.

- [ ] **Step 1: Update Cargo.toml dependencies**
  Modify `src-tauri/Cargo.toml` to add `cdp-lite`, `rand`, `urlencoding`, and `zca-rs`:
  ```toml
  [dependencies]
  # Existing dependencies...
  cdp-lite = "0.1"
  rand = "0.8"
  urlencoding = "2.1"
  zca-rs = { path = "../zca-rs" }
  ```

- [ ] **Step 2: Define new DB models**
  Modify `src-tauri/src/models.rs` to add structs for templates and sessions:
  ```rust
  #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
  pub struct OutreachTemplate {
      pub id: Option<i32>,
      pub name: String,
      pub content: String,
      pub created_at: String,
  }

  #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
  pub struct ZaloSession {
      pub imei: String,
      pub user_agent: String,
      pub cookies_json: String,
      pub language: String,
  }
  ```

- [ ] **Step 3: Update SQLite schema**
  Modify `src-tauri/src/db.rs` to initialize tables for Zalo sessions and templates:
  ```rust
  // Inside init_db function, append:
  conn.execute(
      "CREATE TABLE IF NOT EXISTS zalo_sessions (
          imei TEXT PRIMARY KEY,
          user_agent TEXT NOT NULL,
          cookies_json TEXT NOT NULL,
          language TEXT NOT NULL
      )",
      [],
  )?;

  conn.execute(
      "CREATE TABLE IF NOT EXISTS outreach_templates (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          content TEXT NOT NULL,
          created_at TEXT NOT NULL
      )",
      [],
  )?;
  ```

- [ ] **Step 4: Run backend tests**
  Run: `cargo test` in `src-tauri` directory.
  Expected: PASS

- [ ] **Step 5: Commit changes**
  ```bash
  git add src-tauri/Cargo.toml src-tauri/src/db.rs src-tauri/src/models.rs
  git commit -m "feat: add zalo and template schemas to database"
  ```

---

### Task 2: Headless CloakBrowser Manager implementation

**Files:**
- Create: `src-tauri/src/scrapers/browser.rs`
- Create: `src-tauri/src/scrapers/mod.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: System environment variables and system paths.
- Produces: `BrowserManager` struct and `BrowserSession` handling page navigations and evaluation via CDP.

- [ ] **Step 1: Implement browser.rs**
  Create `src-tauri/src/scrapers/browser.rs` to launch CloakBrowser (or Chrome) with anti-detection flags and handle CDP operations:
  ```rust
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
              "/home/arch/.local/share/containers/storage/overlay/cdfe2df8ba2e3b8c5b1fc1fe76820c0d083daff4e4ad7eb2ef8f67d4badba20d/diff/root/.cloakbrowser/cloakbrowser",
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
  ```

- [ ] **Step 2: Create scrapers/mod.rs**
  Create `src-tauri/src/scrapers/mod.rs` to expose the browser module:
  ```rust
  pub mod browser;
  ```

- [ ] **Step 3: Update src-tauri/src/lib.rs**
  Register the scrapers module:
  ```rust
  // At the top of src-tauri/src/lib.rs:
  mod scrapers;
  ```

- [ ] **Step 4: Verify Compilation**
  Run: `cargo check` in `src-tauri`
  Expected: Success without errors.

- [ ] **Step 5: Commit changes**
  ```bash
  git add src-tauri/src/scrapers/
  git commit -m "feat: implement headless CloakBrowser automation layer"
  ```

---

### Task 3: Google Maps Scraper (Keyword & Grid Modes)

**Files:**
- Create: `src-tauri/src/scrapers/google_maps.rs`
- Create: `src-tauri/src/scrapers/grid.rs`
- Modify: `src-tauri/src/scrapers/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `BrowserSession` actions (navigate, evaluate).
- Produces: Tauri IPC Command `start_google_maps_scrape` which writes leads to SQLite database.

- [ ] **Step 1: Implement google_maps.rs**
  Create `src-tauri/src/scrapers/google_maps.rs` containing Maps-specific DOM selector extraction:
  ```rust
  use anyhow::Result;
  use std::sync::Arc;
  use crate::scrapers::browser::BrowserSession;
  use crate::models::Lead;

  pub struct GoogleMapsScraper {
      session: Arc<BrowserSession>,
  }

  impl GoogleMapsScraper {
      pub fn new(session: Arc<BrowserSession>) -> Self {
          Self { session }
      }

      pub async fn search(&self, query: &str) -> Result<Vec<Lead>> {
          let url = format!("https://www.google.com/maps/search/{}?hl=en", query.replace(' ', "+"));
          self.session.navigate(&url).await?;
          tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

          let scroll_script = r#"
              (async () => {
                  const feed = document.querySelector('div[role="feed"]');
                  if (feed) {
                      for (let i = 0; i < 5; i++) {
                          feed.scrollBy(0, 1000);
                          await new Promise(r => setTimeout(r, 1000));
                      }
                  }
              })()
          "#;
          let _ = self.session.evaluate(scroll_script).await?;

          let extract_script = r#"
              (() => {
                  const results = [];
                  const cards = document.querySelectorAll('a[href*="/maps/place/"]');
                  cards.forEach(card => {
                      const container = card.parentElement;
                      const name = container.querySelector('div.fontHeadlineSmall')?.textContent || '';
                      const phone = container.querySelector('span.UsdlK')?.textContent || '';
                      if (name) {
                          results.push({ name, company: name, phone });
                      }
                  });
                  return results;
              })()
          "#;
          let value = self.session.evaluate(extract_script).await?;
          let mut leads = Vec::new();
          if let Some(arr) = value.as_array() {
              for item in arr {
                  let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                  let company = item.get("company").and_then(|v| v.as_str()).unwrap_or("").to_string();
                  let phone = item.get("phone").and_then(|v| v.as_str()).unwrap_or("").to_string();
                  leads.push(Lead {
                      id: None,
                      name,
                      company,
                      status: "New".to_string(),
                      value: 0.0,
                      last_activity: chrono::Utc::now().to_rfc3339(),
                      owner: "Self".to_string(),
                      email: "".to_string(),
                      phone,
                      about: Some("Scraped from Google Maps".to_string()),
                  });
              }
          }
          Ok(leads)
      }
  }
  ```

- [ ] **Step 2: Implement grid.rs**
  Create `src-tauri/src/scrapers/grid.rs` to generate grid coordinates and adaptively search multiple cells:
  ```rust
  pub struct GridCell {
      pub lat: f64,
      pub lng: f64,
  }

  pub fn generate_grid(lat_min: f64, lat_max: f64, lng_min: f64, lng_max: f64, step_km: f64) -> Vec<GridCell> {
      let step_deg = step_km / 111.0;
      let mut cells = Vec::new();
      let mut lat = lat_min;
      while lat < lat_max {
          let mut lng = lng_min;
          while lng < lng_max {
              cells.push(GridCell { lat, lng });
              lng += step_deg;
          }
          lat += step_deg;
      }
      cells
  }
  ```

- [ ] **Step 3: Register modules in scrapers/mod.rs**
  ```rust
  pub mod browser;
  pub mod google_maps;
  pub mod grid;
  ```

- [ ] **Step 4: Create Tauri Commands for Scraping**
  Modify `src-tauri/src/commands.rs` to add the `start_google_maps_scrape` command:
  ```rust
  // Append new command:
  #[tauri::command]
  pub async fn start_google_maps_scrape(
      keyword: String,
      region: String,
      is_grid: bool,
      lat_min: Option<f64>,
      lat_max: Option<f64>,
      lng_min: Option<f64>,
      lng_max: Option<f64>,
      step_km: Option<f64>,
      state: tauri::State<'_, DbState>,
  ) -> Result<usize, String> {
      let manager = crate::scrapers::browser::BrowserManager::new(1);
      manager.start(true).await.map_err(|e| e.to_string())?;
      let session = manager.acquire().await.map_err(|e| e.to_string())?;
      let scraper = crate::scrapers::google_maps::GoogleMapsScraper::new(session);

      let mut total_added = 0;
      if is_grid {
          let cells = crate::scrapers::grid::generate_grid(
              lat_min.unwrap_or(10.0),
              lat_max.unwrap_or(11.0),
              lng_min.unwrap_or(106.0),
              lng_max.unwrap_or(107.0),
              step_km.unwrap_or(2.0),
          );
          for cell in cells {
              let query = format!("{} near {},{}", keyword, cell.lat, cell.lng);
              if let Ok(leads) = scraper.search(&query).await {
                  let mut conn = state.conn.lock().unwrap();
                  for lead in leads {
                      let exists: i64 = conn.query_row(
                          "SELECT COUNT(*) FROM leads WHERE phone = ?1",
                          [&lead.phone],
                          |row| row.get(0)
                      ).unwrap_or(0);
                      if exists == 0 && !lead.phone.is_empty() {
                          conn.execute(
                              "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) 
                               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                              (
                                  &lead.name,
                                  &lead.company,
                                  &lead.status,
                                  lead.value,
                                  &lead.last_activity,
                                  &lead.owner,
                                  &lead.email,
                                  &lead.phone,
                                  &lead.about,
                              ),
                          ).unwrap();
                          total_added += 1;
                      }
                  }
              }
          }
      } else {
          let query = format!("{} {}", keyword, region);
          if let Ok(leads) = scraper.search(&query).await {
              let mut conn = state.conn.lock().unwrap();
              for lead in leads {
                  let exists: i64 = conn.query_row(
                      "SELECT COUNT(*) FROM leads WHERE phone = ?1",
                      [&lead.phone],
                      |row| row.get(0)
                  ).unwrap_or(0);
                  if exists == 0 && !lead.phone.is_empty() {
                      conn.execute(
                          "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) 
                           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                          (
                              &lead.name,
                              &lead.company,
                              &lead.status,
                              lead.value,
                              &lead.last_activity,
                              &lead.owner,
                              &lead.email,
                              &lead.phone,
                              &lead.about,
                          ),
                      ).unwrap();
                      total_added += 1;
                  }
              }
          }
      }
      let _ = manager.close().await;
      Ok(total_added)
  }
  ```

- [ ] **Step 5: Register commands in lib.rs**
  Modify `src-tauri/src/lib.rs`:
  ```rust
  // inside tauri::generate_handler! add:
  commands::start_google_maps_scrape
  ```

---

### Task 4: Zalo Client Core and QR Login Flow

**Files:**
- Create: `src-tauri/src/zalo/client.rs`
- Create: `src-tauri/src/zalo/mod.rs`
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: SQLite database connection (DbState).
- Produces: Tauri IPC Commands `start_zalo_login` and `get_zalo_status`.

- [ ] **Step 1: Create zalo/mod.rs**
  Create `src-tauri/src/zalo/mod.rs`:
  ```rust
  pub mod client;
  ```

- [ ] **Step 2: Implement Zalo Client Wrapper**
  Create `src-tauri/src/zalo/client.rs` to interact with `zca-rs` APIs:
  ```rust
  use zca_rs::{Zalo, Options};
  use zca_rs::apis::auth::LoginQREvent;
  use std::sync::Mutex;
  use lazy_static::lazy_static;

  lazy_static! {
      pub static ref ACTIVE_ZALO: Mutex<Option<Zalo>> = Mutex::new(None);
  }

  pub async fn setup_zalo_login<F>(on_qr: F) -> Result<Zalo, String> 
  where 
      F: Fn(String, String) + Send + Sync + 'static 
  {
      let options = Options::default();
      let zalo = Zalo::login_qr(options, move |event| {
          if let LoginQREvent::QRCodeGenerated { code, image_base64, .. } = event {
              on_qr(code, image_base64);
          }
      }).await.map_err(|e| e.to_string())?;

      Ok(zalo)
  }
  ```

- [ ] **Step 3: Implement Zalo Tauri Commands**
  Modify `src-tauri/src/commands.rs` to implement Zalo commands:
  ```rust
  use std::sync::Mutex;

  lazy_static::lazy_static! {
      static ref LAST_QR: Mutex<Option<(String, String)>> = Mutex::new(None);
  }

  #[tauri::command]
  pub async fn start_zalo_login(app_handle: tauri::AppHandle) -> Result<(), String> {
      tokio::spawn(async move {
          let _ = crate::zalo::client::setup_zalo_login(move |code, image| {
              *LAST_QR.lock().unwrap() = Some((code.clone(), image.clone()));
              let _ = app_handle.emit("zalo-qr", serde_json::json!({
                  "code": code,
                  "image": image
              }));
          }).await.map(|z| {
              *crate::zalo::client::ACTIVE_ZALO.lock().unwrap() = Some(z);
          });
      });
      Ok(())
  }

  #[tauri::command]
  pub async fn get_zalo_status() -> Result<serde_json::Value, String> {
      let active = crate::zalo::client::ACTIVE_ZALO.lock().unwrap().is_some();
      Ok(serde_json::json!({
          "connected": active
      }))
  }
  ```

- [ ] **Step 4: Register Zalo commands in lib.rs**
  Modify `src-tauri/src/lib.rs` to expose Zalo commands:
  ```rust
  // inside tauri::generate_handler! add:
  commands::start_zalo_login,
  commands::get_zalo_status
  ```

---

### Task 5: Zalo Message Outreach & Template Management

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: outreach_templates database table and `ACTIVE_ZALO` instance.
- Produces: Tauri IPC Commands `get_templates`, `save_template`, `delete_template`, and `send_zalo_message`.

- [ ] **Step 1: Implement template CRUD commands**
  Modify `src-tauri/src/commands.rs` to implement CRUD for message templates:
  ```rust
  use crate::models::OutreachTemplate;

  #[tauri::command]
  pub fn get_templates(state: tauri::State<'_, DbState>) -> Result<Vec<OutreachTemplate>, String> {
      let conn = state.conn.lock().unwrap();
      let mut stmt = conn.prepare("SELECT id, name, content, created_at FROM outreach_templates").map_err(|e| e.to_string())?;
      let rows = stmt.query_map([], |row| {
          Ok(OutreachTemplate {
              id: row.get(0)?,
              name: row.get(1)?,
              content: row.get(2)?,
              created_at: row.get(3)?,
          })
      }).map_err(|e| e.to_string())?;

      let mut list = Vec::new();
      for r in rows {
          if let Ok(t) = r {
              list.push(t);
          }
      }
      Ok(list)
  }

  #[tauri::command]
  pub fn save_template(name: String, content: String, state: tauri::State<'_, DbState>) -> Result<(), String> {
      let conn = state.conn.lock().unwrap();
      let created = chrono::Utc::now().to_rfc3339();
      conn.execute(
          "INSERT INTO outreach_templates (name, content, created_at) VALUES (?1, ?2, ?3)",
          (&name, &content, &created),
      ).map_err(|e| e.to_string())?;
      Ok(())
  }

  #[tauri::command]
  pub fn delete_template(id: i32, state: tauri::State<'_, DbState>) -> Result<(), String> {
      let conn = state.conn.lock().unwrap();
      conn.execute("DELETE FROM outreach_templates WHERE id = ?1", [id]).map_err(|e| e.to_string())?;
      Ok(())
  }
  ```

- [ ] **Step 2: Implement template variable resolution and send_zalo_message**
  Modify `src-tauri/src/commands.rs` to search user profile by phone and transmit outreach message:
  ```rust
  use zca_rs::models::message::MessageContent;
  use zca_rs::models::ThreadType;

  #[tauri::command]
  pub async fn send_zalo_message(
      lead_id: i32,
      template_id: i32,
      state: tauri::State<'_, DbState>,
  ) -> Result<(), String> {
      let (phone, name, company) = {
          let conn = state.conn.lock().unwrap();
          conn.query_row(
              "SELECT phone, name, company FROM leads WHERE id = ?1",
              [lead_id],
              |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
          ).map_err(|e| e.to_string())?
      };

      if phone.is_empty() {
          return Err("Lead has no phone number".to_string());
      }

      let template_content: String = {
          let conn = state.conn.lock().unwrap();
          conn.query_row(
              "SELECT content FROM outreach_templates WHERE id = ?1",
              [template_id],
              |row| row.get(0)
          ).map_err(|e| e.to_string())?
      };

      // Interpolate placeholders
      let resolved_msg = template_content
          .replace("{name}", &name)
          .replace("{company}", &company);

      let zalo_lock = crate::zalo::client::ACTIVE_ZALO.lock().unwrap();
      let zalo = zalo_lock.as_ref().ok_or("Zalo not logged in")?;

      // Find user uid on Zalo by phone
      let user = zalo.find_user(&phone, None).await.map_err(|e| e.to_string())?
          .ok_or("Phone number not found on Zalo")?;

      // Construct and send message
      let payload = MessageContent {
          msg: resolved_msg,
          styles: None,
          urgency: None,
          quote: None,
          mentions: None,
          attachments: None,
          ttl: None,
      };

      zalo.send_message(payload, &user.uid, ThreadType::User).await.map_err(|e| e.to_string())?;

      // Write activity log
      {
          let conn = state.conn.lock().unwrap();
          let created = chrono::Utc::now().to_rfc3339();
          conn.execute(
              "INSERT INTO activities (lead_id, activity_type, content, created_at, performed_by) 
               VALUES (?1, ?2, ?3, ?4, ?5)",
              (lead_id, "Zalo Outreach", "Outreach message successfully sent via Zalo", &created, "Self"),
          ).map_err(|e| e.to_string())?;
      }

      Ok(())
  }
  ```

- [ ] **Step 3: Register commands in lib.rs**
  Modify `src-tauri/src/lib.rs` to register the new commands:
  ```rust
  // inside tauri::generate_handler! add:
  commands::get_templates,
  commands::save_template,
  commands::delete_template,
  commands::send_zalo_message
  ```

- [ ] **Step 4: Verify Compilation**
  Run: `cargo check` in `src-tauri`
  Expected: Success without errors.

---

### Task 6: Svelte frontend Outreach & Scraper Panels

**Files:**
- Create: `src/lib/components/LeadScraper.svelte`
- Create: `src/lib/components/OutreachPanel.svelte`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: `start_google_maps_scrape`, `start_zalo_login`, `get_zalo_status`, `get_templates`, `save_template`, `send_zalo_message`.
- Produces: Fully interactive UI for Lead scraping and Zalo marketing panels.

- [ ] **Step 1: Create LeadScraper.svelte**
  Create `src/lib/components/LeadScraper.svelte` with controls for search keyword, coordinates grid, and triggering the scrape command:
  ```html
  <script lang="ts">
    import { invoke } from '@tauri-apps/api/core';

    let keyword = $state('');
    let region = $state('');
    let isGrid = $state(false);
    let latMin = $state(10.0);
    let latMax = $state(11.0);
    let lngMin = $state(106.0);
    let lngMax = $state(107.0);
    let stepKm = $state(2.0);

    let status = $state('Idle');
    let resultsCount = $state<number | null>(null);

    async function handleScrape() {
      status = 'Scraping...';
      try {
        const added = await invoke<number>('start_google_maps_scrape', {
          keyword,
          region,
          isGrid,
          latMin: isGrid ? latMin : null,
          latMax: isGrid ? latMax : null,
          lngMin: isGrid ? lngMin : null,
          lngMax: isGrid ? lngMax : null,
          stepKm: isGrid ? stepKm : null,
        });
        resultsCount = added;
        status = 'Completed';
      } catch (err) {
        status = 'Error: ' + err;
      }
    }
  </script>

  <div class="scraper-panel">
    <h2>Google Maps Scraper</h2>
    <div class="form-group">
      <label for="keyword">Keyword</label>
      <input type="text" id="keyword" bind:value={keyword} placeholder="Spa, Cafe..." />
    </div>

    <div class="form-group">
      <label>
        <input type="checkbox" bind:checked={isGrid} />
        Enable Grid Search (Location coords Bounding Box)
      </label>
    </div>

    {#if !isGrid}
      <div class="form-group">
        <label for="region">Region</label>
        <input type="text" id="region" bind:value={region} placeholder="Ho Chi Minh" />
      </div>
    {:else}
      <div class="coords-grid">
        <input type="number" step="0.0001" bind:value={latMin} placeholder="Min Lat" />
        <input type="number" step="0.0001" bind:value={latMax} placeholder="Max Lat" />
        <input type="number" step="0.0001" bind:value={lngMin} placeholder="Min Lng" />
        <input type="number" step="0.0001" bind:value={lngMax} placeholder="Max Lng" />
        <input type="number" step="0.1" bind:value={stepKm} placeholder="Grid Step (km)" />
      </div>
    {/if}

    <button onclick={handleScrape} class="btn-primary">Start Scrape</button>
    <p>Status: {status}</p>
    {#if resultsCount !== null}
      <p>Successfully added {resultsCount} new leads.</p>
    {/if}
  </div>

  <style>
    .scraper-panel {
      padding: var(--spacing-lg);
      background: var(--color-surface-container-lowest);
      border: 1px solid var(--color-outline-variant);
      border-radius: var(--rounded-md);
      display: flex;
      flex-direction: column;
      gap: var(--spacing-md);
    }
    .coords-grid {
      display: grid;
      grid-template-columns: repeat(2, 1fr);
      gap: var(--spacing-sm);
    }
  </style>
  ```

- [ ] **Step 2: Create OutreachPanel.svelte**
  Create `src/lib/components/OutreachPanel.svelte` for Zalo connection QR codes and message triggers:
  ```html
  <script lang="ts">
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';

    let zaloConnected = $state(false);
    let qrImage = $state('');
    let templateName = $state('');
    let templateContent = $state('');
    let templates = $state<any[]>([]);

    $effect(() => {
      checkZaloStatus();
      loadTemplates();

      const unlisten = listen('zalo-qr', (event: any) => {
        qrImage = event.payload.image as string;
      });

      return () => {
        unlisten.then(f => f());
      };
    });

    async function checkZaloStatus() {
      const res: any = await invoke('get_zalo_status');
      zaloConnected = res.connected;
    }

    async function loadTemplates() {
      templates = await invoke('get_templates');
    }

    async function connectZalo() {
      await invoke('start_zalo_login');
    }

    async function handleSaveTemplate() {
      await invoke('save_template', { name: templateName, content: templateContent });
      templateName = '';
      templateContent = '';
      loadTemplates();
    }
  </script>

  <div class="outreach-panel">
    <h2>Zalo Messaging Outreach</h2>

    <div class="zalo-status">
      {#if zaloConnected}
        <span class="status-badge connected">Zalo Connected</span>
      {:else}
        <span class="status-badge disconnected">Zalo Disconnected</span>
        <button onclick={connectZalo} class="btn-primary">Get QR Login Code</button>
      {/if}
    </div>

    {#if qrImage && !zaloConnected}
      <div class="qr-box">
        <p>Scan this QR code with your Zalo App to Login:</p>
        <img src="data:image/png;base64,{qrImage}" alt="Zalo QR Login" />
      </div>
    {/if}

    <div class="template-manager">
      <h3>Manage Outreach Templates</h3>
      <input type="text" bind:value={templateName} placeholder="Template Title" />
      <textarea bind:value={templateContent} placeholder="Message content... Use {name} and {company} placeholders"></textarea>
      <button onclick={handleSaveTemplate} class="btn-secondary">Save Template</button>

      <ul>
        {#each templates as t}
          <li>
            <strong>{t.name}</strong>: {t.content}
          </li>
        {/each}
      </ul>
    </div>
  </div>

  <style>
    .outreach-panel {
      padding: var(--spacing-lg);
      background: var(--color-surface-container-lowest);
      border: 1px solid var(--color-outline-variant);
      border-radius: var(--rounded-md);
      display: flex;
      flex-direction: column;
      gap: var(--spacing-md);
    }
    .status-badge.connected {
      background: #e0f2fe;
      color: #0369a1;
      padding: var(--spacing-xs) var(--spacing-sm);
      border-radius: var(--rounded-full);
    }
    .status-badge.disconnected {
      background: rgba(244, 63, 94, 0.1);
      color: var(--color-danger);
      padding: var(--spacing-xs) var(--spacing-sm);
      border-radius: var(--rounded-full);
      margin-right: var(--spacing-sm);
    }
    .qr-box img {
      width: 200px;
      height: 200px;
    }
  </style>
  ```

- [ ] **Step 3: Update Sidebar.svelte**
  Add items in `src/lib/components/Sidebar.svelte` for "Lead Scraper" and "Outreach".

- [ ] **Step 4: Update +page.svelte**
  Integrate and switch tabs in `src/routes/+page.svelte` to render the newly created components.

- [ ] **Step 5: Validate build**
  Run: `bun run check && bun run build`
  Expected: Success without compilation errors.
