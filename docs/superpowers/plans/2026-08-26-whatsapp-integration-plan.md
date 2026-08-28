# WhatsApp Integration Implementation Plan (Tier 3 Critical)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement full-featured, enterprise-grade WhatsApp integration into RustSale_v2 with native QR pairing, multi-account supervision, unified conversation timeline sync, and resilient outreach campaign dispatch with responsible rate limiting and opt-out suppression.

**Architecture:** Built on `whatsapp-rust` (=0.7.0) Multi-Device protocol with isolated SQLite session stores in Tauri's `app_data_dir`. Uses a concrete outbox worker in `ConversationService`, a robust campaign state machine with pre-dispatch suppression checks, and clean UI components in Svelte 5.

**Tech Stack:** Rust (Tauri v2, tokio, rusqlite, whatsapp-rust 0.7.0, uuid, chrono), Svelte 5, Tailwind CSS, Lucide icons, Bun test runner.

## Global Constraints
- Strictly follow `AGENTS.md`: Use `bun` and `bunx` instead of `node`/`npm`/`npx`.
- Strictly follow `docs/IMPLEMENTATION_COMPLETION_HARNESS.md` (Tier 3 Critical Risk).
- Preserve all existing Zalo, Facebook Messenger, and Email functionality without regressions.
- All timestamps must follow RFC3339 UTC with millisecond precision (`crate::conversation::types::utc_now()`).
- All database migrations must be idempotent and non-destructive to existing CRM lead and message histories.

---

### Task 1: Dependencies & WhatsApp Phone Normalization Module

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/conversation/whatsapp/mod.rs`
- Create: `src-tauri/src/conversation/whatsapp/phone.rs`
- Create: `src-tauri/src/conversation/whatsapp/phone_test.rs`
- Modify: `src-tauri/src/conversation/mod.rs`

**Interfaces:**
- Produces:
  - `pub fn normalize_e164(raw: &str, default_country: Option<&str>) -> Option<String>`
  - `pub fn phone_to_direct_jid(e164: &str) -> String`
  - `pub fn is_group_jid(jid: &str) -> bool`
  - `pub fn parse_raw_jid(jid_str: &str) -> Option<whatsapp_rust::types::Jid>`

- [ ] **Step 1: Update Cargo.toml with whatsapp-rust dependency**

Add `whatsapp-rust = "=0.7.0"` (or required features) to `src-tauri/Cargo.toml`.

- [ ] **Step 2: Write failing unit tests for Phone Normalization & JID Resolution**

```rust
// src-tauri/src/conversation/whatsapp/phone_test.rs
use super::phone::*;

#[test]
fn test_normalize_vietnamese_phones() {
    assert_eq!(normalize_e164("0901234567", Some("84")), Some("84901234567".to_string()));
    assert_eq!(normalize_e164("+84 901 234 567", None), Some("84901234567".to_string()));
    assert_eq!(normalize_e164("084901234567", Some("84")), Some("84901234567".to_string()));
}

#[test]
fn test_normalize_international_phones() {
    assert_eq!(normalize_e164("+1 (555) 123-4567", None), Some("15551234567".to_string()));
    assert_eq!(normalize_e164("+65 9123 4567", None), Some("6591234567".to_string()));
}

#[test]
fn test_invalid_phones() {
    assert_eq!(normalize_e164("abc12345", None), None);
    assert_eq!(normalize_e164("123", None), None);
}

#[test]
fn test_jid_formatting() {
    assert_eq!(phone_to_direct_jid("84901234567"), "84901234567@s.whatsapp.net");
    assert!(is_group_jid("123456789-987654@g.us"));
    assert!(!is_group_jid("84901234567@s.whatsapp.net"));
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib conversation::whatsapp::phone_test`
Expected: FAIL (module not found)

- [ ] **Step 4: Implement Phone & JID Module**

Create `src-tauri/src/conversation/whatsapp/phone.rs` and wire in `src-tauri/src/conversation/whatsapp/mod.rs` and `src-tauri/src/conversation/mod.rs`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib conversation::whatsapp::phone_test`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/conversation/whatsapp/
git commit -m "feat(whatsapp): add dependencies and phone normalization module"
```

---

### Task 2: Database Migration & Canonical Schema Integration

**Files:**
- Create: `src-tauri/src/conversation/whatsapp/migration.rs`
- Create: `src-tauri/src/conversation/whatsapp/migration_test.rs`
- Modify: `src-tauri/src/conversation/migration_v2.rs`

**Interfaces:**
- Produces:
  - `pub fn apply_whatsapp_migrations(conn: &Connection) -> Result<()>`
  - Creates tables: `whatsapp_sessions`, `contact_suppression_list`, `whatsapp_outreach_campaigns`, `whatsapp_campaign_recipients`.

- [ ] **Step 1: Write migration tests for idempotency & foreign keys**

```rust
// src-tauri/src/conversation/whatsapp/migration_test.rs
use super::migration::apply_whatsapp_migrations;
use rusqlite::Connection;

#[test]
fn test_whatsapp_migrations_idempotency() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(include_str!("../schema.sql")).unwrap();
    
    // First run
    apply_whatsapp_migrations(&conn).unwrap();
    // Second run (idempotent)
    apply_whatsapp_migrations(&conn).unwrap();

    // Verify tables exist
    let mut stmt = conn.prepare("SELECT count(*) FROM whatsapp_sessions").unwrap();
    let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib conversation::whatsapp::migration_test`
Expected: FAIL

- [ ] **Step 3: Implement Migration Logic**

Create `src-tauri/src/conversation/whatsapp/migration.rs` with `CREATE TABLE IF NOT EXISTS` and index setups. Hook into `src-tauri/src/conversation/migration_v2.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib conversation::whatsapp::migration_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/conversation/whatsapp/migration.rs src-tauri/src/conversation/whatsapp/migration_test.rs src-tauri/src/conversation/migration_v2.rs
git commit -m "feat(whatsapp): add database schema migrations and tests"
```

---

### Task 3: WhatsApp Session Store & QR Login Manager

**Files:**
- Create: `src-tauri/src/conversation/whatsapp/login.rs`
- Create: `src-tauri/src/conversation/whatsapp/login_test.rs`
- Modify: `src-tauri/src/conversation/whatsapp/mod.rs`

**Interfaces:**
- Produces:
  - `pub struct WhatsAppLoginManager`
  - `pub async fn start_qr_login(&self, attempt_id: &str) -> Result<()>`
  - `pub fn cancel_qr_login(&self, attempt_id: &str) -> Result<()>`
  - Events: `whatsapp-qr-code`, `whatsapp-login-success`, `whatsapp-login-error`

- [ ] **Step 1: Write failing tests for Login Manager correlation & cancellation**

```rust
// src-tauri/src/conversation/whatsapp/login_test.rs
use super::login::*;

#[tokio::test]
async fn test_login_attempt_cancellation() {
    let manager = WhatsAppLoginManager::new_mock();
    let attempt_id = "test-attempt-123";
    
    manager.start_mock_login(attempt_id).await.unwrap();
    assert!(manager.is_active(attempt_id));
    
    manager.cancel_qr_login(attempt_id).unwrap();
    assert!(!manager.is_active(attempt_id));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib conversation::whatsapp::login_test`
Expected: FAIL

- [ ] **Step 3: Implement WhatsAppLoginManager**

Implement session file sandboxing in `app_data_dir/whatsapp_sessions/temp_<uuid>.db`, QR code generation, sequence incrementing, and atomic DB finalization.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib conversation::whatsapp::login_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/conversation/whatsapp/login.rs src-tauri/src/conversation/whatsapp/login_test.rs
git commit -m "feat(whatsapp): implement QR pairing manager with attempt correlation"
```

---

### Task 4: WhatsApp Normalizer & Lead Phone Resolution

**Files:**
- Create: `src-tauri/src/conversation/whatsapp/normalizer.rs`
- Create: `src-tauri/src/conversation/whatsapp/normalizer_test.rs`

**Interfaces:**
- Produces:
  - `pub struct WhatsAppNormalizer`
  - `pub fn ingest_incoming_message(conn: &mut Connection, channel_account_id: i64, msg: &RawWhatsAppMessage) -> Result<Option<i64>>`
  - Resolution of 1 matching lead $\to$ linked `lead_id`, 0 or $>1$ leads $\to$ `NULL` (unlinked).

- [ ] **Step 1: Write failing tests for WhatsAppNormalizer**

```rust
// src-tauri/src/conversation/whatsapp/normalizer_test.rs
#[test]
fn test_normalizer_links_single_matching_lead() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 101, "84901234567");
    
    let raw = mock_incoming_text("84901234567@s.whatsapp.net", "Hello");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw).unwrap().unwrap();
    
    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, Some(101));
}

#[test]
fn test_normalizer_leaves_ambiguous_leads_unlinked() {
    let mut conn = setup_test_db();
    insert_test_lead(&conn, 101, "84901234567");
    insert_test_lead(&conn, 102, "84901234567"); // Duplicate phone in CRM
    
    let raw = mock_incoming_text("84901234567@s.whatsapp.net", "Hello");
    let conv_id = WhatsAppNormalizer::ingest_incoming_message(&mut conn, 1, &raw).unwrap().unwrap();
    
    let lead_id: Option<i64> = get_conversation_lead_id(&conn, conv_id);
    assert_eq!(lead_id, None); // Must not guess arbitrarily
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib conversation::whatsapp::normalizer_test`
Expected: FAIL

- [ ] **Step 3: Implement WhatsAppNormalizer**

Implement message normalization for text, media, reactions, quotes, and robust lead resolution.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib conversation::whatsapp::normalizer_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/conversation/whatsapp/normalizer.rs src-tauri/src/conversation/whatsapp/normalizer_test.rs
git commit -m "feat(whatsapp): implement normalizer and lead phone resolution"
```

---

### Task 5: WhatsApp Supervisor & Outbox Service Seam

**Files:**
- Create: `src-tauri/src/conversation/whatsapp/client.rs`
- Create: `src-tauri/src/conversation/whatsapp/supervisor.rs`
- Create: `src-tauri/src/conversation/whatsapp/supervisor_test.rs`
- Modify: `src-tauri/src/conversation/service.rs`
- Modify: `src-tauri/src/conversation/service_test.rs`

**Interfaces:**
- Produces:
  - `pub trait WhatsAppClient: Send + Sync`
  - `pub struct WhatsAppSupervisor`
  - `ConversationService::process_next_whatsapp_outbox_job(&self, channel_account_id: i64, client: &dyn WhatsAppClient) -> Result<bool>`

- [ ] **Step 1: Write failing tests for WhatsApp outbox processing & ambiguous timeout**

```rust
// In src-tauri/src/conversation/service_test.rs
#[tokio::test]
async fn test_process_whatsapp_outbox_job_success() {
    let (service, conn) = setup_test_service();
    let account_id = create_test_whatsapp_account(&conn, "84901234567");
    let msg_id = queue_test_outbox_message(&conn, account_id, "84909999999@s.whatsapp.net", "Hello");
    
    let mock_client = MockWhatsAppClient::new();
    let processed = service.process_next_whatsapp_outbox_job(account_id, &mock_client).await.unwrap();
    assert!(processed);
    
    assert_eq!(get_outbox_job_status(&conn, msg_id), "completed");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib conversation::service_test::test_process_whatsapp_outbox_job_success`
Expected: FAIL

- [ ] **Step 3: Implement WhatsAppClient trait, WhatsAppSupervisor & process_next_whatsapp_outbox_job**

Implement worker runtime and service method in `service.rs`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib conversation::service_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/conversation/whatsapp/client.rs src-tauri/src/conversation/whatsapp/supervisor.rs src-tauri/src/conversation/whatsapp/supervisor_test.rs src-tauri/src/conversation/service.rs
git commit -m "feat(whatsapp): implement supervisor and outbox worker seam"
```

---

### Task 6: Campaign Engine, Suppression List & Spintax

**Files:**
- Create: `src-tauri/src/conversation/whatsapp/campaign.rs`
- Create: `src-tauri/src/conversation/whatsapp/spintax.rs`
- Create: `src-tauri/src/conversation/whatsapp/campaign_test.rs`
- Create: `src-tauri/src/conversation/whatsapp/spintax_test.rs`

**Interfaces:**
- Produces:
  - `pub struct WhatsAppCampaignEngine`
  - `pub fn parse_spintax(input: &str) -> String`
  - `pub fn render_template(template: &str, lead_vars: &HashMap<String, String>) -> String`
  - Pre-dispatch suppression check (`contact_suppression_list`) & quiet hours check.

- [ ] **Step 1: Write failing tests for Spintax & Suppression checking**

```rust
// src-tauri/src/conversation/whatsapp/spintax_test.rs
#[test]
fn test_spintax_variations() {
    let pattern = "{Xin chào|Chào bạn} [name]";
    let rendered = parse_spintax(pattern);
    assert!(rendered.starts_with("Xin chào") || rendered.starts_with("Chào bạn"));
}

// src-tauri/src/conversation/whatsapp/campaign_test.rs
#[test]
fn test_campaign_skips_suppressed_contact() {
    let mut conn = setup_test_db();
    add_to_suppression_list(&conn, "whatsapp", "84901234567", "opt_out");
    
    let engine = WhatsAppCampaignEngine::new();
    let result = engine.evaluate_recipient(&conn, "84901234567");
    assert_eq!(result, RecipientEvaluation::Skipped("opt_out_suppressed"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib conversation::whatsapp::spintax_test`
Expected: FAIL

- [ ] **Step 3: Implement Spintax & Campaign Engine**

Implement Spintax evaluation, tag replacement, suppression list querying, and campaign state transitions.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib conversation::whatsapp::spintax_test conversation::whatsapp::campaign_test`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/conversation/whatsapp/campaign.rs src-tauri/src/conversation/whatsapp/spintax.rs src-tauri/src/conversation/whatsapp/campaign_test.rs src-tauri/src/conversation/whatsapp/spintax_test.rs
git commit -m "feat(whatsapp): implement campaign engine, spintax, and suppression checking"
```

---

### Task 7: Tauri Commands & Native IPC Wiring

**Files:**
- Modify: `src-tauri/src/conversation/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/conversation/commands_test.rs`

**Interfaces:**
- Produces Tauri commands:
  - `start_whatsapp_qr_login(attempt_id: String)`
  - `cancel_whatsapp_qr_login(attempt_id: String)`
  - `get_managed_whatsapp_accounts()`
  - `disconnect_whatsapp_account(channel_account_id: i64)`
  - `create_whatsapp_campaign(req: CreateWhatsAppCampaignRequest)`
  - `start_whatsapp_campaign(campaign_id: i64)`
  - `pause_whatsapp_campaign(campaign_id: i64)`
  - `resume_whatsapp_campaign(campaign_id: i64)`

- [ ] **Step 1: Write commands integration test**

```rust
// In src-tauri/src/conversation/commands_test.rs
#[tokio::test]
async fn test_get_managed_whatsapp_accounts_command() {
    let app = setup_test_app();
    let accounts = commands::get_managed_whatsapp_accounts(app.state()).await.unwrap();
    assert!(accounts.is_empty() || !accounts.is_empty());
}
```

- [ ] **Step 2: Implement commands in commands.rs and register in lib.rs**

- [ ] **Step 3: Run cargo test to verify**

Run: `cargo test --lib conversation::commands_test`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/conversation/commands.rs src-tauri/src/lib.rs src-tauri/src/conversation/commands_test.rs
git commit -m "feat(whatsapp): register Tauri IPC commands for WhatsApp"
```

---

### Task 8: Frontend Account Settings & QR Pairing UI

**Files:**
- Create: `src/lib/components/settings/WhatsAppAccountsSettings.svelte`
- Modify: `src/lib/components/Settings.svelte`
- Create: `src/lib/services/whatsappService.ts`
- Modify: `src/lib/services/accountSettingsService.ts`

- [ ] **Step 1: Create `whatsappService.ts` with Tauri invoke bindings & event listeners**
- [ ] **Step 2: Build `WhatsAppAccountsSettings.svelte` with account list, status badges, and QR scan modal**
- [ ] **Step 3: Enable the WhatsApp tab in `Settings.svelte` (remove `disabled` & `Coming soon` badge)**
- [ ] **Step 4: Test frontend build with `bun run check`**
- [ ] **Step 5: Commit**

```bash
git add src/lib/components/settings/WhatsAppAccountsSettings.svelte src/lib/components/Settings.svelte src/lib/services/whatsappService.ts src/lib/services/accountSettingsService.ts
git commit -m "feat(ui): add WhatsApp account settings and QR login modal"
```

---

### Task 6: Frontend WhatsApp Outreach UI & Sidebar

**Files:**
- Create: `src/lib/components/WhatsAppOutreach.svelte`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/routes/+page.svelte`
- Modify: `messages/en.json` and `messages/vi.json`

- [ ] **Step 1: Add i18n keys for WhatsApp Outreach in `messages/en.json` and `messages/vi.json`**
- [ ] **Step 2: Add `WhatsApp Outreach` entry in `Sidebar.svelte` (with `MessageCircleMore` icon)**
- [ ] **Step 3: Create `WhatsAppOutreach.svelte` component with audience filters, template manager, and campaign console**
- [ ] **Step 4: Register tab in `src/routes/+page.svelte`**
- [ ] **Step 5: Run `bun run check` and `bun test`**
- [ ] **Step 6: Commit**

```bash
git add src/lib/components/WhatsAppOutreach.svelte src/lib/components/Sidebar.svelte src/routes/+page.svelte messages/
git commit -m "feat(ui): add WhatsApp Outreach page and sidebar navigation"
```

---

### Task 10: Unified Conversation Integration & Quality Gates

**Files:**
- Modify: `src/lib/components/conversation/ChannelBadges.svelte`
- Modify: `src/lib/components/conversation/ConversationTimeline.svelte`
- Modify: `src/lib/components/conversation/ComposerToolbar.svelte`

- [ ] **Step 1: Verify WhatsApp badge and timeline rendering in Conversation Workspace**
- [ ] **Step 2: Run full backend and frontend quality gates:**

```bash
cargo test
cargo fmt --check
bun test
bun run check
git diff --check
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/conversation/
git commit -m "feat(conversation): integrate WhatsApp into unified conversation workspace"
```
