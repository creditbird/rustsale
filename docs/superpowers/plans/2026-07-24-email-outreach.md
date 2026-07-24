# Email Outreach Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete Email Outreach feature in RustSale_v2 based on the Zalo Outreach architecture, enabling users to add SMTP server credentials, manage email templates with subjects, target CRM leads, and run batch email campaigns with real-time progress logging.

**Architecture:** Extend SQLite database with `smtp_servers`, `email_templates`, and `email_outreach_history` tables. Use Rust `lettre` crate in `src-tauri` for async SMTP transport and template rendering. Create a symmetrical `EmailOutreach.svelte` component on the Svelte frontend and integrate it into the app sidebar navigation.

**Tech Stack:** Rust (Tauri v2, Tokio, Lettre, Rusqlite), Svelte 5 (SvelteKit, TypeScript), Bun.

## Global Constraints
- **ALWAYS use `bun` and `bunx` instead of `npm` and `npx`** for all commands in this repository.
- **MUST run `bunx gitnexus analyze` after committing** code changes to keep the graph index fresh.
- **MUST run impact analysis (`gitnexus_impact`)** before editing existing Rust symbols.
- **TDD:** Write unit tests for new database functions & commands before implementing logic.

---

### Task 1: Add Lettre Dependency & Database Schema Migration

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/db.rs`
- Test: `src-tauri/src/db_test.rs`

**Interfaces:**
- Consumes: Existing SQLite connection pool in `db.rs`
- Produces: SQLite tables `smtp_servers`, `email_templates`, `email_outreach_history`

- [ ] **Step 1: Add lettre to `Cargo.toml`**

Add `lettre` dependency with tokio and native-tls support:
```toml
lettre = { version = "0.11", features = ["tokio1-native-tls", "builder", "smtp-transport"] }
```

- [ ] **Step 2: Write failing unit test in `db_test.rs`**

Add a test in `src-tauri/src/db_test.rs` verifying table creation:
```rust
#[test]
fn test_email_outreach_tables_exist() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::init_db_conn(&mut conn).unwrap();

    let smtp_count: i64 = conn.query_row("SELECT COUNT(*) FROM smtp_servers", [], |r| r.get(0)).unwrap();
    assert_eq!(smtp_count, 0);

    let template_count: i64 = conn.query_row("SELECT COUNT(*) FROM email_templates", [], |r| r.get(0)).unwrap();
    assert_eq!(template_count, 0);

    let history_count: i64 = conn.query_row("SELECT COUNT(*) FROM email_outreach_history", [], |r| r.get(0)).unwrap();
    assert_eq!(history_count, 0);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_email_outreach_tables_exist`
Expected: FAIL with missing tables error.

- [ ] **Step 4: Update `db.rs` with table creation DDL**

Add `smtp_servers`, `email_templates`, and `email_outreach_history` table creation logic inside `init_db`:
```rust
tx.execute(
    "CREATE TABLE IF NOT EXISTS smtp_servers (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        host TEXT NOT NULL,
        port INTEGER NOT NULL DEFAULT 587,
        username TEXT NOT NULL,
        password TEXT NOT NULL,
        from_email TEXT NOT NULL,
        from_name TEXT NOT NULL,
        security TEXT NOT NULL DEFAULT 'STARTTLS',
        created_at TEXT NOT NULL
    )",
    [],
)?;

tx.execute(
    "CREATE TABLE IF NOT EXISTS email_templates (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT NOT NULL,
        subject TEXT NOT NULL,
        content TEXT NOT NULL,
        created_at TEXT NOT NULL
    )",
    [],
)?;

tx.execute(
    "CREATE TABLE IF NOT EXISTS email_outreach_history (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        lead_id INTEGER NOT NULL,
        template_id INTEGER NOT NULL,
        smtp_id INTEGER NOT NULL,
        status TEXT NOT NULL,
        error_message TEXT,
        sent_at TEXT NOT NULL,
        FOREIGN KEY(lead_id) REFERENCES leads(id) ON DELETE CASCADE,
        FOREIGN KEY(template_id) REFERENCES email_templates(id) ON DELETE CASCADE,
        FOREIGN KEY(smtp_id) REFERENCES smtp_servers(id) ON DELETE CASCADE
    )",
    [],
)?;
```

- [ ] **Step 5: Run cargo test to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_email_outreach_tables_exist`
Expected: PASS

- [ ] **Step 6: Commit changes & analyze**

```bash
git add src-tauri/Cargo.toml src-tauri/src/db.rs src-tauri/src/db_test.rs
git commit -m "feat(db): add email outreach tables and lettre dependency"
bunx gitnexus analyze
```

---

### Task 2: Implement Rust Data Models & DB Helpers for SMTP & Email Templates

**Files:**
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db.rs`
- Test: `src-tauri/src/db_test.rs`

**Interfaces:**
- Consumes: Database schema created in Task 1
- Produces: Data models `SmtpServer`, `EmailTemplate`, `EmailOutreachHistory` and DB functions (`save_smtp_server`, `get_smtp_servers`, `delete_smtp_server`, `create_email_template`, `get_email_templates`, etc.)

- [ ] **Step 1: Define models in `models.rs`**

Add structs to `src-tauri/src/models.rs`:
```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SmtpServer {
    pub id: Option<i32>,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub security: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailTemplate {
    pub id: Option<i32>,
    pub name: String,
    pub subject: String,
    pub content: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailOutreachHistory {
    pub id: Option<i32>,
    pub lead_id: i32,
    pub template_id: i32,
    pub smtp_id: i32,
    pub status: String,
    pub error_message: Option<String>,
    pub sent_at: String,
}
```

- [ ] **Step 2: Write failing tests in `db_test.rs`**

Add tests for SMTP and Email Template CRUD:
```rust
#[test]
fn test_smtp_and_email_template_crud() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::db::init_db_conn(&mut conn).unwrap();

    let smtp_id = crate::db::save_smtp_server(
        &conn, None, "Test SMTP", "smtp.test.com", 587, "user", "pass", "outreach@test.com", "Test Sender", "STARTTLS"
    ).unwrap();
    assert!(smtp_id > 0);

    let servers = crate::db::get_smtp_servers(&conn).unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "Test SMTP");

    let tpl_id = crate::db::create_email_template(&conn, "Welcome Email", "Welcome {{name}}", "Hello {{name}} from {{company}}!").unwrap();
    let tpls = crate::db::get_email_templates(&conn).unwrap();
    assert_eq!(tpls.len(), 1);
    assert_eq!(tpls[0].subject, "Welcome {{name}}");

    crate::db::delete_smtp_server(&conn, smtp_id).unwrap();
    assert_eq!(crate::db::get_smtp_servers(&conn).unwrap().len(), 0);
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_smtp_and_email_template_crud`
Expected: FAIL with unresolved functions.

- [ ] **Step 4: Implement DB functions in `db.rs`**

Implement `save_smtp_server`, `get_smtp_servers`, `delete_smtp_server`, `get_smtp_server_by_id`, `create_email_template`, `update_email_template`, `delete_email_template`, `get_email_templates`, `log_email_outreach_attempt`.

- [ ] **Step 5: Run cargo test to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_smtp_and_email_template_crud`
Expected: PASS

- [ ] **Step 6: Commit changes & analyze**

```bash
git add src-tauri/src/models.rs src-tauri/src/db.rs src-tauri/src/db_test.rs
git commit -m "feat(db): add db helpers and models for smtp servers and email templates"
bunx gitnexus analyze
```

---

### Task 3: Implement Tauri IPC Commands for SMTP & Email Templates

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands_test.rs`

**Interfaces:**
- Consumes: DB helpers from Task 2
- Produces: Tauri IPC commands `get_smtp_servers`, `save_smtp_server`, `delete_smtp_server`, `test_smtp_connection`, `get_email_templates`, `create_email_template`, `update_email_template`, `delete_email_template`.

- [ ] **Step 1: Write failing tests in `commands_test.rs`**

Add unit test for template interpolation & command wrappers in `commands_test.rs`.

- [ ] **Step 2: Run test to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml test_email_commands`
Expected: FAIL

- [ ] **Step 3: Implement IPC commands in `commands.rs`**

Add commands:
```rust
#[tauri::command]
pub fn get_smtp_servers(state: tauri::State<DbState>) -> Result<Vec<crate::models::SmtpServer>, String>;

#[tauri::command]
pub fn save_smtp_server(server: crate::models::SmtpServer, state: tauri::State<DbState>) -> Result<i32, String>;

#[tauri::command]
pub fn delete_smtp_server(id: i32, state: tauri::State<DbState>) -> Result<(), String>;

#[tauri::command]
pub async fn test_smtp_connection(server: crate::models::SmtpServer) -> Result<String, String>;

#[tauri::command]
pub fn get_email_templates(state: tauri::State<DbState>) -> Result<Vec<crate::models::EmailTemplate>, String>;

#[tauri::command]
pub fn create_email_template(name: String, subject: String, content: String, state: tauri::State<DbState>) -> Result<i32, String>;

#[tauri::command]
pub fn update_email_template(id: i32, name: String, subject: String, content: String, state: tauri::State<DbState>) -> Result<(), String>;

#[tauri::command]
pub fn delete_email_template(id: i32, state: tauri::State<DbState>) -> Result<(), String>;
```

- [ ] **Step 4: Register commands in `lib.rs`**

Register the new commands in `lib.rs` invoke handler list.

- [ ] **Step 5: Run tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 6: Commit changes & analyze**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/commands_test.rs
git commit -m "feat(api): implement tauri IPC commands for SMTP server management and email templates"
bunx gitnexus analyze
```

---

### Task 4: Implement Async Email Outreach Campaign Engine

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands_test.rs`

**Interfaces:**
- Consumes: `SmtpServer`, `EmailTemplate`, `Lead` records from DB
- Produces: `send_email_outreach` Tauri command and events `email-outreach-progress`, `email-outreach-log`, `email-outreach-complete`

- [ ] **Step 1: Write helper function for template placeholder resolution**

Implement variable replacement for `{{name}}`, `{{company}}`, `{{email}}`, `{{phone}}`.

- [ ] **Step 2: Implement `send_email_outreach` async handler in `commands.rs`**

```rust
#[tauri::command]
pub async fn send_email_outreach(
    app_handle: tauri::AppHandle,
    smtp_id: i32,
    lead_ids: Vec<i32>,
    template_id: i32,
) -> Result<(), String>
```
Iterate over selected leads, build `lettre` Message, connect via `AsyncSmtpTransport`, send email, emit realtime logs/progress to frontend via `app_handle.emit()`, and record activity entry.

- [ ] **Step 3: Register `send_email_outreach` in `lib.rs`**

Add `send_email_outreach` to `generate_handler![...]` in `src-tauri/src/lib.rs`.

- [ ] **Step 4: Run cargo test to verify build and tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS

- [ ] **Step 5: Commit changes & analyze**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs src-tauri/src/commands_test.rs
git commit -m "feat(outreach): implement async email outreach delivery engine with lettre transport"
bunx gitnexus analyze
```

---

### Task 5: Create Frontend `EmailOutreach.svelte` Component

**Files:**
- Create: `src/lib/components/EmailOutreach.svelte`
- Test: `src/lib/components/__tests__/EmailOutreach.spec.ts`

**Interfaces:**
- Consumes: Tauri commands (`get_smtp_servers`, `save_smtp_server`, `delete_smtp_server`, `test_smtp_connection`, `get_email_templates`, `create_email_template`, `update_email_template`, `delete_email_template`, `send_email_outreach`) and CRM lead services.
- Produces: Fully interactive Svelte 5 UI for Email Outreach.

- [ ] **Step 1: Create `EmailOutreach.svelte`**

Implement symmetrical UI matching `ZaloOutreach.svelte`:
1. **SMTP Server Cards & Modal**: Manage SMTP configuration (Host, Port, Username, Password, From Email, Security dropdown, Test Connection button).
2. **Email Template Cards & Modal**: Manage Email templates with Name, Subject, Content, placeholder tags (`{{name}}`, `{{company}}`, etc.).
3. **Target Lead Selection**: Search, filter by Category and Segmentation, select all / individual leads.
4. **Campaign Console**: Realtime progress bar, status text, live console log output listener (`email-outreach-log`, `email-outreach-progress`, `email-outreach-complete`).

- [ ] **Step 2: Add frontend unit test in `EmailOutreach.spec.ts`**

Create `src/lib/components/__tests__/EmailOutreach.spec.ts` to test rendering and user interactions using Vitest/Testing Library.

- [ ] **Step 3: Run frontend tests**

Run: `bun run test`
Expected: PASS

- [ ] **Step 4: Commit changes & analyze**

```bash
git add src/lib/components/EmailOutreach.svelte src/lib/components/__tests__/EmailOutreach.spec.ts
git commit -m "feat(ui): create EmailOutreach.svelte component with SMTP server management and template editor"
bunx gitnexus analyze
```

---

### Task 6: Integrate Email Outreach into App Navigation

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: `EmailOutreach.svelte` component
- Produces: Navigation item in sidebar and tab rendering in root page.

- [ ] **Step 1: Add Email Outreach to `Sidebar.svelte`**

Add menu item to `menuItems` array:
```typescript
{ id: 'email_outreach', label: 'Email Outreach', icon: 'mail' }
```

- [ ] **Step 2: Update `+page.svelte`**

Import `EmailOutreach` and add container tab:
```svelte
import EmailOutreach from '$lib/components/EmailOutreach.svelte';

...

<div class:hidden={activeTab !== 'email_outreach'}>
  <EmailOutreach />
</div>
```

- [ ] **Step 3: Verify build**

Run: `bun run build`
Expected: Build succeeds cleanly with zero errors.

- [ ] **Step 4: Commit changes & analyze**

```bash
git add src/lib/components/Sidebar.svelte src/routes/+page.svelte
git commit -m "feat(nav): integrate Email Outreach into sidebar menu and app shell"
bunx gitnexus analyze
```
