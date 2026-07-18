# Zalo Outreach Production-Grade Upgrade Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a production-ready, quota-optimized Zalo outreach system in Tauri + SvelteKit + SQLite with custom categories, sending history, and automatic Zalo account rotation.

**Architecture:** Use a decoupled verification/sending process. Zalo UIDs are cached to bypass future lookups. The backend rotates active Zalo sessions upon rate limits or session failures, and records a structured history table for follow-up filtering. SQLite WAL mode prevents concurrency lockups.

**Tech Stack:** Rust (Tauri), SvelteKit, SQLite (rusqlite).

## Global Constraints
*   All commands must use `bun` and `bunx` instead of `npm` and `npx`.
*   Maintain database compatibility and run migrations safely.
*   Enable WAL mode for SQLite to prevent write locks during background campaign runs.
*   Sleep 2 to 4 seconds between Zalo API calls to emulate human behaviour and prevent spam bans.
*   Never run Zalo `find_user` lookup if the lead has a cached `zalo_uid`.

---

### Task 1: Database Migration and WAL Configuration

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/models.rs`
- Modify: `src-tauri/src/db_test.rs`

**Interfaces:**
- Consumes: Existing SQLite connection pool.
- Produces: 
  - WAL mode enabled.
  - New DB tables: `categories`, `lead_categories`, `outreach_history`.
  - New columns on `leads`: `zalo_uid`, `zalo_status`, `relationship_status`, `last_lookup_attempt`.
  - Database functions: `create_category`, `delete_category`, `add_lead_to_category`, `remove_lead_from_category`, `log_outreach_attempt`, `get_categories`, `get_leads_by_category`.

- [ ] **Step 1: Enable WAL mode and add schema migrations**
  Modify `src-tauri/src/db.rs` to enable WAL mode and execute the migration statements for new tables and columns:
  ```rust
  // Insert inside init_db before creating tables:
  conn.execute("PRAGMA journal_mode = WAL;", [])?;
  conn.execute("PRAGMA synchronous = NORMAL;", [])?;
  
  // Alter leads table safely by running:
  let _ = conn.execute("ALTER TABLE leads ADD COLUMN zalo_uid TEXT", []);
  let _ = conn.execute("ALTER TABLE leads ADD COLUMN zalo_status TEXT DEFAULT 'UNTESTED'", []);
  let _ = conn.execute("ALTER TABLE leads ADD COLUMN relationship_status TEXT DEFAULT 'UNKNOWN'", []);
  let _ = conn.execute("ALTER TABLE leads ADD COLUMN last_lookup_attempt TEXT", []);
  ```
  And add creation code for `categories`, `lead_categories`, and `outreach_history` tables.

- [ ] **Step 2: Add Structs in models.rs**
  Modify `src-tauri/src/models.rs` to define:
  ```rust
  #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
  pub struct Category {
      pub id: Option<i32>,
      pub name: String,
  }

  #[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
  pub struct OutreachHistory {
      pub id: Option<i32>,
      pub lead_id: i32,
      pub template_id: i32,
      pub status: String,
      pub error_message: Option<String>,
      pub sent_at: String,
      pub session_imei: String,
  }
  ```

- [ ] **Step 3: Implement database queries in db.rs**
  Add helper functions to `src-tauri/src/db.rs` for category management and history logs:
  - `pub fn create_category(conn: &Connection, name: &str) -> Result<i32>`
  - `pub fn delete_category(conn: &Connection, id: i32) -> Result<()>`
  - `pub fn add_lead_to_category(conn: &Connection, lead_id: i32, category_id: i32) -> Result<()>`
  - `pub fn remove_lead_from_category(conn: &Connection, lead_id: i32, category_id: i32) -> Result<()>`
  - `pub fn log_outreach_attempt(conn: &Connection, history: &OutreachHistory) -> Result<()>`

- [ ] **Step 4: Update db_test.rs and run tests**
  Modify `src-tauri/src/db_test.rs` to add tests verifying WAL mode, categories creation, and outreach logs.
  Run: `bun tauri test` (or execute standard cargo tests using `cargo test --manifest-path src-tauri/Cargo.toml --lib db_test`)
  Expected: All database tests PASS.

- [ ] **Step 5: Commit changes**
  Run: `git commit -am "feat: database migrations for WAL mode, categories, and outreach history"`

---

### Task 2: Tauri Category Management Commands

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/commands_test.rs`

**Interfaces:**
- Consumes: Database helper functions from Task 1.
- Produces: Tauri IPC Commands: `get_categories`, `create_category`, `delete_category`, `add_leads_to_category`, `remove_leads_from_category`, `get_leads_by_category`.

- [ ] **Step 1: Create Tauri Commands in commands.rs**
  Add the following commands to `src-tauri/src/commands.rs`:
  *   `get_categories(state: tauri::State<'_, DbState>) -> Result<Vec<Category>, String>`
  *   `create_category(state: tauri::State<'_, DbState>, name: String) -> Result<i32, String>`
  *   `delete_category(state: tauri::State<'_, DbState>, id: i32) -> Result<(), String>`
  *   `add_leads_to_category(state: tauri::State<'_, DbState>, lead_ids: Vec<i32>, category_id: i32) -> Result<(), String>`
  *   `remove_leads_from_category(state: tauri::State<'_, DbState>, lead_ids: Vec<i32>, category_id: i32) -> Result<(), String>`
  *   `get_leads_by_category(state: tauri::State<'_, DbState>, category_id: i32) -> Result<Vec<Lead>, String>`

- [ ] **Step 2: Register Commands in lib.rs**
  Modify `src-tauri/src/lib.rs` to add the new commands to `.invoke_handler(tauri::generate_handler![...])`.

- [ ] **Step 3: Add integration tests**
  Modify `src-tauri/src/commands_test.rs` to add tests invoking these commands, verifying they successfully create categories and update the database.
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands_test`
  Expected: PASS

- [ ] **Step 4: Commit**
  Run: `git commit -am "feat: tauri commands for category management"`

---

### Task 3: Backend Campaign Outreach and Session Rotation

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/commands_test.rs`

**Interfaces:**
- Consumes: `zca-rs` APIs for Zalo login, `find_user`, and `send_message`.
- Produces: `send_zalo_campaign` command. Emits events: `campaign-log`, `campaign-progress`, `campaign-complete`.

- [ ] **Step 1: Implement send_zalo_campaign**
  Create `pub async fn send_zalo_campaign` in `src-tauri/src/commands.rs`:
  - Query leads in the category.
  - Filter out leads where `zalo_status = 'VERIFIED_NOT_EXIST'`, or `RATE_LIMITED` within the last hour.
  - Filter out leads who already have a `SUCCESS` record for the given `template_id` in `outreach_history`.
  - Log in to all provided `session_imeis` and store active connections.
  - Iterate through eligible leads:
    - Determine active Zalo client.
    - If `zalo_uid` is empty:
      - Call `find_user`.
      - If found: set `zalo_status = 'VERIFIED_EXIST'` and save `zalo_uid` in DB.
      - If not found (code 216): set `zalo_status = 'VERIFIED_NOT_EXIST'` in DB, log `FAILED` to history, proceed to next lead.
      - If rate-limited or error: set `zalo_status = 'RATE_LIMITED'`, record `last_lookup_attempt = now()`, rotate session index, and retry.
    - If `zalo_uid` exists:
      - Send message directly using UID.
      - If success: log `SUCCESS` in `outreach_history`.
      - If fail: log `FAILED` in `outreach_history`. If session failure, rotate session.
    - Perform a random `sleep` of 2-4 seconds between sends.

- [ ] **Step 2: Add mock tests for campaign execution**
  Modify `src-tauri/src/commands_test.rs` to mock `send_zalo_campaign` (using mock-imeis) and verify it skips lookups for cached UIDs, rotates sessions upon simulated failure, and updates DB correctly.
  Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands_test`
  Expected: PASS

- [ ] **Step 3: Commit**
  Run: `git commit -am "feat: zalo outreach campaign execution with session rotation and cache bypass"`

---

### Task 4: Frontend Category Management UI

**Files:**
- Create: `src/lib/components/CategoryManager.svelte`
- Modify: `src/lib/components/LeadsList.svelte`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: Category Tauri commands from Task 2.
- Produces: Fully interactive category creation and lead-to-category mapping dashboard.

- [ ] **Step 1: Create CategoryManager Component**
  Create `src/lib/components/CategoryManager.svelte`. Implement:
  - Sidebar showing created categories.
  - CRUD operations (add category, delete category).
  - Main panel showing leads in the selected category.
  - A search field and list of leads NOT in the category, allowing multi-select and bulk-addition.
  - Option to remove leads from the category.

- [ ] **Step 2: Integrate category tabs in +page.svelte**
  Modify `src/routes/+page.svelte` to add a new tab "Categories" importing and displaying the `CategoryManager` component.

- [ ] **Step 3: Add Bulk Category Action to LeadsList**
  Modify `src/lib/components/LeadsList.svelte` to add a button/dropdown "Assign to Category" when leads are selected, opening a modal to add selected leads to a chosen Category.

- [ ] **Step 4: Commit**
  Run: `git commit -am "feat: frontend category manager UI and bulk leads list assignment"`

---

### Task 5: Frontend Zalo Campaign Configuration & Console Panel

**Files:**
- Modify: `src/lib/components/ZaloOutreach.svelte`

**Interfaces:**
- Consumes: `send_zalo_campaign` Tauri command, Category listings.
- Produces: Multi-account session checkboxes, Category filtering, follow-up segmentation filters, and real-time logs.

- [ ] **Step 1: Update Session Selection to Checkboxes**
  Modify `src/lib/components/ZaloOutreach.svelte` to replace the single session dropdown with a list of checkable active Zalo sessions. Users select which accounts are added to the rotation pool.

- [ ] **Step 2: Integrate Category Target Dropdown**
  Add a dropdown to select the target Category for the campaign.

- [ ] **Step 3: Implement Campaign Segmentation Filters**
  Add follow-up selection options:
  - Radio options or dropdown for target leads:
    *   `Never Sent` (no history of sending).
    *   `Template Success` (dropdown to select previous template, showing leads who received it).
    *   `Last Attempt Failed` (leads whose most recent outreach failed, allowing quick retry).
  - Query DB/State to filter selected leads in Svelte state, showing dynamic count.

- [ ] **Step 4: Update Start Campaign Execution and Console**
  - Update `startCampaign` to call `send_zalo_campaign` passing chosen `category_id`, `template_id`, and array of active `session_imeis`.
  - Update the campaign logs list to display the real-time events emitted from Tauri backend (session rotations, lookup status updates, sending status).

- [ ] **Step 5: Verify integration and Commit**
  Verify Svelte components compile cleanly.
  Run: `bun run build`
  Expected: Clean compilation with zero typescript or framework errors.
  Commit: `git commit -am "feat: updated Zalo outreach frontend with account rotation and campaign filtering"`
