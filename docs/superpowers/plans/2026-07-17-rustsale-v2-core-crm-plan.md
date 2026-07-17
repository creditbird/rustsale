# RustSale v2 - Core CRM & Base Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the base shell layout, dashboard, leads list, contact detail, and Kanban deal pipeline in a Tauri v2 + Svelte 5 (TypeScript) desktop app, storing data in SQLite via Rust command handlers.

**Architecture:** A local-first desktop application with a Rust backend executing raw SQLite queries via the synchronous `rusqlite` crate. Giao tiếp qua Tauri IPC (`tauri::command` and Svelte frontend `invoke()`). Giao diện sử dụng Svelte 5 Runes để cập nhật trạng thái động, được thiết kế hoàn toàn bằng Vanilla CSS (CSS variables) theo đúng chuẩn thiết kế của Stitch.

**Tech Stack:** Tauri v2, Svelte 5 (Vite + TS), Bun (package manager), SQLite (`rusqlite` version "0.31"), Vanilla CSS.

## Global Constraints

- Tauri version: v2 (latest stable)
- Frontend package manager: bun
- Database: local SQLite file (saved under app data directory)
- Styling: strict Light Mode, Vanilla CSS variables, zero Tailwind CSS.
- Svelte MCP tools: always search or verify Svelte 5 documentation using `svelte` MCP server (e.g., `get-documentation`) before writing complex runes.

---

## 1. File Structure Map

Before writing code, we map out the exact files to be created and their roles:

- `src-tauri/src/db.rs`: Database connection, schema migrations, and connection pool wrapper.
- `src-tauri/src/models.rs`: Data structures (`Lead`, `Activity`, `Deal`) representing DB records.
- `src-tauri/src/commands.rs`: Tauri command handlers invoked by Svelte.
- `src/index.css`: Global styles, design tokens (colors, fonts, layout variables).
- `src/lib/services/crmService.ts`: Frontend typescript service calling Tauri backend `invoke`.
- `src/lib/components/Sidebar.svelte`: Standard left navigation panel.
- `src/lib/components/Dashboard.svelte`: Stats card widgets, funnel container, schedule lists.
- `src/lib/components/LeadsList.svelte`: Data table displaying leads, with search and Quick View drawer.
- `src/lib/components/LeadDetails.svelte`: Details editor and activity timeline.
- `src/lib/components/KanbanBoard.svelte`: Stage-based drag-and-drop Deal cards.
- `src/App.svelte`: Page router and main window wrapper.

---

## 2. Tasks

### Task 1: Tauri v2 + Svelte 5 Scaffolding
Initialize the project structure with Tauri v2 and Svelte 5 using `bunx create-tauri-app`.

**Files:**
- Create: `package.json`
- Create: `src-tauri/Cargo.toml`
- Create: `vite.config.ts`

**Interfaces:**
- Consumes: None
- Produces: Base Tauri + Svelte 5 TS workspace.

- [ ] **Step 1: Check template setup and scaffold project**

Run: `bunx create-tauri-app ./ --manager bun --template svelte-ts --tauri-version 2 --yes --force`
Expected: Project folders (`src/`, `src-tauri/`) initialized inside the current directory.

- [ ] **Step 2: Install additional dev dependencies**

Run: `bun install`
Expected: Frontend packages installed.

- [ ] **Step 3: Verify the base app compile and runs**

Run: `bun run tauri dev` (Note: We run this to ensure target folder compiles. If you're running headless, compile via `cargo check` inside `src-tauri`).
Expected: App compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add package.json src-tauri/Cargo.toml src-tauri/tauri.conf.json vite.config.ts
git commit -m "feat: scaffold tauri v2 + svelte 5 ts project using bun"
```

---

### Task 2: Database Initialization & Rust Models
Configure the SQLite integration in the Rust backend, define models, and write initial schema migrations.

**Files:**
- Create: `src-tauri/src/models.rs`
- Create: `src-tauri/src/db.rs`
- Modify: `src-tauri/Cargo.toml` (Add `rusqlite` and `serde` dependencies)

**Interfaces:**
- Consumes: SQLite database connection.
- Produces: `db::init_db(db_path: &str)` and structs `Lead`, `Activity`, `Deal`.

- [ ] **Step 1: Add Rust dependencies to Cargo.toml**

Append to `src-tauri/Cargo.toml` dependencies:
```toml
rusqlite = { version = "0.31.0", features = ["bundled"] }
serde = { version = "1.0", features = ["derive"] }
```

- [ ] **Step 2: Write failing Rust unit test for database initialization**

Create: `src-tauri/src/db_test.rs`
```rust
#[cfg(test)]
mod tests {
    use super::super::db;
    use std::fs;

    #[test]
    fn test_db_init_and_tables() {
        let temp_db = "test_crm.db";
        let _ = fs::remove_file(temp_db);
        let conn = db::init_db(temp_db).expect("Failed to init db");
        
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table'").unwrap();
        let tables: Vec<String> = stmt.query_map([], |row| row.get(0)).unwrap()
            .map(|r| r.unwrap()).collect();
            
        assert!(tables.contains(&"leads".to_string()));
        assert!(tables.contains(&"activities".to_string()));
        assert!(tables.contains(&"deals".to_string()));
        let _ = fs::remove_file(temp_db);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: Fails because module `db` and function `init_db` do not exist.

- [ ] **Step 4: Write Rust models**

Create: `src-tauri/src/models.rs`
```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Lead {
    pub id: Option<i32>,
    pub name: String,
    pub company: String,
    pub status: String,
    pub value: f64,
    pub last_activity: String,
    pub owner: String,
    pub email: String,
    pub phone: String,
    pub about: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Activity {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub activity_type: String,
    pub content: String,
    pub created_at: String,
    pub performed_by: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Deal {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub title: String,
    pub value: f64,
    pub stage: String,
    pub created_at: String,
}
```

- [ ] **Step 5: Write minimal `db.rs` implementation to pass the test**

Create: `src-tauri/src/db.rs`
```rust
use rusqlite::{Connection, Result};

pub fn init_db(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS leads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            company TEXT NOT NULL,
            status TEXT NOT NULL,
            value REAL NOT NULL DEFAULT 0.0,
            last_activity TEXT NOT NULL,
            owner TEXT NOT NULL,
            email TEXT NOT NULL,
            phone TEXT NOT NULL,
            about TEXT
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS activities (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            type TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            performed_by TEXT NOT NULL,
            FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
        )",
        [],
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS deals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            lead_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            value REAL NOT NULL DEFAULT 0.0,
            stage TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE
        )",
        [],
    )?;

    Ok(conn)
}
```
Add to `src-tauri/src/main.rs` (or `src-tauri/src/lib.rs`):
```rust
mod db;
mod models;
#[cfg(test)]
mod db_test;
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 7: Seed initial dummy data**

Modify `src-tauri/src/db.rs` to insert seed data if `leads` table is empty:
```rust
pub fn seed_data(conn: &Connection) -> Result<()> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM leads", [], |row| row.get(0))?;
    if count == 0 {
        conn.execute(
            "INSERT INTO leads (name, company, status, value, last_activity, owner, email, phone, about) VALUES 
             ('Sarah Harrington', 'Acme Corp', 'Qualified', 45000.0, '2026-07-15T10:00:00Z', 'Alex Rivera', 'sarah@acme.com', '+12345678', 'CTO of Acme Corp'),
             ('Jordan Smith', 'CloudSync', 'New', 15000.0, '2026-07-16T15:30:00Z', 'Jordan Smith', 'jordan@cloudsync.com', '+87654321', 'Procurement lead')",
            [],
        )?;
        conn.execute(
            "INSERT INTO deals (lead_id, title, value, stage, created_at) VALUES 
             (1, 'Acme Platform Expansion', 45000.0, 'Proposal', '2026-07-15T10:00:00Z'),
             (2, 'CloudSync Core Migration', 15000.0, 'Prospecting', '2026-07-16T15:30:00Z')",
            [],
        )?;
        conn.execute(
            "INSERT INTO activities (lead_id, type, content, created_at, performed_by) VALUES 
             (1, 'Email', 'Initial discovery email sent.', '2026-07-15T10:00:00Z', 'Alex Rivera')",
            [],
        )?;
    }
    Ok(())
}
```

- [ ] **Step 8: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/db.rs src-tauri/src/models.rs src-tauri/src/db_test.rs src-tauri/src/main.rs
git commit -m "feat: implement database init, models, and mock data seeding"
```

---

### Task 3: Tauri Command Handlers for CRM Operations
Expose database CRUD operations to the frontend via Tauri State command handlers.

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/main.rs` (or `src-tauri/src/lib.rs` to register commands and state)

**Interfaces:**
- Consumes: SQLite Connection via Tauri State.
- Produces: `get_leads`, `get_lead_details`, `add_lead`, `update_lead_status`, `get_deals`, `update_deal_stage`, `get_activities`, `add_activity` commands.

- [ ] **Step 1: Write failing Rust integration test for command handlers**

Create: `src-tauri/src/commands_test.rs`
```rust
#[cfg(test)]
mod tests {
    use super::super::commands;
    use super::super::db;
    use std::sync::Mutex;
    use rusqlite::Connection;

    struct AppState {
        pub conn: Mutex<Connection>,
    }

    #[test]
    fn test_crm_commands() {
        let conn = db::init_db("test_commands.db").unwrap();
        db::seed_data(&conn).unwrap();
        let state = Mutex::new(conn);

        let leads = commands::get_leads_internal(&state).unwrap();
        assert_eq!(leads.len(), 2);
        assert_eq!(leads[0].name, "Sarah Harrington");

        let _ = std::fs::remove_file("test_commands.db");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: Fails because commands module is missing.

- [ ] **Step 3: Implement `commands.rs`**

Create: `src-tauri/src/commands.rs`
```rust
use std::sync::Mutex;
use rusqlite::Connection;
use crate::models::{Lead, Deal, Activity};

pub struct DbState {
    pub conn: Mutex<Connection>,
}

#[tauri::command]
pub fn get_leads(state: tauri::State<DbState>) -> Result<Vec<Lead>, String> {
    get_leads_internal(&state.conn)
}

pub fn get_leads_internal(lock: &Mutex<Connection>) -> Result<Vec<Lead>, String> {
    let conn = lock.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, name, company, status, value, last_activity, owner, email, phone, about FROM leads").map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map([], |row| {
        Ok(Lead {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            company: row.get(2)?,
            status: row.get(3)?,
            value: row.get(4)?,
            last_activity: row.get(5)?,
            owner: row.get(6)?,
            email: row.get(7)?,
            phone: row.get(8)?,
            about: row.get(9)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_deals(state: tauri::State<DbState>) -> Result<Vec<Deal>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, lead_id, title, value, stage, created_at FROM deals").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(Deal {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            title: row.get(2)?,
            value: row.get(3)?,
            stage: row.get(4)?,
            created_at: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn get_lead_details(id: i32, state: tauri::State<DbState>) -> Result<Lead, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.query_row(
        "SELECT id, name, company, status, value, last_activity, owner, email, phone, about FROM leads WHERE id = ?1",
        [id],
        |row| {
            Ok(Lead {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                company: row.get(2)?,
                status: row.get(3)?,
                value: row.get(4)?,
                last_activity: row.get(5)?,
                owner: row.get(6)?,
                email: row.get(7)?,
                phone: row.get(8)?,
                about: row.get(9)?,
            })
        }
    ).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_deal_stage(id: i32, stage: String, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE deals SET stage = ?1 WHERE id = ?2",
        [stage, id.to_string()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn get_activities(lead_id: i32, state: tauri::State<DbState>) -> Result<Vec<Activity>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn.prepare("SELECT id, lead_id, type, content, created_at, performed_by FROM activities WHERE lead_id = ?1").map_err(|e| e.to_string())?;
    let rows = stmt.query_map([lead_id], |row| {
        Ok(Activity {
            id: Some(row.get(0)?),
            lead_id: row.get(1)?,
            activity_type: row.get(2)?,
            content: row.get(3)?,
            created_at: row.get(4)?,
            performed_by: row.get(5)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

#[tauri::command]
pub fn add_activity(lead_id: i32, activity_type: String, content: String, performed_by: String, state: tauri::State<DbState>) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    let now = "2026-07-17T18:00:00Z".to_string(); // In production, generate current ISO-8601
    conn.execute(
        "INSERT INTO activities (lead_id, type, content, created_at, performed_by) VALUES (?1, ?2, ?3, ?4, ?5)",
        [lead_id.to_string(), activity_type, content, now, performed_by],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
```

Add tests to `src-tauri/src/main.rs`:
```rust
mod commands;
#[cfg(test)]
mod commands_test;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 5: Setup Tauri state and command registration in main.rs**

Update: `src-tauri/src/main.rs` (or `src-tauri/src/lib.rs`) to initialize the SQLite file and register the state and commands:
```rust
use std::sync::Mutex;
use tauri::Manager;

fn main() {
    let conn = db::init_db("rustsale.db").expect("Failed to initialize database");
    db::seed_data(&conn).expect("Failed to seed database");

    tauri::Builder::default()
        .manage(commands::DbState {
            conn: Mutex::new(conn),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_leads,
            commands::get_deals,
            commands::get_lead_details,
            commands::update_deal_stage,
            commands::get_activities,
            commands::add_activity
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/commands_test.rs src-tauri/src/main.rs
git commit -m "feat: implement tauri command handlers and connect to sqlite state"
```

---

### Task 4: Base Styling (Design System) & Shell Layout
Write global styles with CSS Custom Properties matching Stitch design rules and implement Sidebar layout.

**Files:**
- Create/Overwrite: `src/index.css`
- Create: `src/lib/components/Sidebar.svelte`
- Modify: `src/App.svelte`

**Interfaces:**
- Consumes: Svelte navigation states.
- Produces: Shared visual layout shell, global typographic and color styles.

- [ ] **Step 1: Write Svelte test checking Sidebar renders with correct tags**

Create: `src/lib/components/__tests__/Sidebar.spec.ts`
```typescript
import { render, screen } from '@testing-library/svelte';
import Sidebar from '../Sidebar.svelte';
import { expect, test } from 'bun:test';

test('renders Sidebar with CRM logo and options', () => {
  render(Sidebar, { activeTab: 'dashboard' });
  expect(screen.getByText('RustSale Pro')).toBeTruthy();
  expect(screen.getByText('Dashboard')).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/lib/components/__tests__/Sidebar.spec.ts`
Expected: Fails because Sidebar component does not exist.

- [ ] **Step 3: Define design system custom properties in index.css**

Overwrite `src/index.css` with the design token variables from Stitch spec (Section 4 of Design Spec).

- [ ] **Step 4: Create Sidebar component using Svelte 5 Runes**

Create: `src/lib/components/Sidebar.svelte`
```html
<script lang="ts">
  let { activeTab = $bindable() } = $props();

  const menuItems = [
    { id: 'dashboard', label: 'Dashboard', icon: 'dashboard' },
    { id: 'leads', label: 'Leads & Contacts', icon: 'people' },
    { id: 'pipeline', label: 'Deal Pipeline', icon: 'view_kanban' }
  ];
</script>

<aside class="sidebar">
  <div class="logo-container">
    <span class="material-icons logo-icon">insights</span>
    <span class="logo-text">RustSale Pro</span>
  </div>
  <nav class="sidebar-nav">
    {#each menuItems as item}
      <button 
        class="nav-item" 
        class:active={activeTab === item.id} 
        onclick={() => activeTab = item.id}
      >
        <span class="material-icons">{item.icon}</span>
        <span class="nav-label">{item.label}</span>
      </button>
    {/each}
  </nav>
</aside>

<style>
  .sidebar {
    width: 240px;
    height: 100vh;
    background: var(--color-primary-container);
    color: var(--color-on-primary-container);
    display: flex;
    flex-direction: column;
    border-right: 1px solid var(--color-outline);
  }
  .logo-container {
    padding: var(--spacing-lg) var(--spacing-md);
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  .logo-icon {
    color: var(--color-success);
    font-size: 24px;
  }
  .logo-text {
    font-family: var(--font-family);
    font-weight: 700;
    font-size: 16px;
    color: var(--color-on-primary);
  }
  .sidebar-nav {
    display: flex;
    flex-direction: column;
    padding: var(--spacing-md) 0;
    gap: var(--spacing-xs);
    flex: 1;
  }
  .nav-item {
    background: none;
    border: none;
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
    color: var(--color-on-primary-container);
    cursor: pointer;
    text-align: left;
    transition: var(--transition-fast);
    font-family: var(--font-family);
    font-size: 14px;
    width: 100%;
  }
  .nav-item:hover {
    background: rgba(255, 255, 255, 0.05);
    color: var(--color-on-primary);
  }
  .nav-item.active {
    background: var(--color-primary);
    color: var(--color-on-primary);
    border-left: 4px solid var(--color-success);
  }
</style>
```

- [ ] **Step 5: Run tests and verify Svelte Sidebar component passes**

Run: `bun test src/lib/components/__tests__/Sidebar.spec.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/index.css src/lib/components/Sidebar.svelte src/lib/components/__tests__/Sidebar.spec.ts
git commit -m "feat: implement global styling variables and sidebar component"
```

---

### Task 5: CRM Dashboard Overview UI
Implement the main dashboard metrics, actual vs target revenue chart, and conversion funnel widgets.

**Files:**
- Create: `src/lib/components/Dashboard.svelte`
- Create: `src/lib/components/__tests__/Dashboard.spec.ts`
- Modify: `src/App.svelte` (Integrate Dashboard screen)

**Interfaces:**
- Consumes: Lead and Deal lists from backend via `crmService.ts`.
- Produces: CRM Dashboard screen interface.

- [ ] **Step 1: Write Svelte test checking Dashboard stats cards**

Create: `src/lib/components/__tests__/Dashboard.spec.ts`
```typescript
import { render, screen } from '@testing-library/svelte';
import Dashboard from '../Dashboard.svelte';
import { expect, test } from 'bun:test';

test('renders Dashboard screen headings', () => {
  render(Dashboard);
  expect(screen.getByText('Sales Performance')).toBeTruthy();
  expect(screen.getByText('Revenue Target vs. Actual')).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun test src/lib/components/__tests__/Dashboard.spec.ts`
Expected: FAIL because Dashboard.svelte doesn't exist.

- [ ] **Step 3: Build CRM Dashboard screen**

Create: `src/lib/components/Dashboard.svelte`
```html
<script lang="ts">
  // Dashboard mock metrics derived from Stitch design rules
  const stats = [
    { label: 'Total Revenue Gen', value: '$124,500.00', change: '+12.4%', up: true },
    { label: 'Deals Won', value: '14', change: '+2', up: true },
    { label: 'Active Leads', value: '26', change: '-4%', up: false }
  ];
</script>

<div class="dashboard-container">
  <header class="dashboard-header">
    <h1>Sales Performance</h1>
  </header>

  <div class="stats-grid">
    {#each stats as stat}
      <div class="stat-card">
        <span class="stat-label">{stat.label}</span>
        <div class="stat-value-container">
          <span class="stat-value">{stat.value}</span>
          <span class="stat-change" class:up={stat.up}>{stat.change}</span>
        </div>
      </div>
    {/each}
  </div>

  <div class="dashboard-grid">
    <div class="dashboard-card">
      <h3>Revenue Target vs. Actual</h3>
      <div class="funnel-container">
        <!-- SVG Chart mock for target vs actual -->
        <svg viewBox="0 0 400 150" class="chart-svg">
          <rect x="20" y="20" width="360" height="20" rx="4" fill="var(--color-surface)"></rect>
          <rect x="20" y="20" width="280" height="20" rx="4" fill="var(--color-success)"></rect>
          <text x="25" y="35" fill="var(--color-on-primary)" font-size="11">Actual: $124.5k (80%)</text>
          <line x1="320" y1="15" x2="320" y2="45" stroke="var(--color-danger)" stroke-dasharray="2"></line>
          <text x="325" y="15" fill="var(--color-danger)" font-size="9">Target: $150k</text>
        </svg>
      </div>
    </div>

    <div class="dashboard-card">
      <h3>Today's Schedule</h3>
      <ul class="schedule-list">
        <li>
          <span class="material-icons">check_circle</span>
          <div>
            <strong>Quarterly Review with CloudSync</strong>
            <p>14:00 - 15:00</p>
          </div>
        </li>
      </ul>
    </div>
  </div>
</div>

<style>
  .dashboard-container {
    padding: var(--spacing-lg);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
    background: var(--color-background);
    flex: 1;
    overflow-y: auto;
  }
  .dashboard-header h1 {
    font-size: 24px;
    font-weight: 700;
    color: var(--color-on-surface);
  }
  .stats-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--spacing-md);
  }
  .stat-card {
    background: var(--color-surface-lowest);
    border: 1px solid var(--color-outline);
    border-radius: var(--radius-md);
    padding: var(--spacing-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }
  .stat-label {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-on-surface-variant);
    font-weight: 700;
  }
  .stat-value {
    font-size: 20px;
    font-weight: 600;
    color: var(--color-on-surface);
  }
  .stat-change {
    font-size: 11px;
    font-weight: bold;
    color: var(--color-danger);
  }
  .stat-change.up {
    color: var(--color-success);
  }
  .dashboard-grid {
    display: grid;
    grid-template-columns: 2fr 1fr;
    gap: var(--spacing-md);
  }
  .dashboard-card {
    background: var(--color-surface-lowest);
    border: 1px solid var(--color-outline);
    border-radius: var(--radius-md);
    padding: var(--spacing-md);
  }
  .dashboard-card h3 {
    margin-bottom: var(--spacing-md);
    font-size: 16px;
    font-weight: 600;
    color: var(--color-on-surface);
  }
  .chart-svg {
    width: 100%;
    height: auto;
  }
  .schedule-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .schedule-list li {
    display: flex;
    gap: var(--spacing-sm);
    padding: var(--spacing-sm) 0;
    border-bottom: 1px solid var(--color-outline);
  }
</style>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `bun test src/lib/components/__tests__/Dashboard.spec.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Dashboard.svelte src/lib/components/__tests__/Dashboard.spec.ts
git commit -m "feat: implement dashboard stats overview with mock chart"
```

---

### Task 6: Leads List, Details Drawer, & Kanban Deal Pipeline
Develop the complete Leads List view, detailed contact profile activities drawer, and drag-and-drop Kanban deal pipeline, querying data from Rust command APIs.

**Files:**
- Create: `src/lib/services/crmService.ts`
- Create: `src/lib/components/LeadsList.svelte`
- Create: `src/lib/components/LeadDetails.svelte`
- Create: `src/lib/components/KanbanBoard.svelte`
- Create: `src/lib/components/__tests__/CRM.spec.ts`
- Modify: `src/App.svelte` (Tie everything together with activeTab state)

**Interfaces:**
- Consumes: Tauri invoke backend APIs.
- Produces: Complete core interactive CRM application dashboard.

- [ ] **Step 1: Write Svelte service checking database endpoints**

Create: `src/lib/services/crmService.ts`
```typescript
import { invoke } from '@tauri-apps/api/core';

export interface Lead {
  id?: number;
  name: string;
  company: string;
  status: string;
  value: number;
  last_activity: string;
  owner: string;
  email: string;
  phone: string;
  about: string;
}

export interface Deal {
  id?: number;
  lead_id: number;
  title: string;
  value: number;
  stage: string;
  created_at: string;
}

export interface Activity {
  id?: number;
  lead_id: number;
  activity_type: string;
  content: string;
  created_at: string;
  performed_by: string;
}

export const crmService = {
  getLeads: (): Promise<Lead[]> => invoke<Lead[]>('get_leads'),
  getDeals: (): Promise<Deal[]> => invoke<Deal[]>('get_deals'),
  getLeadDetails: (id: number): Promise<Lead> => invoke<Lead>('get_lead_details', { id }),
  updateDealStage: (id: number, stage: string): Promise<void> => invoke<void>('update_deal_stage', { id, stage }),
  getActivities: (leadId: number): Promise<Activity[]> => invoke<Activity[]>('get_activities', { leadId }),
  addActivity: (leadId: number, type: string, content: string, performedBy: string): Promise<void> => 
    invoke<void>('add_activity', { leadId, activityType: type, content, performedBy })
};
```

- [ ] **Step 2: Write integration tests for Svelte rendering of Leads**

Create: `src/lib/components/__tests__/CRM.spec.ts`
```typescript
import { render, screen } from '@testing-library/svelte';
import LeadsList from '../LeadsList.svelte';
import { expect, test } from 'bun:test';

test('renders LeadsList with columns', () => {
  render(LeadsList, { leads: [] });
  expect(screen.getByText('Name')).toBeTruthy();
  expect(screen.getByText('Company')).toBeTruthy();
  expect(screen.getByText('Status')).toBeTruthy();
});
```

- [ ] **Step 3: Implement LeadsList, LeadDetails and Kanban components**

Create: `src/lib/components/LeadsList.svelte`
```html
<script lang="ts">
  import type { Lead } from '../services/crmService';
  let { leads = [] } = $props();
  let selectedLeadId = $state<number | null>(null);
</script>

<div class="leads-container">
  <div class="list-header">
    <h2>Leads & Contacts</h2>
  </div>
  <table class="leads-table">
    <thead>
      <tr>
        <th>Name</th>
        <th>Company</th>
        <th>Status</th>
        <th>Value</th>
        <th>Owner</th>
      </tr>
    </thead>
    <tbody>
      {#each leads as lead}
        <tr onclick={() => selectedLeadId = lead.id}>
          <td><strong>{lead.name}</strong></td>
          <td>{lead.company}</td>
          <td><span class="status-badge {lead.status.toLowerCase()}">{lead.status}</span></td>
          <td>${lead.value.toLocaleString()}</td>
          <td>{lead.owner}</td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .leads-container {
    padding: var(--spacing-lg);
    background: var(--color-background);
    flex: 1;
    overflow-y: auto;
  }
  .list-header {
    margin-bottom: var(--spacing-md);
  }
  .leads-table {
    width: 100%;
    border-collapse: collapse;
    background: var(--color-surface-lowest);
    border: 1px solid var(--color-outline);
    border-radius: var(--radius-md);
    overflow: hidden;
  }
  .leads-table th, .leads-table td {
    padding: 12px;
    text-align: left;
    border-bottom: 1px solid var(--color-outline);
  }
  .leads-table th {
    background: var(--color-surface-low);
    color: var(--color-on-surface-variant);
    font-weight: 700;
    text-transform: uppercase;
    font-size: 11px;
    letter-spacing: 0.05em;
  }
  .leads-table tbody tr {
    cursor: pointer;
    transition: var(--transition-fast);
  }
  .leads-table tbody tr:hover {
    background: var(--color-surface-low);
  }
  .status-badge {
    padding: 2px 8px;
    border-radius: var(--radius-full);
    font-size: 11px;
    font-weight: bold;
  }
  .status-badge.new {
    background: #e0f2fe;
    color: #0369a1;
  }
  .status-badge.qualified {
    background: var(--color-success-bg);
    color: var(--color-success);
  }
</style>
```

Create: `src/lib/components/KanbanBoard.svelte`
```html
<script lang="ts">
  import { onMount } from 'svelte';
  import { crmService } from '../services/crmService';
  import type { Deal } from '../services/crmService';
  
  let deals = $state<Deal[]>([]);
  const stages = ['Prospecting', 'Demo', 'Proposal', 'Negotiation', 'Won'];

  async function loadDeals() {
    deals = await crmService.getDeals();
  }

  async function handleDragOver(e: DragEvent) {
    e.preventDefault();
  }

  async function handleDrop(e: DragEvent, targetStage: string) {
    e.preventDefault();
    const dealIdStr = e.dataTransfer?.getData('text/plain');
    if (dealIdStr) {
      const dealId = parseInt(dealIdStr);
      await crmService.updateDealStage(dealId, targetStage);
      await loadDeals();
    }
  }

  onMount(loadDeals);
</script>

<div class="kanban-container">
  <h2>Deal Pipeline</h2>
  <div class="kanban-grid">
    {#each stages as stage}
      <div 
        class="kanban-column"
        ondragover={handleDragOver}
        ondrop={(e) => handleDrop(e, stage)}
      >
        <h3>{stage}</h3>
        <div class="card-list">
          {#each deals.filter(d => d.stage === stage) as deal}
            <div 
              class="deal-card" 
              draggable="true"
              ondragstart={(e) => e.dataTransfer?.setData('text/plain', deal.id!.toString())}
            >
              <h4>{deal.title}</h4>
              <p class="value">${deal.value.toLocaleString()}</p>
            </div>
          {/each}
        </div>
      </div>
    {/each}
  </div>
</div>

<style>
  .kanban-container {
    padding: var(--spacing-lg);
    background: var(--color-background);
    flex: 1;
    overflow-y: auto;
  }
  .kanban-grid {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: var(--spacing-md);
    margin-top: var(--spacing-md);
  }
  .kanban-column {
    background: var(--color-surface-low);
    border-radius: var(--radius-md);
    padding: var(--spacing-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    min-height: 500px;
  }
  .kanban-column h3 {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-on-surface);
    border-bottom: 1px solid var(--color-outline);
    padding-bottom: var(--spacing-sm);
  }
  .card-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }
  .deal-card {
    background: var(--color-surface-lowest);
    border: 1px solid var(--color-outline);
    border-radius: var(--radius-sm);
    padding: var(--spacing-md);
    cursor: grab;
    box-shadow: 0px 2px 4px rgba(15, 23, 42, 0.05);
  }
  .deal-card h4 {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-on-surface);
  }
  .deal-card .value {
    font-size: 12px;
    color: var(--color-success);
    font-weight: bold;
    margin-top: 4px;
  }
</style>
```

- [ ] **Step 4: Update App.svelte Router view**

Update `src/App.svelte` to orchestrate navigation shell and toggle pages:
```html
<script lang="ts">
  import { onMount } from 'svelte';
  import Sidebar from './lib/components/Sidebar.svelte';
  import Dashboard from './lib/components/Dashboard.svelte';
  import LeadsList from './lib/components/LeadsList.svelte';
  import KanbanBoard from './lib/components/KanbanBoard.svelte';
  import { crmService } from './lib/services/crmService';
  import type { Lead } from './lib/services/crmService';

  let activeTab = $state('dashboard');
  let leads = $state<Lead[]>([]);

  async function loadLeads() {
    leads = await crmService.getLeads();
  }

  onMount(loadLeads);
</script>

<div class="app-layout">
  <Sidebar bind:activeTab={activeTab} />
  <main class="main-content">
    {#if activeTab === 'dashboard'}
      <Dashboard />
    {:else if activeTab === 'leads'}
      <LeadsList leads={leads} />
    {:else if activeTab === 'pipeline'}
      <KanbanBoard />
    {/if}
  </main>
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }
  .main-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
```

- [ ] **Step 5: Run Svelte test checking Leads grid renders correctly**

Run: `bun test src/lib/components/__tests__/CRM.spec.ts`
Expected: PASS

- [ ] **Step 6: Final QA gate build check**

Compile: `bun run tauri build` (or check build using dev server `bun run tauri dev`).
Expected: No compile or TypeScript errors.

- [ ] **Step 7: Commit**

```bash
git add src/lib/services/crmService.ts src/lib/components/LeadsList.svelte src/lib/components/KanbanBoard.svelte src/App.svelte src/lib/components/__tests__/CRM.spec.ts
git commit -m "feat: complete leads list with database commands and drag drop kanban deal pipeline"
```
